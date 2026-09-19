//! Mixtapes. A tape is a name, an optional cover image, and an ordered list of audio files.
//!
//! On disk a tape is a directory: `_index.jcard` (the track listing, as on a cassette's J-card),
//! an optional `_cover.<ext>`, and the audio files beside them. A `.tape` file is that directory
//! as a plain tar archive. A `.jcard` can also stand alone, listing absolute paths to files that
//! stay where they are.
//!
//! The index is M3U-compatible text so it is easy to read, edit, and open elsewhere:
//!
//! ```text
//! #EXTM3U
//! #PLAYLIST:Summer '98
//! #EXTIMG:_cover.jpg
//! #FROM:Shane
//! #NOTE:Made this for the drive up.
//! #NOTE:Side B is the good one.
//!
//! 01 Roygbiv.flac
//! 02 Don't Stop.mp3
//! ```
//!
//! `#FROM:` and `#NOTE:` are nap's own: who made the tape, and what they wrote to go with it, one
//! `#NOTE:` per line. M3U has no such fields, and other players skip `#` lines they do not know.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const INDEX: &str = "_index.jcard";
const AUDIO: [&str; 15] =
    ["mp3", "flac", "wav", "ogg", "oga", "opus", "m4a", "m4b", "aac", "aiff", "aif", "wma", "ape", "wv", "mpc"];
const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tape {
    pub name: String,
    /// Who made it, as they signed it.
    pub from: String,
    /// What they wrote to go with it; may run to several lines.
    pub note: String,
    pub cover: Option<PathBuf>,
    pub tracks: Vec<PathBuf>,
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

/// Read an index. Paths in it are relative to `base`, the directory the index sits in.
pub fn parse(text: &str, base: &Path) -> Tape {
    let mut tape = Tape::default();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(name) = line.strip_prefix("#PLAYLIST:") {
            tape.name = name.trim().to_owned();
        } else if let Some(cover) = line.strip_prefix("#EXTIMG:") {
            tape.cover = Some(base.join(cover.trim()));
        } else if let Some(from) = line.strip_prefix("#FROM:") {
            tape.from = from.trim().to_owned();
        } else if let Some(note) = line.strip_prefix("#NOTE:") {
            tape.note = format!("{}\n{}", tape.note, note.trim()).trim_start_matches('\n').to_owned();
        } else if !line.starts_with('#') {
            tape.tracks.push(base.join(line));
        }
    }
    tape
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ").trim().to_owned()
}

/// Write `tape`'s index listing `entries`, one per track, with `cover` as it should appear in the file.
pub fn render(tape: &Tape, cover: Option<&str>, entries: &[String]) -> String {
    let mut text = String::from("#EXTM3U\n");
    let headed = [
        ("#PLAYLIST:", one_line(&tape.name)),
        ("#EXTIMG:", cover.unwrap_or_default().to_owned()),
        ("#FROM:", one_line(&tape.from)),
    ];
    for (directive, value) in headed.iter().filter(|(_, value)| !value.is_empty()) {
        text += &format!("{directive}{value}\n");
    }
    for line in tape.note.trim().lines().filter(|_| !tape.note.trim().is_empty()) {
        text += &format!("#NOTE:{}\n", line.trim_end());
    }
    text.push('\n');
    for entry in entries {
        text += entry;
        text.push('\n');
    }
    text
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().into_owned()
}

/// A second file that would share a name inside the archive gets " (2)", and so on.
fn distinct(name: &str, taken: &HashMap<String, PathBuf>) -> String {
    let (stem, ext) = name.rsplit_once('.').map_or((name, String::new()), |(s, e)| (s, format!(".{e}")));
    let candidates = std::iter::once(name.to_owned()).chain((2..).map(|n| format!("{stem} ({n}){ext}")));
    candidates.into_iter().find(|candidate| !taken.contains_key(candidate)).unwrap_or_default()
}

/// The name each track takes inside an archive. A file listed twice is stored once.
pub fn archive_names(tracks: &[PathBuf]) -> Vec<String> {
    let mut taken: HashMap<String, PathBuf> = HashMap::new();
    let mut names = Vec::new();
    for track in tracks {
        let known = taken.iter().find(|(_, path)| *path == track).map(|(name, _)| name.clone());
        let name = known.unwrap_or_else(|| distinct(&file_name(track), &taken));
        taken.insert(name.clone(), track.clone());
        names.push(name);
    }
    names
}

fn cover_name(cover: &Path) -> String {
    format!("_cover.{}", extension(cover))
}

/// Bytes an export will copy, for its progress bar.
pub fn export_size(tape: &Tape) -> u64 {
    let mut files: Vec<&PathBuf> = tape.tracks.iter().chain(tape.cover.iter()).collect();
    files.sort();
    files.dedup();
    files.iter().filter_map(|file| fs::metadata(file).ok()).map(|meta| meta.len()).sum()
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

fn append_file(tar: &mut tar::Builder<File>, inside: &Path, source: &Path, done: &AtomicU64) -> Result<(), String> {
    let file = File::open(source).map_err(|e| describe(source, e))?;
    let mut header = tar::Header::new_gnu();
    header.set_metadata(&file.metadata().map_err(|e| describe(source, e))?);
    header.set_mode(0o644);
    tar.append_data(&mut header, inside, Counted { inner: file, done }).map_err(|e| describe(source, e))
}

fn append_text(tar: &mut tar::Builder<File>, inside: &Path, text: &str) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(text.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    tar.append_data(&mut header, inside, text.as_bytes())
}

fn write_archive(tape: &Tape, partial: &Path, folder: &Path, done: &AtomicU64) -> Result<(), String> {
    let mut tar = tar::Builder::new(File::create(partial).map_err(|e| describe(partial, e))?);
    let names = archive_names(&tape.tracks);
    let cover = tape.cover.as_deref().map(cover_name);
    let index = render(tape, cover.as_deref(), &names);
    append_text(&mut tar, &folder.join(INDEX), &index).map_err(|e| describe(partial, e))?;
    if let (Some(source), Some(name)) = (&tape.cover, &cover) {
        append_file(&mut tar, &folder.join(name), source, done)?;
    }
    // A file listed twice shares a name and is stored once, at its first appearance.
    let first_listing = |(at, name): &(usize, &String)| names.iter().position(|other| other == *name) == Some(*at);
    for (at, name) in names.iter().enumerate().filter(first_listing) {
        append_file(&mut tar, &folder.join(name), &tape.tracks[at], done)?;
    }
    tar.into_inner().and_then(|file| file.sync_all()).map_err(|e| describe(partial, e))
}

/// Save a standalone index that points at the files where they are.
fn export_index(tape: &Tape, dest: &Path) -> Result<(), String> {
    let absolute = |path: &PathBuf| path.to_string_lossy().into_owned();
    let entries: Vec<String> = tape.tracks.iter().map(absolute).collect();
    let text = render(tape, tape.cover.as_ref().map(absolute).as_deref(), &entries);
    fs::write(dest, text).map_err(|e| describe(dest, e))
}

/// Save `tape` to `dest`: a self-contained archive, or for a `.jcard` destination just the index.
/// An archive is written beside its destination and moved into place only once complete.
pub fn export(tape: &Tape, dest: &Path, done: &AtomicU64) -> Result<(), String> {
    if kind(dest) == Kind::Index {
        return export_index(tape, dest);
    }
    let folder = PathBuf::from(dest.file_stem().filter(|stem| !stem.is_empty()).unwrap_or("mixtape".as_ref()));
    let partial = dest.with_extension("tape.partial");
    let written = write_archive(tape, &partial, &folder, done)
        .and_then(|()| fs::rename(&partial, dest).map_err(|e| describe(dest, e)));
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written
}

/// Bytes an archive holds, for its progress bar.
pub fn archive_size(archive: &Path) -> u64 {
    fs::metadata(archive).map_or(0, |meta| meta.len())
}

/// The private directory extractions live in. `/tmp` is cleared for us on reboot.
pub fn scratch_root() -> PathBuf {
    // SAFETY: getuid has no preconditions and cannot fail.
    std::env::temp_dir().join(format!("nap-{}", unsafe { libc::getuid() }))
}

/// A fresh, private directory for one extraction.
pub fn scratch(root: &Path, archive: &Path) -> Result<PathBuf, String> {
    let stem = archive.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let made = |dir: &Path| fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|e| describe(dir, e));
    made(root)?;
    let free = (0..).map(|n| root.join(format!("{}-{stem}-{n}", std::process::id()))).find(|dir| !dir.exists());
    let dir = free.unwrap_or_default();
    made(&dir).map(|()| dir)
}

/// Every regular file beneath `dir`, at most two levels down, in name order.
fn files_in(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
    entries.sort();
    let (folders, mut files): (Vec<PathBuf>, Vec<PathBuf>) = entries.into_iter().partition(|path| path.is_dir());
    if depth > 0 {
        files.extend(folders.iter().flat_map(|folder| files_in(folder, depth - 1)));
    }
    files
}

/// The tape in an extracted directory: its index if it has one, otherwise its audio in name order.
pub fn read_folder(dir: &Path, fallback_name: &str) -> Result<Tape, String> {
    let files = files_in(dir, 2);
    let index = files.iter().find(|file| file_name(file) == INDEX);
    let mut tape = match index {
        Some(index) => {
            parse(&fs::read_to_string(index).map_err(|e| describe(index, e))?, index.parent().unwrap_or(dir))
        }
        None => Tape { tracks: files.iter().filter(|f| kind(f) == Kind::Audio).cloned().collect(), ..Tape::default() },
    };
    if tape.name.is_empty() {
        tape.name = fallback_name.to_owned();
    }
    tape.tracks.retain(|track| track.is_file());
    tape.cover = tape.cover.filter(|cover| cover.is_file());
    if tape.tracks.is_empty() { Err("This tape has no playable tracks".into()) } else { Ok(tape) }
}

/// Unpack `archive` into `into` and read the tape inside. Only plain files and directories are
/// unpacked, and never outside `into`.
pub fn extract(archive: &Path, into: &Path, done: &AtomicU64) -> Result<Tape, String> {
    fs::create_dir_all(into).map_err(|e| describe(into, e))?;
    let mut tar = tar::Archive::new(File::open(archive).map_err(|e| describe(archive, e))?);
    for entry in tar.entries().map_err(|e| describe(archive, e))? {
        let mut entry = entry.map_err(|e| describe(archive, e))?;
        let plain = matches!(entry.header().entry_type(), tar::EntryType::Regular | tar::EntryType::Directory);
        if plain {
            entry.unpack_in(into).map_err(|e| describe(archive, e))?;
        }
        done.fetch_add(entry.size() + 512, Ordering::Relaxed);
    }
    read_folder(into, &archive.file_stem().unwrap_or_default().to_string_lossy())
}

/// Read a standalone index, whose paths are absolute or relative to where it sits.
pub fn read_index(index: &Path) -> Result<Tape, String> {
    let text = fs::read_to_string(index).map_err(|e| describe(index, e))?;
    let mut tape = parse(&text, index.parent().unwrap_or(Path::new(".")));
    let listed = tape.tracks.len();
    tape.tracks.retain(|track| track.is_file());
    tape.cover = tape.cover.filter(|cover| cover.is_file());
    match (listed, tape.tracks.len()) {
        (0, _) => Err("This J-card lists no tracks".into()),
        (_, 0) => Err("None of this J-card's tracks could be found".into()),
        _ => Ok(tape),
    }
}
