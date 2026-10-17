"""The rows' reference compilers are pinned: a distro clang (or any version but the pin) on PATH makes the script refuse, as the 71 rows were taken against Ubuntu clang 20.1.8 while the plan names 24.0.0git d1106deb71cc."""
import os
import stat
import subprocess
import tempfile
from pathlib import Path

PINNED = Path(__file__).resolve().parent / "rows" / "pinned.sh"


def fake(directory, name, line):
    path = Path(directory) / name
    path.write_text(f"#!/bin/sh\necho '{line}'\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


def refuses(clang_line, gcc_line):
    with tempfile.TemporaryDirectory() as d:
        fake(d, "clang", clang_line)
        fake(d, "gcc", gcc_line)
        env = {k: v for k, v in os.environ.items() if not k.startswith("VSGCC_")}
        env["PATH"] = d + ":" + env["PATH"]
        done = subprocess.run(["bash", "-c", f". {PINNED}; pin_references"], env=env, capture_output=True, text=True)
        return done.returncode, done.stdout, done.stderr


def test_a_distro_clang_makes_the_rows_refuse():
    code, _, why = refuses("Ubuntu clang version 20.1.8 (0ubuntu4)", "gcc (Ubuntu 13.4.0-4ubuntu1) 13.4.0")
    assert code != 0 and "20.1.8" in why


def test_another_gcc_makes_the_rows_refuse():
    code, _, why = refuses("clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", "gcc (GCC) 12.2.0")
    assert code != 0 and "12.2.0" in why


def test_the_pinned_pair_is_named_in_the_header():
    code, out, _ = refuses("clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", "gcc (Ubuntu 13.4.0-4ubuntu1) 13.4.0")
    assert code == 0 and "d1106deb71cc" in out and "13.4.0" in out
