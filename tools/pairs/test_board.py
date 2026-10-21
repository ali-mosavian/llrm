"""The compile board's header names every input of its numbers: 1278 M was posted next to 1412 M from a run whose input nobody could name (the header gave neither the file nor its content)."""
from __future__ import annotations

import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIELDS = ("input", "clang", "clang md5", "clang version", "includes", "llrm head", "uptime")


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def shim(path: Path, text: str) -> str:
    path.write_text(f"#!/bin/bash\necho '{text}'\n")
    path.chmod(0o755)
    return str(path)


def made(tmp_path, source_text="int a;", uptime="up 1 day, load average: 0.10"):
    board = load("board")
    source = tmp_path / "d_faces.c"
    source.write_text(source_text)
    clang = shim(tmp_path / "clang", "clang version 24.0.0git (pinned d1106deb71cc)")
    head = shim(tmp_path / "llrm-c", "head")
    lines = board.header(source, ["-I", str(tmp_path)], clang, [("head", head)], uptime)
    return board, lines, source


def test_the_header_names_every_input_of_the_numbers(tmp_path):
    _, lines, source = made(tmp_path)
    text = "\n".join(lines)
    for field in FIELDS:
        assert any(line.startswith(f"{field}:") for line in lines), f"no `{field}:` line in\n{text}"
    assert str(source) in text and "load average: 0.10" in text and "pinned d1106deb71cc" in text


def test_two_runs_on_the_same_inputs_have_the_same_header(tmp_path):
    _, first, _ = made(tmp_path)
    _, second, _ = made(tmp_path)
    assert first == second


def test_an_input_that_changes_content_but_not_path_changes_the_header(tmp_path):
    _, first, source = made(tmp_path, "int a;")
    board = load("board")
    source.write_text("int b;")
    second = board.header(source, ["-I", str(tmp_path)], str(tmp_path / "clang"), [("head", str(tmp_path / "llrm-c"))], "up 1 day, load average: 0.10")
    assert first != second
    assert [line for line in first if line.startswith("input:")] != [line for line in second if line.startswith("input:")]


def test_a_changed_compiler_changes_the_header(tmp_path):
    _, first, _ = made(tmp_path)
    shim(tmp_path / "llrm-c", "other")
    board = load("board")
    second = board.header(tmp_path / "d_faces.c", ["-I", str(tmp_path)], str(tmp_path / "clang"), [("head", str(tmp_path / "llrm-c"))], "up 1 day, load average: 0.10")
    assert first != second
