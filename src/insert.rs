//! The cassette's paper insert: the cover, the lyrics, every tag in the file, and its technical details.
//! Standard fields come from lofty's format-neutral view; custom ones (TXXX frames, extra Vorbis
//! comments, freeform MP4 atoms, APE items) are read from each format's own tag so nothing is left out.

use crate::app::file_size;
use lofty::ape::ApeTag;
use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::id3::v2::{Frame, Id3v2Tag};
use lofty::mp4::{AtomData, AtomIdent, Ilst};
use lofty::ogg::tag::VorbisComments;
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, ItemValue, TagType};
use serde_json::{Value, json};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Covers beyond this are skipped rather than pushed through the JSON boundary.
const COVER_LIMIT: usize = 12 * 1024 * 1024;
const VALUE_LIMIT: usize = 4000;

type Rows = Vec<(String, String)>;

fn push(rows: &mut Rows, label: String, value: &str) {
    let value: String = value.trim().chars().take(VALUE_LIMIT).collect();
    if !value.is_empty() && !rows.iter().any(|(l, v)| *l == label && *v == value) {
        rows.push((label, value));
    }
}

/// `TrackArtist` becomes "Track artist"; `REPLAYGAIN_TRACK_GAIN` becomes "Replaygain track gain".
pub fn humanize(key: &str) -> String {
    let shouting = !key.chars().any(|c| c.is_lowercase());
    let mut words = String::new();
    for (i, c) in key.chars().enumerate() {
        if c == '_' {
            words.push(' ');
        } else if c.is_uppercase() && !shouting && i > 0 {
            words.push(' ');
            words.extend(c.to_lowercase());
        } else if shouting && i > 0 {
            words.extend(c.to_lowercase());
        } else {
            words.push(c);
        }
    }
    for (from, to) in
        [("Music brainz", "MusicBrainz"), ("Isrc", "ISRC"), ("Bpm", "BPM"), (" id", " ID"), (" url", " URL")]
    {
        words = words.replace(from, to);
    }
    words
}

fn value_text(value: &ItemValue) -> String {
    match value {
        ItemValue::Text(text) | ItemValue::Locator(text) => text.clone(),
        ItemValue::Binary(data) => format!("binary data, {}", file_size(data.len() as u64)),
    }
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn vorbis_extras(tag: &VorbisComments, rows: &mut Rows) {
    for (key, value) in tag.items() {
        if ItemKey::from_key(TagType::VorbisComments, key).is_none() {
            push(rows, humanize(key), value);
        }
    }
}

fn id3v2_extras(tag: &Id3v2Tag, rows: &mut Rows) {
    for frame in tag {
        match frame {
            Frame::UserText(f) if ItemKey::from_key(TagType::Id3v2, &f.description).is_none() => {
                push(rows, humanize(&f.description), &f.content)
            }
            Frame::UserUrl(f) if ItemKey::from_key(TagType::Id3v2, &f.description).is_none() => {
                push(rows, humanize(&f.description), &f.content)
            }
            _ => {}
        }
    }
}

fn ilst_extras(tag: &Ilst, rows: &mut Rows) {
    for atom in tag {
        let AtomIdent::Freeform { mean, name } = atom.ident() else { continue };
        if ItemKey::from_key(TagType::Mp4Ilst, &format!("----:{mean}:{name}")).is_some() {
            continue;
        }
        for data in atom.data() {
            match data {
                AtomData::UTF8(text) | AtomData::UTF16(text) => push(rows, humanize(name), text),
                AtomData::SignedInteger(n) => push(rows, humanize(name), &n.to_string()),
                AtomData::UnsignedInteger(n) => push(rows, humanize(name), &n.to_string()),
                _ => {}
            }
        }
    }
}

fn ape_extras(tag: &ApeTag, rows: &mut Rows) {
    for item in tag {
        if ItemKey::from_key(TagType::Ape, item.key()).is_none() && !matches!(item.value(), ItemValue::Binary(_)) {
            push(rows, humanize(item.key()), &value_text(item.value()));
        }
    }
}

/// Fields the format-neutral view cannot name, read from the format's own tag.
fn extras(path: &Path, kind: FileType, rows: &mut Rows) -> Option<()> {
    let mut reader = BufReader::new(File::open(path).ok()?);
    let options = ParseOptions::new().read_cover_art(false);
    macro_rules! read {
        ($file:ty) => {
            <$file>::read_from(&mut reader, options).ok()?
        };
    }
    match kind {
        FileType::Flac => {
            let file = read!(lofty::flac::FlacFile);
            if let Some(t) = file.vorbis_comments() {
                vorbis_extras(t, rows);
            }
            if let Some(t) = file.id3v2() {
                id3v2_extras(t, rows);
            }
        }
        FileType::Vorbis => vorbis_extras(read!(lofty::ogg::VorbisFile).vorbis_comments(), rows),
        FileType::Opus => vorbis_extras(read!(lofty::ogg::OpusFile).vorbis_comments(), rows),
        FileType::Speex => vorbis_extras(read!(lofty::ogg::SpeexFile).vorbis_comments(), rows),
        FileType::Mpeg => {
            let file = read!(lofty::mpeg::MpegFile);
            if let Some(t) = file.id3v2() {
                id3v2_extras(t, rows);
            }
            if let Some(t) = file.ape() {
                ape_extras(t, rows);
            }
        }
        FileType::Mp4 => {
            if let Some(t) = read!(lofty::mp4::Mp4File).ilst() {
                ilst_extras(t, rows);
            }
        }
        FileType::Wav => {
            if let Some(t) = read!(lofty::iff::wav::WavFile).id3v2() {
                id3v2_extras(t, rows);
            }
        }
        FileType::Aiff => {
            if let Some(t) = read!(lofty::iff::aiff::AiffFile).id3v2() {
                id3v2_extras(t, rows);
            }
        }
        FileType::Aac => {
            if let Some(t) = read!(lofty::aac::AacFile).id3v2() {
                id3v2_extras(t, rows);
            }
        }
        FileType::Ape => {
            let file = read!(lofty::ape::ApeFile);
            if let Some(t) = file.ape() {
                ape_extras(t, rows);
            }
            if let Some(t) = file.id3v2() {
                id3v2_extras(t, rows);
            }
        }
        FileType::WavPack => {
            if let Some(t) = read!(lofty::wavpack::WavPackFile).ape() {
                ape_extras(t, rows);
            }
        }
        FileType::Mpc => {
            let file = read!(lofty::musepack::MpcFile);
            if let Some(t) = file.ape() {
                ape_extras(t, rows);
            }
            if let Some(t) = file.id3v2() {
                id3v2_extras(t, rows);
            }
        }
        _ => {}
    }
    Some(())
}

fn format_name(kind: FileType) -> String {
    match kind {
        FileType::Aac => "AAC".into(),
        FileType::Aiff => "AIFF".into(),
        FileType::Ape => "Monkey's Audio".into(),
        FileType::Flac => "FLAC".into(),
        FileType::Mpeg => "MPEG audio".into(),
        FileType::Mp4 => "MPEG-4 audio".into(),
        FileType::Mpc => "Musepack".into(),
        FileType::Opus => "Opus".into(),
        FileType::Vorbis => "Ogg Vorbis".into(),
        FileType::Speex => "Speex".into(),
        FileType::Wav => "WAV".into(),
        FileType::WavPack => "WavPack".into(),
        other => format!("{other:?}"),
    }
}

fn tag_name(kind: TagType) -> String {
    match kind {
        TagType::Ape => "APE".into(),
        TagType::Id3v1 => "ID3v1".into(),
        TagType::Id3v2 => "ID3v2".into(),
        TagType::Mp4Ilst => "MP4 atoms".into(),
        TagType::VorbisComments => "Vorbis comments".into(),
        TagType::RiffInfo => "RIFF info".into(),
        TagType::AiffText => "AIFF text".into(),
        other => format!("{other:?}"),
    }
}

fn file_rows(path: &Path, tagged: Option<&TaggedFile>) -> Rows {
    let mut rows = Rows::new();
    if let Some(tagged) = tagged {
        let p = tagged.properties();
        push(&mut rows, "Format".into(), &format_name(tagged.file_type()));
        let seconds = p.duration().as_secs();
        if seconds > 0 {
            push(&mut rows, "Length".into(), &format!("{}:{:02}", seconds / 60, seconds % 60));
        }
        if let Some(rate) = p.sample_rate() {
            push(&mut rows, "Sample rate".into(), &format!("{} kHz", f64::from(rate) / 1000.0));
        }
        if let Some(depth) = p.bit_depth() {
            push(&mut rows, "Bit depth".into(), &format!("{depth}-bit"));
        }
        match p.channels() {
            Some(1) => push(&mut rows, "Channels".into(), "Mono"),
            Some(2) => push(&mut rows, "Channels".into(), "Stereo"),
            Some(n) => push(&mut rows, "Channels".into(), &n.to_string()),
            None => {}
        }
        if let Some(rate) = p.audio_bitrate().or(p.overall_bitrate()) {
            push(&mut rows, "Bitrate".into(), &format!("{rate} kbps"));
        }
        let kinds: Vec<String> = tagged.tags().iter().map(|t| tag_name(t.tag_type())).collect();
        push(&mut rows, "Tagged with".into(), &kinds.join(", "));
    }
    if let Ok(meta) = std::fs::metadata(path) {
        push(&mut rows, "Size".into(), &file_size(meta.len()));
    }
    if let Some(name) = path.file_name() {
        push(&mut rows, "File".into(), &name.to_string_lossy());
    }
    if let Some(folder) = path.parent() {
        push(&mut rows, "Folder".into(), &folder.to_string_lossy());
    }
    rows
}

/// Everything the insert prints, as JSON for the Qt side.
pub fn read(path: &Path) -> Value {
    let tagged = Probe::open(path).ok().and_then(|p| p.guess_file_type().ok()).and_then(|p| p.read().ok());
    let (mut tags, mut lyrics, mut cover) = (Rows::new(), String::new(), String::new());
    let mut first = |key: ItemKey| -> String {
        let found = tagged.iter().flat_map(|f| f.tags()).find_map(|t| t.get_string(key).map(str::to_owned));
        found.unwrap_or_default()
    };
    let (title, artist, album) = (first(ItemKey::TrackTitle), first(ItemKey::TrackArtist), first(ItemKey::AlbumTitle));
    let year = [ItemKey::Year, ItemKey::RecordingDate, ItemKey::ReleaseDate]
        .into_iter()
        .map(&mut first)
        .find(|v| !v.is_empty());
    let year: String = year.unwrap_or_default().chars().take(4).collect();
    if let Some(file) = &tagged {
        for tag in file.tags() {
            for item in tag.items() {
                let text = value_text(item.value());
                match item.key() {
                    ItemKey::Lyrics | ItemKey::UnsyncLyrics => {
                        if lyrics.is_empty() {
                            lyrics = text.replace("\r\n", "\n").replace('\r', "\n").trim().to_owned();
                        }
                    }
                    ItemKey::TrackTotal | ItemKey::DiscTotal => {}
                    key @ (ItemKey::TrackNumber | ItemKey::DiscNumber) => {
                        let total = if key == ItemKey::TrackNumber { ItemKey::TrackTotal } else { ItemKey::DiscTotal };
                        let of = tag.get_string(total).map(|n| format!(" of {n}")).unwrap_or_default();
                        push(&mut tags, humanize(&format!("{key:?}")), &format!("{text}{of}"));
                    }
                    key => push(&mut tags, humanize(&format!("{key:?}")), &text),
                }
            }
        }
        let pictures = || file.tags().iter().flat_map(|t| t.pictures());
        let front = pictures().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| pictures().next());
        if let Some(picture) = front.filter(|p| p.data().len() <= COVER_LIMIT) {
            let mime = picture.mime_type().map_or("image/jpeg", |m| m.as_str());
            cover = format!("data:{mime};base64,{}", base64(picture.data()));
        }
        let count = pictures().count();
        if count > 0 {
            push(&mut tags, "Pictures".into(), &count.to_string());
        }
        extras(path, file.file_type(), &mut tags);
    }
    let rows = |rows: Rows| rows.into_iter().map(|(label, value)| json!([label, value])).collect::<Vec<_>>();
    json!({"title": title, "artist": artist, "album": album, "year": year, "lyrics": lyrics, "cover": cover,
        "tags": rows(tags), "file": rows(file_rows(path, tagged.as_ref()))})
}
