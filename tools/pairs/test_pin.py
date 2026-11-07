"""Every run of the pin's tools names its target: without one they priced for x86-64 with SSE, and numbers taken so were quoted."""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent


def load():
    spec = importlib.util.spec_from_file_location("pin", HERE / "pin.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules["pin"] = module  # a dataclass reads its module
    spec.loader.exec_module(module)
    return module


MODULE = "define i32 @f(i32 %x) {\n  ret i32 %x\n}\n"


def fake(tmp_path, name):
    """A stand-in tool: prints its argv and its input."""
    path = tmp_path / name
    path.write_text('#!/bin/bash\necho "ARGS $*"\ncat\n')
    path.chmod(0o755)
    return tmp_path


def test_the_pins_tools_refuse_to_run_without_a_target(tmp_path):
    pin = load()
    fake(tmp_path, "opt")
    fake(tmp_path, "llc")
    with pytest.raises(pin.Refused):
        pin.opt("default<O2>", MODULE, bin_dir=tmp_path)
    with pytest.raises(pin.Refused):
        pin.llc(["-O2"], MODULE, bin_dir=tmp_path)
    with pytest.raises(pin.Refused):
        pin.opt("default<O2>", MODULE, "i386-pc-linux-gnu", bin_dir=tmp_path)


def test_an_incomplete_target_is_refused():
    pin = load()
    with pytest.raises(pin.Refused):
        pin.Target("i386-pc-linux-gnu", "i486", "", "-sse")
    with pytest.raises(pin.Refused):
        pin.target("m99")


def test_the_output_starts_with_the_target_line_and_the_module_carries_it(tmp_path):
    pin = load()
    fake(tmp_path, "opt")
    fake(tmp_path, "llc")
    of = pin.target("m32", optsize=True)
    done = pin.opt("default<O2>", MODULE, of, bin_dir=tmp_path)
    first = done.stdout.splitlines()[0]
    assert first.startswith("; target: triple=") and f"cpu={of.cpu}" in first and "features=-sse,-sse2,-mmx" in first and "optsize=true" in first
    assert f'target triple = "{of.triple}"' in done.stdout and f'target datalayout = "{of.datalayout}"' in done.stdout
    assert '"target-cpu"="i486"' in done.stdout and '"target-features"="-sse,-sse2,-mmx" optsize' in done.stdout and " #9999 {" in done.stdout
    llc = pin.llc(["-O2"], MODULE, of, bin_dir=tmp_path)
    assert f"-mtriple={of.triple} -mcpu={of.cpu} -mattr=-sse,-sse2,-mmx" in llc.stdout


def test_the_64_bit_target_has_the_sse_its_cpu_has():
    """m64 got the i486's `-sse,-sse2,-mmx`: the pin's llc did every double in x87 (nbody, stft_single -Os), 150+ bytes off the real column."""
    pin = load()
    assert "+sse2" in pin.target("m64").features and "-sse" not in pin.target("m64").features
    assert pin.target("m32").features == "-sse,-sse2,-mmx"


def test_a_module_that_names_its_own_layout_keeps_it():
    pin = load()
    text = 'target datalayout = "e-p:16:16"\n' + MODULE
    out = pin.retarget(text, pin.target("m32"))
    assert out.count("target datalayout") == 1 and 'target datalayout = "e-p:16:16"' in out


def test_llrms_own_attributes_are_written_in_the_dialect_the_pin_reads(tmp_path):
    """A dump's `threeway` reached the pin's llc as a bare word and llc refused the module (2026-10-25: every ambt pin column needed its own normalise)."""
    pin = load()
    fake(tmp_path, "llc")
    text = "define i32 @f(i32 %x) threeway {\n  ret i32 %x\n}\n"
    done = pin.llc(["-O2"], text, pin.target("m32"), bin_dir=tmp_path)
    assert '"threeway"' in done.stdout and "threeway {" not in done.stdout


def test_backend_ir_is_the_switchconv_stage_whatever_its_number(tmp_path):
    """pincount and ambt_pin read `30-switchconv.ll`; the pipeline numbered it `31-` and every pin-clock row died with FileNotFoundError."""
    (tmp_path / "07-lsr.ll").write_text("")
    (tmp_path / "31-switchconv.ll").write_text("")
    pin = load()
    assert pin.backend_ir(tmp_path) == tmp_path / "31-switchconv.ll"
    assert pin.backend_ir(tmp_path / "missing") is None


GOT_MODULE = "@g = global i32 0\ndeclare i32 @ext(i32)\ndefine i32 @f(i32 %x) {\n  ret i32 %x\n}\n"
LOCAL_MODULE = (
    "@g = dso_local global i32 0\n@s = internal constant i32 1\ndeclare dso_local i32 @ext(i32)\ndeclare i32 @llvm.abs.i32(i32, i1)\n"
    "define dso_local i32 @f(i32 %x) {\n  ret i32 %x\n}\ndefine internal fastcc i32 @h(i32 %x) {\n  ret i32 %x\n}\n"
)


def test_a_same_ir_pin_compare_at_x86_64_refuses_ir_without_dso_local(tmp_path):
    """2026-11-05: the pin's llc is PIC on clang IR and ours is not; bench_jpeg_fixed x86_64 read -276 B for the pin (`mov r,imm`
    against ours `lea r,[rip]`) where the like-for-like gap was +78 B. The row went to a card before it was caught."""
    pin = load()
    fake(tmp_path, "llc")
    with pytest.raises(pin.Refused, match="dso_local"):
        pin.llc(["-O2"], GOT_MODULE, pin.target("m64"), bin_dir=tmp_path)
    done = pin.llc(["-O2"], LOCAL_MODULE, pin.target("m64"), bin_dir=tmp_path)
    assert "define dso_local i32 @f" in done.stdout
    pin.llc(["-O2"], GOT_MODULE, pin.target("m32"), bin_dir=tmp_path)  # 32-bit flows are static: no guard


def test_the_symbols_missing_dso_local_are_named():
    spec = importlib.util.spec_from_file_location("irguard", HERE / "irguard.py")
    guard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guard)
    assert guard.missing_dso_local(GOT_MODULE) == ["g", "ext", "f"]
    assert guard.missing_dso_local(LOCAL_MODULE) == []
