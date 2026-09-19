//! The real executable, run against temporary homes with stand-in system tools on its PATH.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::{fs, os::unix::fs::PermissionsExt};

struct Sandbox {
    temp: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let sandbox = Sandbox { temp: tempfile::tempdir().unwrap() };
        for dir in ["bin", "config/hypr", "state", "prefix"] {
            fs::create_dir_all(sandbox.path(dir)).unwrap();
        }
        let tool = sandbox.path("bin/xdg-mime");
        fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(sandbox.path("config/hypr/hyprland.lua"), "-- user's existing settings\n").unwrap();
        sandbox
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.temp.path().join(relative)
    }

    /// Run nap with no route to the user's configuration, desktop session, or display.
    fn nap(&self, args: &[&str]) -> Output {
        let path = format!("{}:{}", self.path("bin").display(), std::env::var("PATH").unwrap_or_default());
        Command::new(env!("CARGO_BIN_EXE_nap"))
            .args(args)
            .env("PATH", path)
            .env("HOME", self.temp.path())
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("XDG_STATE_HOME", self.path("state"))
            .env("NAP_PREFIX", self.path("prefix"))
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .envs([("QT_QPA_PLATFORM", "offscreen"), ("QT_QPA_PLATFORMTHEME", "generic")])
            .envs([("QT_QUICK_BACKEND", "software"), ("QT_QUICK_CONTROLS_STYLE", "Basic")])
            .output()
            .unwrap()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn information_flags_print_and_exit_cleanly() {
    let sandbox = Sandbox::new();
    let help = sandbox.nap(&["--help"]);
    assert!(help.status.success() && stdout(&help).contains("--screenshot PATH"));
    assert_eq!(stdout(&sandbox.nap(&["--version"])), format!("nap {}\n", env!("CARGO_PKG_VERSION")));
    let mimes = stdout(&sandbox.nap(&["--mime-types"]));
    assert!(mimes.lines().count() > 5 && mimes.lines().all(|line| line.starts_with("audio/")), "{mimes}");
}

#[test]
fn mistakes_exit_with_status_two_and_say_why() {
    let sandbox = Sandbox::new();
    for (args, reason) in [
        (vec!["--volume", "101"], "volume must be between 0 and 100"),
        (vec!["--bogus"], "unknown option --bogus"),
        (vec!["a.flac", "mix.tape"], "open one tape at a time"),
        (vec!["/does/not/exist.wav"], "not a readable file"),
        (vec!["--install-desktop", "/does/not/exist"], "nap: "),
    ] {
        let output = sandbox.nap(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(reason), "{args:?}: {output:?}");
    }
}

#[test]
fn hyprland_rules_install_and_uninstall_in_a_temporary_config() {
    let sandbox = Sandbox::new();
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("hypr/nap.lua");
    assert_eq!(
        stdout(&sandbox.nap(&["--install-hyprland", "--link", rules.to_str().unwrap()])),
        "nap Hyprland rules installed\n"
    );
    assert_eq!(fs::canonicalize(sandbox.path("config/hypr/nap.lua")).unwrap(), rules);
    assert_eq!(stdout(&sandbox.nap(&["--uninstall-hyprland"])), "nap Hyprland rules removed\n");
    assert!(!sandbox.path("config/hypr/nap.lua").exists());
    assert!(sandbox.nap(&["--install-hyprland"]).status.success());
    assert!(sandbox.path("config/hypr/nap.lua").is_file());
}

#[test]
fn desktop_integration_installs_and_uninstalls_under_a_temporary_prefix() {
    let sandbox = Sandbox::new();
    let checkout = sandbox.path("checkout");
    for file in ["target/release/nap", "nap.desktop", "hypr/nap.lua"] {
        fs::create_dir_all(checkout.join(file).parent().unwrap()).unwrap();
        fs::write(checkout.join(file), "stand-in").unwrap();
    }
    let installed = sandbox.nap(&["--install-desktop", checkout.to_str().unwrap()]);
    assert!(stdout(&installed).starts_with("nap installed"), "{installed:?}");
    assert!(sandbox.path("prefix/bin/nap").exists());
    let removed = sandbox.nap(&["--uninstall-desktop"]);
    assert!(stdout(&removed).starts_with("nap removed"), "{removed:?}");
    assert!(!sandbox.path("prefix/bin/nap").exists());
}

#[test]
fn the_deck_renders_offscreen_with_a_file_loaded() {
    let sandbox = Sandbox::new();
    let image = sandbox.path("deck.png");
    let tape = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/silence.flac");
    let output =
        sandbox.nap(&["--paused", "--volume", "0", "--screenshot", image.to_str().unwrap(), tape.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert!(fs::read(&image).unwrap().starts_with(b"\x89PNG"));
}

#[test]
fn several_files_and_a_saved_tape_both_load() {
    let sandbox = Sandbox::new();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let (a, b) = (fixtures.join("silence.flac"), fixtures.join("silence.ogg"));
    let image = sandbox.path("two.png");
    let common = ["--paused", "--volume", "0", "--screenshot", image.to_str().unwrap()];
    let output = sandbox.nap(&[&common[..], &[a.to_str().unwrap(), b.to_str().unwrap()]].concat());
    assert!(output.status.success() && image.is_file(), "{output:?}");

    // A tape made by hand, the way the format invites: a folder, an index, and tar.
    let folder = sandbox.path("Handmade");
    fs::create_dir_all(&folder).unwrap();
    fs::copy(&a, folder.join("one.flac")).unwrap();
    fs::write(folder.join("_index.jcard"), "#EXTM3U\n#PLAYLIST:Handmade\n\none.flac\n").unwrap();
    let tape = sandbox.path("Handmade.tape");
    let tarred =
        Command::new("tar").arg("-cf").arg(&tape).arg("-C").arg(sandbox.path("")).arg("Handmade").status().unwrap();
    assert!(tarred.success());
    fs::remove_file(&image).unwrap();
    let output = sandbox.nap(&[&common[..], &[tape.to_str().unwrap()]].concat());
    assert!(output.status.success() && image.is_file(), "{output:?}");
}
