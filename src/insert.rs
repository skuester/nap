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
use lofty::tag::{ItemKey, ItemValue, Tag, TagType};
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

fn atom_text(data: &AtomData) -> Option<String> {
    match data {
        AtomData::UTF8(text) | AtomData::UTF16(text) => Some(text.clone()),
        AtomData::SignedInteger(n) => Some(n.to_string()),
        AtomData::UnsignedInteger(n) => Some(n.to_string()),
        _ => None,
    }
}

fn ilst_extras(tag: &Ilst, rows: &mut Rows) {
    let custom = tag.into_iter().filter_map(|atom| match atom.ident() {
        AtomIdent::Freeform { mean, name }
            if ItemKey::from_key(TagType::Mp4Ilst, &format!("----:{mean}:{name}")).is_none() =>
        {
            Some((name, atom))
        }
        _ => None,
    });
    for (name, atom) in custom {
        for text in atom.data().filter_map(atom_text) {
            push(rows, humanize(name), &text);
        }
    }
}

fn ape_extras(tag: &ApeTag, rows: &mut Rows) {
    let custom = tag.into_iter().filter(|item| ItemKey::from_key(TagType::Ape, item.key()).is_none());
    for item in custom.filter(|item| !matches!(item.value(), ItemValue::Binary(_))) {
        push(rows, humanize(item.key()), &value_text(item.value()));
    }
}

/// The format's own tags, whichever of them it carries.
#[derive(Default)]
struct Native<'a> {
    id3v2: Option<&'a Id3v2Tag>,
    ape: Option<&'a ApeTag>,
    vorbis: Option<&'a VorbisComments>,
    ilst: Option<&'a Ilst>,
}

impl Native<'_> {
    fn extras(&self, rows: &mut Rows) {
        if let Some(tag) = self.vorbis {
            vorbis_extras(tag, rows);
        }
        if let Some(tag) = self.id3v2 {
            id3v2_extras(tag, rows);
        }
        if let Some(tag) = self.ilst {
            ilst_extras(tag, rows);
        }
        if let Some(tag) = self.ape {
            ape_extras(tag, rows);
        }
    }
}

trait NativeTags: AudioFile + Sized {
    fn native(&self) -> Native<'_>;

    fn gather(reader: &mut BufReader<File>, rows: &mut Rows) -> Option<()> {
        let file = Self::read_from(reader, ParseOptions::new().read_cover_art(false)).ok()?;
        file.native().extras(rows);
        Some(())
    }
}
impl NativeTags for lofty::flac::FlacFile {
    fn native(&self) -> Native<'_> {
        Native { vorbis: self.vorbis_comments(), id3v2: self.id3v2(), ..Native::default() }
    }
}
impl NativeTags for lofty::ogg::VorbisFile {
    fn native(&self) -> Native<'_> {
        Native { vorbis: Some(self.vorbis_comments()), ..Native::default() }
    }
}
impl NativeTags for lofty::ogg::OpusFile {
    fn native(&self) -> Native<'_> {
        Native { vorbis: Some(self.vorbis_comments()), ..Native::default() }
    }
}
impl NativeTags for lofty::ogg::SpeexFile {
    fn native(&self) -> Native<'_> {
        Native { vorbis: Some(self.vorbis_comments()), ..Native::default() }
    }
}
impl NativeTags for lofty::mpeg::MpegFile {
    fn native(&self) -> Native<'_> {
        Native { id3v2: self.id3v2(), ape: self.ape(), ..Native::default() }
    }
}
impl NativeTags for lofty::mp4::Mp4File {
    fn native(&self) -> Native<'_> {
        Native { ilst: self.ilst(), ..Native::default() }
    }
}
impl NativeTags for lofty::iff::wav::WavFile {
    fn native(&self) -> Native<'_> {
        Native { id3v2: self.id3v2(), ..Native::default() }
    }
}
impl NativeTags for lofty::iff::aiff::AiffFile {
    fn native(&self) -> Native<'_> {
        Native { id3v2: self.id3v2(), ..Native::default() }
    }
}
impl NativeTags for lofty::aac::AacFile {
    fn native(&self) -> Native<'_> {
        Native { id3v2: self.id3v2(), ..Native::default() }
    }
}
impl NativeTags for lofty::ape::ApeFile {
    fn native(&self) -> Native<'_> {
        Native { ape: self.ape(), id3v2: self.id3v2(), ..Native::default() }
    }
}
impl NativeTags for lofty::wavpack::WavPackFile {
    fn native(&self) -> Native<'_> {
        Native { ape: self.ape(), ..Native::default() }
    }
}
impl NativeTags for lofty::musepack::MpcFile {
    fn native(&self) -> Native<'_> {
        Native { ape: self.ape(), id3v2: self.id3v2(), ..Native::default() }
    }
}

/// Fields the format-neutral view cannot name, read from the format's own tag.
fn extras(path: &Path, kind: FileType, rows: &mut Rows) -> Option<()> {
    let reader = &mut BufReader::new(File::open(path).ok()?);
    match kind {
        FileType::Flac => lofty::flac::FlacFile::gather(reader, rows),
        FileType::Vorbis => lofty::ogg::VorbisFile::gather(reader, rows),
        FileType::Opus => lofty::ogg::OpusFile::gather(reader, rows),
        FileType::Speex => lofty::ogg::SpeexFile::gather(reader, rows),
        FileType::Mpeg => lofty::mpeg::MpegFile::gather(reader, rows),
        FileType::Mp4 => lofty::mp4::Mp4File::gather(reader, rows),
        FileType::Wav => lofty::iff::wav::WavFile::gather(reader, rows),
        FileType::Aiff => lofty::iff::aiff::AiffFile::gather(reader, rows),
        FileType::Aac => lofty::aac::AacFile::gather(reader, rows),
        FileType::Ape => lofty::ape::ApeFile::gather(reader, rows),
        FileType::WavPack => lofty::wavpack::WavPackFile::gather(reader, rows),
        FileType::Mpc => lofty::musepack::MpcFile::gather(reader, rows),
        _ => Some(()),
    }
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

fn channel_name(channels: u8) -> String {
    match channels {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        n => n.to_string(),
    }
}

fn audio_rows(tagged: &TaggedFile, rows: &mut Rows) {
    let p = tagged.properties();
    let seconds = p.duration().as_secs();
    let length = (seconds > 0).then(|| format!("{}:{:02}", seconds / 60, seconds % 60));
    let kinds: Vec<String> = tagged.tags().iter().map(|t| tag_name(t.tag_type())).collect();
    let facts = [
        ("Format", Some(format_name(tagged.file_type()))),
        ("Length", length),
        ("Sample rate", p.sample_rate().map(|rate| format!("{} kHz", f64::from(rate) / 1000.0))),
        ("Bit depth", p.bit_depth().map(|depth| format!("{depth}-bit"))),
        ("Channels", p.channels().map(channel_name)),
        ("Bitrate", p.audio_bitrate().or(p.overall_bitrate()).map(|rate| format!("{rate} kbps"))),
        ("Tagged with", Some(kinds.join(", "))),
    ];
    for (label, value) in facts {
        push(rows, label.into(), &value.unwrap_or_default());
    }
}

fn file_rows(path: &Path, tagged: Option<&TaggedFile>) -> Rows {
    let mut rows = Rows::new();
    if let Some(tagged) = tagged {
        audio_rows(tagged, &mut rows);
    }
    let size = std::fs::metadata(path).map(|meta| file_size(meta.len())).unwrap_or_default();
    push(&mut rows, "Size".into(), &size);
    push(&mut rows, "File".into(), &path.file_name().unwrap_or_default().to_string_lossy());
    push(&mut rows, "Folder".into(), &path.parent().unwrap_or(Path::new("")).to_string_lossy());
    rows
}

/// The first value any of the file's tags gives for `key`.
fn first(file: &TaggedFile, key: ItemKey) -> String {
    file.tags().iter().find_map(|t| t.get_string(key)).unwrap_or_default().to_owned()
}

fn year(file: &TaggedFile) -> String {
    let dates = [ItemKey::Year, ItemKey::RecordingDate, ItemKey::ReleaseDate].map(|key| first(file, key));
    dates.iter().find(|date| !date.is_empty()).map(|date| date.chars().take(4).collect()).unwrap_or_default()
}

fn lyrics(file: &TaggedFile) -> String {
    let text = [ItemKey::Lyrics, ItemKey::UnsyncLyrics].map(|key| first(file, key)).into_iter().find(|t| !t.is_empty());
    text.unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n").trim().to_owned()
}

/// "7 of 18" when the tag also knows how many there are.
fn numbered(tag: &Tag, key: ItemKey, text: &str) -> String {
    let total = if key == ItemKey::TrackNumber { ItemKey::TrackTotal } else { ItemKey::DiscTotal };
    tag.get_string(total).map_or(text.to_owned(), |n| format!("{text} of {n}"))
}

fn tag_rows(file: &TaggedFile) -> Rows {
    let mut rows = Rows::new();
    for tag in file.tags() {
        for item in tag.items() {
            let text = value_text(item.value());
            match item.key() {
                ItemKey::Lyrics | ItemKey::UnsyncLyrics | ItemKey::TrackTotal | ItemKey::DiscTotal => {}
                key @ (ItemKey::TrackNumber | ItemKey::DiscNumber) => {
                    push(&mut rows, humanize(&format!("{key:?}")), &numbered(tag, key, &text))
                }
                key => push(&mut rows, humanize(&format!("{key:?}")), &text),
            }
        }
    }
    let pictures = file.tags().iter().map(|t| t.pictures().len()).sum::<usize>();
    push(&mut rows, "Pictures".into(), &if pictures > 0 { pictures.to_string() } else { String::new() });
    rows
}

/// The front cover, or failing that the first picture, as a data URL the interface can show.
fn cover(file: &TaggedFile) -> String {
    let pictures = || file.tags().iter().flat_map(|t| t.pictures());
    let front = pictures().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| pictures().next());
    let Some(picture) = front.filter(|p| p.data().len() <= COVER_LIMIT) else { return String::new() };
    let mime = picture.mime_type().map_or("image/jpeg", |m| m.as_str());
    format!("data:{mime};base64,{}", base64(picture.data()))
}

/// Everything the insert prints, as JSON for the Qt side.
pub fn read(path: &Path) -> Value {
    let tagged = Probe::open(path).ok().and_then(|p| p.guess_file_type().ok()).and_then(|p| p.read().ok());
    let rows = |rows: Rows| rows.into_iter().map(|(label, value)| json!([label, value])).collect::<Vec<_>>();
    let mut card = json!({"title": "", "artist": "", "album": "", "year": "", "lyrics": "", "cover": "", "tags": []});
    if let Some(file) = &tagged {
        let mut tags = tag_rows(file);
        extras(path, file.file_type(), &mut tags);
        card = json!({"title": first(file, ItemKey::TrackTitle), "artist": first(file, ItemKey::TrackArtist),
            "album": first(file, ItemKey::AlbumTitle), "year": year(file), "lyrics": lyrics(file),
            "cover": cover(file), "tags": rows(tags)});
    }
    card["file"] = json!(rows(file_rows(path, tagged.as_ref())));
    card
}

/// A track listing's worth: title, artist, and length in seconds, empty where the file says nothing.
pub fn brief(path: &Path) -> (String, String, u64) {
    let options = ParseOptions::new().read_cover_art(false);
    let file =
        Probe::open(path).ok().and_then(|p| p.options(options).guess_file_type().ok()).and_then(|p| p.read().ok());
    file.map_or_else(Default::default, |file| {
        (first(&file, ItemKey::TrackTitle), first(&file, ItemKey::TrackArtist), file.properties().duration().as_secs())
    })
}
