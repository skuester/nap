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
