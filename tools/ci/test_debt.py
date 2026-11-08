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
NUMBER = re.compile(r"(?<![\w#-])(\d+)(?!\w)")


def numbers_of(limit: str) -> list[int]:
    """The standalone numbers of a limit text (not those in names like bq32 or -O2, nor issue numbers)."""
    return [int(n) for n in NUMBER.findall(limit.split("(")[0] if "waivers/" in limit else limit)]


def ratchet_problems(text: str, ratchet: dict) -> list[str]:
    """One message per debt row with a numeric limit that is missing from the ratchet or not at or below the accepted numbers."""
    out = []
    for line in text.splitlines():
        m = LINE.match(line) if line.startswith("- DEBT") else None
        if not m:
            continue
        numbers = numbers_of(m["limit"])
        if not numbers:
            continue
        if m["id"] not in ratchet:
            out.append(f"{m['id']}: numeric limit {numbers} is not in debt_ratchet.toml")
            continue
        floor = ratchet[m["id"]]
        if len(numbers) != len(floor) or any(a > b for a, b in zip(numbers, floor)):
            out.append(f"{m['id']}: limit numbers {numbers} are not at or below the accepted {floor}")
    return out


def test_no_debt_limit_goes_up_and_every_numeric_limit_is_in_the_ratchet():
    import tomllib

    assert ratchet_problems(DOC.read_text(), tomllib.loads(RATCHET.read_text())) == []


def test_a_raised_limit_is_red():
    """lru-os-m16-bytes carried 402 B for days after the code measured 382: a limit nobody lowers is a waiver."""
    row = "- DEBT lru-os-m16-bytes | limit lru c -Os 402 B / 129 insns / 57 memops, lru bq32 -Os 515 B, lru nib -Os 147 memops (x) | deadline 2026-12-01 | owner m16 | w"
    floor = {"lru-os-m16-bytes": [382, 129, 57, 515, 147]}
    assert ratchet_problems(row, floor) != []
    assert ratchet_problems(row.replace("402", "381"), floor) == []


def test_a_numeric_limit_missing_from_the_ratchet_is_red():
    row = "- DEBT new-row | limit 7 tests | deadline 2026-12-01 | owner m16 | w"
    assert ratchet_problems(row, {}) == ["new-row: numeric limit [7] is not in debt_ratchet.toml"]
    assert ratchet_problems(row, {"new-row": [7]}) == []
    assert ratchet_problems("- DEBT prose | limit the whole listing | deadline 2026-12-01 | owner m16 | w", {}) == []
