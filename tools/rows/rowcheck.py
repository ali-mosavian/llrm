"""The census every rows summary prints first, and the refusal when a row failed or is missing.

A summary built from the rows that have counters leaves out the rows that failed: 33 wrong-output rows under LLRM_GREEDY=1 never reached a table of
worse counts and geomeans. A row that exists in either run and has no counters makes the summary refuse, unless `--allow-failed` names it, and the
failed rows are listed by name.
"""

from __future__ import annotations

COUNTERS = ("ins", "mem", "clocks", "code")


def failed(row: dict, counters=COUNTERS) -> bool:
    """A row that did not run to a result: marked not ok, carrying an error, or without a counter."""
    return row.get("ok") is False or "error" in row or any(c not in row for c in counters)


def census(sides: dict[str, dict], counters=COUNTERS) -> tuple[str, list[str]]:
    """(the line to print first, the names of failed or missing rows). `sides` maps a side's name to {row name: row}."""
    names = sorted(set().union(*(set(rows) for rows in sides.values())))
    bad = []
    for name in names:
        for side, rows in sides.items():
            if name not in rows:
                bad.append(f"{name} (missing in {side})")
            elif failed(rows[name], counters):
                bad.append(f"{name} (failed in {side}: {rows[name].get('error') or 'no result'})")
    wanted = len(names) * len(sides)
    failures = sum("failed in" in one for one in bad)
    missing = sum("missing in" in one for one in bad)
    line = f"rows total {len(names)} / with counters {wanted - failures - missing} of {wanted} / failed {failures} / missing {missing}"
    return line, bad


def require(line: str, bad: list[str], allow_failed: bool) -> list[str]:
    """The lines to print: the census, and the failed rows when allowed. SystemExit naming them when not."""
    n = sum("failed in" in one for one in bad)
    lead = f"{n} rows failed, {len(bad) - n} missing\n" if bad else ""
    if bad and not allow_failed:
        raise SystemExit(lead + line + "\nrefusing a geomean or a worse-count over a table with failed or missing rows (--allow-failed prints it anyway):\n  " + "\n  ".join(bad))
    return ([lead.rstrip()] if bad else []) + [line] + [f"allowed: {one}" for one in bad]
