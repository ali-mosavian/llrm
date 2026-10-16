import struct
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
