# nap — Nice Audio Player

A little cassette deck for your desktop: a paper label, two moving reels, a live display behind
the plastic window, and a J-card insert with everything the file knows about itself. Play one
file, or line several up as a mixtape, sign it, and save it as a single `.tape` to hand to a
friend. Made for the same quiet, keyboard-first desktop as npr and nvp.

![nap cassette deck](docs/preview.png)

![the unfolded insert](docs/insert.png)

| bars | vu |
| --- | --- |
| ![spectrum analyzer](docs/bars.png) | ![VU meters](docs/vu.png) |
| **peak** | **spectrogram** |
| ![peak ladders and tape counter](docs/peak.png) | ![spectrogram](docs/spectrogram.png) |

## Build and run

Requires Linux, a Rust toolchain, a C++17 compiler, Make, pkg-config, and Qt **6.8 or later** with Quick,
Quick Controls, Dialogs, and the FFmpeg Multimedia backend. On Arch the relevant
Qt packages are `qt6-base`, `qt6-declarative`, `qt6-multimedia`, and
`qt6-multimedia-ffmpeg`. Tests also use Python 3 and Qt Test. `make install` uses `xdg-mime`
(xdg-utils), `update-mime-database` (shared-mime-info), and `update-desktop-database`
(desktop-file-utils).

```sh
make
./target/release/nap                         # empty deck; open or drop a file
./target/release/nap ~/Music/song.flac
./target/release/nap side-a/*.flac            # several files line up as one tape
./target/release/nap "Summer '98.tape"         # or a saved mixtape, or its .jcard
./target/release/nap --paused --time 1:02.5 song.mp3
./target/release/nap --track 4 --time 0:30 "Summer '98.tape"   # thirty seconds into its fourth track
./target/release/nap --volume 40 --loop song.ogg
./target/release/nap --ignore-bookmark song.wav
make test
```

`--time` also accepts `--start` and `--timestamp`; values can be seconds,
`m:ss`, or `h:mm:ss`, including fractional seconds. Explicit timestamps take
precedence over bookmarks. On a tape a time is a time in its first track unless `--track` names
another, counted from one; `--track` alone starts at that track's top, and on a single file, which
has only the one, it is ignored. `--help` lists all options. Use `--` before a filename
that begins with a dash.

```sh
make install                       # checkout links, Hyprland rules, file types, icons, defaults
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

nap's own file types are part of this. [`nap-mime.xml`](nap-mime.xml) defines
`application/x-nap-tape` (`*.tape`) and `application/x-nap-jcard` (`*.jcard`); the installer links
it into `~/.local/share/mime/packages`, then runs `update-mime-database` and
`update-desktop-database`, so a file manager knows what a `.tape` is and that nap opens it. A
`.tape` is declared a kind of tar, so archive tools still offer to open it, and it is known by
what is in it as well as by its name: a tar whose first entry is a `tape.jcard` is a tape,
whatever the file is called. Selecting several
audio files and opening them with nap lines them up as one tape.
Uninstall removes the definitions again.

nap and its two file types have their own icons, in [`icons/`](icons): the deck for the
application, a cassette for a `.tape`, and an unfolded J-card for a `.jcard`. They are linked into
the prefix's hicolor icon theme under the names the icon theme specification looks up.

`nap --install-hyprland [--link path/to/hypr/nap.lua]` and
`nap --uninstall-hyprland` manage just the window rules. An active Hyprland session
is reloaded and checked for configuration errors after installation/removal.
The full installer requires an existing Hyprland Lua configuration, `xdg-mime`,
`update-mime-database`, and `update-desktop-database`.

## The deck

- **Label's ruled line:** click or drag to seek; it inks over as the tape plays. A small diamond marks your bookmark.
- **REW / FWD:** tap to skip five seconds; hold to wind at 20 audio seconds per second.
  On a tape they are PREV / NEXT instead, the way a CD player's keys work: tap to change track
  (playing carries on playing), hold to wind through the current one.
- **PLAY / PAUSE, STOP, OPEN:** physical-style keys that follow a real deck's mechanism. PLAY is lit only
  while the head is engaged (playing or paused). STOP lifts the head and leaves the tape where it is;
  REW and FWD then become PREV and NEXT, which find the start of a track (on a single file, its beginning).
  While a mixtape has unsaved changes, OPEN is a red REC key that saves it (see [Mixtapes](#mixtapes)).
- **Lower cassette ridges:** click, drag, or scroll to set the player's volume.
- **LOOP / MARK:** toggle repeat or save your place. LOOP latches down; each key's lamp lights while it applies.
- **KEYS:** shows the key reference. On a tape with two sides it is a FLIP key instead, which turns the tape over,
  its lamp lit while side B is up; the keys are still a ? away.
- **The label's title strip:** click anywhere on it (or press I) to unfold the insert; the A mark
  turns over under the pointer as the hint. It is a J-card with the
  embedded cover art, a spine, and liner notes listing the lyrics, every tag in the file
  (custom fields included), and its technical details. Click the folder to open it in your file
  manager, and a cover picture to lift it off the card for a closer look. Files without art get a
  typeset cover.
- **Plastic window:** a live display drawn from decoded PCM samples, with no synthetic animation.
  Click it (or press V) to step through its scenes; the choice is remembered in
  `$XDG_STATE_HOME/nap/visualizer`:
  - *bars* (the default): a 24-band spectrum analyzer in shaded cells, with caps that hang and then fall.
  - *vu*: a pair of needle VU meters with real meter ballistics; 0 VU is -10 dBFS RMS.
  - *peak*: a deck's front panel, with segmented level ladders (-30 to +6 dB around -5 dBFS, with peak hold) over a
    mechanical tape counter whose wheels roll with the elapsed time and whirr when you seek.
  - *spectrogram*: the last few seconds of spectrum scrolling past in shaded cells, bass at the
    bottom; pausing holds the picture.
  - *scope*: the waveform in character cells.
  Reels rotate only while playing, the thinner pack turning faster; tape transfers from the left spool to the right.

The open button replaces what is in the deck; cancellation leaves it alone. Dragging
files, a `.tape`, or a `.jcard` onto the window loads them. nap has no library: it plays
the file or the tape you hand it. Common formats include MP3, FLAC, WAV, Ogg, Opus, M4A, AAC,
and AIFF; actual decoding support follows the installed Qt FFmpeg backend.

| Key | Action |
| --- | --- |
| Space | Play / pause |
| ← / → | Back / forward five seconds; keyboard repeat continues seeking |
| ↑ / ↓ | Volume in five-percent steps |
| M | Mute / unmute |
| S | Stop, leaving the tape where it is |
| , / . | Previous / next track start |
| L | Toggle loop |
| B | Bookmark current position |
| Shift+B | Remove bookmark |
| Enter | Return to bookmark |
| I | Unfold / put away the insert (↑ ↓ PgUp PgDn scroll it) |
| F | Flip a two-sided tape over; with a track picked out in the insert, start side B there |
| V | Change the visualizer |
| Ctrl+S | Save the tape (.tape or .jcard) |
| O / Ctrl+O | Open files, a tape, or a J-card |
| ? / K / Ctrl+K | Toggle help |
| Delete | Take the picked-out track off the tape (with the insert open) |
| Esc | Close help; otherwise put back a lifted cover or note; otherwise put away the insert |
| Q | Quit (asked twice if a tape is unsaved) |

Bookmarks are stored as milliseconds in the file's `user.nap.bookmark` extended
attribute. They survive renaming and moving on the same filesystem; copying
between filesystems requires preserving extended attributes. Read-only files or
filesystems without xattrs show an error instead of pretending to save. There is
one explicit bookmark per file, and quitting does not overwrite it.

A tape has one bookmark too, for the whole tape: which track, and how far into it. It is kept the
same way, on the `.tape` (or `.jcard`) itself and never on the files it lists, as `3:62500` for
62.5 seconds into the third track. Open the tape again and it comes up where you left it, one side
or two; `--ignore-bookmark`, `--time`, and `--track` start it where they say instead. The diamond shows on the
label's ruled line while the marked track is up and beside that track in the insert's listing, the
MARK lamp is lit while the tape has a mark anywhere, and Enter goes to it from any track. The mark is
a track number and a time and nothing else, because a tape once made is as good as fixed: a file
listed twice keeps its true place, and if you do move tracks about, the mark stays with the
number. Saving writes a new file, and the mark is put on it again. Files lined up but not yet saved as a tape have nowhere to keep one. Being
an attribute of the file rather than part of its contents, the mark stays behind when a tape is
handed on: whoever gets it starts at the top, not where its maker stopped listening.

## Mixtapes

Open the insert and drop audio onto it: the front panel becomes a track listing, and the deck is
now playing a tape. Drag rows to reorder, double-click one to play it, and use the × on a row (or
Delete) to take it off. Double-click the tape's name to retitle it in place. Drop an image on the
cover to make it the tape's own. The liner notes follow whichever track you pick out.

A tape for a friend wants signing: double-click "Sign your name" under the tape's name and type
straight onto the card. "Tuck in a note" beneath it lifts out a slip of ruled paper to write on;
once there is one it reads "Read the note", and whoever gets the tape finds it in the same place.
On the slip, Enter starts a new line; Ctrl+Enter or a click elsewhere keeps it, and Esc gives it
up. Click a cover picture to lift it off the card for a closer look too. Neither the picture nor
the note ever lands quite straight; a click beside them or Esc puts them back.

A tape can have two sides. Point at a track in the listing and click the B that appears beside
its ×, or pick the track out and press F: side B starts there, under a rule of its own, and the
card numbers its tracks the way J-cards do, A1, A2, B1. Click the same B again to go back to one
side. Tracks dragged across the turn change sides, and the turn goes away by itself if either
side is emptied. The side mark on the cassette's label says which side is up, and FLIP (or F)
turns the tape over to the start of the other one, carrying on playing if it was. When side A
runs out the deck stops, as a deck does, with side B turned up ready for PLAY; with LOOP on it is
an auto-reverse deck and plays straight through.

![a mixtape's insert](docs/mixtape.png)

![the note that came with the tape](docs/note.png)

Ctrl+S saves the tape, and the display between the reels becomes the progress bar while it packs.
While a tape has unsaved changes the OPEN key becomes a red REC key that does the same, since
OPEN is what would throw the tape away. Opening something else, or quitting, then has to be asked
twice: the first time nap only says the tape is unsaved; doing it again within a few seconds
discards it.

- **`Name.tape`** is self-contained: a plain, uncompressed tar of one folder holding the index
  (`tape.jcard`), the optional cover (`cover.jpg`, or whatever kind of image it is), and the
  audio. `tar -xf Name.tape` gets everything back out. nap plays it as it is: because the tar is
  not compressed, each track lies inside it whole, so a tape of any size opens at once and
  nothing is unpacked anywhere. That is also why a tape is never compressed; the audio already
  is. Entries carry no owner, so a tape says nothing about whose machine it was made on. A
  tape can be saved over itself while it plays.
- **`Name.jcard`** is the index alone, for tapes whose audio stays where it is. Choose it in the
  save dialog, or write one by hand. If some of its files have since gone missing, nap plays
  the rest and says which it could not find.

The index is M3U-compatible text, so it is easy to edit and other players can read it. One track
per line, in order; paths are relative to the index or absolute; `#` lines that nap does not
know are ignored. Inside a `.tape` the index can list only what the tape holds:

```text
#EXTM3U
#EXTENC:UTF-8
#NAP:1
#PLAYLIST:Summer '98
#EXTIMG:cover.jpg
#NAP-FROM:Shane
#NAP-NOTE:Made this for the drive up.
#NAP-NOTE:Side B is the good one.

#NAP-SIDE:A
#EXTINF:151,Boards of Canada - Roygbiv
01 Roygbiv.flac

#NAP-SIDE:B
#EXTINF:242,The Rapture - Don't Stop
02 Don't Stop.mp3
/home/me/Music/03 far away.ogg
```

The `#NAP` lines are nap's own; M3U has no field for them, but players skip `#` lines they do not
know, so the file stays a valid playlist. `#NAP:` is the version of the format, so that a later
nap can tell an old tape from a broken one, and this one refuses a tape from a nap newer than
itself rather than misread it. `#NAP-FROM:` is who signed the tape and `#NAP-NOTE:` what they
wrote, one per line of the note. `#NAP-SIDE:B` goes before the first track of side B on a tape
that has two sides (`#NAP-SIDE:A` is written for symmetry; side A is whatever comes first).
`#EXTENC:` and `#EXTINF:` (length in seconds, then artist and
title) are for other players, which otherwise guess at the encoding and show file names; nap
reads titles and lengths from the audio itself, and uses `#EXTINF:` only to name a track that
has gone missing. A file whose own name starts with `#` is written as `./#name`, so it is not
taken for a comment.

On a tape, PREV and NEXT move between tracks, across sides too, and always wrap around. When the last track ends the
deck auto-stops, cued back at track one; with LOOP on the tape starts over instead. A tape
has one bookmark of its own (see above), so its tracks always start at their beginning whatever
marks their files may carry.

## Omarchy

nap reads `background`, `foreground`, `accent`, and `color1` (its red, for REC) from the active theme's
`colors.toml`, checking `$XDG_STATE_HOME/omarchy/current/theme` first, then
`$XDG_CONFIG_HOME/omarchy/current/theme` (with standard home-directory defaults).
Every surface is mixed from the first three, so the deck follows light and dark themes alike:
the label is foreground-colored paper printed in background-colored ink. The red is used for
the REC key and nothing else.
Restart nap after changing themes.

The app ID is `nap`. The installed [Hyprland 0.55+ Lua rules](hypr/nap.lua)
float and center the window, preserve its 720:504 aspect ratio on resize, and
disable dimming and Omarchy's default translucency. Tiling still works; the deck
scales uniformly inside the available window. Ordinary playback does not modify
desktop settings.

## Development

Rust owns CLI parsing, startup/seek/volume policy, the transport's state machine, file
validation, bookmarks, tag reading, mixtapes (the index format, `.tape` archives, the tape in the
deck and its editing), the visualizers' signal analysis, theme loading, desktop installation, and
MIME restoration. The C++ adapter in
`native/` owns Qt Multimedia objects and forwards requests to the Rust core over
a synchronous C ABI. QML draws the cassette and routes input. The Qt adapter and
QML resources are embedded in the Rust executable by `build.rs`; there is no
runtime dependency on a separate helper process, Quickshell, or a browser.
[docs/design.md](docs/design.md) records the design language and how to work on the interface.

`cargo test --locked` runs Rust unit/integration tests for the core and installer.
`make check` runs those plus Qt/CLI tests, `cargo fmt --all --check`, and
`cargo clippy --all-targets --locked -- -D warnings`, following the siblings.
Installer tests use temporary configurations, a fake MIME backend, and stand-in scripts for the
system tools; the tests that run the real executable give it a sandboxed home. None of them touch
your desktop preferences.

`make test` generates a silent-output test tone in a temporary directory, checks
real decoding and nonzero waveform samples, playback, seeks, rename-safe bookmark
restoration, a whole mixtape (track changes, auto-advance, editing, saving, reloading), keyboard
and mouse input, CLI validation, and offscreen rendering.
Tests need access to an initializing Qt audio backend, even at zero volume.
The rendered empty deck is saved to `build/preview.png`, with the insert, a mixtape's insert, and
its note beside it.

`make crap` scores every function for change risk (CRAP = complexity² × (1 − coverage)³ +
complexity) and fails if any exceeds 12. Rust is scored by
[crap4rs](https://crates.io/crates/crap4rs) over `cargo llvm-cov` coverage; the C++ adapter by
`tests/crap_cpp.py`, which does the same sum with gcov over both the Qt test and the real binary.
It needs `cargo install crap4rs cargo-llvm-cov`.

For a preview without displaying a window:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic \
QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic \
./target/release/nap --screenshot /tmp/nap.png
```

The visualizers use Qt's [QAudioBufferOutput](https://doc.qt.io/qt-6/qaudiobufferoutput.html),
available with the FFmpeg backend since Qt 6.8. It reflects decoded audio before
the volume control, so lowering volume does not flatten them.

## License

Copyright © 2026 Shane Kuester

nap is free software: you can redistribute it and/or modify it under the terms
of the GNU Affero General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. It comes with no warranty. See [LICENSE](LICENSE) for the full text.
