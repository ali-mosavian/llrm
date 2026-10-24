"""select_shadow.py's buckets: every defined function lands in exactly one."""

import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import select_shadow as tool  # noqa: E402

IR = "define i32 @a() {\n ret i32 0\n}\ndefine i32 @b() {\n ret i32 0\n}\n"


def func(unselected=None):
    return {"blocks": 1, "built": 1, "machine": Counter({"RET": 1}), "unselected": Counter(unselected or {})}


def test_m16_has_no_oracle_so_a_clean_function_selects():
    rows = tool.rows_of("p", {"a": func(), "b": func({"GlobalAddress": 1})}, {}, None, IR)
    assert {f: b for _, f, b in rows} == {"a": "selects", "b": "unselected GlobalAddress"}


def test_a_function_missing_from_the_output_is_still_a_row():
    rows = tool.rows_of("p", {"a": func()}, {}, None, IR)
    assert len(rows) == 2


def test_a_quoted_function_name_is_the_name_the_shadow_prints():
    """m16 names like `_f@3` are quoted in the IR; the quotes made 87 functions "missing from the output"."""
    assert tool.defined('define i16 @"_f@3"(i16 %0) {\n ret i16 0\n}\n') == ["_f@3"]
