"""arm64_parity.py off macOS: it imports without `xcrun`, and reads a Mach-O's __text itself."""
import os
import struct
import subprocess
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent


def macho(sections):
    """A one-segment arm64 Mach-O object holding `sections` as (segment, name, size)."""
    commands = struct.pack("<II16sQQQQiiII", 0x19, 72 + 80 * len(sections), b"", 0, 0, 0, 0, 7, 7, len(sections), 0)
    for seg, name, size in sections:
        commands += struct.pack(
            "<16s16sQQIIIIIIII", name.encode(), seg.encode(), 0, size, 0, 0, 0, 0, 0, 0, 0, 0
        )
    header = struct.pack("<IiiIIIII", 0xFEEDFACF, 0x0100000C, 0, 1, 1, len(commands), 0, 0)
    return header + commands


def test_importing_without_xcrun_does_not_raise(tmp_path):
    """`xcrun --show-sdk-path` at import raised FileNotFoundError on Linux, so the corpus leg never ran."""
    env = {"PATH": str(tmp_path), "PYTHONDONTWRITEBYTECODE": "1"}
    done = subprocess.run(
        [sys.executable, "-c", "import arm64_parity as a; print(repr(a.SDK))"],
        cwd=TOOLS, env=env, capture_output=True, text=True,
    )
    assert done.returncode == 0, done.stderr
    assert done.stdout.strip() == "''"


def test_sysroot_comes_from_the_environment(tmp_path):
    env = {"PATH": str(tmp_path), "LLRM_SYSROOT": "/sdk", "PYTHONDONTWRITEBYTECODE": "1"}
    done = subprocess.run(
        [sys.executable, "-c", "import arm64_parity as a; print(a.SDK, a.FRONT[-2:])"],
        cwd=TOOLS, env=env, capture_output=True, text=True,
    )
    assert done.returncode == 0, done.stderr
    assert done.stdout.strip() == "/sdk ['-isysroot', '/sdk']"


def test_text_size_is_the_text_sections_of_a_known_object(tmp_path):
    """Positive control: 8 bytes of __text and 4 of __text_startup are 12; __data is not code."""
    import importlib
    sys.path.insert(0, str(TOOLS))
    env = dict(os.environ)
    try:
        os.environ["PATH"] = str(tmp_path)
        a = importlib.import_module("arm64_parity")
    finally:
        os.environ.clear()
        os.environ.update(env)
    obj = tmp_path / "t.o"
    obj.write_bytes(macho([("__TEXT", "__text", 8), ("__TEXT", "__text_startup", 4), ("__DATA", "__data", 100)]))
    assert a.text_size(obj) == 12
    assert a.insn_count(obj) == 3
    obj.write_bytes(b"not a mach-o object at all, long enough to read a header field....")
    assert a.text_size(obj) is None
