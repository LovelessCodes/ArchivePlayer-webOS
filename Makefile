CARGO ?= cargo

.PHONY: check fmt clippy test run ipk

check: fmt clippy test

fmt:
	$(CARGO) fmt --all --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --workspace

run:
	$(CARGO) run -p archiveplayer-webos

# The native package step needs the webOS NDK and a cross-built binary. Keep the
# failure explicit instead of producing a host x86_64 .ipk that cannot run.
ipk:
	@echo "The .ipk target is not implemented yet."
	@echo "See docs/build.md; it requires the webOS NDK on macOS or arm64 Linux."
	@exit 1
