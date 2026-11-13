# Building

## Host check

This works on x86_64 Linux, macOS, and Windows:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The host check does not build an `.ipk` and does not need SDL or the webOS NDK.

## Native prerequisites

- Official **webOS NDK** (the native SDK, not the web app SDK).
- Rust stable with the `armv7-unknown-linux-gnueabi` target.
- SDL2 built for the webOS target (LG ships a patched fork; keep the event
  struct layout in mind).
- A macOS (Intel or Apple silicon) or **arm64 Linux** host. The official NDK
  installer has no x86_64 Linux build.
- `zip`/`ar` tooling for the `.ipk` (an `.ipk` is an `ar` archive containing
  `control.tar.gz` and `data.tar.gz`).

## Environment

```sh
export WEBOS_NDK="$HOME/webOS_TV_SDK/NDK"
export TARGET=armv7-unknown-linux-gnueabi
rustup target add "$TARGET"
```

Add a target linker in `.cargo/config.toml` once the NDK path is known:

```toml
[target.armv7-unknown-linux-gnueabi]
linker = "$WEBOS_NDK/arm-linux-gnueabi/bin/arm-linux-gnueabi-gcc"
rustflags = ["-C", "link-arg=--sysroot=$WEBOS_NDK/arm-linux-gnueabi/sysroot"]
```

Do not commit a machine-specific absolute path; use an untracked
`.cargo/config.local.toml` or a wrapper script. The final build must keep
`DT_NEEDED` minimal: link only what the firmware provides, and use runtime
`dlsym`/capability checks for firmware-specific symbols.

## Packaging

The planned package layout:

```text
pkg/
  appinfo.json
  icon.png
  largeIcon.png
  splash.png
  archiveplayer-webos       # ARMv7 ELF
  lib/                      # only libraries the TV does not provide
```

`appinfo.json` uses `"type": "native"`, `"main": "archiveplayer-webos"`, and
`nativeLifeCycleInterfaceVersion: 2`. A native package cannot be submitted to
LG Seller Lounge; distribution is via Developer Mode or the Homebrew Channel.

The `.ipk` is unsigned. Publish it with a SHA256 checksum and a Homebrew
manifest. Never tell users to disable verification or install a package from an
untrusted mirror.

## Device loop

1. Enable Developer Mode on the TV and install the Developer Mode app.
2. `ares-setup-device` / `ares-install` the `.ipk`.
3. `ares-launch codes.loveless.archiveplayer`.
4. Watch `journalctl`/`ares-inspect` logs for the first frame and the media
   pipeline result.

The first hardware milestone is deliberately small: launch, focus events, one
animated quad, a frame-time overlay, and one Archive.org MP4 through
`com.webos.media`. Do not build the full UI before that spike passes.
