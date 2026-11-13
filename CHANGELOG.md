# Changelog

## 0.2.0 — hardware playback

- Play Archive.org MP4s through `com.webos.media` on an exported SDL window.
- Clear the UI surface's Wayland opaque region so the hardware plane shows
  through beneath the player overlay.
- Drive play, pause, seek, position, end-of-stream, and teardown from inside the
  app via `libluna-service2`; no `luna-send` shell-out.
- Remote keys: focus the card after its first present, translate LG's raw
  scancode keycodes, and handle webOS 5's private Back key (`482`).
- Report real playback position to the server every 15 s, on pause, on Back, and
  at completion.
- Document the hardware findings in `docs/hardware-spike.md`.

## 0.1.0 — first native preview

- Native 1920×1080 UI with seven screens and spatial D-pad focus.
- Device pairing, stored bearer sessions, live library/detail/settings.
- Archive.org search and add-to-library.
- mDNS discovery, remote poster and backdrop decoding.
- ARMv7 build with the openlgtv NDK and a deterministic unsigned `.ipk`.
