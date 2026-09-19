# Repository Guidelines

## Product & Design Direction

Read [brief.md](brief.md) before changing behavior or UI. Keep nap simple and consistent with sibling projects `~/code/npr` and `~/code/nvp`. Balance Omarchy minimalism with a tactile, skeuomorphic cassette player: filename label, spools that rotate only during playback, a waveform between them, and large square transport buttons below. Preserve five-second REW/FWD taps and continuous seeking while held. Follow sibling keyboard conventions, including `?` for help, Space for play/pause, `b` for bookmark, and `q` for quit.

## Project Structure & Module Organization

nap is a Linux audio player built with Rust 2024, C++17, and Qt 6.8+.

- `src/*.rs`: CLI parsing, playback policy, bookmarks, themes, desktop installation, and the C ABI in `ffi.rs`.
- `native/`: Qt Multimedia adapter. Keep policy in Rust and Qt objects here.
- `src/Main.qml`, `Reel.qml`, `Transport.qml`: interface; `resources.qrc` registers QML resources.
- `build.rs`: compiles native code and embeds resources.
- `tests/`: Rust integration tests, Qt Test coverage, and Python CLI/render smoke checks.
- `hypr/nap.lua` and `nap.desktop`: desktop integration. Build output: `target/` and `build/`.

## Build, Test, and Development Commands

Install Rust, a C++17 compiler, Make, pkg-config, Qt 6.8+ Quick/Controls/Dialogs/Multimedia with the FFmpeg backend, Qt Test, and Python 3.

- `make -j4`: build the release executable.
- `./target/release/nap song.flac`: run locally.
- `cargo test --locked`: run Rust tests.
- `make test -j4`: run Rust, Qt, and Python tests; render `build/preview.png`.
- `make check`: run all tests, formatting checks, and Clippy with warnings treated as errors.
- `cargo fmt --all`: format Rust code.

## Coding Style & Naming Conventions

Use four-space indentation and Makefile recipe tabs. Follow `rustfmt.toml`: 120-column width and `use_small_heuristics = "Max"`. Use `snake_case` for Rust functions/modules, `PascalCase` for types and QML components, and camelCase for Qt methods and QML properties. Explain unsafe Rust with `SAFETY` comments.

## Testing Guidelines

Name Rust integration files `*_tests.rs` and test functions descriptively in `snake_case`. Add regression coverage in the relevant Rust suite, `tests/player_test.cpp`, or `tests/smoke.py`. No numeric coverage threshold is configured. Playback tests require an initializing Qt audio backend. Keep installer tests isolated with temporary configurations and fake MIME handlers.

## Commit & Pull Request Guidelines

Use focused commits with short subjects, like the existing `Improve labels`; no formal prefix convention exists. PRs should describe changes, link relevant issues, report checks, and include screenshots for UI changes.

## Configuration Precautions

`make install` modifies desktop links, Hyprland configuration, and audio defaults. Develop locally; validate installers with isolated fixtures.
