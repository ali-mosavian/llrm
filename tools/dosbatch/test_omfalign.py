"""The image the emulator prices is linked from objects with no data segment wider than a dword (tools/dosbatch/omfalign.py)."""

from __future__ import annotations

import struct
import sys
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import omfalign  # noqa: E402


def record(kind: int, body: bytes) -> bytes:
    return bytes([kind]) + struct.pack("<H", len(body) + 1) + body + b"\x00"


def lnames(*names: str) -> bytes:
    return record(0x96, b"".join(bytes([len(name)]) + name.encode() for name in names))


def segdef(field: int, size: int, name: int, class_: int) -> bytes:
    """A 16-bit relocatable SEGDEF: alignment `field`, public combine, `size` bytes, LNAMES index `name`, class `class_`."""
    return record(0x98, bytes([field << 5 | 2 << 2 | 1]) + struct.pack("<H", size) + bytes([name, class_, 1]))


def obj(path: Path, *segments: tuple[str, str, int], size: int = 8) -> Path:
    """An object of one SEGDEF of `size` bytes per (name, class, alignment field)."""
    names = ["", *sorted({n for n, _, _ in segments} | {c for _, c, _ in segments})]
    body = lnames(*names) + b"".join(segdef(field, size, names.index(name) + 1, names.index(class_) + 1) for name, class_, field in segments)
    path.write_bytes(record(0x80, b"\x01x") + body + record(0x8A, b"\x00"))
    return path


def sizes(path: Path) -> dict[str, int]:
    data = path.read_bytes()
    out, names, at = {}, [], 0
    while at < len(data):
        kind, length = data[at], struct.unpack_from("<H", data, at + 1)[0]
        body = data[at + 3 : at + 2 + length]
        if kind == 0x96:
            while body:
                names.append(body[1 : 1 + body[0]].decode())
                body = body[1 + body[0] :]
        elif kind == 0x98:
            out[names[body[3] - 1]] = struct.unpack_from("<H", body, 1)[0]
        at += 3 + length
    return out


def fields(path: Path) -> dict[str, int]:
    data = path.read_bytes()
    out, names, at = {}, [], 0
    while at < len(data):
        kind, length = data[at], struct.unpack_from("<H", data, at + 1)[0]
        body = data[at + 3 : at + 2 + length]
        if kind == 0x96:
            while body:
                names.append(body[1 : 1 + body[0]].decode())
                body = body[1 + body[0] :]
        elif kind == 0x98:
            out[names[body[3] - 1]] = body[0] >> 5
        at += 3 + length
    return out


def test_a_paragraph_aligned_data_segment_is_a_dword_and_code_and_dword_data_are_as_they_were(tmp_path):
    one = obj(tmp_path / "a.obj", ("CONST2", "DATA", 3), ("_BSS", "BSS", 3), ("FRAMES_CODE", "CODE", 3), ("_DATA", "DATA", 5))
    (copy,) = omfalign.dword_laid_out([one], tmp_path / "out")
    assert fields(copy) == {"CONST2": 5, "_BSS": 5, "FRAMES_CODE": 3, "_DATA": 5}
    assert fields(one)["CONST2"] == 3, "the object itself is not rewritten"


def test_the_flat_image_is_linked_from_dword_aligned_copies_and_the_executable_from_the_objects(tmp_path):
    """frames at bq32 linked as an ELF with CONST2 paragraph-aligned had its section tables written over its code (0x102f in code to 0x19f5):
    the emulator ran relocation records, and 21 of 39 bq32 rows had no clocks."""
    program = obj(tmp_path / "p.obj", ("CONST2", "DATA", 3))
    library = obj(tmp_path / "l.obj", ("CONST2", "DATA", 5), ("_BSS", "BSS", 3))
    commands: list[list[str]] = []
    recipe = {"first": [], "last": [], "final": [], "format": ["format", "os2", "le"]}
    with (
        mock.patch.object(dosbatch, "_host", side_effect=commands.append),
        mock.patch.object(dosbatch, "target_link", return_value=recipe),
        mock.patch.object(dosbatch, "os_defines", return_value=()),
    ):
        dosbatch.link_target("t", program, tmp_path / "p.exe", tmp_path, objects_after=(library,), image=tmp_path / "p.elf")
    exe, elf = commands
    objects = lambda command: [Path(command[at + 1]) for at, word in enumerate(command) if word == "file"]  # noqa: E731
    assert objects(exe) == [program, library]
    assert [fields(one) for one in objects(elf)] == [{"CONST2": 5}, {"CONST2": 5, "_BSS": 5}]


def test_a_segment_is_a_multiple_of_a_dword_long_so_the_next_objects_variables_start_aligned(tmp_path):
    """nhstutil's _BSS is 265 bytes: jwlink's ELF writer put the next object's variables at odd addresses (a string descriptor at 0x804e48f,
    whose low bit the string space reads as its free flag), and stft_fixed's string compaction walked into a wild pointer."""
    one = obj(tmp_path / "n.obj", ("_BSS", "BSS", 5), ("CODE1", "CODE", 3), size=265)
    (copy,) = omfalign.dword_laid_out([one], tmp_path / "out")
    assert sizes(copy) == {"_BSS": 268, "CODE1": 268} and sizes(one) == {"_BSS": 265, "CODE1": 265}


def test_two_images_linked_at_once_do_not_share_their_object_copies(tmp_path):
    """The bench builds its rows in threads in one work directory: the copies were named by their place in the link, so two rows wrote 014-rtterm.obj
    under each other's jwlink (E2146, `an invalid object file`) and the rows had no build."""
    program = obj(tmp_path / "p.obj", ("CONST2", "DATA", 3))
    recipe = {"first": [], "last": [], "final": [], "format": ["format", "os2", "le"]}
    commands: list[list[str]] = []
    with (
        mock.patch.object(dosbatch, "_host", side_effect=commands.append),
        mock.patch.object(dosbatch, "target_link", return_value=recipe),
        mock.patch.object(dosbatch, "os_defines", return_value=()),
    ):
        for name in ("a", "b"):
            dosbatch.link_target("t", program, tmp_path / f"{name}.exe", tmp_path, image=tmp_path / f"{name}.elf")
    copies = [Path(command[command.index("file") + 1]) for command in commands if "elf" in command]
    assert len(copies) == 2 and copies[0] != copies[1] and copies[0].parent != copies[1].parent
