"""Knowingly missing checks are debt lines in docs/working/ci.md, and a passed deadline is red.

Gate run 2 found skips nobody had written down; a note in prose has no date, so nothing ever came due. Format (one line each):
`- DEBT <id> | limit <text> | deadline YYYY-MM-DD | owner <name> | <what and why>`"""

import re
from datetime import date
from pathlib import Path

DOC = Path(__file__).resolve().parents[2] / "docs/working/ci.md"
LINE = re.compile(r"^- DEBT (?P<id>[a-z0-9][a-z0-9-]*) \| limit (?P<limit>[^|]+) \| deadline (?P<deadline>\d{4}-\d{2}-\d{2}) \| owner (?P<owner>[^|\s][^|]*) \| (?P<what>\S.*)$")


def problems(text: str, today: date) -> list[str]:
    """One message per debt line that is malformed, repeats an id or is past its deadline."""
    out, seen = [], set()
    for line in text.splitlines():
        if not line.startswith("- DEBT"):
            continue
        m = LINE.match(line)
        if not m:
            out.append(f"malformed: {line[:80]}")
            continue
        try:
            due = date.fromisoformat(m["deadline"])
        except ValueError:
            out.append(f"{m['id']}: {m['deadline']} is not a date")
            continue
        if m["id"] in seen:
            out.append(f"{m['id']}: repeated")
        seen.add(m["id"])
        if due < today:
            out.append(f"{m['id']}: deadline {m['deadline']} has passed (owner {m['owner'].strip()}): pay it or move the date and say why")
    return out


def test_no_debt_line_in_the_ci_doc_is_malformed_or_overdue():
    assert problems(DOC.read_text(), date.today()) == []


def test_the_doc_lists_debt_lines():
    """A rename of the section or of the marker would make the check above pass on nothing."""
    assert any(line.startswith("- DEBT ") for line in DOC.read_text().splitlines())  # any row: a paid row must not fail this


def test_a_past_dated_row_is_red():
    row = "- DEBT fake-old | limit 1 test | deadline 2026-01-01 | owner w0 | a row whose date has gone by"
    assert problems(row, date(2026, 10, 29)) == ["fake-old: deadline 2026-01-01 has passed (owner w0): pay it or move the date and say why"]


def test_a_malformed_row_is_red():
    for row in ("- DEBT x | deadline 2026-12-01 | owner w0 | no limit", "- DEBT x | limit 1 | deadline 2026-13-45 | owner w0 | bad date", "- DEBT x | limit 1 | deadline 2026-12-01 | owner w0 |"):
        assert problems(row, date(2026, 10, 29)), row


def test_a_repeated_id_is_red():
    row = "- DEBT x | limit 1 | deadline 2026-12-01 | owner w0 | a\n- DEBT x | limit 1 | deadline 2026-12-01 | owner w0 | b"
    assert problems(row, date(2026, 10, 29)) == ["x: repeated"]


RATCHET = Path(__file__).with_name("debt_ratchet.toml")
NUMBER = re.compile(r"(?<![A-Za-z0-9#.-])\d+(?![A-Za-z0-9.-])")  # a bare integer: not -O2, bq16, #8191, 0.4 or a date


def limit_numbers(limit: str) -> list[int]:
    return [int(n) for n in NUMBER.findall(limit)]


def ratchet_problems(text: str, ratchet: dict) -> list[str]:
    """A limit only goes down, and the accepted numbers of every row that has a number are in the toml.

    Red: a row with numbers that the toml lacks, a toml id with no row, a different count of numbers, a number above the accepted one, a number
    below it (lower the toml in the same PR, or the floor is loose and the limit can climb back)."""
    out, seen = [], set()
    for line in text.splitlines():
        m = LINE.match(line) if line.startswith("- DEBT") else None
        if not m:
            continue
        seen.add(m["id"])
        numbers, floor = limit_numbers(m["limit"]), ratchet.get(m["id"])
        if floor is None:
            if numbers:
                out.append(f"{m['id']}: limit numbers {numbers} are not in debt_ratchet.toml")
        elif len(numbers) != len(floor):
            out.append(f"{m['id']}: {len(numbers)} limit numbers {numbers}, the toml has {len(floor)} {floor}")
        elif any(a > b for a, b in zip(numbers, floor)):
            out.append(f"{m['id']}: limit numbers {numbers} are above the accepted {floor}")
        elif numbers != floor:
            out.append(f"{m['id']}: limit numbers {numbers} are below the accepted {floor}: lower debt_ratchet.toml")
    out += [f"{name}: in debt_ratchet.toml, no such debt row" for name in ratchet if name not in seen]
    return out


def test_every_debt_limit_is_in_the_ratchet_and_none_went_up():
    import tomllib

    assert ratchet_problems(DOC.read_text(), tomllib.loads(RATCHET.read_text())) == []


ROW = "- DEBT lru | limit lru c -Os 382 B / 129 insns, bq32 -Os 515 B (pin 342) | deadline 2026-12-01 | owner m16 | why"
FLOOR = {"lru": [382, 129, 515, 342]}


def test_a_raised_limit_is_red():
    """lru-os-m16-bytes carried 402 B for days after the code measured 382: a limit nobody lowers is a waiver. Digits inside -Os, bq32 and dates are not limits."""
    assert ratchet_problems(ROW, FLOOR) == []
    assert ratchet_problems(ROW.replace("382 B", "383 B"), FLOOR) != []


def test_a_lowered_limit_with_an_unlowered_floor_is_red():
    assert ratchet_problems(ROW.replace("382 B", "381 B"), FLOOR) != []


def test_a_row_with_numbers_missing_from_the_toml_is_red():
    """A row left out of the toml was never checked: any new debt with a number must be listed."""
    assert ratchet_problems(ROW, {}) == ["lru: limit numbers [382, 129, 515, 342] are not in debt_ratchet.toml"]
    assert ratchet_problems("- DEBT x | limit the current listing | deadline 2026-12-01 | owner m16 | why", {}) == []


def test_a_toml_row_with_no_debt_row_is_red():
    assert ratchet_problems(ROW, {**FLOOR, "gone": [1]}) == ["gone: in debt_ratchet.toml, no such debt row"]
