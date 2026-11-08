"""The z80 corpus IR is made with the shared neutral flag set, and the IR carries no distro default (frame pointer, stack protector, unwind tables, branch protection)."""

import importlib.util
import os
import shutil
import subprocess
from pathlib import Path

import pytest

import external_flags

ROOT = Path(__file__).resolve().parent.parent
RUN = ROOT / "crates/target/llrm-z80/tests/programs/run.py"
PROBE = "extern void use(char *);\nint f(int n) { char b[64]; use(b); return b[n & 63]; }\n"
DISTRO = ("\"frame-pointer\"=\"all\"", "\"frame-pointer\"=\"non-leaf\"", "stack-protector\"=\"", "uwtable", "cf-protection")


def corpus_flags():
    spec = importlib.util.spec_from_file_location("z80_run", RUN)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.CLANG_FLAGS


def test_the_corpus_ir_command_line_holds_the_shared_flag_set():
    """Before: the msp430 target's defaults were relied on, none of the list was on the command line."""
    flags = corpus_flags()
    assert set(external_flags.flags("none")) <= set(flags)
    assert flags[:2] == ["-target", "msp430"]


def test_the_ir_of_a_probe_carries_no_distro_default(tmp_path):
    clang = os.environ.get("LLRM_PIN_CLANG") or str(Path.home() / "scratch/llvm-build/bin/clang")
    if not (shutil.which(clang) or Path(clang).exists()):
        pytest.skip("no pin clang")
    source = tmp_path / "p.c"
    source.write_text(PROBE)
    done = subprocess.run([clang, "-O2", *corpus_flags(), str(source), "-o", str(tmp_path / "p.ll")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    ir = (tmp_path / "p.ll").read_text()
    assert not [mark for mark in DISTRO if mark in ir], ir
