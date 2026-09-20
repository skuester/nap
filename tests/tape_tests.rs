use nap::app::App;
use nap::tape::{self, Brief, Kind, Source, Tape};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
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

fn paths(tracks: &[Source]) -> Vec<PathBuf> {
    tracks.iter().map(|track| track.path.clone()).collect()
}

fn bytes(source: &Source) -> Vec<u8> {
    let mut data = Vec::new();
    source.open().unwrap().read_to_end(&mut data).unwrap();
    data
}

#[test]
fn the_index_is_plain_m3u_text() {
    let text = "\u{feff}#EXTM3U\r\n#PLAYLIST: Summer '98 \n#EXTIMG:cover.jpg\n# a note to self\n\n#EXTINF:151,Boards of Canada - Roygbiv\n01 Roygbiv.flac\n  02 Don't Stop.mp3  \n#EXTINF:-1,\n/music/03 far away.ogg\n";
    let index = tape::parse(text, Path::new("/tapes/summer")).unwrap();
    assert_eq!(index.tape.name, "Summer '98");
    assert_eq!(index.tape.cover, Some("/tapes/summer/cover.jpg".into()));
    let tracks: Vec<PathBuf> =
        ["/tapes/summer/01 Roygbiv.flac", "/tapes/summer/02 Don't Stop.mp3", "/music/03 far away.ogg"]
            .map(Into::into)
            .into();
    assert_eq!(paths(&index.tape.tracks), tracks);
    // A track that goes missing is named by its title where the index gives one.
    assert_eq!(index.labels, ["Boards of Canada - Roygbiv", "02 Don't Stop.mp3", "03 far away.ogg"]);

    let signed = Tape {
        name: "Summer\n'98".into(),
        from: " Shane ".into(),
        note: "For the drive up.\n\nSide B is the good one.\n".into(),
        ..Tape::default()
    };
    let briefs =
        [Brief { title: "Roygbiv".into(), artist: "Boards of\nCanada".into(), seconds: 151 }, Brief::default()];
    let entries = ["01 Roygbiv.flac".into(), "02 Don't Stop.mp3".into(), "bare.ogg".into()];
    let written = tape::render(&signed, Some("cover.jpg"), &entries, &briefs);
    let expected = "#EXTM3U\n#EXTENC:UTF-8\n#NAP:1\n#PLAYLIST:Summer '98\n#EXTIMG:cover.jpg\n#NAP-FROM:Shane\n#NAP-NOTE:For the drive up.\n#NAP-NOTE:\n#NAP-NOTE:Side B is the good one.\n\n#EXTINF:151,Boards of Canada - Roygbiv\n01 Roygbiv.flac\n#EXTINF:-1,02 Don't Stop\n02 Don't Stop.mp3\nbare.ogg\n";
    assert_eq!(written, expected);
    let blank = tape::render(&Tape { name: "  ".into(), ..Tape::default() }, None, &[], &[]);
    assert_eq!(blank, "#EXTM3U\n#EXTENC:UTF-8\n#NAP:1\n\n");
    let read = tape::parse(&written, Path::new("/t")).unwrap().tape;
    let words = (read.name.as_str(), read.from.as_str(), read.note.as_str());
    assert_eq!(words, ("Summer '98", "Shane", "For the drive up.\n\nSide B is the good one."));
    assert_eq!(read.tracks.len(), 3, "a signature and a note are not tracks");
}

#[test]
fn a_tape_can_have_two_sides() {
    let sided = Tape { name: "Sides".into(), side_b: Some(2), ..Tape::default() };
    let entries: Vec<String> = ["a1.mp3", "a2.mp3", "b1.mp3"].map(Into::into).into();
    let written = tape::render(&sided, None, &entries, &[]);
    assert!(written.ends_with("\n\n#NAP-SIDE:A\na1.mp3\na2.mp3\n\n#NAP-SIDE:B\nb1.mp3\n"), "{written}");
    assert_eq!(tape::parse(&written, Path::new("/t")).unwrap().tape.side_b, Some(2));
    // A turn with nothing on one side of it is not written, and one side is read as one side.
    let lopsided = tape::render(&Tape { side_b: Some(3), ..Tape::default() }, None, &entries, &[]);
    assert!(!lopsided.contains("#NAP-SIDE"), "{lopsided}");
    assert_eq!(tape::parse("#EXTM3U\n#NAP-SIDE:A\na.mp3\nb.mp3\n", Path::new("/t")).unwrap().tape.side_b, None);

    // Side B still starts at the same track when ones before it have gone missing, and a side
    // that has gone missing entirely leaves a tape with one.
    let temp = tempfile::tempdir().unwrap();
    for name in ["a2.ogg", "b1.ogg", "b2.ogg"] {
        copy(temp.path(), "silence.ogg", name);
    }
    let card = temp.path().join("sides.jcard");
    fs::write(&card, "#EXTM3U\n#NAP-SIDE:A\na1.ogg\na2.ogg\n#NAP-SIDE:B\nb1.ogg\ngone.ogg\nb2.ogg\n").unwrap();
    let (read, missing) = tape::read_index(&card).unwrap();
    assert_eq!((read.side_b, missing), (Some(1), vec!["a1.ogg".to_owned(), "gone.ogg".to_owned()]));
    assert_eq!(paths(&read.tracks)[1], temp.path().join("b1.ogg"));
    fs::write(&card, "#EXTM3U\ngone.ogg\n#NAP-SIDE:B\nb1.ogg\nb2.ogg\n").unwrap();
    assert_eq!(tape::read_index(&card).unwrap().0.side_b, None);
}

#[test]
fn a_name_starting_with_a_hash_is_still_a_track() {
    let written = tape::render(&Tape::default(), None, &["#1 Crush.mp3".into(), "plain.mp3".into()], &[]);
    assert!(written.ends_with("\n./#1 Crush.mp3\nplain.mp3\n"), "{written}");
    let read = tape::parse(&written, Path::new("/t")).unwrap();
    assert_eq!(paths(&read.tape.tracks), [PathBuf::from("/t/#1 Crush.mp3"), "/t/plain.mp3".into()]);
}

#[test]
fn a_tape_from_a_newer_nap_is_refused_rather_than_misread() {
    assert!(tape::parse("#EXTM3U\n#NAP:1\na.mp3\n", Path::new("/t")).is_ok());
    assert!(tape::parse("#EXTM3U\n#NAP:one\na.mp3\n", Path::new("/t")).is_ok(), "a garbled version is not a newer one");
    let refused = tape::parse(&format!("#EXTM3U\n#NAP:{}\na.mp3\n", tape::VERSION + 1), Path::new("/t")).unwrap_err();
    assert_eq!(refused, "This tape was made by a newer nap than this one");
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
    let tracks: Vec<Source> =
        ["/a/intro.mp3", "/b/intro.mp3", "/a/intro.mp3", "/c/intro.mp3", "/c/README"].map(Into::into).into();
    assert_eq!(tape::archive_names(&tracks), ["intro.mp3", "intro (2).mp3", "intro.mp3", "intro (3).mp3", "README"]);
}

#[test]
fn a_saved_tape_is_read_where_it_lies() {
    let temp = tempfile::tempdir().unwrap();
    let first = copy(temp.path(), "silence.flac", "one/intro.flac");
    let second = copy(temp.path(), "silence.mp3", "two/intro.flac");
    let hashed = copy(temp.path(), "silence.ogg", "#1 Crush.ogg");
    let cover = temp.path().join("art.PNG");
    fs::write(&cover, b"\x89PNG stand-in").unwrap();
    let tape = Tape {
        name: "Summer '98".into(),
        cover: Some(cover.into()),
        tracks: vec![first.clone().into(), second.clone().into(), first.clone().into(), hashed.clone().into()],
        ..Tape::default()
    };
    let dest = temp.path().join("Summer.tape");
    let done = AtomicU64::new(0);
    tape::export(&tape, &[], &dest, &done).unwrap();
    assert_eq!(done.load(Ordering::Relaxed), tape::export_size(&tape));
    assert!(!temp.path().join("Summer.tape.partial").exists());
    assert_eq!(
        entries(&dest),
        ["Summer/tape.jcard", "Summer/cover.png", "Summer/intro.flac", "Summer/intro (2).flac", "Summer/#1 Crush.ogg"]
    );
    // A tape is for handing on, so it does not say who owned its files.
    let mut tar = tar::Archive::new(fs::File::open(&dest).unwrap());
    for entry in tar.entries().unwrap() {
        let header = entry.unwrap().header().clone();
        assert_eq!((header.uid().unwrap(), header.gid().unwrap(), header.mode().unwrap()), (0, 0, 0o644));
        assert!(header.mtime().unwrap() > 0 && header.username().unwrap().unwrap_or("").is_empty());
    }

    let (read, missing) = tape::read_archive(&dest).unwrap();
    assert!(missing.is_empty());
    assert_eq!(read.name, "Summer '98");
    let names = ["Summer/intro.flac", "Summer/intro (2).flac", "Summer/intro.flac", "Summer/#1 Crush.ogg"];
    assert_eq!(paths(&read.tracks), names.map(|name| dest.join(name)));
    // Nothing is unpacked: each track is a stretch of the archive holding exactly the file's bytes.
    for (track, original) in read.tracks.iter().zip([&first, &second, &first, &hashed]) {
        assert_eq!(track.file(), dest);
        assert_eq!(bytes(track), fs::read(original).unwrap());
        assert_eq!(track.size(), fs::metadata(original).unwrap().len());
    }
    let cover = read.cover.clone().unwrap();
    assert_eq!((cover.path.clone(), bytes(&cover)), (dest.join("Summer/cover.png"), b"\x89PNG stand-in".to_vec()));
    let index = Source { path: dest.join("Summer/tape.jcard"), span: None };
    assert!(!index.is_there(), "a held file's path names it without being on disk");
    // Tags read through the window, by name and by content alike.
    assert_eq!(nap::insert::read(&read.tracks[0])["file"][0][1], "FLAC");
    assert_eq!(nap::insert::read(&read.tracks[1])["file"][0][1], "MPEG audio");
    assert_eq!(nap::insert::read(&read.tracks[0])["file"].as_array().unwrap().last().unwrap()[1], json!(temp.path()));
    assert!(nap::insert::held_cover(&cover).starts_with("data:image/png;base64,iVBORyBz"));

    // The window seeks like a file of its own, and never reads past its end.
    let mut window = read.tracks[3].open().unwrap();
    let whole = fs::read(&hashed).unwrap();
    let mut tail = Vec::new();
    assert_eq!(window.seek(SeekFrom::End(-4)).unwrap(), whole.len() as u64 - 4);
    window.read_to_end(&mut tail).unwrap();
    assert_eq!(tail, whole[whole.len() - 4..]);
    assert_eq!(window.seek(SeekFrom::Start(2)).unwrap(), 2);
    assert_eq!(window.seek(SeekFrom::Current(3)).unwrap(), 5);
    let mut four = [0; 4];
    window.read_exact(&mut four).unwrap();
    assert_eq!(four, whole[5..9]);
    assert!(window.seek(SeekFrom::Current(-99)).is_err());
    window.seek(SeekFrom::End(10)).unwrap();
    assert_eq!(window.read(&mut four).unwrap(), 0);

    // A J-card alone cannot list what is inside a tape.
    let refused = tape::export(&read, &[], &temp.path().join("list.jcard"), &done).unwrap_err();
    assert!(refused.starts_with("A J-card lists files where they are"), "{refused}");
}

#[test]
fn a_standalone_jcard_points_at_files_where_they_are() {
    let temp = tempfile::tempdir().unwrap();
    let track = copy(temp.path(), "silence.ogg", "music/far away.ogg");
    let tape = Tape { tracks: vec![track.clone().into()], ..Tape::default() };
    let dest = temp.path().join("mix.jcard");
    tape::export(&tape, &[], &dest, &AtomicU64::new(0)).unwrap();
    assert_eq!(fs::read_to_string(&dest).unwrap(), format!("#EXTM3U\n#EXTENC:UTF-8\n#NAP:1\n\n{}\n", track.display()));
    assert_eq!(paths(&tape::read_index(&dest).unwrap().0.tracks), [track]);

    let listing =
        "#EXTM3U\n#PLAYLIST:Ghosts\n#EXTIMG:gone.png\n#EXTINF:9,Casper - Boo\nmissing.mp3\nmusic/far away.ogg\n";
    fs::write(&dest, listing).unwrap();
    let (read, missing) = tape::read_index(&dest).unwrap();
    assert_eq!((read.name.as_str(), read.cover, read.tracks.len()), ("Ghosts", None, 1));
    assert_eq!(missing, ["Casper - Boo"]);
    fs::write(&dest, "#EXTM3U\nmissing.mp3\n").unwrap();
    assert_eq!(tape::read_index(&dest).unwrap_err(), "None of this J-card's tracks could be found");
    fs::write(&dest, "#EXTM3U\n").unwrap();
    assert_eq!(tape::read_index(&dest).unwrap_err(), "This J-card lists no tracks");
    assert!(tape::read_index(&temp.path().join("absent.jcard")).is_err());
}

#[test]
fn a_failed_export_leaves_nothing_behind() {
    let temp = tempfile::tempdir().unwrap();
    let tape = Tape { name: "Lost".into(), tracks: vec![temp.path().join("missing.flac").into()], ..Tape::default() };
    let dest = temp.path().join("Lost.tape");
    assert!(tape::export(&tape, &[], &dest, &AtomicU64::new(0)).unwrap_err().contains("missing.flac"));
    assert!(!dest.exists() && !temp.path().join("Lost.tape.partial").exists());
    assert!(tape::export(&tape, &[], &temp.path().join("no/such/dir/Lost.tape"), &AtomicU64::new(0)).is_err());
}

#[test]
fn archives_without_an_index_or_with_odd_entries_are_handled() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("Loose.tape");
    let mut tar = tar::Builder::new(fs::File::create(&archive).unwrap());
    tar.append_path_with_name(fixture("silence.ogg"), "b side.ogg").unwrap();
    tar.append_path_with_name(fixture("silence.mp3"), "./deep/down/a side.mp3").unwrap();
    tar.append_path_with_name(fixture("silence.mp3"), "notes.txt").unwrap();
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    tar.append_link(&mut link, "escape.mp3", "/etc/passwd").unwrap();
    tar.finish().unwrap();
    drop(tar);
    let (read, _) = tape::read_archive(&archive).unwrap();
    assert_eq!(read.name, "Loose");
    assert_eq!(paths(&read.tracks), [archive.join("b side.ogg"), archive.join("deep/down/a side.mp3")]);

    let empty = temp.path().join("Empty.tape");
    tar::Builder::new(fs::File::create(&empty).unwrap()).finish().unwrap();
    assert_eq!(tape::read_archive(&empty).unwrap_err(), "This tape has no playable tracks");
    assert!(tape::read_archive(&temp.path().join("absent.tape")).is_err());
    // Anything else that happens to be called .tape, such as a terminal recorder's script.
    fs::write(&empty, "Output demo.gif\nType \"echo hello\"\nEnter\n".repeat(40)).unwrap();
    assert!(tape::read_archive(&empty).unwrap_err().contains("not a tape nap can read"));
    // A tape that lost its end on the way here says so, rather than playing into nothing.
    let whole = fs::read(&archive).unwrap();
    fs::write(&empty, &whole[..700]).unwrap();
    assert!(tape::read_archive(&empty).unwrap_err().ends_with("this tape is cut short"));
}

#[test]
fn an_index_inside_a_tape_can_list_only_what_the_tape_holds() {
    let temp = tempfile::tempdir().unwrap();
    let outside = copy(temp.path(), "silence.flac", "private.flac");
    let archive = temp.path().join("Nosy.tape");
    let listing = format!(
        "#EXTM3U\n#PLAYLIST:Nosy\n#EXTIMG:{0}\n{0}\n../private.flac\n../../../../etc/passwd\n#EXTINF:1,Gone - Away\ngone.mp3\n./mine.ogg\n",
        outside.display()
    );
    fs::create_dir(temp.path().join("Nosy")).unwrap();
    fs::write(temp.path().join("Nosy/tape.jcard"), listing).unwrap();
    let mut tar = tar::Builder::new(fs::File::create(&archive).unwrap());
    tar.append_path_with_name(temp.path().join("Nosy/tape.jcard"), "Nosy/tape.jcard").unwrap();
    tar.append_path_with_name(fixture("silence.ogg"), "Nosy/mine.ogg").unwrap();
    tar.finish().unwrap();
    drop(tar);
    let (read, missing) = tape::read_archive(&archive).unwrap();
    assert_eq!(paths(&read.tracks), [archive.join("Nosy/mine.ogg")]);
    assert_eq!(read.cover, None);
    assert_eq!(missing, ["private.flac", "private.flac", "passwd", "Gone - Away"]);
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
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [a]}))["notice"], "");
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
        json!({"index": 1, "path": temp.path().join("b.mp3"), "position": 0, "play": true, "notice": ""})
    );
    assert_eq!(ask(&mut app, json!({"op":"ended", "looping": false}))["play"], false);
    ask(&mut app, json!({"op":"select", "index": 1}));
    assert_eq!(
        ask(&mut app, json!({"op":"ended", "looping": true})),
        json!({"index": 0, "path": temp.path().join("c.ogg"), "position": 0, "play": true, "notice": ""})
    );

    assert!(app.dispatch(&json!({"op":"export"})).is_err());
    assert_eq!(ask(&mut app, json!({"op":"export", "dest": temp.path().join("Summer")}))["job"], true);
    assert_eq!(finish(&mut app), json!({"active": false, "notice": "Saved Summer.tape"}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["dirty"], false);
    assert_eq!(
        entries(&temp.path().join("Summer.tape")),
        ["Summer/tape.jcard", "Summer/cover.png", "Summer/c.ogg", "Summer/b.mp3"]
    );

    let saved = temp.path().join("Summer.tape");
    let index = Source {
        path: saved.join("Summer/tape.jcard"),
        span: tape::read_archive(&saved).unwrap().0.tracks[0].span.clone(),
    };
    assert!(index.span.is_some());
    let written = fs::read(&saved).unwrap();
    let listing = String::from_utf8_lossy(&written[512..1024]).into_owned();
    assert!(listing.contains("#EXTINF:-1,c\nc.ogg\n#EXTINF:-1,b\nb.mp3\n"), "{listing}");

    // Reloading the archive plays it from where it is: in the deck at once, nothing unpacked.
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [saved]}))["notice"], "");
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((tape["name"].as_str(), files(&tape)), (Some("Summer '98"), vec!["c.ogg", "b.mp3"]));
    let words = (tape["from"].as_str(), tape["note"].as_str());
    assert_eq!(words, (Some("Shane"), Some("For the drive up.\nSide B is the good one.")));
    assert_eq!((&tape["cover"], &tape["coverHeld"]), (&json!(saved.join("Summer/cover.png")), &json!(true)));
    assert_eq!(ask(&mut app, json!({"op":"cover"}))["url"], "data:image/png;base64,c3RhbmQtaW4=");
    let track = &tape["tracks"][1];
    assert_eq!((&track["path"], &track["folder"]), (&json!(saved.join("Summer/b.mp3")), &json!(temp.path())));
    // Opening a held track says which stretch of the archive to play; it has nowhere to keep a bookmark.
    let opened = ask(&mut app, json!({"op":"open", "path": track["path"]}));
    assert_eq!(
        (&opened["path"], &opened["mark"], &opened["held"]["archive"]),
        (&track["path"], &json!(-1), &json!(saved))
    );
    let (offset, length) = (opened["held"]["offset"].as_u64().unwrap(), opened["held"]["length"].as_u64().unwrap());
    assert_eq!(
        fs::read(&saved).unwrap()[offset as usize..][..length as usize],
        fs::read(temp.path().join("b.mp3")).unwrap()
    );
    assert_eq!(ask(&mut app, json!({"op":"insert", "index": 1}))["file"][0][1], "MPEG audio");
    ask(&mut app, json!({"op":"export", "dest": temp.path().join("list.jcard")}));
    assert!(finish(&mut app)["notice"].as_str().unwrap().starts_with("A J-card lists files where they are"));

    // Saved over the archive it is playing from, the tape finds its tracks again where they now lie.
    ask(&mut app, json!({"op":"edit", "action":"move", "from": 1, "to": 0}));
    ask(&mut app, json!({"op":"export", "dest": saved}));
    assert_eq!(finish(&mut app)["notice"], "Saved Summer.tape");
    let reopened = ask(&mut app, json!({"op":"open", "path": track["path"]}));
    assert_ne!(reopened["held"]["offset"], opened["held"]["offset"]);
    let (offset, length) = (reopened["held"]["offset"].as_u64().unwrap(), reopened["held"]["length"].as_u64().unwrap());
    assert_eq!(
        fs::read(&saved).unwrap()[offset as usize..][..length as usize],
        fs::read(temp.path().join("b.mp3")).unwrap()
    );
    assert_eq!(files(&ask(&mut app, json!({"op":"tape"}))), ["b.mp3", "c.ogg"]);
    // A cover of the tape's own again, no longer held.
    ask(&mut app, json!({"op":"edit", "action":"cover", "path": temp.path().join("art.png")}));
    assert_eq!(ask(&mut app, json!({"op":"cover"}))["url"], "");

    // Two sides: marked at a track, kept through edits, flipped between, and saved with the tape.
    ask(
        &mut app,
        json!({"op":"load", "paths": [temp.path().join("a.flac"), temp.path().join("b.mp3"), temp.path().join("c.ogg")]}),
    );
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((&tape["sideB"], &tape["side"]), (&json!(-1), &json!("")));
    assert!(app.dispatch(&json!({"op":"flip"})).unwrap_err().starts_with("This tape has one side"));
    assert_eq!(
        app.dispatch(&json!({"op":"edit", "action":"side", "index": 0})).unwrap_err(),
        "Side B needs a side A before it"
    );
    assert!(app.dispatch(&json!({"op":"edit", "action":"side", "index": 3})).is_err());
    ask(&mut app, json!({"op":"edit", "action":"side", "index": 2}));
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((&tape["sideB"], &tape["side"], &tape["dirty"]), (&json!(2), &json!("A"), &json!(true)));
    assert_eq!(ask(&mut app, json!({"op":"flip"}))["index"], 2);
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["side"], "B");
    assert_eq!(ask(&mut app, json!({"op":"flip"}))["index"], 0);
    // Side A runs out: the deck stops with side B up, and says so. Looping, it plays straight on.
    ask(&mut app, json!({"op":"select", "index": 1}));
    let ended = ask(&mut app, json!({"op":"ended", "looping": false}));
    assert_eq!((&ended["index"], &ended["play"]), (&json!(2), &json!(false)));
    assert_eq!(ended["notice"], "That was side A. Side B is up; press PLAY");
    assert_eq!(
        ask(&mut app, json!({"op":"ended", "looping": false}))["notice"],
        "",
        "the end of the tape is not a turn"
    );
    ask(&mut app, json!({"op":"select", "index": 1}));
    let ended = ask(&mut app, json!({"op":"ended", "looping": true}));
    assert_eq!((&ended["play"], &ended["notice"]), (&json!(true), &json!("")));
    // Edits carry the turn with them.
    ask(&mut app, json!({"op":"edit", "action":"move", "from": 0, "to": 2}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["sideB"], 1);
    ask(&mut app, json!({"op":"export", "dest": temp.path().join("Sides.tape")}));
    finish(&mut app);
    ask(&mut app, json!({"op":"load", "paths": [temp.path().join("Sides.tape")]}));
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((files(&tape), &tape["sideB"]), (vec!["b.mp3", "c.ogg", "a.flac"], &json!(1)));
    ask(&mut app, json!({"op":"edit", "action":"remove", "index": 0}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["sideB"], -1, "side A emptied leaves one side");
    ask(&mut app, json!({"op":"edit", "action":"side", "index": 1}));
    ask(&mut app, json!({"op":"edit", "action":"side", "index": 1}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["sideB"], -1, "asked of the track it starts at, back to one side");

    // A listing whose files have wandered off says which.
    fs::write(temp.path().join("list.jcard"), "#EXTM3U\n#EXTINF:3,Gone - Away\ngone.mp3\nlost.mp3\na.flac\n").unwrap();
    let loaded = ask(&mut app, json!({"op":"load", "paths": [temp.path().join("list.jcard")]}));
    assert_eq!(loaded["notice"], "Could not find 2 of this tape's tracks: Gone - Away, lost.mp3");
    fs::write(temp.path().join("list.jcard"), "#EXTM3U\ngone.mp3\na.flac\n").unwrap();
    let loaded = ask(&mut app, json!({"op":"load", "paths": [temp.path().join("list.jcard")]}));
    assert_eq!(loaded["notice"], "Could not find gone.mp3");
    assert!(
        app.dispatch(&json!({"op":"load", "paths": [temp.path().join("absent.tape")]}))
            .unwrap_err()
            .contains("absent.tape")
    );
}

#[test]
fn a_tape_keeps_one_bookmark_on_its_own_file() {
    use nap::bookmark::{self, Mark};
    let temp = tempfile::tempdir().unwrap();
    let tracks = [
        copy(temp.path(), "silence.flac", "a.flac"),
        copy(temp.path(), "silence.mp3", "b.mp3"),
        copy(temp.path(), "silence.ogg", "c.ogg"),
    ];
    let saved = temp.path().join("Marked.tape");
    let cue =
        |path: &Path, position: i64, resumed: bool| json!({"path": path, "position": position, "resumed": resumed});
    let mut app = App::default();
    // Loose files lined up are not yet a tape anywhere, so there is nowhere to keep its bookmark.
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": tracks}))["cue"], cue(&tracks[0], -1, false));
    ask(&mut app, json!({"op":"open", "path": tracks[1]}));
    let refused = app.dispatch(&json!({"op":"bookmark", "position": 5})).unwrap_err();
    assert_eq!(refused, "Save the tape first: its bookmark is kept on the tape");
    for track in &tracks {
        assert_eq!(bookmark::read(track).unwrap(), None, "a tape never marks the files it lists");
    }
    ask(&mut app, json!({"op":"export", "dest": saved}));
    finish(&mut app);

    // Saved, it is a tape: the mark is the number of the track that is up and how far in, kept
    // on the .tape itself.
    ask(&mut app, json!({"op":"select", "index": 1}));
    let marked = ask(&mut app, json!({"op":"bookmark", "position": 120}));
    assert_eq!(marked, json!({"mark": 120, "notice": "Bookmarked"}));
    assert_eq!(bookmark::read(&saved).unwrap(), Some(Mark { track: 1, millisecond: 120 }));
    let tar = fs::read(&saved).unwrap();
    assert!(!tar.windows(12).any(|bytes| bytes == b"nap.bookmark"), "the mark is on the file, not in it");
    let tape = ask(&mut app, json!({"op":"tape"}));
    assert_eq!((&tape["markIndex"], &tape["mark"], &tape["keepsMark"]), (&json!(1), &json!(120), &json!(true)));
    // Only the marked track shows the mark; Enter from anywhere else brings that track back up.
    assert_eq!(ask(&mut app, json!({"op":"open", "path": tracks[1]}))["mark"], 120);
    ask(&mut app, json!({"op":"select", "index": 2}));
    assert_eq!(ask(&mut app, json!({"op":"open", "path": tracks[2]}))["mark"], -1);
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["mark"], -1);
    assert_eq!(ask(&mut app, json!({"op":"resume"})), json!({"index": 1, "path": tracks[1], "position": 120}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["index"], 1);

    // Opened again it comes up where it was left, inside the archive now.
    let held = |name: &str| saved.join("Marked").join(name);
    let mut app = App::default();
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [saved]}))["cue"], cue(&held("b.mp3"), 120, true));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["index"], 1);
    assert_eq!(ask(&mut app, json!({"op":"open", "path": held("b.mp3"), "start": 120, "ignore": true}))["mark"], 120);
    // Told otherwise, it does not: a time alone is a time in the first track, and a track is
    // its start, or with a time that far into it. The mark is passed over, not forgotten.
    for (asked, index, name) in [
        (json!({"ignore": true}), 0, "a.flac"),
        (json!({"start": 50}), 0, "a.flac"),
        (json!({"track": 3}), 2, "c.ogg"),
        (json!({"track": 2, "start": 50}), 1, "b.mp3"),
        (json!({"track": 1}), 0, "a.flac"),
    ] {
        let mut request = json!({"op":"load", "paths": [saved]});
        request.as_object_mut().unwrap().extend(asked.as_object().unwrap().clone());
        let mut fresh = App::default();
        let loaded = ask(&mut fresh, request);
        assert_eq!((&loaded["cue"], &loaded["notice"]), (&cue(&held(name), -1, false), &json!("")), "{asked}");
        let tape = ask(&mut fresh, json!({"op":"tape"}));
        assert_eq!((&tape["index"], &tape["markIndex"]), (&json!(index), &json!(1)), "{asked}");
    }
    // A track the tape does not have is its first instead, and says so; a single file has only
    // the one, so there a track number is quietly ignored.
    let loaded = ask(&mut app, json!({"op":"load", "paths": [saved], "track": 9}));
    assert_eq!(loaded["cue"], cue(&held("a.flac"), -1, false));
    assert_eq!(loaded["notice"], "This tape has only 3 tracks, so track 9 is its first instead");
    let alone = ask(&mut app, json!({"op":"load", "paths": [tracks[0]], "track": 9}));
    assert_eq!((&alone["cue"], &alone["notice"]), (&cue(&tracks[0], -1, false), &json!("")));

    // The mark is a number and a time and nothing else. Tracks moved about, it stays with the
    // number; saving puts it on the new file as it stands.
    ask(&mut app, json!({"op":"load", "paths": [saved]}));
    ask(&mut app, json!({"op":"edit", "action":"move", "from": 1, "to": 0}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["markIndex"], 1);
    ask(&mut app, json!({"op":"export", "dest": saved}));
    assert_eq!(finish(&mut app)["notice"], "Saved Marked.tape");
    assert_eq!(bookmark::read(&saved).unwrap(), Some(Mark { track: 1, millisecond: 120 }));
    // Two sides change nothing about it.
    ask(&mut app, json!({"op":"edit", "action":"side", "index": 2}));
    ask(&mut app, json!({"op":"flip"}));
    assert_eq!(ask(&mut app, json!({"op":"resume"}))["index"], 1);
    // A tape shortened past its mark has none to go to.
    ask(&mut app, json!({"op":"select", "index": 2}));
    ask(&mut app, json!({"op":"bookmark", "position": 30}));
    ask(&mut app, json!({"op":"edit", "action":"remove", "index": 0}));
    assert_eq!(ask(&mut app, json!({"op":"tape"}))["markIndex"], -1);
    assert_eq!(ask(&mut app, json!({"op":"resume"})), json!({}));

    // The same file listed twice keeps its true place: the mark is on the second, not the first.
    let twice = temp.path().join("Twice.tape");
    ask(&mut app, json!({"op":"load", "paths": [tracks[0], tracks[1], tracks[0]]}));
    ask(&mut app, json!({"op":"export", "dest": twice}));
    finish(&mut app);
    ask(&mut app, json!({"op":"select", "index": 2}));
    ask(&mut app, json!({"op":"bookmark", "position": 70}));
    assert_eq!(bookmark::read(&twice).unwrap(), Some(Mark { track: 2, millisecond: 70 }));
    let mut fresh = App::default();
    let loaded = ask(&mut fresh, json!({"op":"load", "paths": [twice]}));
    assert_eq!(loaded["cue"], cue(&twice.join("Twice/a.flac"), 70, true));
    let tape = ask(&mut fresh, json!({"op":"tape"}));
    assert_eq!((&tape["index"], &tape["mark"]), (&json!(2), &json!(70)));
    ask(&mut fresh, json!({"op":"select", "index": 0}));
    assert_eq!(ask(&mut fresh, json!({"op":"tape"}))["mark"], -1, "the same file, but not the marked track");

    // Removing is the same in reverse, and a J-card keeps its mark just as a tape does.
    ask(&mut fresh, json!({"op":"open", "path": twice.join("Twice/a.flac")}));
    assert_eq!(
        ask(&mut fresh, json!({"op":"bookmark", "remove": true})),
        json!({"mark": -1, "notice": "Bookmark removed"})
    );
    assert_eq!(bookmark::read(&twice).unwrap(), None);
    let card = temp.path().join("list.jcard");
    fs::write(&card, "#EXTM3U\na.flac\nc.ogg\n").unwrap();
    bookmark::write(&card, Mark { track: 1, millisecond: 90 }).unwrap();
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [card]}))["cue"], cue(&tracks[2], 90, true));
    // A mark on a track the listing does not have is no mark at all.
    bookmark::write(&card, Mark { track: 7, millisecond: 90 }).unwrap();
    assert_eq!(ask(&mut app, json!({"op":"load", "paths": [card]}))["cue"], cue(&tracks[0], -1, false));
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

    // Quitting, once agreed to, stays agreed for the moment it takes: the window closing because
    // of it asks again. Only quitting, and not for long: a quit that never happened starts over.
    use nap::session::WAY_OUT;
    let agreed = now + Duration::from_secs(41);
    assert!(!allowed(session.may_discard("quit", now + Duration::from_secs(40))));
    assert!(allowed(session.may_discard("quit", agreed)));
    assert!(allowed(session.may_discard("quit", agreed + WAY_OUT)));
    assert!(!allowed(session.may_discard("open", agreed + WAY_OUT)));
    assert!(allowed(session.may_discard("quit", agreed + WAY_OUT)), "asking to open does not take it back");
    assert!(!allowed(session.may_discard("quit", agreed + WAY_OUT * 2 + Duration::from_millis(1))));

    let mut app = App::default();
    assert_eq!(ask(&mut app, json!({"op":"discard", "action":"quit"}))["allowed"], true);
}
