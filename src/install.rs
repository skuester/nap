//! One-shot Hyprland setup: our rules file next to the user's config (a
//! symlink into the checkout when installed from one, a copy otherwise) plus
//! one `require` line in `hyprland.lua`, which is the only edit to a file we
//! do not own. Idempotent, reversible, and everything touched is backed up.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const RULES_FILE: &str = "nap.lua";
pub const REQUIRE_LINE: &str = "require(\"hypr.nap\")";
const REQUIRE_COMMENT: &str = "-- Nice Audio Player (nap): floating, centred, aspect-locked, undimmed.";

/// `hypr/nap.lua` from the repo, embedded so a bare binary can still install.
pub const RULES_LUA: &str = include_str!("../hypr/nap.lua");

/// Where the rules file should come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Write the embedded copy.
    Embedded,
    /// Symlink to this file (the checkout's `hypr/nap.lua`).
    Link(PathBuf),
}

/// The `hyprland.lua` text with our require appended, or `None` if it is
/// already there.
pub fn ensure_require(hyprland_lua: &str) -> Option<String> {
    if hyprland_lua.lines().any(|l| l.trim() == REQUIRE_LINE) {
        return None;
    }
    let mut out = hyprland_lua.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(REQUIRE_COMMENT);
    out.push('\n');
    out.push_str(REQUIRE_LINE);
    out.push('\n');
    Some(out)
}

/// The `hyprland.lua` text without our require (and the comment above it,
/// and the blank line we added before that), or `None` if it was not there.
pub fn remove_require(hyprland_lua: &str) -> Option<String> {
    if !hyprland_lua.lines().any(|l| l.trim() == REQUIRE_LINE) {
        return None;
    }
    let mut lines: Vec<&str> = hyprland_lua.lines().collect();
    let at = lines.iter().position(|l| l.trim() == REQUIRE_LINE)?;
    lines.remove(at);
    if at > 0 && lines[at - 1].trim() == REQUIRE_COMMENT {
        lines.remove(at - 1);
        if at > 1 && lines[at - 2].trim().is_empty() {
            lines.remove(at - 2);
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    Some(out)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub rules_written: bool,
    pub require_added: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Removal {
    pub rules_removed: bool,
    pub require_removed: bool,
}

/// Install into a Hyprland config directory (normally `~/.config/hypr`).
pub fn install(hypr_dir: &Path, source: Source) -> Result<Report, String> {
    let main = hypr_dir.join("hyprland.lua");
    let current = fs::read_to_string(&main).map_err(|e| format!("{}: {e}", main.display()))?;
    let mut report = Report::default();

    let rules = hypr_dir.join(RULES_FILE);
    report.rules_written = match &source {
        Source::Embedded => place_copy(&rules)?,
        Source::Link(target) => place_link(&rules, target)?,
    };

    if let Some(updated) = ensure_require(&current) {
        backup(&main)?;
        fs::write(&main, updated).map_err(|e| format!("{}: {e}", main.display()))?;
        report.require_added = true;
    }
    Ok(report)
}

/// Undo [`install`]: remove the rules file (or link) and the require line.
pub fn uninstall(hypr_dir: &Path) -> Result<Removal, String> {
    let mut removal = Removal::default();
    let rules = hypr_dir.join(RULES_FILE);
    if fs::symlink_metadata(&rules).is_ok() {
        retire(&rules)?;
        removal.rules_removed = true;
    }
    let main = hypr_dir.join("hyprland.lua");
    if let Ok(current) = fs::read_to_string(&main)
        && let Some(updated) = remove_require(&current)
    {
        backup(&main)?;
        fs::write(&main, updated).map_err(|e| format!("{}: {e}", main.display()))?;
        removal.require_removed = true;
    }
    Ok(removal)
}

/// Write the embedded rules unless an identical regular file is there.
fn place_copy(rules: &Path) -> Result<bool, String> {
    let meta = fs::symlink_metadata(rules).ok();
    if meta.as_ref().is_some_and(|m| m.is_file()) && fs::read_to_string(rules).ok().as_deref() == Some(RULES_LUA) {
        return Ok(false);
    }
    if meta.is_some() {
        retire(rules)?;
    }
    fs::write(rules, RULES_LUA).map_err(|e| format!("{}: {e}", rules.display()))?;
    Ok(true)
}

/// Symlink the rules to `target` unless that link already exists.
fn place_link(rules: &Path, target: &Path) -> Result<bool, String> {
    if !target.is_file() {
        return Err(format!("{}: not a file", target.display()));
    }
    let target = fs::canonicalize(target).map_err(|e| format!("{}: {e}", target.display()))?;
    if fs::read_link(rules).ok().as_deref() == Some(&target) {
        return Ok(false);
    }
    if fs::symlink_metadata(rules).is_ok() {
        retire(rules)?;
    }
    std::os::unix::fs::symlink(&target, rules).map_err(|e| format!("{}: {e}", rules.display()))?;
    Ok(true)
}

/// Remove our rules file: a link or a pristine copy just goes; anything
/// else was edited by hand and is kept as a backup.
fn retire(rules: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(rules).map_err(|e| format!("{}: {e}", rules.display()))?;
    if meta.is_file() && fs::read_to_string(rules).ok().as_deref() != Some(RULES_LUA) {
        let to = stamped(rules);
        fs::rename(rules, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    } else {
        fs::remove_file(rules).map_err(|e| format!("{}: {e}", rules.display()))?;
    }
    Ok(())
}

fn backup(file: &Path) -> Result<PathBuf, String> {
    let to = stamped(file);
    fs::copy(file, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(to)
}

fn stamped(file: &Path) -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut candidate = file.with_file_name(format!("{name}.bak.{stamp}"));
    let mut n = 1;
    while candidate.exists() {
        candidate = file.with_file_name(format!("{name}.bak.{stamp}.{n}"));
        n += 1;
    }
    candidate
}
