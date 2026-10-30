"""The census every rows summary prints first, and the refusal when a row failed or is missing.

A summary built from the rows that have counters leaves out the rows that failed: 33 wrong-output rows under LLRM_GREEDY=1 never reached a table of
worse counts and geomeans. A row that exists in either run and has no counters makes the summary refuse, unless `--allow-failed` names it, and the
failed rows are listed by name.
"""

from __future__ import annotations

COUNTERS = ("ins", "mem", "clocks", "code")
PINNED_CLANG = "d1106deb71cc"  # the reference clang (tools/llvmpin.py COMMIT): its objects' .comment names it


def failed(row: dict, counters=COUNTERS, checked: bool = False) -> bool:
    """A row that did not run to a result: marked not ok, carrying an error, or without a counter. With `checked` a row with no instruction
    count must say its output was compared (`checked: true`): sized only, it was never run."""
    if checked and "instructions" not in row and not row.get("checked") and "error" not in row:
        return True
    return row.get("ok") is False or "error" in row or any(c not in row for c in counters)


def census(sides: dict[str, dict], counters=COUNTERS, checked: bool = False) -> tuple[str, list[str]]:
    """(the line to print first, the names of failed or missing rows). `sides` maps a side's name to {row name: row}."""
    names = sorted(set().union(*(set(rows) for rows in sides.values())))
    bad = []
    for name in names:
        for side, rows in sides.items():
            if name not in rows:
                bad.append(f"{name} (missing in {side})")
            elif str(rows[name].get("error", "")).startswith("unmeasured:"):
                bad.append(f"{name} (unmeasured in {side}: {rows[name]['error'][len('unmeasured: '):]})")
            elif failed(rows[name], counters, checked):
                bad.append(f"{name} (failed in {side}: {rows[name].get('error') or ('wrong output: got ' + str(rows[name].get('got')) if rows[name].get('ok') is False else 'unchecked: ' + rows[name]['unchecked'] if 'unchecked' in rows[name] else 'sized only, never run')})")
    wanted = len(names) * len(sides)
    failures = sum("failed in" in one for one in bad)
    missing = sum("missing in" in one or "unmeasured in" in one for one in bad)
    line = f"rows total {len(names)} / with counters {wanted - failures - missing} of {wanted} / failed {failures} / missing {missing}"
    return line, bad


def require(line: str, bad: list[str], allow_failed: bool) -> list[str]:
    """The lines to print: the census, and the failed rows when allowed. SystemExit naming them when not."""
    n = sum("failed in" in one for one in bad)  # a row that was not measured is missing, not failed
    lead = f"{n} rows failed, {len(bad) - n} missing\n" if bad else ""
    if bad and not allow_failed:
        raise SystemExit(lead + line + "\nrefusing a geomean or a worse-count over a table with failed or missing rows (--allow-failed prints it anyway):\n  " + "\n  ".join(bad))
    return ([lead.rstrip()] if bad else []) + [line] + [f"allowed: {one}" for one in bad]


def clang_comment(path) -> str:
    """The `.comment` section of a clang-built object (the compiler's version string), "" where there is none."""
    import subprocess

    done = subprocess.run(["readelf", "-p", ".comment", str(path)], capture_output=True, text=True)
    return " ".join(line.split("]", 1)[-1].strip() for line in done.stdout.splitlines() if "]" in line)


def compiler_premise(directory) -> str:
    """The rows' reference column is clang's: every `*.clang*.o` under `directory` must name the pinned build in its `.comment`. SystemExit naming the
    objects that do not (Ubuntu clang 20.1.8 built rows/main for days: its numbers were never a reference); a line saying what was checked."""
    from pathlib import Path

    objects = sorted(Path(directory).rglob("*.clang*.o"))
    wrong = [(one.name, clang_comment(one)) for one in objects if PINNED_CLANG not in clang_comment(one)]
    if wrong:
        shown = "; ".join(f"{name}: {comment or 'no .comment'}" for name, comment in wrong[:5])
        raise SystemExit(f"{len(wrong)} of {len(objects)} clang objects under {directory} were not built by the pinned clang ({PINNED_CLANG}): {shown}")
    return f"clang objects {len(objects)}, all built by the pinned clang ({PINNED_CLANG})" if objects else f"no clang objects under {directory}: compiler premise unchecked"
