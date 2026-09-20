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
    assert_eq!(transport::after_end(0, 3, false, None), (1, true));
    assert_eq!(transport::after_end(2, 3, false, None), (0, false));
    assert_eq!(transport::after_end(2, 3, true, None), (0, true));
    assert_eq!(transport::after_end(0, 1, false, None), (0, false));
    assert_eq!(transport::after_end(0, 0, true, None), (0, true));
}

#[test]
fn a_tape_with_two_sides_stops_at_the_turn_and_keeps_it_through_edits() {
    // Side A running out stops the deck with side B cued; looping is auto-reverse and plays on.
    assert_eq!(transport::after_end(0, 5, false, Some(2)), (1, true));
    assert_eq!(transport::after_end(1, 5, false, Some(2)), (2, false));
    assert_eq!(transport::after_end(1, 5, true, Some(2)), (2, true));
    assert_eq!(transport::after_end(2, 5, false, Some(2)), (3, true));
    assert_eq!(transport::after_end(4, 5, false, Some(2)), (0, false));
    // FLIP finds the start of the other side; one side has nowhere to turn.
    assert_eq!(transport::flipped(1, Some(2)), Some(2));
    assert_eq!(transport::flipped(2, Some(2)), Some(0));
    assert_eq!(transport::flipped(4, Some(2)), Some(0));
    assert_eq!(transport::flipped(1, None), None);
    // Two sides need a track each.
    assert_eq!(transport::sided(Some(0), 5), None);
    assert_eq!(transport::sided(Some(5), 5), None);
    assert_eq!(transport::sided(Some(4), 5), Some(4));
    assert_eq!(transport::sided(None, 5), None);
    // Taking a track off side A brings side B up by one; side B emptied leaves one side.
    assert_eq!(transport::side_after_remove(Some(2), 0, 4), Some(1));
    assert_eq!(transport::side_after_remove(Some(2), 2, 4), Some(2));
    assert_eq!(transport::side_after_remove(Some(1), 0, 4), None);
    assert_eq!(transport::side_after_remove(Some(4), 4, 4), None);
    assert_eq!(transport::side_after_remove(None, 1, 4), None);
    // A track carried across the turn changes sides; set down right at it, it keeps its own.
    assert_eq!(transport::side_after_move(Some(2), 0, 3, 5), Some(1), "A to deep in B");
    assert_eq!(transport::side_after_move(Some(2), 0, 1, 5), Some(2), "A to the end of A");
    assert_eq!(transport::side_after_move(Some(2), 4, 0, 5), Some(3), "B to the top of A");
    assert_eq!(transport::side_after_move(Some(2), 4, 2, 5), Some(2), "B to the top of B");
    assert_eq!(transport::side_after_move(Some(2), 3, 4, 5), Some(2), "within B");
    assert_eq!(transport::side_after_move(Some(1), 0, 4, 5), None, "the last of side A leaves");
    assert_eq!(transport::side_after_move(None, 0, 4, 5), None);
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
