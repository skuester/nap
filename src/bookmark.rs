//! One explicit bookmark per audio file, stored as elapsed milliseconds in
//! `user.nap.bookmark`. Compatible with the original Qt implementation;
//! follows the file across renames without a separate database.

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub const ATTR: &str = "user.nap.bookmark";

/// The attribute's text for a millisecond index.
pub fn encode(millisecond: u64) -> String {
    millisecond.to_string()
}

/// The millisecond index an attribute's bytes name, if they name one.
pub fn decode(bytes: &[u8]) -> Option<u64> {
    std::str::from_utf8(bytes).ok()?.trim().parse().ok()
}

/// The bookmarked millisecond on `path`, `None` when it has none.
pub fn read(path: &Path) -> Result<Option<u64>, String> {
    let (path, name) = names(path)?;
    let mut buf = [0u8; 32];
    // SAFETY: both strings are NUL-terminated and the buffer's length is passed.
    let n = unsafe { libc::getxattr(path.as_ptr(), name.as_ptr(), buf.as_mut_ptr().cast(), buf.len()) };
    if n >= 0 {
        return Ok(decode(&buf[..n as usize]));
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ENODATA) => Ok(None),
        // A value longer than the buffer cannot be a millisecond number.
        Some(libc::ERANGE) => Ok(None),
        _ => Err(format!("cannot read the bookmark on {}: {err}", path.to_string_lossy())),
    }
}

/// Bookmark millisecond `millisecond` on `path`, replacing any bookmark it had.
pub fn write(path: &Path, millisecond: u64) -> Result<(), String> {
    let (path, name) = names(path)?;
    let value = encode(millisecond);
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
