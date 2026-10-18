"""A merge needs the candidate perf run of the head, on a main that has not moved past docs: the loop's check, mirrored here."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import candidate_ok  # noqa: E402

HEAD, MAIN, NEWMAIN = "a" * 40, "b" * 40, "c" * 40
INERT = ["docs/**", "**/*.md", ".github/**"]
CAND = f"GATE full CANDIDATE PASS at {HEAD[:12]} on {MAIN[:12]}"


def ok(line, head=HEAD, main=MAIN, changed=()):
    return candidate_ok.check(line, head, main, list(changed), INERT)[0]


def test_a_candidate_pass_of_the_head_on_the_current_main_lets_a_merge():
    assert ok(CAND)


def test_a_line_that_is_not_a_candidate_pass_cannot_merge():
    """A main push's PASS names no pull request's head; a FAIL names none that merges."""
    assert not ok(f"GATE full PASS at {HEAD[:12]} on {MAIN[:12]}")
    assert not ok(f"GATE full FAIL at {HEAD[:12]}: plan=success gate=failure")


def test_a_candidate_pass_of_another_head_cannot_merge():
    """The label was on, a push followed, and the run in flight finished for the older head."""
    assert not ok(CAND, head="d" * 40)


def test_a_candidate_pass_on_a_main_that_moved_by_code_cannot_merge():
    assert not ok(CAND, main=NEWMAIN, changed=["crates/opt/llrm-transforms/src/gvn.rs"])


def test_a_main_that_moved_by_docs_only_still_lets_a_merge():
    assert ok(CAND, main=NEWMAIN, changed=["docs/working/ci.md", "README.md", ".github/workflows/ci.yml"])


def test_the_command_line_exits_1_for_a_stale_head(capsys, monkeypatch):
    monkeypatch.setattr(sys, "argv", ["candidate_ok.py", "d" * 40, MAIN, CAND])
    assert candidate_ok.main() == 1
    assert "push after the label" in capsys.readouterr().out
