# Architecture

## Layers

```text
┌──────────────────────────────────────────────┐
│ sim: SDL2 window, texture upload, screenshots │
├──────────────────────────────────────────────┤
│ app: screen state, focus targets, actions,    │
│      demo catalogue, software screen drawing  │
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
- **API worker** (next milestone) performs one request at a time on a
  `std::thread`, then posts a `Result<Response, ApiError>` back through a
  channel. The render loop drains results at the start of a frame.
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

`crates/sim` is the only crate that links SDL2. It has two modes:

- Windowed: render, upload the RGBA texture, drain keyboard events, and tick the
  player. F11 toggles fullscreen; F12 writes a screenshot.
- Headless: `--all` or `--screenshot` renders deterministic screens to PNG
  without initializing video, so CI and screenshots work over SSH.

## Error handling

- `ApiError::Unauthorized` always returns to pairing.
- `ApiError::Status` carries the server `code`, message, and `Retry-After`.
  Pairing polls honor `slow_down`/`rate_limited` and back off.
- `ApiError::ApiVersion` is a terminal setup error with a clear message.
- A corrupt stored session is cleared and treated as "not paired" rather than
  crashing the app.
