"""docs/working/above-main-rows.md: every row has an owner and an exit, and ids are unique."""

from __future__ import annotations

from pathlib import Path

TABLE = Path(__file__).resolve().parents[1] / "docs/working/above-main-rows.md"
COLUMNS = ["id", "program/module", "level", "metric", "main", "parity", "delta", "cause", "owner", "exit", "PR"]


def rows(text: str) -> list[dict[str, str]]:
    lines = [line for line in text.splitlines() if line.startswith("| ")]
    header = [cell.strip() for cell in lines[0].strip("|").split("|")]
    return [dict(zip(header, (cell.strip() for cell in line.strip("|").split("|")))) for line in lines[2:]]


def test_the_table_has_its_columns_and_rows():
    found = rows(TABLE.read_text())
    assert found and list(found[0]) == COLUMNS


def test_every_row_has_an_owner_and_an_exit():
    missing = [row["id"] for row in rows(TABLE.read_text()) if not row["owner"] or not row["exit"]]
    assert not missing, f"rows without an owner or an exit: {missing}"


def test_a_row_without_an_owner_or_an_exit_is_refused():
    text = TABLE.read_text() + "| AM-999 | x | O2 | clocks | 1 | 2 | +1 | c |  |  |  |\n"
    refused = [row["id"] for row in rows(text) if not row["owner"] or not row["exit"]]
    assert refused == ["AM-999"]


def test_row_ids_are_unique():
    ids = [row["id"] for row in rows(TABLE.read_text())]
    assert len(ids) == len(set(ids))
