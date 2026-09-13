use nap::{
    desktop::{self, MIME_TYPES, MimeBackend},
    install,
};
use std::{cell::RefCell, collections::BTreeMap, fs};

#[derive(Default)]
struct FakeMime(RefCell<BTreeMap<String, String>>);
impl MimeBackend for FakeMime {
    fn current(&self, mime: &str) -> Result<String, String> {
        Ok(self.0.borrow().get(mime).cloned().unwrap_or_default())
    }
    fn set(&self, mime: &str, handler: &str) -> Result<(), String> {
        self.0.borrow_mut().insert(mime.into(), handler.into());
        Ok(())
    }
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
    for file in ["target/release/nap", "nap.desktop", "hypr/nap.lua"] {
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
    let text = fs::read_to_string(config.join("hypr/hyprland.lua")).unwrap();
    assert_eq!(text.matches(install::REQUIRE_LINE).count(), 1);
    desktop::uninstall_desktop(&prefix, &config, &state, &backend).unwrap();
    assert!(!prefix.join("bin/nap").exists());
    assert!(!prefix.join("share/applications/nap.desktop").exists());
    assert!(!config.join("hypr/nap.lua").exists());
    assert_eq!(fs::read_to_string(config.join("hypr/hyprland.lua")).unwrap(), "-- user config\n");
}
#[test]
fn registry_matches_desktop_and_rules_preserve_aspect() {
    let desktop = include_str!("../nap.desktop");
    let line = desktop.lines().find_map(|l| l.strip_prefix("MimeType=")).unwrap();
    assert_eq!(line.split(';').filter(|s| !s.is_empty()).collect::<Vec<_>>(), MIME_TYPES);
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
