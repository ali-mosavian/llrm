import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

import external_flags

ROOT = Path(__file__).resolve().parent.parent
PROBE = "extern void use(char *);\nint f(int n) { char b[64]; use(b); return b[n & 63]; }\n"
MODES = {
    "m32": (["-m32"], "movl\\s+%esp,\\s*%ebp"),
    "m64": (["-m64"], "movq\\s+%rsp,\\s*%rbp"),
}
DISTRO = ["-fno-omit-frame-pointer", "-fstack-protector-all", "-fcf-protection=full"]


def compilers():
    pinned = os.environ.get("LLRM_PIN_CLANG")  # the host's clang is a different compiler: it is labelled so, not passed off as the pin
    clang = (("pin", pinned),) if pinned else (("host-clang", shutil.which("clang")),)
    return [(name, path) for name, path in (("gcc", shutil.which("gcc")), *clang) if path]


def markers(frame):
    return {"endbr": re.compile(r"endbr(32|64)"), "canary": re.compile(r"__stack_chk"), "frame setup": re.compile(frame)}


def asm(compiler, mode, flags, tmp_path):
    out = tmp_path / "probe.s"
    (tmp_path / "probe.c").write_text(PROBE)
    done = subprocess.run([compiler, *MODES[mode][0], "-Os", *flags, "-S", "-o", str(out), str(tmp_path / "probe.c")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return out.read_text()


def test_the_flags_the_shell_callers_print_are_the_modules():
    printed = subprocess.run(["python3", str(ROOT / "tools" / "external_flags.py"), "--reloc=-fno-pic"], capture_output=True, text=True, check=True).stdout
    assert printed.split() == list(external_flags.flags("x86", ("-fno-pic",)))


def test_relocation_is_the_callers_and_zero_init_padding_is_gcc_15s():
    assert not any("pic" in f or "pie" in f.lower() for f in external_flags.flags("x86"))
    assert "-fPIE" in external_flags.flags("x86", ("-fPIE",))
    assert external_flags.ZERO_INIT_PADDING in external_flags.flags("none", gcc_major=15)
    assert external_flags.ZERO_INIT_PADDING not in external_flags.flags("none", gcc_major=13)


def test_the_hash_is_twelve_hex_digits():
    assert re.fullmatch("[0-9a-f]{12}", external_flags.config_hash())


@pytest.mark.parametrize("path", ["crates/target/llrm-x86-m32/vsgcc/build.sh", "tools/pairs/pinclocks.sh", "tools/bench/native.py"])
def test_every_external_bar_takes_its_flags_from_the_one_list(path):
    """Ubuntu gcc keeps the frame pointer: x_fir -Os was a 10 B-too-easy bar (176 B vs 166 B) because the bar's command line named its own flags."""
    text = (ROOT / path).read_text()
    assert "external_flags" in text
    assert "-fno-stack-protector" not in text


@pytest.mark.parametrize("mode", MODES)
@pytest.mark.parametrize("name,compiler", compilers())
def test_the_probe_sees_what_the_distro_defaults_add(name, compiler, mode, tmp_path):
    """The detector must fire on a build that has the feature, or a clean probe below proves nothing."""
    text = asm(compiler, mode, DISTRO, tmp_path)
    assert all(marker.search(text) for marker in markers(MODES[mode][1]).values()), (name, mode)


@pytest.mark.parametrize("mode", MODES)
@pytest.mark.parametrize("name,compiler", compilers())
def test_a_neutral_external_has_no_endbr_canary_or_frame_pointer(name, compiler, mode, tmp_path):
    text = asm(compiler, mode, external_flags.flags("x86", ("-fno-pic",)), tmp_path)
    assert not [kind for kind, marker in markers(MODES[mode][1]).items() if marker.search(text)], f"{name} {mode}: {text}"


def gcc_calls(flags, tmp_path):
    source = ROOT / "tests" / "run" / "c" / "frameless.c"
    out = tmp_path / "frameless.s"
    done = subprocess.run(["gcc", "-m32", "-Os", "-ffreestanding", "-fno-pic", *flags, "-S", "-o", str(out), str(source)], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return set(re.findall(r"^\s*call\s+(\w+)", out.read_text(), re.M)) - {"report"}


@pytest.mark.skipif(not shutil.which("gcc"), reason="no gcc")
def test_gcc_gets_no_inlining_where_a_test_says_fno_inline_functions(tmp_path):
    """frameless x86_32 -Os read gcc 239 B against ours 394: gcc's -fno-inline-functions still inlined every called-once static,
    ours and the pin's meant no inlining (gcc 304 B; the pin 625; gcc -fno-inline 501)."""
    raw = gcc_calls(["-fno-inline-functions"], tmp_path)
    translated = gcc_calls(external_flags.gcc_program_flags(["-fno-inline-functions"]), tmp_path)
    assert not any(name.startswith(("mix", "add4", "chain")) for name in raw)
    assert {"mix", "add4"} <= translated


def test_other_flags_pass_to_gcc_unchanged():
    tokens = ["-fno-inline-functions-called-once", "-mabi=watcom", "-fno-tree-vectorize"]
    assert external_flags.gcc_program_flags(tokens) == tokens
