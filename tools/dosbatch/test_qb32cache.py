"""The dos32 object cache is keyed on sources and on the tools' builds: a changed tool, source or header misses it."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qb32  # noqa: E402


def test_a_changed_compiler_build_misses_the_cache(tmp_path, monkeypatch):
    """A rebuilt llrm-qb must not be handed the object an older one made."""
    source = tmp_path / "a.bas"
    source.write_text("PRINT 1\n")
    monkeypatch.setattr(qb32, "_stamp", lambda *names: "build-1")
    before = qb32.basic_key(source, ("-O2",))
    assert qb32.basic_key(source, ("-O2",)) == before
    monkeypatch.setattr(qb32, "_stamp", lambda *names: "build-2")
    assert qb32.basic_key(source, ("-O2",)) != before


def test_a_changed_source_or_flag_misses_the_cache(tmp_path, monkeypatch):
    monkeypatch.setattr(qb32, "_stamp", lambda *names: "build")
    source = tmp_path / "a.bas"
    source.write_text("PRINT 1\n")
    key = qb32.basic_key(source, ("-O2",))
    assert qb32.basic_key(source, ("-O1",)) != key
    source.write_text("PRINT 2\n")
    assert qb32.basic_key(source, ("-O2",)) != key


def test_a_changed_header_or_tool_misses_the_runtime_cache():
    source = Path("runtime/qb/string.c")
    key = qb32.runtime_key(source, "tree-1", "tools-1")
    assert qb32.runtime_key(source, "tree-2", "tools-1") != key
    assert qb32.runtime_key(source, "tree-1", "tools-2") != key


def test_the_tools_stamp_follows_the_binary(tmp_path, monkeypatch):
    """The stamp is the size and time of the file: replacing the binary changes it."""
    monkeypatch.setattr(qb32.dosbatch, "BIN", tmp_path)
    tool = tmp_path / "llrm-qb"
    tool.write_bytes(b"one")
    first = qb32._stamp("llrm-qb")
    tool.write_bytes(b"two!")
    assert qb32._stamp("llrm-qb") != first


def test_a_cached_object_is_not_made_again(tmp_path, monkeypatch):
    monkeypatch.setattr(qb32, "OBJECTS", tmp_path / "kept")
    made = []

    def produce(obj: Path):
        made.append(1)
        obj.write_bytes(b"object")

    one, two = tmp_path / "one.obj", tmp_path / "two.obj"
    qb32.cached("k", one, lambda: produce(one))
    qb32.cached("k", two, lambda: produce(two))
    assert made == [1] and two.read_bytes() == b"object"
    qb32.cached("other", tmp_path / "three.obj", lambda: produce(tmp_path / "three.obj"))
    assert made == [1, 1]
