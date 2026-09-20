use nap::app::App;
use nap::tape::{self, Kind, Tape};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// Copies of fixtures under `dir`, so tests can also make same-named files in different folders.
fn copy(dir: &Path, from: &str, to: &str) -> PathBuf {
    let path = dir.join(to);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::copy(fixture(from), &path).unwrap();
    path
}

fn entries(archive: &Path) -> Vec<String> {
    let mut tar = tar::Archive::new(fs::File::open(archive).unwrap());
    tar.entries().unwrap().map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned()).collect()
}

#[test]
fn the_index_is_plain_m3u_text() {
    let text = "#EXTM3U\n#PLAYLIST: Summer '98 \n#EXTIMG:_cover.jpg\n# a note to self\n\n01 Roygbiv.flac\n  02 Don't Stop.mp3  \n/music/03 far away.ogg\n";
    let tape = tape::parse(text, Path::new("/tapes/summer"));
    assert_eq!(tape.name, "Summer '98");
    assert_eq!(tape.cover, Some("/tapes/summer/_cover.jpg".into()));
    let tracks: Vec<PathBuf> =
        ["/tapes/summer/01 Roygbiv.flac", "/tapes/summer/02 Don't Stop.mp3", "/music/03 far away.ogg"]
            .map(Into::into)
            .into();
    assert_eq!(tape.tracks, tracks);

    let signed = Tape {
        name: "Summer\n'98".into(),
        from: " Shane ".into(),
        note: "For the drive up.\n\nSide B is the good one.\n".into(),
        ..Tape::default()
    };
    let written = tape::render(&signed, Some("_cover.jpg"), &["01 Roygbiv.flac".into(), "02 Don't Stop.mp3".into()]);
    let expected = "#EXTM3U\n#PLAYLIST:Summer '98\n#EXTIMG:_cover.jpg\n#FROM:Shane\n#NOTE:For the drive up.\n#NOTE:\n#NOTE:Side B is the good one.\n\n01 Roygbiv.flac\n02 Don't Stop.mp3\n";
    assert_eq!(written, expected);
    assert_eq!(tape::render(&Tape { name: "  ".into(), ..Tape::default() }, None, &[]), "#EXTM3U\n\n");
    let read = tape::parse(&written, Path::new("/t"));
    let words = (read.name.as_str(), read.from.as_str(), read.note.as_str());
    assert_eq!(words, ("Summer '98", "Shane", "For the drive up.\n\nSide B is the good one."));
    assert_eq!(read.tracks.len(), 2, "a signature and a note are not tracks");
}

#[test]
fn files_are_known_by_their_extensions() {
    for (name, kind) in [
        ("mix.tape", Kind::Archive),
        ("Mix.JCARD", Kind::Index),
        ("art.JPG", Kind::Image),
        ("a.flac", Kind::Audio),
        ("notes.txt", Kind::Other),
        ("bare", Kind::Other),
    ] {
        assert_eq!(tape::kind(Path::new(name)), kind, "{name}");
    }
}

#[test]
fn archived_names_stay_distinct_and_repeats_share_one() {
    let tracks: Vec<PathBuf> =
        ["/a/intro.mp3", "/b/intro.mp3", "/a/intro.mp3", "/c/intro.mp3", "/c/README"].map(Into::into).into();
    assert_eq!(tape::archive_names(&tracks), ["intro.mp3", "intro (2).mp3", "intro.mp3", "intro (3).mp3", "README"]);
}

#[test]
fn a_tape_survives_export_and_extraction() {
    let temp = tempfile::tempdir().unwrap();
    let first = copy(temp.path(), "silence.flac", "one/intro.flac");
    let second = copy(temp.path(), "silence.mp3", "two/intro.flac");
    let cover = temp.path().join("art.PNG");
    fs::write(&cover, b"\x89PNG stand-in").unwrap();
    let tape = Tape {
        name: "Summer '98".into(),
        cover: Some(cover),
        tracks: vec![first.clone(), second, first],
        ..Tape::default()
    };
    let dest = temp.path().join("Summer.tape");
    let done = AtomicU64::new(0);
    tape::export(&tape, &dest, &done).unwrap();
    assert_eq!(done.load(Ordering::Relaxed), tape::export_size(&tape));
    assert!(!temp.path().join("Summer.tape.partial").exists());
    assert_eq!(
        entries(&dest),
        ["Summer/_index.jcard", "Summer/_cover.png", "Summer/intro.flac", "Summer/intro (2).flac"]
    );

    let into = tape::scratch(&temp.path().join("scratch"), &dest).unwrap();
    let unpacked = AtomicU64::new(0);
    let read = tape::extract(&dest, &into, &unpacked).unwrap();
    assert!(unpacked.load(Ordering::Relaxed) > 0);
    assert_eq!(read.name, "Summer '98");
    assert_eq!(read.cover, Some(into.join("Summer/_cover.png")));
    let names: Vec<String> =
        read.tracks.iter().map(|t| t.strip_prefix(&into).unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["Summer/intro.flac", "Summer/intro (2).flac", "Summer/intro.flac"]);
    let index = fs::read_to_string(into.join("Summer/_index.jcard")).unwrap();
    assert_eq!(index, "#EXTM3U\n#PLAYLIST:Summer '98\n#EXTIMG:_cover.png\n\nintro.flac\nintro (2).flac\nintro.flac\n");
    // A second extraction of the same archive gets its own directory.
    assert_ne!(tape::scratch(&temp.path().join("scratch"), &dest).unwrap(), into);
}

#[test]
fn a_standalone_jcard_points_at_files_where_they_are() {
    let temp = tempfile::tempdir().unwrap();
    let track = copy(temp.path(), "silence.ogg", "music/far away.ogg");
    let tape = Tape { tracks: vec![track.clone()], ..Tape::default() };
    let dest = temp.path().join("mix.jcard");
    tape::export(&tape, &dest, &AtomicU64::new(0)).unwrap();
    assert_eq!(fs::read_to_string(&dest).unwrap(), format!("#EXTM3U\n\n{}\n", track.display()));
    assert_eq!(tape::read_index(&dest).unwrap().tracks, [track]);

    fs::write(&dest, "#EXTM3U\n#PLAYLIST:Ghosts\n#EXTIMG:gone.png\nmissing.mp3\nmusic/far away.ogg\n").unwrap();
    let read = tape::read_index(&dest).unwrap();
    assert_eq!((read.name.as_str(), read.cover, read.tracks.len()), ("Ghosts", None, 1));
    fs::write(&dest, "#EXTM3U\nmissing.mp3\n").unwrap();
    assert_eq!(tape::read_index(&dest).unwrap_err(), "None of this J-card's tracks could be found");
    fs::write(&dest, "#EXTM3U\n").unwrap();
    assert_eq!(tape::read_index(&dest).unwrap_err(), "This J-card lists no tracks");
    assert!(tape::read_index(&temp.path().join("absent.jcard")).is_err());
}

#[test]
fn a_failed_export_leaves_nothing_behind() {
    let temp = tempfile::tempdir().unwrap();
    let tape = Tape { name: "Lost".into(), tracks: vec![temp.path().join("missing.flac")], ..Tape::default() };
    let dest = temp.path().join("Lost.tape");
    assert!(tape::export(&tape, &dest, &AtomicU64::new(0)).unwrap_err().contains("missing.flac"));
    assert!(!dest.exists() && !temp.path().join("Lost.tape.partial").exists());
    assert!(tape::export(&tape, &temp.path().join("no/such/dir/Lost.tape"), &AtomicU64::new(0)).is_err());
}

#[test]
fn archives_without_an_index_or_with_odd_entries_are_handled() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("Loose.tape");
    let mut tar = tar::Builder::new(fs::File::create(&archive).unwrap());
    tar.append_path_with_name(fixture("silence.ogg"), "b side.ogg").unwrap();
    tar.append_path_with_name(fixture("silence.mp3"), "deep/a side.mp3").unwrap();
    tar.append_path_with_name(fixture("silence.mp3"), "too/deep/down/lost.mp3").unwrap();
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    tar.append_link(&mut link, "escape.mp3", "/etc/passwd").unwrap();
    tar.finish().unwrap();
    drop(tar);
    let into = temp.path().join("out");
    let read = tape::extract(&archive, &into, &AtomicU64::new(0)).unwrap();
    assert_eq!(read.name, "Loose");
    assert_eq!(read.tracks, [into.join("b side.ogg"), into.join("deep/a side.mp3")]);
    assert!(fs::symlink_metadata(into.join("escape.mp3")).is_err());

    let empty = temp.path().join("Empty.tape");
    tar::Builder::new(fs::File::create(&empty).unwrap()).finish().unwrap();
    assert_eq!(
        tape::extract(&empty, &temp.path().join("nothing"), &AtomicU64::new(0)).unwrap_err(),
        "This tape has no playable tracks"
    );
    assert!(tape::extract(&temp.path().join("absent.tape"), &into, &AtomicU64::new(0)).is_err());
    fs::write(&empty, "not a tar at all, just some text that is long enough to look like a header maybe").unwrap();
    assert!(tape::extract(&empty, &temp.path().join("garbled"), &AtomicU64::new(0)).is_err());
}

fn ask(app: &mut App, request: Value) -> Value {
    app.dispatch(&request).unwrap()
}

fn finish(app: &mut App) -> Value {
    loop {
        let report = ask(app, json!({"op":"job"}));
        if report["active"] != true {
            return report;
        }
        assert!(report["label"].is_string() && report["total"].is_u64());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn files(tape: &Value) -> Vec<&str> {
    tape["tracks"].as_array().unwrap().iter().map(|t| t["file"].as_str().unwrap()).collect()
}

#[test]
fn the_deck_edits_saves_and_reloads_a_tape() {
    let temp = tempfile::tempdir().unwrap();
    let (a, b, c) = (
        copy(temp.path(), "silence.flac", "a.flac"),
        copy(temp.path(), "silence.mp3", "b.mp3"),
        copy(temp.path(), "silence.ogg", "c.ogg"),
    );
    let mut app = App::default();
    assert_eq!(ask(&mut app, json!({"op":"job"}))["active"], false);
    assert!(app.dispatch(&json!({"op":"load", "paths": []})).is_err());
    assert!(app.dispatch(&json!({"op":"load", "paths": [temp.path().join("art.png")]})).is_err());

    // A lone file is a tape of one, and not yet a mixtape.
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [a]}))["job"], false);
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((files(&tape), &tape["mixtape"], &tape["dirty"]), (vec!["a.flac"], &json!(false), &json!(false)));
    assert!(app.dispatch(&json!({"op":"edit", "action":"remove", "index": 0})).is_err());

    ask(&mut app, json!({"op":"load", "append": true, "paths": [b, temp.path().join("art.png"), c]}));
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!(
        (files(&tape), &tape["mixtape"], &tape["dirty"]),
        (vec!["a.flac", "b.mp3", "c.ogg"], &json!(true), &json!(true))
    );
    assert!(tape["tracks"][0]["seconds"].is_u64());

    // Track search walks the loop; the track that is up is followed through edits.
    assert_eq!(ask(&mut app, json!({"op":"track", "forward": true, "position": 0}))["index"], 1);
    assert_eq!(ask(&mut app, json!({"op":"edit", "action":"move", "from": 1, "to": 0}))["replaced"], false);
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["index"], 0);
    assert_eq!(ask(&mut app, json!({"op":"track", "forward": false, "position": 0}))["index"], 2);
    assert_eq!(ask(&mut app, json!({"op":"edit", "action":"move", "from": 2, "to": 99}))["replaced"], false);
    assert!(app.dispatch(&json!({"op":"edit", "action":"move", "from": 9, "to": 0})).is_err());
    assert_eq!(ask(&mut app, json!({"op":"select", "index": 1}))["path"], json!(temp.path().join("a.flac")));
    assert!(app.dispatch(&json!({"op":"select", "index": 7})).is_err());
    assert_eq!(ask(&mut app, json!({"op":"edit", "action":"remove", "index": 0}))["replaced"], false);
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["index"], 0);
    let removed = ask(&mut app, json!({"op":"edit", "action":"remove", "index": 0}));
    assert_eq!((&removed["replaced"], &removed["path"]), (&json!(true), &json!(temp.path().join("c.ogg"))));
    assert!(app.dispatch(&json!({"op":"edit", "action":"scribble"})).is_err());
    assert_eq!(ask(&mut app, json!({"op":"insert", "index": 0}))["file"][0][1], "Ogg Vorbis");

    ask(&mut app, json!({"op":"load", "append": true, "paths": [temp.path().join("b.mp3")]}));
    ask(&mut app, json!({"op":"edit", "action":"name", "text": "  Summer '98 "}));
    ask(&mut app, json!({"op":"edit", "action":"from", "text": "Shane"}));
    ask(&mut app, json!({"op":"edit", "action":"note", "text": "For the drive up.\nSide B is the good one.\n"}));
    assert!(app.dispatch(&json!({"op":"edit", "action":"cover", "path": temp.path().join("c.ogg")})).is_err());
    fs::write(temp.path().join("art.png"), "stand-in").unwrap();
    ask(&mut app, json!({"op":"edit", "action":"cover", "path": temp.path().join("art.png")}));

    // The last track ending stops the tape cued at its start, unless it loops.
    assert_eq!(
        ask(&mut app, json!({"op":"ended", "looping": false})),
        json!({"index": 1, "path": temp.path().join("b.mp3"), "position": 0, "play": true})
    );
    assert_eq!(ask(&mut app, json!({"op":"ended", "looping": false}))["play"], false);
    ask(&mut app, json!({"op":"select", "index": 1}));
    assert_eq!(
        ask(&mut app, json!({"op":"ended", "looping": true})),
        json!({"index": 0, "path": temp.path().join("c.ogg"), "position": 0, "play": true})
    );

    assert!(app.dispatch(&json!({"op":"export"})).is_err());
    assert_eq!(ask(&mut app, json!({"op":"export", "dest": temp.path().join("Summer")}))["job"], true);
    assert_eq!(finish(&mut app), json!({"active": false, "loaded": false, "notice": "Saved Summer.tape"}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["dirty"], false);
    assert_eq!(
        entries(&temp.path().join("Summer.tape")),
        ["Summer/_index.jcard", "Summer/_cover.png", "Summer/c.ogg", "Summer/b.mp3"]
    );

    // Reloading the archive unpacks it to scratch space that goes away with the deck.
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [temp.path().join("Summer.tape")]}))["job"], true);
    assert_eq!(finish(&mut app)["loaded"], true);
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((tape["name"].as_str(), files(&tape)), (Some("Summer '98"), vec!["c.ogg", "b.mp3"]));
    let words = (tape["from"].as_str(), tape["note"].as_str());
    assert_eq!(words, (Some("Shane"), Some("For the drive up.\nSide B is the good one.")));
    let scratch =
        PathBuf::from(tape["tracks"][0]["path"].as_str().unwrap()).parent().unwrap().parent().unwrap().to_owned();
    assert!(scratch.starts_with(tape::scratch_root()) && scratch.is_dir());
    ask(&mut app, json!({"op":"export", "dest": temp.path().join("list.jcard")}));
    assert_eq!(finish(&mut app)["notice"], "Saved list.jcard");
    ask(&mut app, json!({"op":"load", "paths": [temp.path().join("list.jcard")]}));
    assert!(!scratch.exists(), "a replaced tape's scratch space is removed");
    ask(&mut app, json!({"op":"load", "paths": [temp.path().join("Summer.tape")]}));
    finish(&mut app);
    let scratch = PathBuf::from(ask(&mut app, json!({"op":"tape"}))["tracks"][0]["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_owned();
    drop(app);
    assert!(!scratch.exists(), "quitting removes scratch space");

    let mut app = App::default();
    ask(&mut app, json!({"op":"load", "paths": [temp.path().join("absent.tape")]}));
    assert!(finish(&mut app)["notice"].as_str().unwrap().contains("absent.tape"));
    let mine = format!("{}-absent-", std::process::id());
    let leftovers = fs::read_dir(tape::scratch_root())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&mine));
    assert_eq!(leftovers.count(), 0, "a failed import removes its scratch space");
}

#[test]
fn unsaved_work_is_given_up_only_by_asking_twice() {
    use nap::session::{SECOND_THOUGHTS, Session};
    use std::time::{Duration, Instant};
    let temp = tempfile::tempdir().unwrap();
    let track = copy(temp.path(), "silence.flac", "a.flac");
    let mut session = Session::default();
    let now = Instant::now();
    let allowed = |verdict: Value| verdict["allowed"] == true;
    // A tape with nothing unsaved is never in the way.
    session.load(&json!({"paths": [track]})).unwrap();
    assert!(allowed(session.may_discard("open", now)));
    session.edit(&json!({"action": "name", "text": "Keep me"})).unwrap();

    let refused = session.may_discard("open", now);
    assert_eq!(refused["allowed"], false);
    assert_eq!(refused["notice"], "This tape is unsaved. Open again to discard it, or press REC to save it.");
    assert!(allowed(session.may_discard("open", now + Duration::from_secs(2))), "asked again, soon");
    // The warning is spent: the next request starts over, and a slow second ask does not count.
    assert!(!allowed(session.may_discard("open", now + Duration::from_secs(3))));
    assert!(!allowed(
        session.may_discard("open", now + Duration::from_secs(3) + SECOND_THOUGHTS + Duration::from_millis(1))
    ));
    // Asking for something else is not asking twice.
    let quit = session.may_discard("quit", now + Duration::from_secs(20));
    assert_eq!(quit["notice"], "This tape is unsaved. Quit again to discard it, or press REC to save it.");
    assert!(!allowed(session.may_discard("open", now + Duration::from_secs(21))));
    assert!(allowed(session.may_discard("open", now + Duration::from_secs(22))));

    let mut app = App::default();
    assert_eq!(ask(&mut app, json!({"op":"discard", "action":"quit"}))["allowed"], true);
}
