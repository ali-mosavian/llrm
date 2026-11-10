import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import external_flags
import wasm_pin

PROBE = "extern void use(char *);\nint f(int n) { char b[64]; use(b); return b[n & 63]; }\n"
CLANG = os.environ.get("LLRM_CLANG_WASM") or shutil.which("clang")


def test_the_pin_command_carries_every_shared_flag_and_the_header_names_the_hash():
    """The wasm sweep's pin column was built with `--target=wasm32 -ffreestanding` alone: no shared flag reached it."""
    argv = wasm_pin.pin_command("clang", "a.c", "a.o", "Os")
    assert all(flag in argv for flag in external_flags.flags("none")), argv
    assert wasm_pin.header() == f"# external_flags {external_flags.config_hash()}"


def asm(flags):
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / "p.c"
        src.write_text(PROBE)
        done = subprocess.run([CLANG, "--target=wasm32", "-ffreestanding", "-Os", *flags, "-S", "-o", "-", str(src)], capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        return done.stdout


def test_the_probe_sees_a_canary_only_in_a_distro_flag_build():
    """The detector fires on `-fstack-protector-all` and stays silent with the shared list, so a silent run means something."""
    if not CLANG:
        return
    assert "__stack_chk" in asm(["-fstack-protector-all"])
    assert "__stack_chk" not in asm(list(external_flags.flags("none")))


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print("ok", name)
