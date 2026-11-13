#!/usr/bin/env bash
# Cross-build the native TV binary with the openlgtv Buildroot NDK.
# Usage: WEBOS_NDK=/path/to/arm-webos-linux-gnueabi_sdk-buildroot scripts/build-arm.sh
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "$0")/.." && pwd)
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
: "${WEBOS_NDK:?set WEBOS_NDK to the arm-webos-linux-gnueabi_sdk-buildroot directory}"

SYSROOT="$WEBOS_NDK/arm-webos-linux-gnueabi/sysroot"
TOOLCHAIN="$WEBOS_NDK/bin/arm-webos-linux-gnueabi-"
[ -d "$SYSROOT" ] || { echo "missing sysroot: $SYSROOT" >&2; exit 1; }
[ -x "${TOOLCHAIN}gcc" ] || { echo "missing compiler: ${TOOLCHAIN}gcc" >&2; exit 1; }

export CC_armv7_unknown_linux_gnueabi="${TOOLCHAIN}gcc"
export AR_armv7_unknown_linux_gnueabi="${TOOLCHAIN}ar"
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABI_LINKER="${TOOLCHAIN}gcc"
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABI_RUSTFLAGS="-C link-arg=--sysroot=$SYSROOT"
export PKG_CONFIG="$WEBOS_NDK/bin/pkg-config"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_LIBDIR="$SYSROOT/usr/lib/pkgconfig"

cd "$ROOT"
cargo build --release -p archiveplayer-webos-sim --target armv7-unknown-linux-gnueabi
printf '%s\n' "$ROOT/target/armv7-unknown-linux-gnueabi/release/archiveplayer-webos-sim"
