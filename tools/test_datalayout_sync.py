"""`uv run --project tools python -m pytest tools/test_datalayout_sync.py`

A target's datalayout is stated once, in its `datalayout.toml`; the copies elsewhere follow it. Adding `f80:16` to the m16 string took a
sed over 64 files because nothing named them.
"""
import datalayout_sync as sync

M16 = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32"
OLD = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"


def test_a_copy_of_an_older_spelling_is_rewritten_from_the_description():
    text = f'target datalayout = "{OLD}"\nlet x = "{M16}";\n'
    assert sync.rewritten(text, [M16]) == f'target datalayout = "{M16}"\nlet x = "{M16}";\n'


def test_an_ni_suffix_is_kept_and_an_abbreviated_string_is_left_alone():
    assert sync.rewritten(f'"{OLD}-ni:2:3"', [M16]) == f'"{M16}-ni:2:3"'
    short = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"
    assert sync.rewritten(f'"{short}"', [M16]) == f'"{short}"'


def test_every_copy_in_the_tree_matches_its_description():
    assert sync.stale() == [], "run tools/datalayout_sync.py --sync"
