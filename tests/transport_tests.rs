use nap::app::App;
use nap::transport::{self, Deck, Event};
use serde_json::json;

#[test]
fn keys_move_the_head_like_a_cassette_mechanism() {
    use Deck::*;
    for (from, event, to) in [
        (Stopped, Event::PlayKey, Playing),
        (Playing, Event::PlayKey, Paused),
        (Paused, Event::PlayKey, Playing),
        (Playing, Event::StopKey, Stopped),
        (Paused, Event::StopKey, Stopped),
        (Stopped, Event::StopKey, Stopped),
        (Playing, Event::Ended, Stopped),
        (Stopped, Event::Loaded(false), Playing),
        (Stopped, Event::Loaded(true), Paused),
        (Paused, Event::Loaded(true), Paused),
        (Playing, Event::Loaded(true), Playing),
    ] {
        assert_eq!(from.after(event), to, "{from:?} {event:?}");
        assert_eq!(Deck::parse(to.name()), to);
    }
    assert_eq!(Deck::parse("nonsense"), Stopped);
    assert_eq!(Event::parse("eject", false), None);
}

#[test]
fn track_search_always_lands_somewhere() {
    // Deep into a track PREV restarts it; near its start it goes back one, wrapping around the tape.
    assert_eq!(transport::previous(2, 5, 3001), 2);
    assert_eq!(transport::previous(2, 5, 3000), 1);
    assert_eq!(transport::previous(0, 5, 0), 4);
    assert_eq!(transport::next(3, 5), 4);
    assert_eq!(transport::next(4, 5), 0);
    // A single file is a tape of one: both keys find its beginning.
    assert_eq!(transport::previous(0, 1, 90_000), 0);
    assert_eq!(transport::previous(0, 1, 0), 0);
    assert_eq!(transport::next(0, 1), 0);
    assert_eq!(transport::next(0, 0), 0);
    // The end of a track plays on; the end of the tape stops, cued at the top, unless looping.
    assert_eq!(transport::after_end(0, 3, false), (1, true));
    assert_eq!(transport::after_end(2, 3, false), (0, false));
    assert_eq!(transport::after_end(2, 3, true), (0, true));
    assert_eq!(transport::after_end(0, 1, false), (0, false));
    assert_eq!(transport::after_end(0, 0, true), (0, true));
}

#[test]
fn the_core_answers_transport_requests() {
    let mut app = App::default();
    let state = |app: &mut App, state: &str, event: &str, waiting: bool| {
        app.dispatch(&json!({"op":"transport", "state": state, "event": event, "waiting": waiting})).unwrap()["state"]
            .clone()
    };
    assert_eq!(state(&mut app, "stopped", "play", false), "playing");
    assert_eq!(state(&mut app, "playing", "stop", false), "stopped");
    assert_eq!(state(&mut app, "stopped", "loaded", true), "paused");
    assert_eq!(state(&mut app, "playing", "ended", false), "stopped");
    assert!(app.dispatch(&json!({"op":"transport", "state":"playing", "event":"eject"})).is_err());
    // On an empty deck track search still answers, with nowhere to go.
    assert_eq!(app.dispatch(&json!({"op":"track", "forward": true, "position": 0})).unwrap()["index"], 0);
}
