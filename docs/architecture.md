# Architecture

## Layers

```text
┌──────────────────────────────────────────────┐
│ sim: SDL2 window, texture upload, screenshots │
├──────────────────────────────────────────────┤
│ app: screen state, focus targets, live worker,│
│      catalogue, software screen drawing       │
├──────────────────────────────────────────────┤
│ ui: screens, theme, spatial focus navigation  │
│ gfx: software canvas, type, geometry, PNG     │
├──────────────────────────────────────────────┤
│ api: blocking ureq client, URL policy,        │
│      ServerTarget/StoredSession               │
├──────────────────────────────────────────────┤
│ protocol: serde wire types only               │
├──────────────────────────────────────────────┤
│ platform: secret store, mDNS discovery        │
└──────────────────────────────────────────────┘
```

`protocol` has no networking and no platform code, so the same fixtures can test
a host simulator and the television build. `api` is synchronous on purpose: a
native render loop does not benefit from a nested async runtime, and `ureq` keeps
the ARMv7 `DT_NEEDED` list small.

## Rendering

The screen tree is laid out in a fixed 1920×1080 logical space. `gfx::Canvas`
owns an RGBA pixmap, draws rounded shapes and gradients, rasterizes Inter glyphs
with `fontdue`, and can save a PNG. The host simulator uploads that pixmap to an
SDL2 streaming texture, scaled into a resizable window. This keeps the visual
design testable on a normal machine and leaves the screen code unchanged when the
GLES2 backend arrives.

Screen geometry lives in `app::layout`, and actions live in `app::state`.
`App::targets()` returns the focusable rectangles for the current screen, so the
highlight and the action can never point at different places. Navigation uses a
spatial nearest-neighbour search in `ui::navigate`; a simple row-major
`FocusGrid` remains available for dense lists.

## Threading

- **UI/render thread** owns SDL, the window, the focus tree, and all screen
  state. It never blocks on the network.
- **API worker** (`app::live`) owns the `ApiClient`, the pairing grant, and the
  `SecretStore`. It performs one request at a time on a `std::thread` and posts
  `LiveEvent`s back through a channel. The render loop drains them at the start
  of a frame and never waits for a request.
- **Catalogue mapping** happens on the UI thread: `LibraryResponse` becomes a
  `Catalog`, and `MediaDetails` updates one item in place. The server remains
  the source of truth for file selection and progress.
- **Playback position** is read from `com.webos.media` on a timer; progress is
  written from the API worker every 15 seconds, on pause, and on stop.
- No `tokio`, no `async-std`, no WebView, no JavaScript.

## Startup state machine

Ported from the reference Android TV client:

```text
Loading
  ├─ no saved server ───────────────► Setup
  ├─ saved server, no token ────────► Pairing
  └─ saved token ───────────────────► Ready
Ready ── 401 from any endpoint ─────► Pairing (keep the server)
```

`StartupState` lives in `crates/app`. A successful pairing writes a
`StoredSession` and transitions to `Ready`.

## Pairing flow

`app::pairing::PairingFlow` probes the server through `connect_target`, creates a
device grant with `POST /api/device/start`, and polls
`POST /api/device/token`. It maps the server's RFC 8628-style errors to
`Pending`, `SlowDown`, `Denied`, `Expired`, or `Approved`. The approved token is
persisted through `SecretStore`; `Debug` implementations redact both the device
code and the bearer token.

`app::live::LiveClient` runs the exchange on the API worker. The simulator's
`--server <url> --session <path>` mode displays the live code and QR, polls
until approval, stores the token, and then loads the library. A saved session is
reused only when its server URL matches `--server`; otherwise the worker clears
it and pairs against the requested server. `--pair` remains a terminal-only
check for the same exchange.

## Live catalogue

`Catalog` is the view model shared by the demo and live modes. `from_library`
maps `LibraryItem`s into owned `MediaItem`s, preserving loaded descriptions,
episodes, and playback plans by identifier across refreshes. `apply_details`
fills a title after `GET /api/playback/{identifier}?platform=webos`; a movie with
several video files stays a movie, while a TV item becomes a series with episode
rows.

The server does not expose a poster proxy. Live items therefore fetch their
`posterUrl` directly on two poster threads, decode it off the render loop, and
resize it to a 360×540 RGBA bitmap. Only portrait sources at least 240 px wide
are accepted: Archive.org's item-image service returns a 180×101 frame and
wide backdrops look worse than the procedural art, so both keep the fallback.
Loaded posters survive a library refresh, and cards, the hero, and the detail
screen all draw the same bitmap with a cover transform.

Category chips and search suggestions come from the loaded catalogue rather than
a hardcoded genre list, so a live library with a single "Horror" category shows
only that chip. An empty library renders an explicit empty state.
`App::selected_item` resolves
to a safe placeholder, so the Detail and Player screens cannot panic if a title
is removed by the web app during a refresh.

## Progress reporting

`app::progress::ProgressTracker` is a clock-injected policy shared by every
shell. It arms a timer when a title opens, emits a save every 15 seconds, and
emits completion once when playback reaches the last three seconds. `flush`
covers pause, Back, and quit; it does not overwrite an already completed title.
The tracker resets when the file changes or the same title is played again.

Only a loaded `playbackPlan.fileName` is reportable. A title without a plan is
skipped rather than guessed, because the server stores progress per file. For a
movie the completed request sets `itemCompleted: true`; for a series episode it
stays `false` so the next episode can continue the item. The host simulator
wires the tracker to `LiveCommand::SaveProgress` and
`LiveCommand::MarkCompleted`; the webOS shell will use the same seam.

## Server connection policy

- LAN URL and remote URL are stored separately.
- Try the LAN URL first.
- Fall back to the remote URL only on a **transport** error (timeout, DNS,
  connection refused). Never fall back on `401` or another HTTP status: that
  would hide a revoked token or a server bug.
- Public addresses must be `https://`; plain HTTP is accepted only for
  loopback, RFC1918, link-local, CGNAT/Tailscale, `*.local`, and single-label LAN
  hosts. This is a user-safety guard, not a security boundary.
- The API version is checked from `GET /api/health` before pairing. A newer
  server is refused instead of being driven with an unknown contract.

## Playback

The server selects a file for `platform=webos`:

```text
GET /api/playback/{identifier}?platform=webos[&file=<episode>]
```

`playbackPlan` contains the chosen URL, container, codec, dimensions, duration,
and `compatible`. The intended pipeline is:

1. `playbackPlan.compatible == false` → show `reason`; do not open the player.
2. Export the SDL window id (`option.windowId`).
3. Call `com.webos.media` `load` with the Archive.org URL and the exported
   window.
4. Draw the UI overlay above the video plane; the TV decodes the video.
5. Poll position; send progress on the schedule above.

webOS 5+ is the first target. webOS 4.x needs an ACB plane path and is a later
optional effort; webOS 3.x is out of scope. FFmpeg is not in the v1 dependency
set; if a file is not directly playable the plan should say so.

## Platform seams

- `SecretStore`: one `StoredSession` in v1. `FileSecretStore` writes JSON to the
  app data directory with mode `0600`; `MemorySecretStore` is for tests.
- `ServerDiscovery`: browse `_archiveplayer._tcp.local`, read the `name`, `id`,
  `api`, `version`, `path`, and `url` TXT records. The host build uses a no-op.
- `Screen`/`navigate`: spatial focus for the home hero, sidebar, poster grids,
  episode lists, and player controls. `FocusGrid` remains for dense rows.

## Host simulator

`crates/sim` is the only crate that links SDL2. It has three modes:

- Windowed demo: render the deterministic catalogue, upload the RGBA texture,
  drain keyboard events, and tick the player. F11 toggles fullscreen; F12 writes
  a screenshot.
- Windowed live: `--server <url>` starts the API worker. Actions dispatch
  `LoadDetails` and `Refresh`; `Pair` and `PairRefresh` start a new grant.
- Headless: `--all`, `--screenshot`, and `--live-shot` render to PNG without
  initializing video, so CI and screenshots work over SSH.

## Error handling

- `ApiError::Unauthorized` always returns to pairing and clears the stored
  token; other errors become a toast and leave the session intact.
- `ApiError::Status` carries the server `code`, message, and `Retry-After`.
  Pairing polls honor `slow_down`/`rate_limited` and back off.
- `ApiError::ApiVersion` is a terminal setup error with a clear message.
- A corrupt stored session is treated as "not paired" rather than crashing the
  app. A failed library refresh keeps the last good catalogue on screen.
