use nap::{
    app::{self, App},
    bookmark,
    cli::{self, Action},
    theme::{Rgb, Theme, colors_toml_path},
};
use serde_json::json;
use std::{ffi::OsString, fs};

fn parse(args: &[&str]) -> Result<Action, String> {
    cli::parse(args.iter().map(OsString::from))
}

#[test]
fn timestamps_and_invalid_values() {
    for (text, value) in [("90", 90000), ("1:02.5", 62500), ("1:02:03", 3723000), ("0", 0)] {
        assert_eq!(cli::timestamp(text), Some(value));
    }
    for text in ["", "-1", "nan", "inf", "1:60", "1.5:00", "1::2", "1:2:3:4", "10000000000000"] {
        assert_eq!(cli::timestamp(text), None, "{text}");
    }
}
#[test]
fn cli_aliases_and_literal_files() {
    for key in ["--time", "--timestamp", "--start"] {
        let Action::Run(o) = parse(&[key, "1:02.5", "--paused", "--volume=40", "--", "-song.wav"]).unwrap() else {
            panic!()
        };
        assert_eq!(o.start, 62500);
        assert!(o.paused);
        assert_eq!(o.volume, 0.4);
        assert_eq!(o.paths[0].to_str(), Some("-song.wav"));
    }
    // Several audio files line up into one tape; a .tape or .jcard comes alone.
    let Action::Run(o) = parse(&["a.flac", "b.mp3", "--loop", "c.ogg"]).unwrap() else { panic!() };
    assert_eq!(o.paths.iter().map(|p| p.to_str().unwrap()).collect::<Vec<_>>(), ["a.flac", "b.mp3", "c.ogg"]);
    let Action::Run(o) = parse(&["mix.tape"]).unwrap() else { panic!() };
    assert_eq!(o.paths.len(), 1);
    assert!(matches!(parse(&["--install-hyprland", "--link", "hypr/nap.lua"]).unwrap(), Action::Install(Some(_))));
    for args in [
        &["--time"][..],
        &["--volume", "NaN"],
        &["a.flac", "mix.tape"],
        &["mix.jcard", "b.flac"],
        &["one.tape", "two.tape"],
        &["--link", "a"],
        &["--install-hyprland", "--uninstall-hyprland"],
        &["--bogus"],
    ] {
        assert!(parse(args).is_err());
    }
}
#[test]
fn transport_policy_bounds_and_precedence() {
    assert_eq!(app::seek(-100, 12000), 0);
    assert_eq!(app::seek(20000, 12000), 12000);
    assert_eq!(app::skip(500, -5, 12000), 0);
    assert_eq!(app::skip(500, i64::MAX, 12000), 12000);
    assert_eq!(app::volume(-1.0), 0.0);
    assert_eq!(app::volume(2.0), 1.0);
    assert_eq!(app::volume(f64::NAN), 0.0);
    assert_eq!(app::start_position(0, 7000, false), 0);
    assert_eq!(app::start_position(-1, 7000, false), 7000);
    assert_eq!(app::start_position(-1, 7000, true), -1);
}
#[test]
fn file_sizes_are_label_sized() {
    assert_eq!(app::file_size(999), "999 B");
    assert_eq!(app::file_size(48_213), "48 KB");
    assert_eq!(app::file_size(12_449_000), "12.4 MB");
    assert_eq!(app::file_size(2_150_000_000), "2.15 GB");
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("five.wav");
    fs::write(&file, "audio").unwrap();
    assert_eq!(App::default().dispatch(&json!({"op":"open", "path": file})).unwrap()["size"], "5 B");
}
#[test]
fn bookmark_roundtrip_rename_and_start_policy() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.wav");
    fs::write(&first, "audio").unwrap();
    let mut app = App::default();
    assert_eq!(app.dispatch(&json!({"op":"open", "path": first})).unwrap()["mark"], -1);
    app.dispatch(&json!({"op":"bookmark", "position":7000})).unwrap();
    let renamed = temp.path().join("renamed.wav");
    fs::rename(first, &renamed).unwrap();
    assert_eq!(bookmark::read(&renamed).unwrap(), Some(7000.into()));
    assert_eq!(app.dispatch(&json!({"op":"open", "path":renamed})).unwrap()["pending"], 7000);
    assert_eq!(app.dispatch(&json!({"op":"open", "path":renamed,"start":1000})).unwrap()["pending"], 1000);
    assert_eq!(app.dispatch(&json!({"op":"open", "path":renamed,"ignore":true})).unwrap()["pending"], -1);
    app.dispatch(&json!({"op":"bookmark", "remove":true})).unwrap();
    app.dispatch(&json!({"op":"bookmark", "remove":true})).unwrap();
    assert_eq!(bookmark::read(&renamed).unwrap(), None);
    assert!(app.dispatch(&json!({"op":"open", "path":temp.path()})).is_err());
    assert!(app.dispatch(&json!({"op":"open", "path":temp.path().join("missing")})).is_err());
    assert!(app.dispatch(&json!({"op":"invalid"})).is_err());
}
#[test]
fn corrupt_bookmarks_are_ignored() {
    for value in [b"nope".as_slice(), b"-1", b"18446744073709551616", b"0:500", b"two:500", b"2:", b"2:5:9", b"\xff"] {
        assert_eq!(bookmark::decode(value), None);
    }
    // A bare number is a single file's, or a tape's first track; a tape counts its tracks from one.
    use bookmark::Mark;
    assert_eq!(bookmark::decode(b" 7000\n"), Some(Mark { track: 0, millisecond: 7000 }));
    assert_eq!(bookmark::decode(b"3:62500"), Some(Mark { track: 2, millisecond: 62500 }));
    assert_eq!(bookmark::decode(b"1:9"), Some(9.into()));
    assert_eq!(bookmark::encode(Mark { track: 2, millisecond: 62500 }), "3:62500");
    assert_eq!(bookmark::encode(7000.into()), "7000");
}
#[test]
fn theme_validates_colors_and_prefers_state_directory() {
    assert!(Rgb::parse("#abcdzz").is_none());
    assert!(Rgb::parse("#fff").is_none());
    let t = Theme::parse("background = \"#123456\"\naccent = \"bad\"\ncolor1 = \"#c24f57\"\n");
    assert_eq!(t.red.to_css(), "#c24f57");
    assert_eq!(Theme::parse("").red, Theme::default().red);
    assert_eq!(t.background.to_css(), "#123456");
    assert_eq!(t.accent, Theme::default().accent);
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    let config = temp.path().join("config");
    for root in [&state, &config] {
        fs::create_dir_all(root.join("omarchy/current/theme")).unwrap();
        fs::write(root.join("omarchy/current/theme/colors.toml"), "").unwrap();
    }
    assert_eq!(colors_toml_path(Some(&state), Some(&config)), Some(state.join("omarchy/current/theme/colors.toml")));
}
