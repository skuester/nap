//! One explicit bookmark per audio file, stored as elapsed milliseconds in
//! `user.nap.bookmark`. Compatible with the original Qt implementation;
//! follows the file across renames without a separate database.
//!
//! A tape has one bookmark too, kept the same way on the `.tape` or `.jcard` itself rather than
//! on anything it lists: `3:62500` is 62.5 seconds into its third track, and a bare number is on
//! the first. It is an attribute of the file, never part of its contents, so a tape handed to
//! someone else does not carry the place its maker had reached.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub const ATTR: &str = "user.nap.bookmark";

/// A place: how far into which track. A single file is its own first track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub track: usize,
    pub millisecond: u64,
}

impl From<u64> for Mark {
    fn from(millisecond: u64) -> Self {
        Mark { track: 0, millisecond }
    }
}

/// The attribute's text for a mark. Tracks are counted from one, as a listing counts them.
pub fn encode(mark: Mark) -> String {
    match mark.track {
        0 => mark.millisecond.to_string(),
        track => format!("{}:{}", track + 1, mark.millisecond),
    }
}

/// The mark an attribute's bytes name, if they name one.
pub fn decode(bytes: &[u8]) -> Option<Mark> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    let (track, millisecond) = text.split_once(':').unwrap_or(("1", text));
    let track = track.parse::<usize>().ok()?.checked_sub(1)?;
    Some(Mark { track, millisecond: millisecond.parse().ok()? })
}

/// The bookmark on `path`, `None` when it has none.
pub fn read(path: &Path) -> Result<Option<Mark>, String> {
    let (path, name) = names(path)?;
    let mut buf = [0u8; 48];
    // SAFETY: both strings are NUL-terminated and the buffer's length is passed.
    let n = unsafe { libc::getxattr(path.as_ptr(), name.as_ptr(), buf.as_mut_ptr().cast(), buf.len()) };
    if n >= 0 {
        return Ok(decode(&buf[..n as usize]));
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ENODATA) => Ok(None),
        // A value longer than the buffer cannot be a mark.
        Some(libc::ERANGE) => Ok(None),
        _ => Err(format!("cannot read the bookmark on {}: {err}", path.to_string_lossy())),
    }
}

/// Bookmark `mark` on `path`, replacing any bookmark it had.
pub fn write(path: &Path, mark: Mark) -> Result<(), String> {
    let (path, name) = names(path)?;
    let value = encode(mark);
    // SAFETY: strings are NUL-terminated; the value's length is passed.
    let rc = unsafe { libc::setxattr(path.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("cannot bookmark {}: {}", path.to_string_lossy(), io::Error::last_os_error()))
    }
}

/// Remove the bookmark from `path`; a file without one is left as it is.
pub fn clear(path: &Path) -> Result<(), String> {
    let (path, name) = names(path)?;
    // SAFETY: both strings are NUL-terminated.
    let rc = unsafe { libc::removexattr(path.as_ptr(), name.as_ptr()) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ENODATA) => Ok(()),
        _ => Err(format!("cannot remove the bookmark from {}: {err}", path.to_string_lossy())),
    }
}

fn names(path: &Path) -> Result<(CString, CString), String> {
    let p = CString::new(path.as_os_str().as_bytes()).map_err(|_| format!("{}: NUL in path", path.display()))?;
    Ok((p, CString::new(ATTR).expect("no NUL in the attribute name")))
}
