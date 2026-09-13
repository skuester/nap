# nap — Nice Audio Player

A little cassette deck for your desktop. One file, a paper label, two moving
reels, and a live waveform behind the plastic window. Made for the same quiet,
keyboard-first desktop as npr and nvp.

![nap cassette deck](docs/preview.png)

## Build and run

Requires Linux, a C++17 compiler, Make, and Qt **6.8 or later** with Quick,
Quick Controls, Dialogs, and the FFmpeg Multimedia backend. On Arch the relevant
Qt packages are `qt6-base`, `qt6-declarative`, `qt6-multimedia`, and
`qt6-multimedia-ffmpeg`. Tests also use Python 3 and Qt Test.

```sh
make -j4
./build/nap                         # empty deck; open or drop a file
./build/nap ~/Music/song.flac
./build/nap --paused --time 1:02.5 song.mp3
./build/nap --volume 40 --loop song.ogg
./build/nap --ignore-bookmark song.wav
make test -j4
```

`--time` also accepts `--start` and `--timestamp`; values can be seconds,
`m:ss`, or `h:mm:ss`, including fractional seconds. Explicit timestamps take
precedence over bookmarks. `--help` lists all options. Use `--` before a filename
that begins with a dash.

```sh
make install                       # ~/.local/bin and desktop launcher
make uninstall
```

Installation copies the self-contained executable and desktop entry. Re-run
`make install` after rebuilding. `PREFIX` and `DESTDIR` are supported. Installation
does not change MIME defaults or your window manager configuration.

## The deck

- **Label underline:** click or drag to seek. A small amber tick marks your bookmark.
- **REW / FWD:** tap to skip five seconds; hold to wind at 20 audio seconds per second.
- **PLAY / PAUSE, STOP, OPEN:** physical-style keys. Stop returns to the beginning.
- **Lower cassette ridges:** click, drag, or scroll to set the player's volume.
- **LOOP / MARK:** toggle repeat or save your place.
- **Plastic window:** a waveform drawn from decoded PCM samples, with no synthetic animation.
  Reels rotate only while playing; tape transfers from the left spool to the right.

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
The label keeps its warm paper color. Restart nap after changing themes.

The app ID is `nap`. An optional Lua rule for Hyprland 0.55+ is provided in
[`hypr/nap.lua`](hypr/nap.lua), ready to add to your existing configuration if
you want floating, centered windows. Tiled and resized windows scale the deck
uniformly. No desktop settings are modified by the app.

## Development

The C++ core owns playback, decoded samples, bookmarks, and the theme. QML draws
the cassette and routes input. All QML is embedded in the executable; there is no
runtime dependency on this checkout or a browser.

`make test` generates a silent-output test tone in a temporary directory, checks
real decoding and nonzero waveform samples, playback, seeks, rename-safe bookmark
restoration, keyboard and mouse input, CLI validation, and offscreen rendering.
Tests need access to an initializing Qt audio backend, even at zero volume.
The rendered empty deck is saved to `build/preview.png`.

For a preview without displaying a window:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic \
QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic \
./build/nap --screenshot /tmp/nap.png
```

The waveform uses Qt's [QAudioBufferOutput](https://doc.qt.io/qt-6/qaudiobufferoutput.html),
available with the FFmpeg backend since Qt 6.8. It reflects decoded audio before
the volume control, so lowering volume does not flatten the visualization.
