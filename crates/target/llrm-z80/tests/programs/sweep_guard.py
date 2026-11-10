"""The row-count guard of the size/clock sweep: a program that produced no `ours` row is named and fails the run.

The sweep once printed `112 programs 0 failed` with 3 rows missing (2026-11-09): a program whose build or link
never reached a result left no FAILED row either, so the tally counted only the rows it was shown.
"""


def dropped(expected, results, required=lambda tag: tag == "ours", excused=()):
    """`{program: reason}` for every program of `expected` without a row whose column `required` accepts (`ours`).

    `excused`: programs whose missing row has a recorded reason (an SDCC column the compiler rejects is a gap
    in the bar, listed, not a dropped row). `results`: `(name, [(tag, bytes, clocks), ...])` as the sweep collects them. A program the pool never
    returned and one that returned no `ours` row are both dropped; `reasons` of the sweep refine the message.
    """
    have = {name for name, rows in results if any(required(tag) for tag, _, _ in rows)}
    return {name: "NOROW" for name in expected if name not in have and name not in excused}


def summary(expected, results, reasons=None, required=lambda tag: tag == "ours", excused=()):
    """`(line, ok)`: the sweep's last line and whether it passed. The line names the dropped programs."""
    reasons = reasons or {}
    lost = dropped(expected, results, required, excused)
    failed = {name for name, rows in results for tag, _, _ in rows if tag == "FAILED"} | set(lost)
    have = {name for name, rows in results if any(required(tag) for tag, _, _ in rows)}
    line = f"{len(expected)} programs {len(failed)} failed; rows {len(have & set(expected))} of {len(expected)}"
    if failed:
        line += "; no row: " + " ".join(f"{n}({reasons.get(n, lost.get(n, 'FAILED'))})" for n in sorted(failed))
    return line, not failed
