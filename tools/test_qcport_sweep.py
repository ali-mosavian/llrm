"""The QCport sweep: a failure is grouped by its kind, only a kind no known entry covers is new, and the bisection finds the first bad squash."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import qcport_sweep as q  # noqa: E402

KNOWN = [{"match": "with no instruction choosing", "issue": 2454}]


def test_kinds_drop_the_ids_that_differ_per_module():
    """menu (block 139) and triggers (block 73) are one bug: their messages differ in function suffixes and ids only."""
    a = q.kind("llrm-c: @_menu_key@3: _menu_key@3: block 139 leaves for (133, 150, 156) with no instruction choosing")
    b = q.kind("llrm-c: @_ent_lightning@3: _ent_lightning@3: block 73 leaves for (20, 77, 75) with no instruction choosing\n")
    assert a == b and a.endswith("with no instruction choosing")
    assert q.kind("llrm-c: a jump to L1_125, which is nowhere") == q.kind("llrm-c: a jump to L2_7, which is nowhere") == "llrm-c: a jump to L, which is nowhere"


def test_only_a_kind_no_known_entry_covers_is_new():
    groups = q.grouped([("menu", "-O2", q.kind("x: block 1 leaves for (2) with no instruction choosing")), ("vid", "-Os", "llrm-c: a byte division")])
    assert list(q.new_kinds(groups, KNOWN)) == ["llrm-c: a byte division"]
    assert q.new_kinds(groups, KNOWN + [{"match": "a byte division", "issue": 2456}]) == {}


def test_the_first_bad_commit_of_a_single_flip_is_found_in_log_steps():
    commits = [f"c{i}" for i in range(100)]
    asked = []

    def bad(c):
        asked.append(c)
        return int(c[1:]) >= 37

    assert q.first_bad(commits, bad) == "c37" and len(asked) <= 7


def test_the_issue_names_the_repro_the_head_and_the_first_bad_squash():
    title, body = q.issue_text("llrm-c: a byte division", [("vid", "-O2"), ("vid", "-Os")], "a" * 40, ("b" * 40, "feat(x): y (#1)"))
    assert "vid -O2" in title and "aaaaaaaaa" in body and "-I $QCPORT" in body and "bbbbbbbbb feat(x): y (#1)" in body


def test_the_known_file_parses_and_names_issue_numbers():
    import tomllib

    assert all(isinstance(e["issue"], int) and e["match"] for e in tomllib.loads(q.KNOWN.read_text()).get("known", []))
