# Building

## Host check and simulator

The UI can be developed on x86_64 Linux, macOS, and Windows. SDL2 is needed only
for the windowed simulator; the headless screenshot mode does not initialize
video.

```sh
sudo apt-get install libsdl2-dev      # Debian/Ubuntu
cargo run -p archiveplayer-webos-sim
cargo run -p archiveplayer-webos-sim -- --all docs/screenshots
cargo run -p archiveplayer-webos-sim -- --server http://127.0.0.1:8080
# Or, on a LAN with mDNS enabled:
cargo run -p archiveplayer-webos-sim -- --discover

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The host check does not build an `.ipk` and does not need the webOS NDK.

## Native package

The package is an unsigned webOS `.ipk` for webOS 5+ and ARMv7. It is installed
through Developer Mode or the Homebrew Channel; LG Seller Lounge does not accept
native packages.

### Toolchain

The official webOS NDK installer has no x86_64 Linux build. This project uses
the [openlgtv Buildroot NDK](https://github.com/openlgtv/buildroot-nc4), which
publishes an x86_64 Linux SDK containing the webOS sysroot and SDL2. The release
workflow pins `2026.08-webos` and its SHA-256. A local unpack looks like:

```text
$HOME/webos-ndk/arm-webos-linux-gnueabi_sdk-buildroot/
  bin/arm-webos-linux-gnueabi-gcc
  arm-webos-linux-gnueabi/sysroot/
    usr/include/SDL2/
    usr/lib/libSDL2-2.0.so.0
```

Install the Rust target once:

```sh
rustup target add armv7-unknown-linux-gnueabi
```

### Build

```sh
export WEBOS_NDK=$HOME/webos-ndk/arm-webos-linux-gnueabi_sdk-buildroot
make ipk
```

`make ipk` runs `scripts/build-arm.sh`, then `scripts/package.py`. The build
script sets the linker, C compiler, sysroot, and pkg-config paths for the target;
nothing machine-specific is committed to `.cargo/config.toml`.

The ARM build also links `crates/sim/src/getauxval.c`. webOS ships glibc 2.12,
which predates `getauxval`, but Rust's standard library and ring call it. The
fallback reads `/proc/self/auxv` and caches the answer. It is compiled only for
ARM targets and uses an MIT header.

### Package layout

```text
dist/codes.loveless.archiveplayer_<version>_arm.ipk
  debian-binary
  control.tar.gz            Package/Version/Architecture/Installed-Size
  data.tar.gz
    usr/palm/applications/codes.loveless.archiveplayer/
      appinfo.json
      archiveplayer-webos   ARMv7 ELF, stripped
      icon.png
      largeIcon.png
      splash.png
      LICENSE
      NOTICE
    usr/palm/packages/codes.loveless.archiveplayer/packageinfo.json
```

The staging step also writes

- `dist/codes.loveless.archiveplayer.manifest.json` — the Homebrew Channel
  manifest;
- `dist/codes.loveless.archiveplayer_<version>_arm.ipk.sha256` — the checksum.

The archive is built by Python rather than `ar`/`tar` so member names, mtimes,
owners, modes, and the gzip header are fixed. The same commit and toolchain
produce the same bytes. Do not add the build's local user or timestamp to the
package.

`Installed-Size` is in bytes, matching `ares-package`. The package includes
`usr/palm/packages/<id>/packageinfo.json`; without it the installer can unpack
the files but register no application.

SDL2 is not bundled. The binary's only non-base `DT_NEEDED` is
`libSDL2-2.0.so.0`, which the television firmware provides. The linked binary
also stays within `GLIBC_2.12`, matching the webOS sysroot.

### Homebrew manifest

The manifest is generated from `pkg/appinfo.json`:

```json
{
  "id": "codes.loveless.archiveplayer",
  "version": "0.1.0",
  "type": "native",
  "title": "ArchivePlayer",
  "appDescription": "Native ArchivePlayer client for LG webOS",
  "iconUri": "https://raw.githubusercontent.com/LovelessCodes/ArchivePlayer-webOS/main/pkg/icon.png",
  "sourceUrl": "https://github.com/LovelessCodes/ArchivePlayer-webOS",
  "rootRequired": false,
  "ipkUrl": "codes.loveless.archiveplayer_0.1.0_arm.ipk",
  "ipkHash": { "sha256": "…" },
  "ipkSize": 3307378,
  "installedSize": 6386101
}
```

`ipkUrl` is relative to the manifest, as in the official webOS Homebrew
packages. The release workflow publishes the manifest at
`releases/latest/download/codes.loveless.archiveplayer.manifest.json`, so the
Homebrew URL remains stable across versions. The version in the manifest, the
control file, `packageinfo.json`, the filename, and the git tag must agree.

### Release

Only `pkg/appinfo.json` carries the version. Change it, commit, and push a tag:

```sh
git tag webos-v0.1.1
git push origin webos-v0.1.1
```

`.github/workflows/release.yml` downloads and verifies the pinned NDK, builds
the ARM binary, packages the `.ipk`, and publishes:

- `codes.loveless.archiveplayer_<version>_arm.ipk`
- `codes.loveless.archiveplayer_<version>_arm.ipk.sha256`
- `codes.loveless.archiveplayer.manifest.json`
- `SHA256SUMS.txt`

The workflow refuses to publish when the tag version after `webos-v` differs
from `appinfo.json`. `docs/release-notes/webos-preview.md` is the release body;
update it when the preview's limitations change.

Do not mark a webOS release as a GitHub prerelease: `releases/latest` would stop
resolving and the Homebrew manifest URL would break.

## Install on a TV

1. Enable Developer Mode and install the Developer Mode app, or install the
   Homebrew Channel.
2. Verify the `.ipk` against `SHA256SUMS.txt`.
3. Install with the webOS CLI or the Homebrew Channel:

   ```sh
   ares-install -d <tv> codes.loveless.archiveplayer_0.1.0_arm.ipk
   ares-launch -d <tv> codes.loveless.archiveplayer
   ```

4. On first launch the client browses `_archiveplayer._tcp.local` for five
   seconds. If no server is found it opens the demo library; start the server
   with mDNS enabled and relaunch.

Never tell users to disable package verification or install from an untrusted
mirror.

## Device loop and hardware spike

The first hardware milestone is deliberately small: launch, focus events, one
animated quad, a frame-time overlay, and one Archive.org MP4 through
`com.webos.media`. The full screen design can be developed against the host
simulator meanwhile, but do not port it to GLES2 until the spike proves the
window-export and key-event path.

Native packaging does not prove the media pipeline. Current known gaps:

- SDL2 creates a fullscreen window on ARM, but the exported
  `option.windowId`/`com.webos.media` path is not wired yet.
- The player screen and progress tracker are present, but no video is decoded.
- The hardware key map (`Back`, `Exit`, play/pause, FF/RW) still needs a real
  remote test.
- The app writes its paired session beside `appinfo.json`; verify that directory
  is writable on the target firmware.
