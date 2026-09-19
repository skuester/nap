use std::{env, path::PathBuf, process::Command};

fn output(cmd: &mut Command) -> String {
    let result = cmd.output().expect("native build requires c++, ar, pkg-config and Qt development tools");
    assert!(result.status.success(), "{cmd:?}: {}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout).unwrap()
}

fn main() {
    for path in [
        "native",
        "src/Main.qml",
        "src/Insert.qml",
        "src/Reel.qml",
        "src/Scope.qml",
        "src/Transport.qml",
        "resources.qrc",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=CXX");
    println!("cargo:rerun-if-env-changed=NAP_COVERAGE");
    // `make crap` sets this to measure which native lines the tests reach, with gcov.
    let coverage = env::var_os("NAP_COVERAGE").is_some();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    output(Command::new("pkg-config").args(["--atleast-version=6.8", "Qt6Multimedia"]));
    let packages = ["Qt6Quick", "Qt6QuickControls2", "Qt6Multimedia"];
    let flags = output(Command::new("pkg-config").arg("--cflags").args(packages));
    let libexec = output(Command::new("pkg-config").args(["--variable=libexecdir", "Qt6Core"]));
    output(
        Command::new(PathBuf::from(libexec.trim()).join("moc"))
            .args(["native/player.h", "-o"])
            .arg(out.join("moc_player.cpp")),
    );
    output(
        Command::new(PathBuf::from(libexec.trim()).join("rcc"))
            .args(["-name", "resources", "resources.qrc", "-o"])
            .arg(out.join("qrc_resources.cpp")),
    );
    let sources = [
        PathBuf::from("native/player.cpp"),
        PathBuf::from("native/bridge.cpp"),
        out.join("moc_player.cpp"),
        out.join("qrc_resources.cpp"),
    ];
    let mut objects = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        let object = out.join(format!("native{i}.o"));
        output(
            Command::new(env::var("CXX").unwrap_or_else(|_| "c++".into()))
                .args(["-std=c++17", "-fPIC", "-Wall", "-Wextra", "-Inative"])
                .args(if coverage && source.starts_with("native") { &["-O0", "--coverage"][..] } else { &["-O2"] })
                .args(flags.split_whitespace())
                .arg("-c")
                .arg(source)
                .arg("-o")
                .arg(&object),
        );
        objects.push(object);
    }
    output(Command::new("ar").arg("crs").arg(out.join("libnapqt.a")).args(objects));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=napqt");
    let libs = output(Command::new("pkg-config").arg("--libs").args(packages));
    for flag in libs.split_whitespace() {
        if let Some(lib) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        }
        if let Some(dir) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={dir}");
        }
    }
    println!("cargo:rustc-link-lib=stdc++");
    if coverage {
        println!("cargo:rustc-link-lib=gcov");
    }
}
