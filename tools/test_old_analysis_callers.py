"""The old-analysis callers count: a fixture tree with a known number of callers, and the ones that must not count."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import old_analysis_callers as oac  # noqa: E402

ENTRIES = [
    {"name": "Sums", "ask": "Sums", "defined_in": ["crates/opt/a/src/manager.rs"]},
    {"name": "Stamp", "use": "Stamp", "scope": ["crates/opt/"]},
    {"name": "ranges", "module": "ranges"},
]


def tree(root: Path, files: dict[str, str]) -> None:
    for name, text in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)


def test_only_code_callers_in_scope_are_counted(tmp_path):
    """The count once included comments, imports, tests and a second `Stamp` of another crate: a number that never fell."""
    tree(
        tmp_path,
        {
            "crates/opt/a/src/manager.rs": "fn own() { x.get::<Sums>(); }\n",
            "crates/opt/a/src/gvn.rs": "use m::Sums;\n// get::<Sums>\nfn f() {\n    x.get::<Sums>();\n    y.require::<m::Sums>();\n}\n#[cfg(test)]\nmod tests { fn t() { x.get::<Sums>(); } }\n",
            "crates/opt/a/src/gvn_tests.rs": "fn t() { x.get::<Sums>(); }\n",
            "crates/opt/a/src/tests.rs": "fn t() { x.get::<Sums>(); }\n",
            "crates/opt/a/src/licm.rs": "fn f() { let s = Stamp; ranges::bounded(); }\n",
            "crates/backend/b/src/facts.rs": "struct Stamp(u8);\nfn g() { ranges::scoped(); }\n",
        },
    )
    found = oac.callers(tmp_path, ENTRIES)
    assert found["Sums"] == {"crates/opt/a/src/gvn.rs": [4, 5]}
    assert found["Stamp"] == {"crates/opt/a/src/licm.rs": [1]}
    assert found["ranges"] == {"crates/opt/a/src/licm.rs": [1], "crates/backend/b/src/facts.rs": [2]}


def test_the_last_line_is_the_one_number(tmp_path):
    tree(tmp_path, {"crates/opt/a/src/gvn.rs": "fn f() { x.get::<Sums>(); y.get::<Sums>(); }\n"})
    lines = oac.report(oac.callers(tmp_path, ENTRIES))
    assert lines[-1] == "old-analysis callers under parity: 1; analyses with old callers: 1"
    assert "  crates/opt/a/src/gvn.rs: 1" in lines


def test_the_analysis_that_moves_last_is_split_out(tmp_path):
    """MemRef was 222 of 315 callers: one number hid the other 93, and the trend with it."""
    tree(tmp_path, {"crates/opt/a/src/gvn.rs": "fn f() {\n    x.get::<Sums>();\n    ranges::a();\n    ranges::b();\n}\n"})
    entries = [ENTRIES[0], {"name": "ranges", "module": "ranges", "moves_last": True}]
    lines = oac.report(oac.callers(tmp_path, entries), frozenset({"ranges"}))
    assert lines[-1] == "old-analysis callers under parity: 3 (ranges 2; without it 1); analyses with old callers: 2 (1 without ranges)"


def test_the_old_rows_of_the_registry_are_the_analyses(tmp_path):
    """Seeded by hand the list had 9 names and missed 28 of the registry's 37 Old rows: a count that could only be low."""
    tree(
        tmp_path,
        {
            oac.REGISTRY: 'const A: &[AnalysisEntry] = &[\n    AnalysisEntry { key: "loops", symbol: "Loops", kind: Translated },\n'
            '    old_analysis(\n        "sums",\n        "Sums",\n        "none",\n        "x",\n    ),\n'
            '    old_analysis("callees", "Callees", "none", "y"),\n];\n',
            "crates/opt/a/src/manager.rs": "pub struct Sums;\nimpl Analysis for Sums {}\nfn own() { analyses.get::<Sums>(); }\n",
            "crates/opt/a/src/gvn.rs": "fn f() { x.get::<Sums>(); x.get::<Loops>(); }\n",
        },
    )
    entries = oac.registry_entries(tmp_path)
    assert [entry["name"] for entry in entries] == ["Sums"]
    found = oac.callers(tmp_path, entries)
    assert found["Sums"] == {"crates/opt/a/src/manager.rs": [3], "crates/opt/a/src/gvn.rs": [1]}
