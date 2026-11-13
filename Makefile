CARGO ?= cargo

.PHONY: check fmt clippy test run sim screenshots arm ipk

check: fmt clippy test

fmt:
	$(CARGO) fmt --all --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --workspace

run: sim

sim:
	$(CARGO) run -p archiveplayer-webos-sim

screenshots:
	$(CARGO) run -p archiveplayer-webos-sim -- --all docs/screenshots

# Native package build. The openlgtv Buildroot NDK is an x86_64 Linux
# toolchain, so this works on the same host that runs the simulator.
WEBOS_NDK ?= $(HOME)/webos-ndk/arm-webos-linux-gnueabi_sdk-buildroot
arm:
	WEBOS_NDK="$(WEBOS_NDK)" scripts/build-arm.sh

ipk: arm
	python3 scripts/package.py --strip "$(WEBOS_NDK)/bin/arm-webos-linux-gnueabi-strip"
