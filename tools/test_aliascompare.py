"""aliascompare.py reads aa-eval's lines the way llrm prints its own, and says so where it cannot run."""
import importlib.util
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
    assert aliascompare.FUNCTION.match("Function: pipeline.body: 13 pointers, 0 call sites")[1] == "pipeline.body"


def test_without_the_difftest_normaliser_it_stops_and_says_why(monkeypatch, tmp_path):
    """The normaliser lives with difftest (#2105); a run that cannot find it must not read as 'no difference'."""
    monkeypatch.delenv("DIFFTEST_DIR", raising=False)
    monkeypatch.setattr(aliascompare, "HERE", tmp_path)
    with pytest.raises(SystemExit, match="DIFFTEST_DIR"):
        aliascompare.difftest()
