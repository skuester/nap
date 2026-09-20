use nap::{
    desktop::{self, MIME_TYPES, MimeBackend},
    install,
};
use std::{cell::RefCell, collections::BTreeMap, fs};

#[derive(Default)]
struct FakeMime(RefCell<BTreeMap<String, String>>, RefCell<Vec<std::path::PathBuf>>);
impl MimeBackend for FakeMime {
    fn current(&self, mime: &str) -> Result<String, String> {
        Ok(self.0.borrow().get(mime).cloned().unwrap_or_default())
    }
    fn set(&self, mime: &str, handler: &str) -> Result<(), String> {
        self.0.borrow_mut().insert(mime.into(), handler.into());
        Ok(())
    }
    fn refresh(&self, share: &std::path::Path) -> Result<(), String> {
        self.1.borrow_mut().push(share.into());
        Ok(())
    }
}
/// Answers like the real xdg-mime: for a type with nothing recorded it names whichever desktop
/// entry claims the type, which for nap's own types is nap itself.
#[derive(Default)]
struct GuessingMime(RefCell<BTreeMap<String, String>>);
impl MimeBackend for GuessingMime {
    fn current(&self, mime: &str) -> Result<String, String> {
        let guess = if mime.starts_with("application/x-nap-") { "nap.desktop" } else { "" };
        Ok(self.0.borrow().get(mime).cloned().unwrap_or(guess.into()))
    }
    fn set(&self, mime: &str, handler: &str) -> Result<(), String> {
        match handler {
            "" => self.0.borrow_mut().remove(mime),
            handler => self.0.borrow_mut().insert(mime.into(), handler.into()),
        };
        Ok(())
    }
    fn refresh(&self, _: &std::path::Path) -> Result<(), String> {
        Ok(())
    }
}
#[test]
fn defaults_are_written_down_even_when_a_query_already_names_nap() {
    // A file manager reads the record, not the guess: without one it opened a .tape as an archive.
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("previous.json");
    let backend = GuessingMime::default();
    desktop::install_mimes(&backend, &state).unwrap();
    for mime in MIME_TYPES {
        assert_eq!(backend.0.borrow().get(*mime).map(String::as_str), Some("nap.desktop"), "{mime}");
    }
    // Nothing handled nap's own types before nap, so they are not recorded as displaced...
    assert!(!fs::read_to_string(&state).unwrap().contains("x-nap"));
    // ...and uninstalling forgets them rather than leaving nap named for a type it no longer opens.
    desktop::uninstall_mimes(&backend, &state).unwrap();
    assert!(backend.0.borrow().keys().all(|mime| !mime.contains("x-nap")), "{:?}", backend.0.borrow());
}
#[test]
fn repeated_install_preserves_defaults_and_uninstall_respects_later_choices() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("previous.json");
    let backend = FakeMime::default();
    backend.set("audio/mpeg", "before.desktop").unwrap();
    desktop::install_mimes(&backend, &state).unwrap();
    desktop::install_mimes(&backend, &state).unwrap();
    backend.set("audio/flac", "new-choice.desktop").unwrap();
    desktop::uninstall_mimes(&backend, &state).unwrap();
    assert_eq!(backend.current("audio/mpeg").unwrap(), "before.desktop");
    assert_eq!(backend.current("audio/flac").unwrap(), "new-choice.desktop");
    assert_eq!(backend.current("audio/x-wav").unwrap(), "");
    assert!(!state.exists());
}
#[test]
fn installation_links_checkout_and_uninstalls_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let checkout = temp.path().join("checkout with spaces");
    let prefix = temp.path().join("local");
    let config = temp.path().join("config");
    let state = temp.path().join("state");
    for file in ["target/release/nap", "nap.desktop", "nap-mime.xml", "hypr/nap.lua"] {
        let path = checkout.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "test").unwrap();
    }
    fs::create_dir_all(config.join("hypr")).unwrap();
    fs::write(config.join("hypr/hyprland.lua"), "-- user config\n").unwrap();
    let backend = FakeMime::default();
    for _ in 0..2 {
        desktop::install_desktop(&checkout, &prefix, &config, &state, &backend).unwrap();
    }
    assert_eq!(fs::read_link(prefix.join("bin/nap")).unwrap(), checkout.join("target/release/nap"));
    assert_eq!(fs::read_link(prefix.join("share/applications/nap.desktop")).unwrap(), checkout.join("nap.desktop"));
    assert_eq!(fs::read_link(config.join("hypr/nap.lua")).unwrap(), checkout.join("hypr/nap.lua"));
    // nap's own file types are defined, the database rebuilt, and nap made their default.
    assert_eq!(fs::read_link(prefix.join(desktop::MIME_PACKAGE)).unwrap(), checkout.join("nap-mime.xml"));
    assert_eq!(*backend.1.borrow(), vec![prefix.join("share"); 2]);
    assert_eq!(backend.current("application/x-nap-tape").unwrap(), "nap.desktop");
    assert_eq!(backend.current("application/x-nap-jcard").unwrap(), "nap.desktop");
    let text = fs::read_to_string(config.join("hypr/hyprland.lua")).unwrap();
    assert_eq!(text.matches(install::REQUIRE_LINE).count(), 1);
    desktop::uninstall_desktop(&prefix, &config, &state, &backend).unwrap();
    assert!(!prefix.join("bin/nap").exists());
    assert!(!prefix.join("share/applications/nap.desktop").exists());
    assert!(fs::symlink_metadata(prefix.join(desktop::MIME_PACKAGE)).is_err());
    assert_eq!(backend.1.borrow().len(), 3, "removing the definitions rebuilds the database once more");
    assert_eq!(backend.current("application/x-nap-tape").unwrap(), "");
    // A prefix that never had them is left alone.
    desktop::uninstall_desktop(&prefix, &config, &state, &backend).unwrap();
    assert_eq!(backend.1.borrow().len(), 3);
    assert!(!config.join("hypr/nap.lua").exists());
    assert_eq!(fs::read_to_string(config.join("hypr/hyprland.lua")).unwrap(), "-- user config\n");
}
#[test]
fn registry_matches_desktop_and_rules_preserve_aspect() {
    let desktop = include_str!("../nap.desktop");
    let line = desktop.lines().find_map(|l| l.strip_prefix("MimeType=")).unwrap();
    assert_eq!(line.split(';').filter(|s| !s.is_empty()).collect::<Vec<_>>(), MIME_TYPES);
    // Every type nap claims beyond the audio ones is defined in its own package, with a glob.
    let package = include_str!("../nap-mime.xml");
    for mime in MIME_TYPES.iter().filter(|mime| !mime.starts_with("audio/")) {
        assert!(package.contains(&format!("<mime-type type=\"{mime}\">")), "{mime}");
    }
    assert!(package.contains("<glob pattern=\"*.tape\"/>") && package.contains("<glob pattern=\"*.jcard\"/>"));
    assert!(desktop.contains("Exec=nap %F"), "several selected files open as one tape");
    for rule in ["keep_aspect_ratio = true", "float = true", "center = true", "^nap$"] {
        assert!(install::RULES_LUA.contains(rule));
    }
}
#[test]
fn unset_removes_only_our_default_entry() {
    let text = "[Default Applications]\naudio/mpeg=nap.desktop;\naudio/flac=other.desktop;\n[Added Associations]\naudio/mpeg=nap.desktop;\n";
    assert_eq!(
        desktop::remove_default(text, "audio/mpeg"),
        "[Default Applications]\naudio/flac=other.desktop;\n[Added Associations]\naudio/mpeg=nap.desktop;\n"
    );
}

/// Tests that write a script and then run it take turns. Otherwise one thread's fork can briefly
/// inherit another's still-open script, and running a file open for writing fails (ETXTBSY).
static STAND_INS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// An executable stand-in for a system tool, so nothing here touches the real desktop.
fn script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn xdg_mime_is_driven_through_its_command_line() {
    let _turn = STAND_INS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp = tempfile::tempdir().unwrap();
    let log = temp.path().join("calls");
    let logging = |name: &str, extra: &str| {
        script(temp.path(), name, &format!("echo \"$@\" >> '{}'\n{extra}\nexit 0", log.display()))
    };
    let tools = |program: &std::path::Path, database: &std::path::Path| desktop::XdgMime {
        config: temp.path().into(),
        program: program.into(),
        database_program: database.into(),
        desktop_database_program: database.into(),
    };
    let query = logging("xdg-mime", "[ \"$1\" = query ] && echo ' other.desktop '");
    let mime = tools(&query, &logging("update-database", ""));
    assert_eq!(mime.current("audio/flac").unwrap(), "other.desktop");
    mime.set("audio/flac", "nap.desktop").unwrap();
    // Both caches are rebuilt: what a .tape is, and that nap opens one.
    mime.refresh(&temp.path().join("share")).unwrap();
    let share = temp.path().join("share");
    let calls = format!(
        "query default audio/flac\ndefault nap.desktop audio/flac\n{}\n{}\n",
        share.join("mime").display(),
        share.join("applications").display()
    );
    assert_eq!(fs::read_to_string(&log).unwrap(), calls);

    // Clearing a default edits mimeapps.list directly, and is a no-op without one.
    mime.set("audio/flac", "").unwrap();
    let list = temp.path().join("mimeapps.list");
    fs::write(&list, "[Default Applications]\naudio/flac=nap.desktop\naudio/mpeg=other.desktop\n").unwrap();
    mime.set("audio/flac", "").unwrap();
    assert_eq!(fs::read_to_string(&list).unwrap(), "[Default Applications]\naudio/mpeg=other.desktop\n");

    let broken = script(temp.path(), "broken", "echo nope >&2; exit 3");
    let failing = tools(&broken, &broken);
    assert_eq!(failing.current("audio/flac").unwrap_err().trim(), "nope");
    assert_eq!(failing.set("audio/flac", "nap.desktop").unwrap_err(), "xdg-mime failed for audio/flac");
    assert_eq!(failing.refresh(temp.path()).unwrap_err(), "broken failed");
    let missing = tools(&temp.path().join("absent"), &temp.path().join("absent"));
    assert!(missing.current("audio/flac").is_err() && missing.set("audio/flac", "nap.desktop").is_err());
    assert!(missing.refresh(temp.path()).unwrap_err().starts_with("absent: "));
    let real = desktop::XdgMime::new(temp.path().into());
    let programs = [&real.program, &real.database_program, &real.desktop_database_program].map(|p| p.to_str().unwrap());
    assert_eq!(programs, ["xdg-mime", "update-mime-database", "update-desktop-database"]);
}

#[test]
fn hyprland_reload_reports_configuration_errors() {
    let _turn = STAND_INS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let temp = tempfile::tempdir().unwrap();
    assert!(desktop::reload_hyprland_with(&script(temp.path(), "clean", "exit 0")).is_ok());
    let refuses = script(temp.path(), "refuses", "exit 1");
    assert_eq!(desktop::reload_hyprland_with(&refuses).unwrap_err(), "hyprctl reload failed");
    let complains = script(temp.path(), "complains", "[ \"$1\" = configerrors ] && echo 'line 3: bad rule'\nexit 0");
    assert!(desktop::reload_hyprland_with(&complains).unwrap_err().contains("line 3: bad rule"));
    assert!(desktop::reload_hyprland_with(&temp.path().join("absent")).is_err());
}
