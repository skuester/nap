# Checkout is the source of truth: installed files are symlinks.
PREFIX ?= $(HOME)/.local
BIN = $(CURDIR)/target/release/nap

.PHONY: all build test check install uninstall clean
all: build
build:
	cargo build --release --locked
test: build
	cargo test --locked
	mkdir -p build/tests
	cd build/tests && /usr/lib/qt6/bin/qmake ../../tests/player_test.pro && $(MAKE)
	QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic ./build/tests/player-test
	python3 tests/smoke.py
check: test
	cargo fmt --all --check
	cargo clippy --all-targets --locked -- -D warnings
install: build
	NAP_PREFIX="$(PREFIX)" "$(BIN)" --install-desktop "$(CURDIR)"
uninstall: build
	NAP_PREFIX="$(PREFIX)" "$(BIN)" --uninstall-desktop
clean:
	cargo clean
	rm -rf build
