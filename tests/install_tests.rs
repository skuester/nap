use nap::install::{REQUIRE_LINE, RULES_LUA, Source, ensure_require, install, remove_require, uninstall};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

fn hypr_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let hypr = dir.path().join("hypr");
    fs::create_dir_all(&hypr).unwrap();
    fs::write(hypr.join("hyprland.lua"), "require(\"hypr.monitors\")\n").unwrap();
    (dir, hypr)
}

fn backups(hypr: &Path) -> usize {
    fs::read_dir(hypr)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("hyprland.lua.bak."))
        .count()
}

#[test]
fn the_embedded_rules_are_the_repo_file() {
    assert_eq!(RULES_LUA, fs::read_to_string("hypr/nap.lua").unwrap());
}

#[test]
fn ensure_require_appends_once() {
    let before = "require(\"hypr.monitors\")\n";
    let after = ensure_require(before).expect("should change");
    assert!(after.starts_with(before));
    assert!(after.contains(REQUIRE_LINE));
    assert_eq!(ensure_require(&after), None, "idempotent");
}

#[test]
fn ensure_require_handles_missing_trailing_newline() {
    let after = ensure_require("x = 1").unwrap();
    assert!(after.contains("x = 1\n"));
    assert!(after.ends_with('\n'));
}

#[test]
fn remove_require_strips_the_line_and_its_comment_only() {
    let before = "require(\"hypr.monitors\")\n";
    let added = ensure_require(before).unwrap();
    assert_eq!(remove_require(&added).as_deref(), Some(before));
    assert_eq!(remove_require(before), None, "nothing to remove");
}

#[test]
fn embedded_install_writes_rules_and_backs_up_the_main_config() {
    let (_d, hypr) = hypr_dir();
    let report = install(&hypr, Source::Embedded).unwrap();
    assert!(report.rules_written);
    assert!(report.require_added);
    assert_eq!(fs::read_to_string(hypr.join("nap.lua")).unwrap(), RULES_LUA);
    assert!(fs::read_to_string(hypr.join("hyprland.lua")).unwrap().contains(REQUIRE_LINE));
    assert_eq!(backups(&hypr), 1);

    let again = install(&hypr, Source::Embedded).unwrap();
    assert!(!again.rules_written);
    assert!(!again.require_added);
    assert_eq!(backups(&hypr), 1);
}

#[test]
fn linked_install_symlinks_the_repo_file() {
    let (d, hypr) = hypr_dir();
    let repo_file = d.path().join("checkout/hypr/nap.lua");
    fs::create_dir_all(repo_file.parent().unwrap()).unwrap();
    fs::write(&repo_file, RULES_LUA).unwrap();

    let report = install(&hypr, Source::Link(repo_file.clone())).unwrap();
    assert!(report.rules_written);
    let link = hypr.join("nap.lua");
    assert_eq!(fs::read_link(&link).unwrap(), repo_file);

    let again = install(&hypr, Source::Link(repo_file.clone())).unwrap();
    assert!(!again.rules_written, "already linked there");
}

#[test]
fn linked_install_replaces_an_earlier_copy_or_stale_link() {
    let (d, hypr) = hypr_dir();
    let repo_file = d.path().join("checkout/hypr/nap.lua");
    fs::create_dir_all(repo_file.parent().unwrap()).unwrap();
    fs::write(&repo_file, RULES_LUA).unwrap();

    install(&hypr, Source::Embedded).unwrap();
    let report = install(&hypr, Source::Link(repo_file.clone())).unwrap();
    assert!(report.rules_written);
    assert_eq!(fs::read_link(hypr.join("nap.lua")).unwrap(), repo_file);

    let elsewhere = d.path().join("old/nap.lua");
    fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    fs::write(&elsewhere, "x").unwrap();
    fs::remove_file(hypr.join("nap.lua")).unwrap();
    symlink(&elsewhere, hypr.join("nap.lua")).unwrap();
    let report = install(&hypr, Source::Link(repo_file.clone())).unwrap();
    assert!(report.rules_written);
    assert_eq!(fs::read_link(hypr.join("nap.lua")).unwrap(), repo_file);
}

#[test]
fn a_hand_edited_rules_file_is_kept_as_a_backup() {
    let (_d, hypr) = hypr_dir();
    fs::write(hypr.join("nap.lua"), "-- my own tweaks\n").unwrap();
    install(&hypr, Source::Embedded).unwrap();
    let kept: Vec<_> = fs::read_dir(&hypr)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("nap.lua.bak."))
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(fs::read_to_string(kept[0].path()).unwrap(), "-- my own tweaks\n");
}

#[test]
fn linking_a_missing_file_is_an_error() {
    let (d, hypr) = hypr_dir();
    assert!(install(&hypr, Source::Link(d.path().join("nope.lua"))).is_err());
}

#[test]
fn install_without_hyprland_lua_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(install(dir.path(), Source::Embedded).is_err());
}

#[test]
fn uninstall_removes_the_rules_and_the_require_line() {
    let (_d, hypr) = hypr_dir();
    install(&hypr, Source::Embedded).unwrap();
    let report = uninstall(&hypr).unwrap();
    assert!(report.rules_removed);
    assert!(report.require_removed);
    assert!(!hypr.join("nap.lua").exists());
    assert_eq!(fs::read_to_string(hypr.join("hyprland.lua")).unwrap(), "require(\"hypr.monitors\")\n");
    assert_eq!(backups(&hypr), 2, "the main config is backed up on the way out too");

    let again = uninstall(&hypr).unwrap();
    assert!(!again.rules_removed && !again.require_removed);
}

#[test]
fn rules_match_the_app_id_only() {
    assert!(RULES_LUA.contains("class = \"^nap$\""));
    assert!(RULES_LUA.contains("keep_aspect_ratio = true"));
    assert!(RULES_LUA.contains("float = true"));
    assert!(RULES_LUA.contains("no_dim = true"));
}
