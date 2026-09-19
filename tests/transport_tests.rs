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
    let landed =
        app.dispatch(&json!({"op":"track", "forward": false, "index": 0, "count": 3, "position": 500})).unwrap();
    assert_eq!(landed, json!({"index": 2, "position": 0}));
    let landed =
        app.dispatch(&json!({"op":"track", "forward": true, "index": 2, "count": 3, "position": 500})).unwrap();
    assert_eq!(landed, json!({"index": 0, "position": 0}));
}
