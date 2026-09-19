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

/// A scratch copy of one of the quarter-second silent fixtures.
fn fixture(temp: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    let path = temp.path().join(name);
    fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name), &path).unwrap();
    path
}

#[test]
fn custom_fields_survive_in_every_tag_format() {
    use lofty::ape::{ApeItem, ApeTag};
    use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
    use lofty::ogg::tag::VorbisComments;
    use lofty::tag::ItemValue;
    let temp = tempfile::tempdir().unwrap();
    let options = WriteOptions::default;
    for name in ["silence.flac", "silence.ogg", "silence.opus"] {
        let path = fixture(&temp, name);
        let mut tag = VorbisComments::default();
        tag.set_title("Quiet".into());
        tag.push("TAPE_MOOD".into(), "sleepy".into());
        tag.save_to_path(&path, options()).unwrap();
        let card = insert::read(&path);
        assert_eq!(row(&card["tags"], "Tape mood"), Some("sleepy"), "{name}");
        assert_eq!(row(&card["file"], "Tagged with"), Some("Vorbis comments"), "{name}");
    }
    for name in ["silence.mp3", "silence.aiff", "silence.aac"] {
        let path = fixture(&temp, name);
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::UserText(ExtendedTextFrame::new(TextEncoding::UTF8, "TAPE_MOOD", "sleepy")));
        tag.save_to_path(&path, options()).unwrap();
        assert_eq!(row(&insert::read(&path)["tags"], "Tape mood"), Some("sleepy"), "{name}");
    }
    let path = fixture(&temp, "silence.m4a");
    let mut tag = Ilst::default();
    let freeform = |name: &'static str| AtomIdent::Freeform { mean: "com.apple.iTunes".into(), name: name.into() };
    tag.insert(Atom::new(freeform("TAPE_MOOD"), AtomData::UTF8("sleepy".into())));
    tag.insert(Atom::new(freeform("TAPE_COUNT"), AtomData::UnsignedInteger(42)));
    tag.insert(Atom::new(freeform("TAPE_FLAG"), AtomData::Bool(true)));
    tag.save_to_path(&path, options()).unwrap();
    let card = insert::read(&path);
    assert_eq!(row(&card["tags"], "Tape mood"), Some("sleepy"));
    assert_eq!(row(&card["tags"], "Tape count"), Some("42"));
    // MP4 stores a flag as the integer 1.
    assert_eq!(row(&card["tags"], "Tape flag"), Some("1"));
    assert_eq!(row(&card["file"], "Format"), Some("MPEG-4 audio"));

    for name in ["silence.wv", "silence.mp3"] {
        let path = fixture(&temp, name);
        let mut tag = ApeTag::default();
        tag.insert(ApeItem::new("TapeMood".into(), ItemValue::Text("sleepy".into())).unwrap());
        tag.insert(ApeItem::new("TapeBlob".into(), ItemValue::Binary(vec![1, 2, 3])).unwrap());
        tag.save_to_path(&path, options()).unwrap();
        let card = insert::read(&path);
        assert_eq!(row(&card["tags"], "Tape mood"), Some("sleepy"), "{name}");
        assert_eq!(row(&card["tags"], "Tape blob"), None, "{name}");
    }
}

#[test]
fn numbered_tracks_years_and_pictures_read_naturally() {
    use lofty::tag::{ItemKey, Tag, TagType};
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(&temp, "silence.flac");
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.insert_text(ItemKey::TrackNumber, "7".into());
    tag.insert_text(ItemKey::TrackTotal, "18".into());
    tag.insert_text(ItemKey::DiscNumber, "2".into());
    tag.insert_text(ItemKey::RecordingDate, "1998-04-20".into());
    tag.push_picture(Picture::unchecked(vec![9; 8]).pic_type(PictureType::Other).mime_type(MimeType::Jpeg).build());
    tag.save_to_path(&path, WriteOptions::default()).unwrap();
    let card = insert::read(&path);
    assert_eq!(row(&card["tags"], "Track number"), Some("7 of 18"));
    assert_eq!(row(&card["tags"], "Disc number"), Some("2"));
    assert_eq!(row(&card["tags"], "Track total"), None);
    assert_eq!(row(&card["tags"], "Pictures"), Some("1"));
    assert_eq!(card["year"], "1998");
    assert!(card["cover"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
    assert_eq!(row(&card["file"], "Bit depth"), Some("16-bit"));
}
