import struct

import pytest
import zlib

import qb32demos


def png(path, pixels):
    rows = b"".join(b"\0" + bytes(sum(([c] * 3 for c in row), [])) for row in pixels)
    chunk = lambda kind, body: struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    header = struct.pack(">IIBBBBB", len(pixels[0]), len(pixels), 8, 2, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def test_a_mismatching_checkpoint_keeps_both_screens_for_ci(tmp_path, monkeypatch):
    """GORILLA 'prompt3: 56 pixels' failed in CI with nothing to look at but the count."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "target"))
    want, got = tmp_path / "want.png", tmp_path / "got.png"
    png(want, [[0] * 40] * 40)
    png(got, [[0] * 40] * 39 + [[9] * 40])
    name, checkpoint = next((n, c[2]) for n, (script, _) in qb32demos.PLAYED.items() for c in script if c[2] and not qb32demos.PLAYED[n][1].get(c[2]))
    assert qb32demos.differences(name, {checkpoint: want}, {checkpoint: got}, "x86-m16") == [f"{checkpoint}: 40 pixels"]
    kept = tmp_path / "target/gate-logs/screens"
    assert sorted(p.name for p in kept.iterdir()) == [f"{name}-x86-m16-{checkpoint}-got.png", f"{name}-x86-m16-{checkpoint}-want.png"]


def test_a_mismatching_checkpoint_prints_where_the_screens_differ(tmp_path, capsys):
    """Run 4 and 5 failed NIBBLES m16 'dies: 38 pixels' and the private group's screens are not uploaded: the job log had a count and no place."""
    want, got = tmp_path / "want.png", tmp_path / "got.png"
    png(want, [[0] * 80] * 50)
    png(got, [[0] * 80] * 49 + [[9] * 40 + [0] * 40])
    name, checkpoint = next((n, c[2]) for n, (script, _) in qb32demos.PLAYED.items() for c in script if c[2] and not qb32demos.PLAYED[n][1].get(c[2]))
    assert qb32demos.differences(name, {checkpoint: want}, {checkpoint: got}, "x86-m16") == [f"{checkpoint}: 40 pixels"]
    shown = capsys.readouterr().out
    assert "40 pixels in x 0-39, y 49-49 of 80x50" in shown and "|" + "#" * 32 + " " * 32 + "|" in shown.splitlines()[-1]


def test_a_demo_build_prints_and_keeps_the_hash_of_every_file_it_is_made_of(tmp_path, monkeypatch, capsys):
    """Run 6's NIBBLES m16 red is told from a local green by its inputs: the table is the same build or it is not."""
    import hashlib
    from types import SimpleNamespace

    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "target"))
    exe = tmp_path / "NIBBLES.x86-m16.EXE"
    exe.write_bytes(b"exe")
    (tmp_path / "NIBBLES.x86-m16.obj").write_bytes(b"obj")
    monkeypatch.setattr(qb32demos.qbbuild, "needed", lambda roots, objects: [objects["B$IN"]])
    runtime = tmp_path / "bin.obj"
    runtime.write_bytes(b"rt")
    qb32demos.record_hashes("NIBBLES", {"x86-m16": SimpleNamespace(exe=exe)}, {"x86-m16": {"B$IN": runtime}})
    shown = capsys.readouterr().out
    assert f"hash {hashlib.sha256(b'obj').hexdigest()} NIBBLES.x86-m16.obj" in shown
    assert f"hash {hashlib.sha256(b'rt').hexdigest()} runtime/bin.obj" in shown
    assert f"hash {hashlib.sha256(b'exe').hexdigest()} NIBBLES.x86-m16.EXE" in shown and " dosbox-x" in shown and " demo conf" in shown
    assert (tmp_path / "target/gate-logs/demo-hashes.txt").read_text().count("NIBBLES ") == 5


def test_a_demo_is_seeded_by_one_scripted_edit_of_its_randomize(tmp_path):
    """Run 7: NIBBLES m16 was 38 px off BCOM45's at some start times and not at others, because `RANDOMIZE TIMER` read two builds' clocks a few
    emulated ms apart. Both builds now come from the same edit; a demo with no such line, or two, is an error, not an unseeded run."""
    source = tmp_path / "DEMO.BAS"
    source.write_bytes(b"PRINT 1\r\nRANDOMIZE TIMER\r\nPRINT 2\r\n")
    assert qb32demos.seeded(source, 7, tmp_path).read_bytes() == b"PRINT 1\r\nRANDOMIZE 7\r\nPRINT 2\r\n"
    assert qb32demos.seeded(source, 7, tmp_path).name == "DEMO.BAS"
    for text in (b"PRINT 1\r\n", b"RANDOMIZE TIMER\r\nRANDOMIZE TIMER\r\n"):
        source.write_bytes(text)
        with pytest.raises(AssertionError):
            qb32demos.seeded(source, 7, tmp_path)
