"""aliascompare.py reads aa-eval's lines the way llrm prints its own, and says so where it cannot run."""
import importlib.util
import subprocess
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("aliascompare", HERE / "aliascompare.py")
aliascompare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(aliascompare)


def test_an_aa_eval_pair_line_is_read_with_its_function_and_both_names():
    """Typed and opaque pointers, numbered, global and quoted names: two names, whatever the types around them."""
    lines = {
        "  NoAlias:\tptr* %1, i16* %3": ("NoAlias", ["%1", "%3"]),
        "  MayAlias:\ti32* %14, i32* %19": ("MayAlias", ["%14", "%19"]),
        '  MustAlias:\tptr %a, ptr %"b c"': ("MustAlias", ["%a", '%"b c"']),
        "  NoAlias:\ti32* %14, i16* @_fp_cnt": ("NoAlias", ["%14", "@_fp_cnt"]),
    }
    for line, (kind, names) in lines.items():
        pair = aliascompare.PAIR.match(line)
        assert pair and pair[1] == kind, line
        assert aliascompare.NAME.findall(pair[2]) == names, line
    assert aliascompare.OPERAND.findall("ptr* %1, i16* %3") == [("ptr", "%1"), ("i16", "%3")]
    assert aliascompare.entry('%"b c"', "i32") == ("%b c", 4)
    assert aliascompare.FUNCTION.match("Function: pipeline.body: 13 pointers, 0 call sites")[1] == "pipeline.body"


def test_without_the_difftest_normaliser_it_stops_and_says_why(monkeypatch, tmp_path):
    """The normaliser lives with difftest (#2105); a run that cannot find it must not read as 'no difference'."""
    monkeypatch.delenv("DIFFTEST_DIR", raising=False)
    monkeypatch.setattr(aliascompare, "HERE", tmp_path)
    with pytest.raises(SystemExit, match="DIFFTEST_DIR"):
        aliascompare.difftest()


def test_a_set_the_reference_only_ever_calls_may_alias_is_refused():
    """An analysis that answers one way agrees with anything: a table over such a set is no evidence."""
    import collections

    only_may = collections.Counter({("MayAlias", "MayAlias"): 10, ("NoAlias", "MayAlias"): 3})
    assert "MustAlias, NoAlias" in aliascompare.premise(only_may)
    varied = collections.Counter({("MayAlias", "MayAlias"): 10, ("NoAlias", "NoAlias"): 3, ("MayAlias", "MustAlias"): 1})
    assert aliascompare.premise(varied) is None


def slot_with_loads(loads: int) -> str:
    reads = "".join(f"  %w{n} = load ptr, ptr %slot\n" for n in range(loads))
    return (
        f'target datalayout = "{aliascompare.M32_DATALAYOUT}"\n'
        "define i32 @f(ptr %p, i32 %i) {\nentry:\n  %slot = alloca ptr\n  store ptr %p, ptr %slot\n  %q = load ptr, ptr %slot\n"
        f"  %a = getelementptr inbounds i8, ptr %q, i32 %i\n  store i32 0, ptr %a\n{reads}  ret i32 0\n}}\n"
    )


def test_capture_tracking_gives_up_after_150_uses_in_the_pinned_opt():
    """The cap is 150, not the 100 the plan page said (#2330): 32 pairs of d_faces-O1 differed. The slot has two uses beside its loads."""
    try:
        dt = aliascompare.difftest()
    except SystemExit:
        pytest.skip("the difftest normaliser is not here")
    opt = dt.LLVM_BIN / "opt"
    if not opt.exists():
        pytest.skip("the pinned opt is not built here")
    version = subprocess.run([str(opt), "--version"], capture_output=True, text=True).stdout
    assert "24.0.0git" in version, f"the premise is the pinned opt, this is: {version!r}"
    pair = lambda loads: next(
        v for (_, names), v in aliascompare.llvm_answers(dt, slot_with_loads(loads), "basic").items() if {n for n, _ in names} == {"%a", "%slot"}
    )
    assert pair(148) == "NoAlias" and pair(149) == "MayAlias"


def test_memops_prints_must_alias_for_a_load_and_a_store_of_one_pointer(tmp_path):
    """--memops with BasicAA collapsed MustAlias to MayAlias: 5,657,881 'May vs LLVM Must' cells that were the instrument, not the analysis."""
    import os
    import subprocess

    binary = os.environ.get("LLRM_MIR")
    if not binary or not Path(binary).exists():
        pytest.skip("set LLRM_MIR to a built llrm-mir")
    path = tmp_path / "same.ll"
    path.write_text(
        f'target datalayout = "{aliascompare.M32_DATALAYOUT}"\n'
        "define i32 @f(ptr %p) {\nentry:\n  store i32 1, ptr %p\n  %v = load i32, ptr %p\n  ret i32 %v\n}\n"
    )
    out = subprocess.run([binary, "--memops", str(path)], capture_output=True, text=True, env={**os.environ, "LLRM_ALIASES_BASICAA": "1"}).stdout
    assert "MustAlias" in out, out
