"""CLI validation and an offscreen render, without user files or preferences."""
import os
import pathlib
import subprocess
import tempfile

env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_QPA_PLATFORMTHEME="generic",
           QT_QUICK_BACKEND="software", QT_QUICK_CONTROLS_STYLE="Basic")
binary = pathlib.Path("build/nap").resolve()
for args, expected in [(["--help"], 0), (["--version"], 0), (["--time", "nan"], 2),
                       (["--volume", "101"], 2), (["--time", "1:60"], 2),
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
