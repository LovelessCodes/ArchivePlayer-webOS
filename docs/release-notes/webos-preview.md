# ArchivePlayer for webOS — native preview

This is an **unsigned preview build** of the native LG webOS client for
[ArchivePlayer](https://github.com/LovelessCodes/ArchivePlayer).

It is a real native application: SDL2 renders the interface directly on the
television, without Chromium, a WebView, or JavaScript. It discovers a server
on the local network over mDNS, pairs with an on-screen code, and renders the
live library, continue-watching rail, details, and Archive.org search.

**Video playback is not implemented yet.** The player screen and progress
reporting are ready, but the `com.webos.media` pipeline and exported-window
overlay are the next hardware milestone. Until then this is a UI and
connectivity preview.

## Install

1. Enable Developer Mode on the TV and install the Homebrew Channel, or use
   `ares-install`.
2. Download the `.ipk` from this release.
3. Verify it against `SHA256SUMS.txt` or the SHA-256 in
   `codes.loveless.archiveplayer.manifest.json`.
4. Install and launch **ArchivePlayer**.

The package is unsigned. Do not disable verification for packages from other
sources.

## Requirements

- webOS 5.0 or newer.
- An ArchivePlayer server on the same LAN, with mDNS enabled.
- The TV and server must share a network where multicast DNS is allowed
  (Docker bridge networks need host networking).

The Homebrew Channel manifest is always available at
`releases/latest/download/codes.loveless.archiveplayer.manifest.json`.
