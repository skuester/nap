# Checkout is the source of truth: installed files are symlinks.
PREFIX ?= $(HOME)/.local
BIN = $(CURDIR)/target/release/nap

.PHONY: all build test check crap install uninstall clean
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
# Change-risk scores: no function, Rust or C++, may exceed CRAP 12. Needs `cargo install crap4rs
# cargo-llvm-cov`; a distribution's Rust has no llvm-tools component, so point at the system LLVM.
ifeq ($(shell command -v rustup),)
export LLVM_COV ?= $(shell command -v llvm-cov)
export LLVM_PROFDATA ?= $(shell command -v llvm-profdata)
endif
crap: build
	mkdir -p build
	cargo llvm-cov --locked --lcov --output-path build/lcov.info
	crap4rs --src src --coverage build/lcov.info --threshold 12 --only-failing
	python3 tests/crap_cpp.py --threshold 12
install: build
	NAP_PREFIX="$(PREFIX)" "$(BIN)" --install-desktop "$(CURDIR)"
uninstall: build
	NAP_PREFIX="$(PREFIX)" "$(BIN)" --uninstall-desktop
clean:
	cargo clean
	rm -rf build
