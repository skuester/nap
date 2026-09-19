# nap — Nice Audio Player

A little cassette deck for your desktop. One file, a paper label, two moving
reels, and a live waveform behind the plastic window. Made for the same quiet,
keyboard-first desktop as npr and nvp.

![nap cassette deck](docs/preview.png)

![the unfolded insert](docs/insert.png)

## Build and run

Requires Linux, a Rust toolchain, a C++17 compiler, Make, pkg-config, and Qt **6.8 or later** with Quick,
Quick Controls, Dialogs, and the FFmpeg Multimedia backend. On Arch the relevant
Qt packages are `qt6-base`, `qt6-declarative`, `qt6-multimedia`, and
`qt6-multimedia-ffmpeg`. Tests also use Python 3 and Qt Test.

```sh
make -j4
./target/release/nap                         # empty deck; open or drop a file
./target/release/nap ~/Music/song.flac
./target/release/nap --paused --time 1:02.5 song.mp3
./target/release/nap --volume 40 --loop song.ogg
./target/release/nap --ignore-bookmark song.wav
make test -j4
```

`--time` also accepts `--start` and `--timestamp`; values can be seconds,
`m:ss`, or `h:mm:ss`, including fractional seconds. Explicit timestamps take
precedence over bookmarks. `--help` lists all options. Use `--` before a filename
that begins with a dash.

```sh
make install                       # checkout links, Hyprland rules, audio defaults
make uninstall
```

Like npr and nvp, installation symlinks the release binary and desktop entry
into `~/.local`, and `hypr/nap.lua` into `~/.config/hypr`. Rebuilding updates the
installed application immediately. `PREFIX` changes the binary/desktop-entry
destination; this is a user installation, not a `DESTDIR` package-staging target.

The installer backs up `hyprland.lua` and adds `require("hypr.nap")` once. It sets
nap as the default for every type listed by `nap --mime-types`, recording the
displaced handlers in `$XDG_STATE_HOME/nap/previous-audio-handlers.json` (normally
`~/.local/state/nap`). Repeated installs keep this history; `make clean` leaves it
intact. Uninstall removes the links and require line, restoring defaults only
where nap is still selected. Manually edited rules are kept as a backup.

`nap --install-hyprland [--link path/to/hypr/nap.lua]` and
`nap --uninstall-hyprland` manage just the window rules. An active Hyprland session
is reloaded and checked for configuration errors after installation/removal.
The full installer requires an existing Hyprland Lua configuration and `xdg-mime`.

## The deck

- **Label's ruled line:** click or drag to seek; it inks over as the tape plays. A small diamond marks your bookmark.
- **REW / FWD:** tap to skip five seconds; hold to wind at 20 audio seconds per second.
- **PLAY / PAUSE, STOP, OPEN:** physical-style keys. Stop returns to the beginning.
- **Lower cassette ridges:** click, drag, or scroll to set the player's volume.
- **LOOP / MARK:** toggle repeat or save your place. LOOP latches down; each key's lamp lights while it applies.
- **The label's A mark:** click it (or press I) to unfold the insert, a J-card with the
  embedded cover art, a spine, and liner notes listing the lyrics, every tag in the file
  (custom fields included), and its technical details. Click the folder to open it in your file manager. Files without art get a typeset cover.
- **Plastic window:** a waveform drawn from decoded PCM samples, with no synthetic animation.
  Reels rotate only while playing, the thinner pack turning faster; tape transfers from the left spool to the right.

The open button replaces the loaded file; cancellation leaves it alone. Dragging
a local file onto the window loads it. nap plays one file at a time, without a
library or playlist. Common formats include MP3, FLAC, WAV, Ogg, Opus, M4A, AAC,
and AIFF; actual decoding support follows the installed Qt FFmpeg backend.

| Key | Action |
| --- | --- |
| Space | Play / pause |
| ← / → | Back / forward five seconds; keyboard repeat continues seeking |
| ↑ / ↓ | Volume in five-percent steps |
| M | Mute / unmute |
| S | Stop and rewind to the start |
| L | Toggle loop |
| B | Bookmark current position |
| Shift+B | Remove bookmark |
| Enter | Return to bookmark |
| I | Unfold / put away the insert (↑ ↓ PgUp PgDn scroll it) |
| O / Ctrl+O | Open file |
| ? / K | Toggle help |
| Esc | Close help |
| Q | Quit |

Bookmarks are stored as milliseconds in the file's `user.nap.bookmark` extended
attribute. They survive renaming and moving on the same filesystem; copying
between filesystems requires preserving extended attributes. Read-only files or
filesystems without xattrs show an error instead of pretending to save. There is
one explicit bookmark per file, and quitting does not overwrite it.

## Omarchy

nap reads `background`, `foreground`, and `accent` from the active theme's
`colors.toml`, checking `$XDG_STATE_HOME/omarchy/current/theme` first, then
`$XDG_CONFIG_HOME/omarchy/current/theme` (with standard home-directory defaults).
Every surface is mixed from those three colors, so the deck follows light and dark
themes alike: the label is foreground-colored paper printed in background-colored ink.
Restart nap after changing themes.

The app ID is `nap`. The installed [Hyprland 0.55+ Lua rules](hypr/nap.lua)
float and center the window, preserve its 720:504 aspect ratio on resize, and
disable dimming and Omarchy's default translucency. Tiling still works; the deck
scales uniformly inside the available window. Ordinary playback does not modify
desktop settings.

## Development

Rust owns CLI parsing, startup/seek/volume policy, file validation, bookmarks,
theme loading, desktop installation, and MIME restoration. The C++ adapter in
`native/` owns Qt Multimedia objects and forwards requests to the Rust core over
a synchronous C ABI. QML draws the cassette and routes input. The Qt adapter and
QML resources are embedded in the Rust executable by `build.rs`; there is no
runtime dependency on a separate helper process, Quickshell, or a browser.

`cargo test --locked` runs Rust unit/integration tests for the core and installer.
`make check` runs those plus Qt/CLI tests, `cargo fmt --all --check`, and
`cargo clippy --all-targets --locked -- -D warnings`, following the siblings.
Installer tests use temporary configurations and a fake MIME backend, never
your desktop preferences.

`make test` generates a silent-output test tone in a temporary directory, checks
real decoding and nonzero waveform samples, playback, seeks, rename-safe bookmark
restoration, keyboard and mouse input, CLI validation, and offscreen rendering.
Tests need access to an initializing Qt audio backend, even at zero volume.
The rendered empty deck is saved to `build/preview.png`.

For a preview without displaying a window:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic \
QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic \
./target/release/nap --screenshot /tmp/nap.png
```

The waveform uses Qt's [QAudioBufferOutput](https://doc.qt.io/qt-6/qaudiobufferoutput.html),
available with the FFmpeg backend since Qt 6.8. It reflects decoded audio before
the volume control, so lowering volume does not flatten the visualization.
