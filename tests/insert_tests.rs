use lofty::TextEncoding;
use lofty::config::WriteOptions;
use lofty::id3::v2::{ExtendedTextFrame, Frame, Id3v2Tag, UnsynchronizedTextFrame};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor, TagExt};
use nap::app::App;
use nap::insert;
use serde_json::{Value, json};
use std::fs;

/// One second of silent 8 kHz mono PCM.
fn wav() -> Vec<u8> {
    let data = vec![0u8; 16000];
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(36u32.saturating_add(data.len() as u32).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((data.len() as u32).to_le_bytes());
    bytes.extend(data);
    bytes
}

fn row<'a>(rows: &'a Value, label: &str) -> Option<&'a str> {
    rows.as_array()?.iter().find(|r| r[0] == label)?[1].as_str()
}

#[test]
fn insert_prints_standard_custom_and_technical_fields() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("tagged.wav");
    fs::write(&path, wav()).unwrap();
    let mut tag = Id3v2Tag::new();
    tag.set_title("Roygbiv".into());
    tag.set_artist("Boards of Omarchy".into());
    tag.set_album("Music Has the Right to Nap".into());
    tag.set_date(Timestamp { year: 1998, month: Some(4), day: Some(20), hour: None, minute: None, second: None });
    tag.insert(Frame::UserText(ExtendedTextFrame::new(TextEncoding::UTF8, "TAPE_MOOD", "sleepy")));
    tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
        TextEncoding::UTF8,
        *b"eng",
        String::new(),
        "first line\r\nsecond line",
    )));
    tag.insert_picture(
        Picture::unchecked(vec![1, 2, 3, 4]).pic_type(PictureType::CoverFront).mime_type(MimeType::Png).build(),
    );
    tag.save_to_path(&path, WriteOptions::default()).unwrap();

    let card = insert::read(&path);
    assert_eq!(card["title"], "Roygbiv");
    assert_eq!(card["artist"], "Boards of Omarchy");
    assert_eq!(card["year"], "1998");
    assert_eq!(card["lyrics"], "first line\nsecond line");
    assert_eq!(card["cover"], "data:image/png;base64,AQIDBA==");
    assert_eq!(row(&card["tags"], "Album title"), Some("Music Has the Right to Nap"));
    assert_eq!(row(&card["tags"], "Tape mood"), Some("sleepy"));
    assert_eq!(row(&card["file"], "Format"), Some("WAV"));
    assert_eq!(row(&card["file"], "Sample rate"), Some("8 kHz"));
    assert_eq!(row(&card["file"], "Channels"), Some("Mono"));
    assert_eq!(row(&card["file"], "Tagged with"), Some("ID3v2"));
    assert_eq!(row(&card["file"], "File"), Some("tagged.wav"));

    let mut app = App::default();
    assert!(app.dispatch(&json!({"op":"insert"})).is_err());
    app.dispatch(&json!({"op":"open", "path": path})).unwrap();
    assert_eq!(app.dispatch(&json!({"op":"insert"})).unwrap()["title"], "Roygbiv");
}

#[test]
fn untagged_and_unreadable_files_still_get_a_file_panel() {
    let temp = tempfile::tempdir().unwrap();
    let plain = temp.path().join("plain.wav");
    fs::write(&plain, wav()).unwrap();
    let card = insert::read(&plain);
    assert_eq!(card["title"], "");
    assert_eq!(card["tags"], json!([]));
    assert_eq!(row(&card["file"], "Length"), Some("0:01"));
    let junk = temp.path().join("junk.mp3");
    fs::write(&junk, "not audio").unwrap();
    assert_eq!(row(&insert::read(&junk)["file"], "Size"), Some("9 B"));
}

#[test]
fn keys_read_like_liner_notes() {
    assert_eq!(insert::humanize("TrackArtist"), "Track artist");
    assert_eq!(insert::humanize("REPLAYGAIN_TRACK_GAIN"), "Replaygain track gain");
    assert_eq!(insert::humanize("MusicBrainzRecordingId"), "MusicBrainz recording ID");
    assert_eq!(insert::humanize("Bpm"), "BPM");
}
