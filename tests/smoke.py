"""CLI validation and an offscreen render, without user files or preferences."""
import os
import pathlib
import subprocess
import tempfile

env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="generic",
           QT_QUICK_BACKEND="software", QT_QUICK_CONTROLS_STYLE="Basic")
binary = pathlib.Path("target/release/nap").resolve()
for args, expected in [(["--help"], 0), (["--version"], 0), (["--time", "nan"], 2),
                       (["--volume", "101"], 2), (["--time", "1:60"], 2), (["--track", "0"], 2),
                       (["/does/not/exist.wav"], 2), (["one", "two"], 2)]:
    result = subprocess.run([binary, *args], env=env, capture_output=True, timeout=10)
    assert result.returncode == expected, (args, result.stderr.decode())
with tempfile.TemporaryDirectory() as tmp:
    image = pathlib.Path(tmp) / "preview.png"
    result = subprocess.run([binary, "--screenshot", image], env=env, capture_output=True, timeout=15)
    assert result.returncode == 0, result.stderr.decode()
    assert image.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
    assert "qrc:" not in result.stderr.decode(), result.stderr.decode()
    pathlib.Path("build/preview.png").write_bytes(image.read_bytes())
print("CLI and offscreen render passed")

# Exercise the real installer CLI against an isolated Hyprland configuration.
with tempfile.TemporaryDirectory() as tmp:
    config = pathlib.Path(tmp) / "config"
    hypr = config / "hypr"
    hypr.mkdir(parents=True)
    main = hypr / "hyprland.lua"
    main.write_text("-- user's existing settings\n")
    install_env = dict(env, XDG_CONFIG_HOME=str(config))
    install_env.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
    rules = pathlib.Path("hypr/nap.lua").resolve()
    for _ in range(2):
        subprocess.run([binary, "--install-hyprland", "--link", rules], env=install_env, check=True, capture_output=True, timeout=10)
    assert (hypr / "nap.lua").resolve() == rules
    assert main.read_text().count('require("hypr.nap")') == 1
    subprocess.run([binary, "--uninstall-hyprland"], env=install_env, check=True, capture_output=True, timeout=10)
    assert not (hypr / "nap.lua").exists()
    assert main.read_text() == "-- user's existing settings\n"
    assert list(hypr.glob("hyprland.lua.bak.*"))
print("Isolated Hyprland installer CLI passed")
