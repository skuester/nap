# nap design guide

How nap looks and feels, and why, for whoever works on it next. Read [brief.md](../brief.md)
first; this is how that brief was answered. The screenshots in this folder show the results.

## The one idea

**Skeuomorphic in anatomy, minimal in rendering.** Every part of nap is a real part of a real
cassette or deck, in the right place, behaving the way it really behaves. But it is drawn the
Omarchy way: flat fills mixed from the theme, hairline borders, one monospace face, no gradients
pretending to be plastic, no gloss, no textures.

Get the first half wrong and it is a generic player with a cassette sticker on it. Get the second
half wrong and it is a 2009 iPhone app that looks absurd next to a terminal. Hold both.

When you add something, find the physical thing first. The best features here started as "what
was this, on a real tape?":

| Need | The physical thing it became |
| --- | --- |
| Seek bar | The last ruled line on the cassette's label, inking over as it plays |
| Volume | The grip grooves on the ridge under the tape |
| File metadata | The J-card insert, unfolded: cover, inked spine, liner notes |
| A playlist | A mixtape; its track listing is the J-card's front panel |
| A message with a playlist | A slip of ruled paper tucked into the case |
| Looking closely at cover art | Lifting the print off the card; it never lands quite straight |
| Level meters | VU needles on paper dials; a deck's LED ladder and rolling tape counter |
| Stop vs. pause | The head lifting off the tape; REW/FWD then become track search |
| Skipping tracks on a tape | A CD player's keys: tap to change track, hold to wind |
| Saving an unsaved tape | The red REC key, taking OPEN's place because OPEN is what would discard it |
| A progress bar | The deck's own display, in the same 4px cells as the visualizers |
| Tape length | The "C-60" class printed on every blank tape, computed from the real length |

If a feature has no physical counterpart, that is a signal to ask whether it belongs.

## Colour: three inputs, everything derived

nap reads exactly three colours from the Omarchy theme: `background`, `foreground`, `accent`.
**Never hardcode a colour in QML.** Every surface is mixed from those three, which is why the deck
works in any theme, light or dark, without a second design. The tokens live at the top of
`src/Main.qml`; use them, and pass them into components as properties (components never reach for
`win`).

| Token | What it is | Made from |
| --- | --- | --- |
| `bg` / `fg` | Window and text | The theme |
| `shell` | Cassette body, key caps | `bg` + 7% `fg` |
| `well` | Recesses: tape window, key wells, unlit grooves | `bg` darkened |
| `dim` | Secondary text on the shell | `bg` + 50% `fg` |
| `paper` | The label, the J-card, VU dial faces | `fg` + 10% `bg` |
| `ink` / `inkDim` | Anything printed on paper | `bg`; `paper` + 62% `bg` |
| `glow` | Anything that carries a signal on a dark surface | `accent`, lifted toward `fg` if it lacks contrast |
| `stripe` | Decoration on paper | `accent`, or `inkDim` if it would vanish |
| `accentInk` | Text on an accent fill | Whichever of `fg`/`bg` reads better |

Two rules fall out of this, and both were learned the hard way:

- **The accent is decoration until proven otherwise.** Some themes pick an accent with almost no
  contrast against the background (the author's own did). Anything that must be *read* (waveform,
  lamps, lit grooves, links) uses `glow`, which checks contrast and compensates. Raw `accent` is
  only for fills and stripes.
- **Paper is the inverse of the screen.** `paper` is foreground-coloured with background-coloured
  ink. On paper, information is always `ink`/`inkDim`, never the accent, because accent-on-paper
  contrast is unpredictable across themes.

There is one unmixed colour: `red`, the theme's own `color1`, used only for the REC key, because on
a deck that is the one thing that is always red. Do not reach for it elsewhere.

Icons (`icons/*.svg`) are the other exception: they cannot follow the theme, so they use the warm
palette from the README screenshots.

## Type

One family: `monospace`, which resolves to the user's Omarchy font. Size and weight do all the
work: 19px bold for the label title, 13-15px bold for card headings, 10-11px for detail, 9px with
letter-spacing for key captions. Italic is reserved for the human voice: the signature, the note.

Key captions are capitals (PLAY, REW) because deck keys are engraved that way. Everything else is
sentence case. Copy is plain and tells you what to do: "Drop audio or a tape here, or press O to
open one", "Sign your name", "A tape needs at least one track". Notices name what happened
("Saved Summer '98.tape"). No exclamation marks, no apologies, no cleverness that costs clarity.

## Layout and hierarchy

- The design is a fixed 720x504 canvas scaled to the window (the Hyprland rule keeps the ratio).
  Work in those coordinates.
- **The cassette is the hero.** One memorable object; everything around it stays quiet. When
  something new competes with it, the new thing loses: a paper tab and then a second icon were
  both tried for opening the insert and both removed. The final answer added nothing visible:
  the whole title strip is the target, and the existing "A" mark flips under the pointer as a
  hint.
- **Forgiving targets, quiet hints.** Big invisible hit areas; small visible affordances.
- **On the J-card, left is the tape and right is the track.** Tape-level things (name, signature,
  cover, listing, the note's link) live on the left; the liner notes on the right always describe
  one track. A note was once put on the right and it was wrong for exactly this reason.
- Weight reads as darkness: a thick-stroked icon looks darker than thin text of the same colour.
  Dim heavy things further than you think you need to.

## Motion

Motion is physical and sparse. Things move because a mechanism would move them:

- Keys travel into their wells (70ms); PLAY and LOOP latch down while engaged.
- Reels turn only while playing, anticlockwise (side A), and the thinner pack turns faster because
  tape speed is constant. Held REW/FWD spins them fast the right way.
- The insert rises, then its notes panel unfolds from the spine. One orchestrated moment.
- Bars fall, caps hang then drop with acceleration, VU needles are lightly underdamped springs
  (about 300ms rise, a hint of overshoot), the tape counter's wheels roll and carry like an
  odometer and whirr on a seek.

No hover animations for their own sake, no entrance effects on ordinary elements. Use only affine
transforms (translate, scale, 2D rotate): they work on every Qt Quick backend, including the
software one the tests use.

## The display between the reels

All scenes share a grammar so they feel like one instrument:

- **The 4px cell grid**: 3px cells, 1px gaps. Scope, bars, spectrogram, peak ladder, and the
  save/load progress bar are all built from it. New digital-looking scenes should be too.
- **Shading by level, not by hue**: dim `glow` low, `glow` in the middle, `fg` at the top.
- **Analog scenes use paper**: the VU dials are `paper` with `ink` markings, tying them to the
  label and the J-card rather than to a screen.
- **Honest data, tuned for music.** Everything is drawn from decoded audio (FFT and levels are in
  `src/meter.rs`); nothing is faked. But calibrate against real mastered tracks: a true
  sample-peak meter sat pinned at the top and looked dead until it followed short-term level on a
  deck-style scale instead. Measure a real file before choosing a scale.
- Analysis lives in Rust; ballistics (decay, hang, spring) live in QML.

## Protecting work without dialogs

nap has no modal dialogs of its own. Unsaved work is protected two ways instead: the dangerous
control turns into the safe one (OPEN becomes REC while a tape is unsaved), and anything that
would still discard the tape must be asked twice (the first attempt only explains, in a notice;
the same attempt within a few seconds goes through). Reach for that pattern before a dialog.

## Writing on things

Text the user edits is edited **in place, with no input box**: double-click the words and they
become editable where they sit, same font, same position (`Editable` in `src/Insert.qml`). Enter
keeps a single line; in multi-line notes Enter breaks the line and Ctrl+Enter keeps it; Esc gives
up; a click elsewhere keeps. Empty fields show a faint invitation ("Tuck in a note"). While
anything is being typed, every window shortcut is disabled (`win.typing`), or Space and S would
fire mid-word.

## How to work on the UI here

This process is most of why the design holds together. Do not skip it.

1. **Look at every change.** Render offscreen and open the PNG:
   `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic ./target/release/nap --volume 0 --screenshot out.png file.flac`.
   Always pass `--volume 0`, or the render plays out loud. Crop and upscale the region you
   changed; small details hide at 1x.
2. **States with no CLI route** (insert open, a row selected, a forced progress value): add a
   one-line temporary `Timer` in `Main.qml` tagged `// TEMP-PREVIEW`, render, delete it, and grep
   that none remain before committing.
3. **Check other themes**, at least one light. Point `XDG_STATE_HOME` at a folder containing
   `omarchy/current/theme/colors.toml`. The same folder isolates the visualizer preference
   (`nap/visualizer`) so renders never touch the user's real state.
4. **Make test audio with ffmpeg** (tagged, with cover art and lyrics) rather than guessing how
   real files will look. For meters and visualizers, test against real music too.
5. **The Qt test asserts zero QML warnings.** Keep it that way; extend
   `tests/player_test.cpp` to open any new view so it is covered.
6. Finish with `make check` and `make crap` (see AGENTS.md), and guard commits on both passing.

## Traps already stepped in

- QML ids that shadow built-ins fail in confusing ways: `print` is a global function, `icon` is a
  final property of `Button`, and inside a `TextEdit` the name `insert` is its own method (the
  J-card's id is `insert`, so editors route through the enclosing `Editable`).
- `Canvas` blurs when the design is scaled up. Use `Shape` with `CurveRenderer` for icons and
  arcs, and plain `Rectangle`s for anything on the cell grid.
- A fading overlay still eats clicks. Disable input the moment something is dismissed
  (`enabled: open`), not when the fade ends.
- `TextEdit` has no `lineHeight`. For WYSIWYG editing, do not set one on the matching `Text`
  either, or the words jump when editing starts.
- Icon themes are searched theme by theme, then name by name: a generic icon in the user's theme
  beats a specific one in hicolor. nap's MIME types therefore name their own icon as their
  generic icon.
- Fakes hide integration bugs. The desktop installer passed every unit test and still failed on a
  real desktop twice. For anything touching the desktop, also run the real tools against a
  throwaway home (`HOME`, `XDG_*`, and `NAP_PREFIX` pointed at a scratch folder) before calling it
  done.

## When in doubt

Ask what a person holding the real object would expect, then draw it as plainly as the theme
allows. Remove one thing before adding another. And look at it before you say it is finished.
