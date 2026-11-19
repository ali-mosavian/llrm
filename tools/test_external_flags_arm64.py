"""arm64 probe beside test_external_flags.py: the shared flag list reaches the arm64 externals (gcc, the pin's clang), and the bars and OUR compiler agree
on what the object contains: no PAC/BTI, no canary, and the same relocation kinds. A mismatch refuses the bar."""

import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

import external_flags

TRIPLE = "aarch64-unknown-linux-gnu"
RELOC = ("-fPIE",)  # what ours emits for arm64 Linux (GOT loads for extern data, like a distro PIE); asserted by the relocation probe below
PROBE = (
    "extern void use(char *);\nextern int table[16];\nint counter;\n"
    "int f(int n) { char b[64]; use(b); return b[n & 63] + table[n & 15] + counter; }\n"
)
DISTRO = ["-mbranch-protection=standard", "-fstack-protector-all"]


def tool(name, env=None):
    path = (os.environ.get(env) if env else None) or shutil.which(name)
    assert path, f"the arm64 probe needs {name}" + (f" (or ${env})" if env else "")
    return path


def gcc():
    return tool("aarch64-linux-gnu-gcc")


def pin():
    return tool("clang", "LLRM_PIN_CLANG")


def ours():
    return tool("llrm-cc", "LLRM_ARM64_CC")


def compile_to(compiler, flags, tmp_path, name, kind):
    src = tmp_path / "probe.c"
    src.write_text(PROBE)
    out = tmp_path / f"{name}.{'s' if kind == '-S' else 'o'}"
    cmd = [compiler, *flags, "-Os", kind, "-o", str(out), str(src)]
    done = subprocess.run(cmd, capture_output=True, text=True, env={**os.environ, "LLRM_ARM64_SYSROOT": "/usr/aarch64-linux-gnu"})
    assert done.returncode == 0, f"{cmd}\n{done.stderr}"
    return out


def shared(compiler_kind):
    return list(external_flags.flags("arm64", RELOC, 15 if compiler_kind == "gcc" else None))


def relocation_kinds(obj):
    """The code and data relocation kinds; .eh_frame's (unwind tables) are their own column, see build.sh."""
    readelf = shutil.which("llvm-readelf") or shutil.which("aarch64-linux-gnu-readelf") or tool("readelf")
    text = subprocess.run([readelf, "-r", "-W", str(obj)], capture_output=True, text=True, check=True).stdout
    sections = re.split(r"\n(?=Relocation section )", text)
    return sorted({k for one in sections if one.strip() and ".eh_frame" not in one.strip().splitlines()[0] for k in re.findall(r"R_AARCH64_\w+", one)})


def test_the_arm64_target_field_is_branch_protection_none_and_not_x86s():
    flags = external_flags.flags("arm64", RELOC)
    assert "-mbranch-protection=none" in flags
    assert not any(f.startswith("-fcf-protection") for f in flags)


@pytest.mark.parametrize("kind", ["gcc", "pin"])
def test_a_shared_flag_build_has_no_pac_bti_or_canary(kind, tmp_path):
    compiler = gcc() if kind == "gcc" else pin()
    target = [] if kind == "gcc" else ["-target", TRIPLE]
    asm = compile_to(compiler, [*target, *shared(kind)], tmp_path, kind, "-S").read_text()
    assert not re.search(r"\b(paciasp|autiasp|bti)\b|Tag_Feature_(BTI|PAC)|__stack_chk", asm), asm


@pytest.mark.parametrize("kind", ["gcc", "pin"])
def test_the_detector_fires_on_a_distro_flag_build(kind, tmp_path):
    """Without the list, the same probe shows PAC/BTI and a canary: the asm check above can fail."""
    compiler = gcc() if kind == "gcc" else pin()
    target = [] if kind == "gcc" else ["-target", TRIPLE]
    asm = compile_to(compiler, [*target, *RELOC, *DISTRO], tmp_path, kind, "-S").read_text()
    assert re.search(r"\b(paciasp|bti)\b|Tag_Feature_(BTI|PAC)", asm) and "__stack_chk" in asm, asm


def test_ours_gcc_and_the_pin_have_the_same_relocation_kinds(tmp_path):
    """A bar built with another relocation model is refused: ours loads extern data through the GOT at arm64 Linux, as -fPIE does."""
    mine = relocation_kinds(compile_to(ours(), ["-target", TRIPLE, "-ffreestanding"], tmp_path, "ours", "-c"))
    theirs = {
        "gcc": relocation_kinds(compile_to(gcc(), shared("gcc"), tmp_path, "gcc", "-c")),
        "pin": relocation_kinds(compile_to(pin(), ["-target", TRIPLE, *shared("pin")], tmp_path, "pin", "-c")),
    }
    assert mine, "ours has relocations in the probe"
    for name, kinds in theirs.items():
        assert kinds == mine, f"{name} {kinds} against ours {mine}"


def frame_record(asm):
    """Whether the non-leaf probe function builds the x29/x30 frame record (stp x29, x30 / add x29, sp)."""
    return bool(re.search(r"\bstp\s+x29,\s*x30\b", asm))


def test_ours_and_the_bars_agree_on_the_frame_record(tmp_path):
    """The shared list omits the frame pointer; gcc and the pin then build no x29 record in a non-leaf function. Ours keeps it (clang's AArch64 Linux
    default, 'non-leaf'). A bar frameless against a framed candidate is a different machine contract: open decision, red until oversight rules."""
    mine = frame_record(compile_to(ours(), ["-target", TRIPLE, "-ffreestanding"], tmp_path, "ours", "-S").read_text())
    gcc_asm = compile_to(gcc(), shared("gcc"), tmp_path, "gcc", "-S").read_text()
    pin_asm = compile_to(pin(), ["-target", TRIPLE, *shared("pin")], tmp_path, "pin", "-S").read_text()
    assert (frame_record(gcc_asm), frame_record(pin_asm)) == (mine, mine), f"ours {mine}, gcc {frame_record(gcc_asm)}, pin {frame_record(pin_asm)}"
