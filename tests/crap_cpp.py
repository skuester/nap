"""CRAP scores for the native Qt adapter: complexity^2 * (1 - coverage)^3 + complexity.

crap4rs covers the Rust; nothing equivalent reads C++, so this does the same sum with gcc's own
gcov. Coverage is gathered from both ways the adapter runs under test: the Qt test executable, and
the real binary driven by tests/binary_tests.rs with the native sources instrumented. Complexity is
cyclomatic, counted from each function's decision points.
"""
import argparse
import json
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NATIVE = ROOT / "native"
WORK = ROOT / "build" / "cpp-coverage"
OFFSCREEN = {"QT_QPA_PLATFORM": "offscreen", "QT_QPA_PLATFORMTHEME": "generic",
             "QT_QUICK_BACKEND": "software", "QT_QUICK_CONTROLS_STYLE": "Basic"}
DECISIONS = re.compile(r"\b(?:if|for|while|case|catch)\b|&&|\|\||\?")
LITERALS = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|//.*$')


def run(command, **options):
    result = subprocess.run(command, capture_output=True, text=True, **options)
    if result.returncode:
        sys.exit(f"{' '.join(map(str, command))} failed:\n{result.stdout[-3000:]}{result.stderr[-3000:]}")
    return result.stdout


def gather():
    """Build and run both instrumented programs, leaving .gcda files under WORK."""
    WORK.mkdir(parents=True, exist_ok=True)
    for stale in WORK.rglob("*.gcda"):
        stale.unlink()
    env = dict(os.environ, NAP_COVERAGE="1", CARGO_TARGET_DIR=str(WORK / "cargo"))
    run(["cargo", "test", "--locked", "--test", "binary_tests"], cwd=ROOT, env=env)
    qt = WORK / "qt"
    qt.mkdir(exist_ok=True)
    run(["/usr/lib/qt6/bin/qmake", str(ROOT / "tests/player_test.pro"),
         "QMAKE_CXXFLAGS+=--coverage -O0", "QMAKE_LFLAGS+=--coverage"], cwd=qt)
    run(["make", "-j4"], cwd=qt)
    run([str(qt / "player-test")], cwd=ROOT, env=dict(os.environ, **OFFSCREEN))


def reports():
    for data in WORK.rglob("*.gcda"):
        text = run(["gcov", "--json-format", "--stdout", "--demangled-names", "-o", str(data.parent), str(data)], cwd=ROOT)
        for document in text.splitlines():
            if document.startswith("{"):
                report = json.loads(document)
                # Paths are relative to wherever that program was compiled.
                for file in report["files"]:
                    yield (pathlib.Path(report["current_working_directory"]) / file["file"]).resolve(), file


def analyze():
    """(file, function) -> [start line, {line: hit count}], merged across both programs."""
    functions = {}
    for source, report in reports():
        if NATIVE not in source.parents:
            continue
        starts = {f["name"]: (f["demangled_name"], f["start_line"]) for f in report["functions"]}
        for line in report["lines"]:
            name, start = starts.get(line.get("function_name"), (None, 0))
            if name:
                hits = functions.setdefault((source, name), [start, {}])[1]
                hits[line["line_number"]] = hits.get(line["line_number"], 0) + line["count"]
    return functions


def short(name):
    name = re.sub(r"\(.*", "", name.replace("Player::Player(QObject*)::", "Player::Player::"))
    return re.sub(r"\{lambda.*", "lambda", name)[-46:]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--threshold", type=float, default=12)
    threshold = parser.parse_args().threshold
    gather()
    rows = []
    for (source, name), (start, hits) in analyze().items():
        text = source.read_text().splitlines()
        body = " ".join(LITERALS.sub("", text[number - 1]) for number in hits)
        complexity = 1 + len(DECISIONS.findall(body))
        coverage = sum(1 for count in hits.values() if count) / len(hits)
        crap = complexity ** 2 * (1 - coverage) ** 3 + complexity
        rows.append((crap, complexity, coverage, f"{source.name}:{start}", short(name)))
    if not rows:
        sys.exit("no native coverage was recorded")
    rows.sort(reverse=True)
    print(f"{'CRAP':>7}  {'CC':>3}  {'Cov%':>5}  {'Where':<16} Function")
    for crap, complexity, coverage, where, name in rows[:15]:
        print(f"{crap:7.2f}  {complexity:3}  {coverage * 100:5.1f}  {where:<16} {name}")
    failing = [row for row in rows if row[0] > threshold]
    print(f"{'FAIL' if failing else 'PASS'}: {len(rows)} native functions | {len(failing)} above threshold "
          f"({threshold:g}) | worst: {rows[0][0]:.1f}")
    sys.exit(1 if failing else 0)


main()
