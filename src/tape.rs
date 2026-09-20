//! Mixtapes. A tape is a name, an optional cover image, and an ordered list of audio files.
//!
//! On disk a tape is a directory: `tape.jcard` (the track listing, as on a cassette's J-card),
//! an optional `cover.<ext>`, and the audio files beside them. A `.tape` file is that directory
//! as a plain tar archive, and nap plays it as it is: an uncompressed tar holds each file whole,
//! so a track is just a stretch of the archive. A `.jcard` can also stand alone, listing
//! absolute paths to files that stay where they are.
//!
//! The index is M3U-compatible text so it is easy to read, edit, and open elsewhere:
//!
//! ```text
//! #EXTM3U
//! #EXTENC:UTF-8
//! #NAP:1
//! #PLAYLIST:Summer '98
//! #EXTIMG:cover.jpg
//! #NAP-FROM:Shane
//! #NAP-NOTE:Made this for the drive up.
//! #NAP-NOTE:Side B is the good one.
//!
//! #EXTINF:151,Boards of Canada - Roygbiv
//! 01 Roygbiv.flac
//! #EXTINF:242,The Rapture - Don't Stop
//! 02 Don't Stop.mp3
//! ```
//!
//! `#NAP:` is the version of this format. `#NAP-FROM:` and `#NAP-NOTE:` are who made the tape and
//! what they wrote to go with it, one `#NAP-NOTE:` per line. M3U has no such fields, and other
//! players skip `#` lines they do not know. `#EXTINF:` is for those players, and for naming a
//! track that has gone missing; nap itself reads titles and lengths from the audio.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub const INDEX: &str = "tape.jcard";
/// The format this nap writes, and the newest it reads.
pub const VERSION: u32 = 1;
/// An index is a page of text; one longer than this is not an index.
const INDEX_LIMIT: u64 = 1 << 20;
const AUDIO: [&str; 15] =
    ["mp3", "flac", "wav", "ogg", "oga", "opus", "m4a", "m4b", "aac", "aiff", "aif", "wma", "ape", "wv", "mpc"];
const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Where one file lies inside a `.tape`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub archive: PathBuf,
    pub offset: u64,
    pub length: u64,
}

/// A track or a cover: a file of its own, or one held inside a `.tape`. `path` is where it is
/// listed and what it is called; for a held file that is the archive's path followed by the
/// entry's, which names it without being anywhere on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: PathBuf,
    pub span: Option<Span>,
}

impl<P: Into<PathBuf>> From<P> for Source {
    fn from(path: P) -> Self {
        Source { path: path.into(), span: None }
    }
}

impl Source {
    /// The file on disk its bytes are in.
    pub fn file(&self) -> &Path {
        self.span.as_ref().map_or(&self.path, |span| &span.archive)
    }

    pub fn is_there(&self) -> bool {
        self.file().is_file()
    }

    pub fn open(&self) -> io::Result<Window> {
        let file = File::open(self.file())?;
        let (start, length) = match &self.span {
            Some(span) => (span.offset, span.length),
            None => (0, file.metadata()?.len()),
        };
        Window::new(file, start, length)
    }

    pub fn size(&self) -> u64 {
        self.span.as_ref().map_or_else(|| fs::metadata(&self.path).map_or(0, |meta| meta.len()), |span| span.length)
    }

    fn modified(&self) -> u64 {
        let changed = fs::metadata(self.file()).and_then(|meta| meta.modified()).ok();
        changed.and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |since| since.as_secs())
    }
}

/// A stretch of a file, read as though it were a file of its own.
pub struct Window {
    file: File,
    start: u64,
    length: u64,
    at: u64,
}

impl Window {
    fn new(mut file: File, start: u64, length: u64) -> io::Result<Self> {
        file.seek(SeekFrom::Start(start))?;
        Ok(Window { file, start, length, at: 0 })
    }
}

impl Read for Window {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let room = self.length.saturating_sub(self.at).min(buffer.len() as u64) as usize;
        let n = self.file.read(&mut buffer[..room])?;
        self.at += n as u64;
        Ok(n)
    }
}

impl Seek for Window {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(by) => self.length.checked_add_signed(by),
            SeekFrom::Current(by) => self.at.checked_add_signed(by),
        };
        let at = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        self.file.seek(SeekFrom::Start(self.start + at))?;
        self.at = at;
        Ok(at)
    }
}

/// What a listing says of a track besides where it is.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Brief {
    pub title: String,
    pub artist: String,
    pub seconds: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tape {
    pub name: String,
    /// Who made it, as they signed it.
    pub from: String,
    /// What they wrote to go with it; may run to several lines.
    pub note: String,
    pub cover: Option<Source>,
    pub tracks: Vec<Source>,
}

/// An index as read, before anyone has looked for the files it lists.
#[derive(Debug, Default)]
pub struct Index {
    pub tape: Tape,
    /// What to call each track if it cannot be found: its `#EXTINF` title, or else its file name.
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Archive,
    Index,
    Image,
    Audio,
    Other,
}

fn extension(path: &Path) -> String {
    path.extension().unwrap_or_default().to_string_lossy().to_lowercase()
}

/// What a dropped or opened file is, going by its extension.
pub fn kind(path: &Path) -> Kind {
    match extension(path).as_str() {
        "tape" => Kind::Archive,
        "jcard" => Kind::Index,
        ext if IMAGES.contains(&ext) => Kind::Image,
        ext if AUDIO.contains(&ext) => Kind::Audio,
        _ => Kind::Other,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

fn newer(version: &str) -> Result<(), String> {
    match version.trim().parse::<u32>() {
        Ok(version) if version > VERSION => Err("This tape was made by a newer nap than this one".into()),
        _ => Ok(()),
    }
}

/// Read an index. Paths in it are relative to `base`, the directory the index sits in.
pub fn parse(text: &str, base: &Path) -> Result<Index, String> {
    let mut index = Index::default();
    let tape = &mut index.tape;
    let mut title = None;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(version) = line.strip_prefix("#NAP:") {
            newer(version)?;
        } else if let Some(name) = line.strip_prefix("#PLAYLIST:") {
            tape.name = name.trim().to_owned();
        } else if let Some(cover) = line.strip_prefix("#EXTIMG:") {
            tape.cover = Some(base.join(cover.trim()).into());
        } else if let Some(from) = line.strip_prefix("#NAP-FROM:") {
            tape.from = from.trim().to_owned();
        } else if let Some(note) = line.strip_prefix("#NAP-NOTE:") {
            tape.note = format!("{}\n{}", tape.note, note.trim()).trim_start_matches('\n').to_owned();
        } else if let Some(inf) = line.strip_prefix("#EXTINF:") {
            title = inf.split_once(',').map(|(_, title)| title.trim().to_owned()).filter(|title| !title.is_empty());
        } else if !line.starts_with('#') {
            // A name that itself starts with `#` is written `./#...` so it is not taken for a comment.
            let track = base.join(line.strip_prefix("./").unwrap_or(line));
            index.labels.push(title.take().unwrap_or_else(|| file_name(&track)));
            tape.tracks.push(track.into());
        }
    }
    Ok(index)
}

impl Index {
    /// The tape made of what `find` can find of the listing, and the names of the tracks it cannot.
    fn settle(self, find: impl Fn(&Source) -> Option<Source>) -> (Tape, Vec<String>) {
        let mut tape = self.tape;
        let mut missing = Vec::new();
        for (track, label) in std::mem::take(&mut tape.tracks).iter().zip(self.labels) {
            match find(track) {
                Some(found) => tape.tracks.push(found),
                None => missing.push(label),
            }
        }
        tape.cover = tape.cover.as_ref().and_then(find);
        (tape, missing)
    }
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ").trim().to_owned()
}

/// A track's `#EXTINF` line, for players that show it rather than reading the file.
fn extinf(entry: &str, brief: &Brief) -> String {
    let stem = Path::new(entry).file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let title = if brief.title.is_empty() { stem } else { one_line(&brief.title) };
    let by = if brief.artist.is_empty() { String::new() } else { format!("{} - ", one_line(&brief.artist)) };
    // M3U writes an unknown length as -1.
    let seconds = if brief.seconds > 0 { brief.seconds.to_string() } else { "-1".into() };
    format!("#EXTINF:{seconds},{by}{title}\n")
}

/// Write `tape`'s index listing `entries`, one per track and each with its `brief` where there is
/// one, with `cover` as it should appear in the file.
pub fn render(tape: &Tape, cover: Option<&str>, entries: &[String], briefs: &[Brief]) -> String {
    let mut text = format!("#EXTM3U\n#EXTENC:UTF-8\n#NAP:{VERSION}\n");
    let headed = [
        ("#PLAYLIST:", one_line(&tape.name)),
        ("#EXTIMG:", cover.unwrap_or_default().to_owned()),
        ("#NAP-FROM:", one_line(&tape.from)),
    ];
    for (directive, value) in headed.iter().filter(|(_, value)| !value.is_empty()) {
        text += &format!("{directive}{value}\n");
    }
    for line in tape.note.trim().lines().filter(|_| !tape.note.trim().is_empty()) {
        text += &format!("#NAP-NOTE:{}\n", line.trim_end());
    }
    text.push('\n');
    for (at, entry) in entries.iter().enumerate() {
        text += &briefs.get(at).map(|brief| extinf(entry, brief)).unwrap_or_default();
        // Any other line starting with `#` is a comment, so a name that does is written as a path.
        text += if entry.starts_with('#') { "./" } else { "" };
        text += entry;
        text.push('\n');
    }
    text
}

/// A second file that would share a name inside the archive gets " (2)", and so on.
fn distinct(name: &str, taken: &HashMap<String, PathBuf>) -> String {
    let (stem, ext) = name.rsplit_once('.').map_or((name, String::new()), |(s, e)| (s, format!(".{e}")));
    let candidates = std::iter::once(name.to_owned()).chain((2..).map(|n| format!("{stem} ({n}){ext}")));
    candidates.into_iter().find(|candidate| !taken.contains_key(candidate)).unwrap_or_default()
}

/// The name each track takes inside an archive. A file listed twice is stored once.
pub fn archive_names(tracks: &[Source]) -> Vec<String> {
    let mut taken: HashMap<String, PathBuf> = HashMap::new();
    let mut names = Vec::new();
    for track in tracks {
        let known = taken.iter().find(|(_, path)| **path == track.path).map(|(name, _)| name.clone());
        let name = known.unwrap_or_else(|| distinct(&file_name(&track.path), &taken));
        taken.insert(name.clone(), track.path.clone());
        names.push(name);
    }
    names
}

fn cover_name(cover: &Source) -> String {
    format!("cover.{}", extension(&cover.path))
}

/// Bytes an export will copy, for its progress bar.
pub fn export_size(tape: &Tape) -> u64 {
    let mut files: Vec<&Source> = tape.tracks.iter().chain(tape.cover.iter()).collect();
    files.sort_by_key(|source| &source.path);
    files.dedup();
    files.iter().map(|source| source.size()).sum()
}

/// Counts what passes through, so a copy can report progress.
struct Counted<'a, R> {
    inner: R,
    done: &'a AtomicU64,
}
impl<R: Read> Read for Counted<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buffer)?;
        self.done.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

fn describe(path: &Path, error: io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// Add one file. A tape is for handing to someone else, so it says nothing of who owned its
/// files: every entry is root's, 0644, and only the time it was last changed is carried over.
fn append(tar: &mut tar::Builder<File>, inside: &Path, data: impl Read, size: u64, modified: u64) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(size);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(modified);
    tar.append_data(&mut header, inside, data)
}

fn append_source(tar: &mut tar::Builder<File>, inside: &Path, source: &Source, done: &AtomicU64) -> Result<(), String> {
    let window = source.open().map_err(|e| describe(&source.path, e))?;
    let size = window.length;
    append(tar, inside, Counted { inner: window, done }, size, source.modified()).map_err(|e| describe(&source.path, e))
}

fn write_archive(tape: &Tape, briefs: &[Brief], partial: &Path, folder: &Path, done: &AtomicU64) -> Result<(), String> {
    let mut tar = tar::Builder::new(File::create(partial).map_err(|e| describe(partial, e))?);
    let names = archive_names(&tape.tracks);
    let cover = tape.cover.as_ref().map(cover_name);
    let index = render(tape, cover.as_deref(), &names, briefs);
    let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    // The index goes first, so whatever reads a tape finds its listing without reading the audio.
    append(&mut tar, &folder.join(INDEX), index.as_bytes(), index.len() as u64, now)
        .map_err(|e| describe(partial, e))?;
    if let (Some(source), Some(name)) = (&tape.cover, &cover) {
        append_source(&mut tar, &folder.join(name), source, done)?;
    }
    // A file listed twice shares a name and is stored once, at its first appearance.
    let first_listing = |(at, name): &(usize, &String)| names.iter().position(|other| other == *name) == Some(*at);
    for (at, name) in names.iter().enumerate().filter(first_listing) {
        append_source(&mut tar, &folder.join(name), &tape.tracks[at], done)?;
    }
    tar.into_inner().and_then(|file| file.sync_all()).map_err(|e| describe(partial, e))
}

/// Save a standalone index that points at the files where they are.
fn export_index(tape: &Tape, briefs: &[Brief], dest: &Path) -> Result<(), String> {
    if tape.tracks.iter().chain(tape.cover.iter()).any(|source| source.span.is_some()) {
        return Err("A J-card lists files where they are, and these are inside a tape. Save a .tape instead".into());
    }
    let absolute = |source: &Source| source.path.to_string_lossy().into_owned();
    let entries: Vec<String> = tape.tracks.iter().map(absolute).collect();
    let text = render(tape, tape.cover.as_ref().map(absolute).as_deref(), &entries, briefs);
    fs::write(dest, text).map_err(|e| describe(dest, e))
}

/// Save `tape` to `dest`: a self-contained archive, or for a `.jcard` destination just the index.
/// An archive is written beside its destination and moved into place only once complete, so a
/// tape can be saved over the very archive it is being played from.
pub fn export(tape: &Tape, briefs: &[Brief], dest: &Path, done: &AtomicU64) -> Result<(), String> {
    if kind(dest) == Kind::Index {
        return export_index(tape, briefs, dest);
    }
    let folder = PathBuf::from(dest.file_stem().filter(|stem| !stem.is_empty()).unwrap_or("mixtape".as_ref()));
    let partial = dest.with_extension("tape.partial");
    let written = write_archive(tape, briefs, &partial, &folder, done)
        .and_then(|()| fs::rename(&partial, dest).map_err(|e| describe(dest, e)));
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written
}

/// An entry's path with nothing in it but names, or none if it would climb out of the archive.
fn within(path: &Path) -> Option<PathBuf> {
    let mut inside = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => inside.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(inside)
}

/// Every plain file in `archive` and where it lies, in name order. Only headers are read.
fn held(archive: &Path) -> Result<BTreeMap<PathBuf, Span>, String> {
    let unreadable = |error: io::Error| format!("{}: not a tape nap can read ({error})", archive.display());
    let file = File::open(archive).map_err(|e| describe(archive, e))?;
    let size = file.metadata().map_err(|e| describe(archive, e))?.len();
    let mut tar = tar::Archive::new(file);
    let mut found = BTreeMap::new();
    for entry in tar.entries_with_seek().map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let (offset, length) = (entry.raw_file_position(), entry.size());
        if offset + length > size {
            return Err(format!("{}: this tape is cut short", archive.display()));
        }
        let inside = entry.path().ok().and_then(|path| within(&path));
        if let (tar::EntryType::Regular, Some(inside)) = (entry.header().entry_type(), inside) {
            found.insert(inside, Span { archive: archive.to_owned(), offset, length });
        }
    }
    Ok(found)
}

fn read_text(source: &Source) -> Result<String, String> {
    let mut text = String::new();
    let read = source.open().and_then(|window| window.take(INDEX_LIMIT).read_to_string(&mut text));
    read.map(|_| text).map_err(|e| describe(&source.path, e))
}

/// The tape in an archive, left where it is: its index if it has one, otherwise its audio in
/// name order. An index can list only what the archive holds. Also names any listed tracks that
/// are not there.
pub fn read_archive(archive: &Path) -> Result<(Tape, Vec<String>), String> {
    let held = held(archive)?;
    let source = |inside: &Path, span: &Span| Source { path: archive.join(inside), span: Some(span.clone()) };
    let find = |listed: &Source| {
        let inside = within(&listed.path)?;
        held.get_key_value(&inside).map(|(inside, span)| source(inside, span))
    };
    let (mut tape, missing) = match held.iter().find(|(inside, _)| file_name(inside) == INDEX) {
        Some((inside, span)) => {
            parse(&read_text(&source(inside, span))?, inside.parent().unwrap_or(Path::new("")))?.settle(find)
        }
        None => {
            let audio = held.iter().filter(|(inside, _)| kind(inside) == Kind::Audio);
            (Tape { tracks: audio.map(|(inside, span)| source(inside, span)).collect(), ..Tape::default() }, Vec::new())
        }
    };
    if tape.name.is_empty() {
        tape.name = archive.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    }
    if tape.tracks.is_empty() { Err("This tape has no playable tracks".into()) } else { Ok((tape, missing)) }
}

/// Read a standalone index, whose paths are absolute or relative to where it sits. Also names
/// any listed tracks that could not be found.
pub fn read_index(index: &Path) -> Result<(Tape, Vec<String>), String> {
    let text = fs::read_to_string(index).map_err(|e| describe(index, e))?;
    let (tape, missing) = parse(&text, index.parent().unwrap_or(Path::new(".")))?
        .settle(|listed| listed.is_there().then(|| listed.clone()));
    match (tape.tracks.len(), missing.len()) {
        (0, 0) => Err("This J-card lists no tracks".into()),
        (0, _) => Err("None of this J-card's tracks could be found".into()),
        _ => Ok((tape, missing)),
    }
}
