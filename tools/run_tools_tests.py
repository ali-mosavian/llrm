#!/usr/bin/env python3
"""Run tools/**/test_*.py with the standard library: python3 -I tools/run_tools_tests.py [--root DIR] [FILE...]

unittest cases and plain `def test_x():` functions run in-process. A file that uses pytest (fixtures, raises, parametrize, skip) runs through
`uv run --offline --with pytest` (cached packages only, no fetch); with no uv it is a FAIL named NEEDS-PYTEST, never a skip. An import error is a FAIL. Exit 1 on any FAIL."""

import ast
import importlib.util
import inspect
import re
import shutil
import subprocess
import sys
import traceback
import unittest
from pathlib import Path

FIXTURES = {"tmp_path", "tmp_path_factory", "monkeypatch", "capsys", "capfd", "caplog", "request"}


def needs_pytest(source: str) -> bool:
    """The file imports pytest or has a function taking a pytest fixture. A string that mentions them does not count."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return False  # run in-process: the import error is reported there
    for node in ast.walk(tree):
        if isinstance(node, ast.Import) and any(a.name.split(".")[0] == "pytest" for a in node.names):
            return True
        if isinstance(node, ast.ImportFrom) and (node.module or "").split(".")[0] == "pytest":
            return True
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and FIXTURES & {a.arg for a in node.args.args}:
            return True
    return False


def plain_file(path: Path) -> tuple[int, int, list[str]]:
    """(passed, failed, messages) for one file run in-process."""
    spec = importlib.util.spec_from_file_location(f"_tools_test_{abs(hash(path))}", path)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(path.parent))
    try:
        spec.loader.exec_module(module)
    except BaseException:  # noqa: BLE001 - an import error is a FAIL, whatever raised it
        return 0, 1, [f"import error:\n{traceback.format_exc()}"]
    finally:
        sys.path.remove(str(path.parent))
    passed, failed, messages = 0, 0, []
    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
    result = unittest.TextTestRunner(stream=open("/dev/null", "w"), verbosity=0).run(suite)
    passed += result.testsRun - len(result.failures) - len(result.errors)
    for case, text in [*result.failures, *result.errors]:
        failed += 1
        messages.append(f"{case.id()}:\n{text}")
    for name, function in inspect.getmembers(module, inspect.isfunction):
        if not name.startswith("test_") or function.__module__ != module.__name__:
            continue
        if inspect.signature(function).parameters:
            failed += 1
            messages.append(f"{name}: takes arguments (a fixture) but the file was not detected as pytest")
            continue
        try:
            function()
            passed += 1
        except BaseException:  # noqa: BLE001
            failed += 1
            messages.append(f"{name}:\n{traceback.format_exc()}")
    if passed + failed == 0:
        return 0, 1, ["no tests found: a file named test_*.py that runs nothing is a FAIL"]
    return passed, failed, messages


def pytest_file(path: Path) -> tuple[int, int, list[str]]:
    uv = shutil.which("uv")
    if not uv:
        return 0, 1, ["NEEDS-PYTEST: uses pytest features and `uv` is not on PATH"]
    done = subprocess.run([uv, "run", "--offline", "--with", "pytest", "--with", "iced-x86", "python", "-m", "pytest", "-q", "-p", "no:cacheprovider", str(path)], capture_output=True, text=True, cwd=path.parent)
    tail = (done.stdout + done.stderr).strip().splitlines()[-1:] or [""]
    if done.returncode == 0:
        found = re.search(r"(\d+) passed", tail[0])
        return int(found.group(1)) if found else 1, 0, []
    return 0, 1, [(done.stdout + done.stderr)[-2000:]]


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parent
    if argv[:1] == ["--root"]:
        root, argv = Path(argv[1]).resolve(), argv[2:]
    files = [Path(a).resolve() for a in argv] or sorted(root.rglob("test_*.py"))
    total_pass = total_fail = 0
    for path in files:
        runner = pytest_file if needs_pytest(path.read_text()) else plain_file
        passed, failed, messages = runner(path)
        total_pass, total_fail = total_pass + passed, total_fail + failed
        print(f"{'FAIL' if failed else 'ok  '} {path.relative_to(root) if path.is_relative_to(root) else path}: {passed} passed, {failed} failed")
        for message in messages:
            print("    " + message.strip().replace("\n", "\n    ")[:1500])
    print(f"{total_pass} passed, {total_fail} failed in {len(files)} files")
    return 1 if total_fail or not files else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
