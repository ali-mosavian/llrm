import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import wasm_sweep

# A module whose table sits in Data: 0x1d code bytes, 0x40 data bytes.
HEADERS = """
Sections:

     Type start=0x0000000a end=0x00000012 (size=0x00000008) count: 1
   Import start=0x00000014 end=0x00000020 (size=0x0000000c) count: 1
     Code start=0x00000030 end=0x0000004d (size=0x0000001d) count: 1
     Data start=0x0000004f end=0x0000008f (size=0x00000040) count: 1
"""


def test_bytes_count_the_data_section_the_code_section_alone_does_not():
    """The wasm sweep read the Code section only: a table the pin keeps in Data was free for whoever kept it there."""
    assert wasm_sweep.code_bytes(HEADERS) == 0x1D
    assert wasm_sweep.module_bytes(HEADERS) == 0x1D + 0x40
    assert wasm_sweep.module_bytes(HEADERS) > wasm_sweep.code_bytes(HEADERS)


def test_a_module_without_data_is_its_code():
    assert wasm_sweep.module_bytes("   Code start=0x1 end=0x2 (size=0x00000005) count: 1") == 5


def test_the_header_names_the_hash_the_external_and_the_sections():
    text = "\n".join(wasm_sweep.header("/dist"))
    assert "# external_flags " in text and "pin's clang only" in text and "Code + Data" in text


def test_a_real_object_with_a_table_reads_larger_with_data():
    clang, objdump = os.environ.get("LLRM_CLANG_WASM"), shutil.which("wasm-objdump")
    if not (clang and objdump):
        return
    with tempfile.TemporaryDirectory() as tmp:
        src, obj = Path(tmp) / "t.c", Path(tmp) / "t.o"
        src.write_text("const int t[64] = {" + ",".join(str(i * 7 + 3) for i in range(64)) + "};\nint f(int i) { return t[i & 63]; }\n")
        subprocess.run([clang, "--target=wasm32", "-ffreestanding", "-Os", "-c", str(src), "-o", str(obj)], check=True)
        both, code = wasm_sweep.measured(obj)
        assert both > code, (both, code)


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print("ok", name)
