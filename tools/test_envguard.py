import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import abmeasure  # noqa: E402
import envguard  # noqa: E402


def test_a_check_or_experiment_switch_nobody_named_is_refused_by_name():
    """llvm-order-poc blamed the callee-first order for a failure that came from LLRM_CHECK_* on in one run only."""
    with pytest.raises(SystemExit, match="LLRM_CHECK_ORDER=1"):
        envguard.require_clean("abmeasure", {"LLRM_CHECK_ORDER": "1", "PATH": "/bin"})
    with pytest.raises(SystemExit, match="LLRM_EXP_LLVM=1"):
        envguard.require_clean("difftest", {"LLRM_EXP_LLVM": "1"})


def test_the_switch_under_test_is_allowed_when_named_and_other_variables_are_not_switches():
    envguard.require_clean("abmeasure", {"LLRM_EXP_LLVM": "1", "LLRM_BIN": "/b", "LLRM_DEBUG": "time"}, allow=["LLRM_EXP_LLVM"])
    assert envguard.stray({"LLRM_EXP_LLVM": "1", "LLRM_CHECK_FACTS": "1"}, allow=["LLRM_EXP_LLVM"]) == ["LLRM_CHECK_FACTS=1"]


def test_abmeasure_refuses_a_pair_before_it_builds_anything(monkeypatch):
    monkeypatch.setenv("LLRM_CHECK_ORDER", "1")
    monkeypatch.setattr(abmeasure, "build", lambda *a, **k: (_ for _ in ()).throw(AssertionError("built")))
    monkeypatch.setattr(sys, "argv", ["abmeasure.py", "HEAD^", "HEAD"])
    with pytest.raises(SystemExit, match="LLRM_CHECK_ORDER=1"):
        abmeasure.main()
    monkeypatch.setattr(sys, "argv", ["abmeasure.py", "HEAD^", "HEAD", "--allow-env", "LLRM_CHECK_ORDER"])
    with pytest.raises(AssertionError, match="built"):
        abmeasure.main()


def test_the_off_switch_of_the_llvm_order_is_refused_by_name():
    """The order is on by default; a run with LLRM_NO_LLVM_ORDER in one side only compared two pipelines, not two commits."""
    with pytest.raises(SystemExit, match="LLRM_NO_LLVM_ORDER=1"):
        envguard.require_clean("abmeasure", {"LLRM_NO_LLVM_ORDER": "1", "PATH": "/bin"})
