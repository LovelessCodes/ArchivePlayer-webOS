# ArchivePlayer for webOS

A native LG webOS client for [ArchivePlayer](https://github.com/LovelessCodes/ArchivePlayer),
the self-hosted Archive.org media library. It draws its own UI with SDL2 + GLES2,
talks to the server over the documented HTTP API, and (in the next milestone)
hands Archive.org MP4s to the television's own media pipeline. No Chromium, no
WebView, no JavaScript runtime.

> **Status: scaffold.** The API client, platform seams, and host-testable UI
> model compile today. The SDL/GLES screen tree, pairing screen, and
> `com.webos.media` playback path are the next milestone. This repository is not
> yet installable on a television.

## Why a separate repository

- The desktop/server code is a **Rust + Tauri/Axum** workspace; this client needs
  the webOS NDK, an ARMv7 cross toolchain, SDL2, and LG's native service stubs.
- Its artifact is an unsigned `.ipk` for Developer Mode / Homebrew Channel, not a
  desktop installer or container image.
- It releases on its own cadence (`webos-v0.1.0`), against the versioned
  `apiVersion` contract rather than the server's release tag.
- Keeping the client separate makes the native toolchain and the MIT/GPL
  boundary explicit. This repository is a clean-room MIT implementation; see
  [LICENSE](LICENSE) and [NOTICE](NOTICE).

## What works now

| Area | State |
| --- | --- |
| Server URL validation (LAN HTTP, public HTTPS) | Done |
| LAN-first connect with remote fallback only on transport errors | Done |
| Bearer API client: health, status, library, continue-watching, search, playback | Done |
| Device pairing start/poll DTOs | Done |
| Playback-plan support (`platform=webos`, `file=...`) | Done |
| Startup state machine (`Setup → Pairing → Ready → Pairing` on 401) | Done |
| Token store: private file / memory | Done |
| mDNS `_archiveplayer._tcp` discovery | Interface only |
| SDL2 window, GLES2 renderer, focus tree | Not yet |
| `com.webos.media` playback and overlay | Not yet |
| `.ipk` packaging and device install | Not yet |

## Layout

```text
crates/protocol/   serde DTOs for the server API, no I/O
crates/api/        blocking ureq client, URL policy, server targets, sessions
crates/platform/   secret-store and discovery traits + host implementations
crates/ui/         screen ids and row-major focus model
crates/gfx/        renderer-independent Rect/Color types
crates/app/        startup state machine and the future event loop
pkg/               webOS appinfo and icons
docs/              architecture, build, protocol notes
```

## Host check

The scaffold builds and tests on an ordinary x86_64 machine; it does not need
the webOS NDK until the native shell lands.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Connecting to a server

1. Start ArchivePlayer on the LAN (`archiveplayer-server`, Docker, or the
   desktop app's server mode).
2. The client probes `GET /api/health` and refuses an `apiVersion` newer than it
   understands.
3. It starts the device flow with `POST /api/device/start` and shows the
   `userCode`/QR from the response.
4. After `POST /api/device/token` returns a token, it stores that token in a
   private app-data file and sends it as
   `Authorization: Bearer <token>`.
5. Playback calls `/api/playback/{identifier}?platform=webos` and uses
   `playbackPlan`. If `playbackPlan.compatible` is false, the UI explains why
   instead of starting a decode that will fail.

The full contract is in the server's
[docs/tv-clients.md](https://github.com/LovelessCodes/ArchivePlayer/blob/main/docs/tv-clients.md).

## Building the native package

The official webOS NDK has no x86_64 Linux build. Use macOS (Intel or Apple
silicon) or an **arm64** Linux host, install the NDK and the
`armv7-unknown-linux-gnueabi` Rust target, then set `WEBOS_NDK`. See
[docs/build.md](docs/build.md) for the toolchain, linker, and `.ipk` steps.

## Roadmap

1. Hardware spike on one webOS 5+ television: SDL2 EGL/GLES2 context, D-pad
   events, background/foreground lifecycle, exported window, and an Archive.org
   MP4 through `com.webos.media`.
2. Screen tree and poster/text renderer with fake data.
3. Pairing screen with QR, code entry, and secure token storage.
4. Library, search, detail, and player screens against the real API.
5. Progress every 15 s, on pause, and on stop; completed at end.
6. mDNS discovery, Homebrew manifest, unsigned `.ipk` release.

See [docs/architecture.md](docs/architecture.md) for the design and
[docs/protocol.md](docs/protocol.md) for the API surface this client uses.

## Attribution

ArchivePlayer is derived from the original
[ArchivePlayer](https://github.com/JeppeKjeldal/ArchivePlayer) by **Jeppe
Kjeldal**. This client credits that work in its README, LICENSE, NOTICE, and
future Settings → About screen.

This project is not affiliated with or endorsed by LG Electronics or the Internet
Archive. Archive.org content remains subject to its own terms.
