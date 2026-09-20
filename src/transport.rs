//! The deck's transport, modelled on a real cassette mechanism. PLAY engages the head; pausing
//! holds the tape against it; STOP lifts the head away and leaves the tape where it is. With the
//! head lifted the wind keys become track search, as on decks with music search: PREV finds the
//! start of this track, or of the one before when already near the start, and NEXT the start of
//! the following one. A tape is a loop of tracks, so both always land somewhere; on a single
//! file that is its own beginning.

/// PREV within this long of a track's start goes to the previous track instead of restarting.
pub const RESTART_WINDOW_MS: i64 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deck {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The PLAY/PAUSE key.
    PlayKey,
    StopKey,
    /// A file finished loading; `true` when it should wait rather than start. A PLAY press that
    /// arrived while it was still loading wins over waiting.
    Loaded(bool),
    /// The tape ran out.
    Ended,
}

impl Deck {
    pub fn name(self) -> &'static str {
        match self {
            Deck::Stopped => "stopped",
            Deck::Playing => "playing",
            Deck::Paused => "paused",
        }
    }

    pub fn parse(name: &str) -> Deck {
        match name {
            "playing" => Deck::Playing,
            "paused" => Deck::Paused,
            _ => Deck::Stopped,
        }
    }

    pub fn after(self, event: Event) -> Deck {
        match (event, self) {
            (Event::Loaded(true), Deck::Playing) | (Event::Loaded(false), _) => Deck::Playing,
            (Event::PlayKey, Deck::Playing) | (Event::Loaded(true), _) => Deck::Paused,
            (Event::PlayKey, _) => Deck::Playing,
            (Event::StopKey, _) | (Event::Ended, _) => Deck::Stopped,
        }
    }
}

impl Event {
    pub fn parse(name: &str, waiting: bool) -> Option<Event> {
        match name {
            "play" => Some(Event::PlayKey),
            "stop" => Some(Event::StopKey),
            "loaded" => Some(Event::Loaded(waiting)),
            "ended" => Some(Event::Ended),
            _ => None,
        }
    }
}

/// The track PREV lands at the start of.
pub fn previous(index: usize, count: usize, position: i64) -> usize {
    if position > RESTART_WINDOW_MS { index } else { (index + count.max(1) - 1) % count.max(1) }
}

/// The track NEXT lands at the start of.
pub fn next(index: usize, count: usize) -> usize {
    (index + 1) % count.max(1)
}

/// A track ran out: the track to cue, and whether to keep playing. The last track ends the tape,
/// which stops cued back at its first track unless it is looping. The last track of side A ends
/// the side: the deck stops with side B turned up, unless it is looping, which is auto-reverse.
pub fn after_end(index: usize, count: usize, looping: bool, side_b: Option<usize>) -> (usize, bool) {
    let last = index + 1 >= count.max(1) || side_b == Some(index + 1);
    (next(index, count), !last || looping)
}

/// A tape has two sides only while there are tracks on both.
pub fn sided(side_b: Option<usize>, count: usize) -> Option<usize> {
    side_b.filter(|first| (1..count).contains(first))
}

/// Where side B starts once the track at `gone` is taken off a tape that is left with `count`.
pub fn side_after_remove(side_b: Option<usize>, gone: usize, count: usize) -> Option<usize> {
    sided(side_b.map(|first| first - usize::from(gone < first)), count)
}

/// Where side B starts once a track is moved `from` one place `to` another. The turn stays
/// between the same two neighbours, so a track carried across it changes sides; one set down
/// right at the turn stays on the side it came from.
pub fn side_after_move(side_b: Option<usize>, from: usize, to: usize, count: usize) -> Option<usize> {
    let moved = |first: usize| match from < first {
        true if to >= first => first - 1,
        false if to < first => first + 1,
        _ => first,
    };
    sided(side_b.map(moved), count)
}

/// The track FLIP lands on: the first of the other side. A tape with one side has nowhere to turn.
pub fn flipped(index: usize, side_b: Option<usize>) -> Option<usize> {
    side_b.map(|first| if index < first { first } else { 0 })
}
