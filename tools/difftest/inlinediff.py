#!/usr/bin/env python3
"""Which call sites llrm inlines and LLVM's inliner inlines, on the same module, with the cost LLVM saw.

    tools/difftest/inlinediff.py BEFORE.ll REMARKS [--pipeline inline|default<O2>] [--threshold N]
    tools/difftest/inlinediff.py dumps DIR... [--pipeline P] [--threshold N]     `llrm-c --dump DIR` with its stderr in DIR/remarks.txt

BEFORE is the module before llrm's inline stage (in a dump: the second of the three NN-pipeline.ll) and REMARKS llrm's stderr under
`LLRM_EXP_INLINEDECIDE=1 LLRM_INLINE_REMARKS=1`. Both sides are read from their remarks ('callee' inlined into 'caller' with (cost=C,
threshold=T), and every refusal with its reason): counting the calls left after the stage counted every call of a caller that was itself
inlined and deleted as inlined. A site is a (caller, callee) pair counted by its occurrences, including those in the copies either inliner made.
Each pair's sites are `agree` (both inlined or both kept), `llrm-only` or `llvm-only`. The columns: LLVM's cost, threshold and reason, the
callee's and caller's instructions before, whether the callee is called once.
"""

from __future__ import annotations

import re
import sys
import json
import argparse
import subprocess
import collections
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import difftest  # noqa: E402

CALL = re.compile(r'\b(?:call|invoke)\b[^@\n]*@(?:"([^"]+)"|([\w.$-]+))\(')
INLINED = re.compile(r"'(?P<callee>[^']+)' inlined into '(?P<caller>[^']+)' with \(cost=(?P<cost>-?\d+)(?:, [^)]*?)?, threshold=(?P<threshold>-?\d+)\)")
MISSED = re.compile(r"'(?P<callee>[^']+)' not inlined into '(?P<caller>[^']+)' because (?P<reason>[^(\n]*?)(?: \(cost=(?P<cost>-?\d+|never)(?:, threshold=(?P<threshold>-?\d+))?(?:[,:][^)]*?)?\))?\s*$", re.M)


def premise(inlined: int, sites: int, differing: int) -> str:
    """`difftest.premise` for sites, and failed when LLVM inlined none: a reference that never acts says nothing even where llrm disagrees with it
    (an `internal` caller nothing calls is dead to `opt` and never visited: `idioms.bas` read "ok" with LLVM at 0 of 123 sites and llrm at 6)."""
    if sites and inlined == 0:
        return f"PREMISE FAILED: LLVM inlined none of {sites} sites: the reference never acted (is a caller dead and internal? make the roots external)"
    return difftest.premise(inlined, sites, differing).replace("functions", "sites").replace("changed", "inlined")


def sites(text: str) -> collections.Counter:
    """(caller, callee) -> how many call sites the module has."""
    found: collections.Counter = collections.Counter()
    for caller, lines in difftest.functions(text).items():
        for line in lines[1:]:
            found.update((caller, m[1] or m[2]) for m in CALL.finditer(line.split(";", 1)[0]))
    return found


def sizes(text: str) -> dict[str, int]:
    """Function -> its instructions."""
    return {name: sum(1 for line in lines if difftest.opcode(line)) for name, lines in difftest.functions(text).items()}


def verdicts(remarks: str) -> tuple[dict, dict]:
    """(inlined, refused) of `-pass-remarks=inline` text, opt's or llrm's (`LLRM_INLINE_REMARKS`): (caller, callee) -> [(cost, threshold)] and
    -> [(reason, cost, threshold)]."""
    inlined: dict = collections.defaultdict(list)
    refused: dict = collections.defaultdict(list)
    for m in INLINED.finditer(remarks):
        inlined[(m["caller"], m["callee"])].append((int(m["cost"]), int(m["threshold"])))
    for m in MISSED.finditer(remarks):
        refused[(m["caller"], m["callee"])].append((m["reason"].strip(), m["cost"], m["threshold"]))
    return inlined, refused


def llvm_verdicts(before: str, pipeline: str, threshold: int | None) -> tuple[dict, dict, str]:
    """(inlined, refused, the module LLVM wrote): (caller, callee) -> [(cost, threshold)] and -> [(reason, cost, threshold)]."""
    passes = f"-passes={pipeline}"
    flags = ["-pass-remarks=inline", "-pass-remarks-missed=inline", "-pass-remarks-analysis=inline", *([f"-inline-threshold={threshold}"] if threshold is not None else [])]
    done = subprocess.run([str(difftest.LLVM_BIN / "opt"), *difftest.target_args(), passes, "-S", *flags, "-"], input=difftest.normalise(before), capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"opt {passes}: " + (done.stderr.strip().splitlines() or ["failed"])[0])
    inlined, refused = verdicts(done.stderr)
    return inlined, refused, done.stdout


def compare(before: str, remarks: str, pipeline: str = "inline", threshold: int | None = None) -> dict:
    """The sites of `before` and of the copies either inliner made, by (caller, callee): llrm's inlined count from its remarks, LLVM's from opt's,
    and the classes of the sites."""
    was = sites(before)
    ours_in, ours_out = verdicts(remarks)
    inlined, refused, _ = llvm_verdicts(before, pipeline, threshold)
    size = sizes(before)
    calls_of = collections.Counter()
    for (_, callee), n in was.items():
        calls_of[callee] += n
    rows = []
    totals = collections.Counter()
    for caller, callee in sorted({*was, *ours_in, *ours_out, *inlined, *refused}):
        ours, theirs = len(ours_in.get((caller, callee), [])), len(inlined.get((caller, callee), []))
        n = max(was.get((caller, callee), 0), ours + len(ours_out.get((caller, callee), [])), theirs + len(refused.get((caller, callee), [])))
        agree, llrm_only, llvm_only = n - abs(ours - theirs), max(0, ours - theirs), max(0, theirs - ours)
        why = inlined.get((caller, callee)) or refused.get((caller, callee)) or [None]
        rows.append(
            {
                "caller": caller,
                "callee": callee,
                "sites": n,
                "llrm_inlined": ours,
                "llvm_inlined": theirs,
                "agree": agree,
                "llrm_only": llrm_only,
                "llvm_only": llvm_only,
                "llvm": why[0],
                "callee_instructions": size.get(callee),
                "caller_instructions": size.get(caller),
                "callee_called_once": calls_of[callee] == 1,
            }
        )
        band = "unknown" if size.get(callee) is None else "<=20" if size[callee] <= 20 else "<=60" if size[callee] <= 60 else ">60"
        for kind, count in (("agree", agree), ("llrm-only", llrm_only), ("llvm-only", llvm_only)):
            totals[(kind, band)] += count
    llvm_inlined = sum(row["llvm_inlined"] for row in rows)
    return {"pipeline": pipeline, "threshold": threshold, "sites": sum(row["sites"] for row in rows), "llvm_inlined": llvm_inlined, "premise": premise(llvm_inlined, sum(row["sites"] for row in rows), sum(row["llrm_only"] + row["llvm_only"] for row in rows)), "rows": rows, "by_class_and_size": {f"{k}|{b}": n for (k, b), n in sorted(totals.items())}}


def opt_header() -> str:
    version = subprocess.run([str(difftest.LLVM_BIN / "opt"), "--version"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    return f"opt: {difftest.LLVM_BIN / 'opt'} ({version})"


def table(report: dict, examples: int = 6) -> str:
    totals = collections.Counter()
    for key, n in report["by_class_and_size"].items():
        totals[key.split("|")[0]] += n
    out = [
        opt_header(),
        "",
        f"### inlining: llrm against `opt -passes={report['pipeline']}`" + (f" -inline-threshold={report['threshold']}" if report["threshold"] is not None else ""),
        "",
        f"premise (LLVM inlined {report['llvm_inlined']} of {report['sites']} sites): {report['premise']}",
        f"sites {report['sites']}: agree {totals['agree']}, llrm-only {totals['llrm-only']}, llvm-only {totals['llvm-only']}",
        "",
        "| class | callee <=20 | <=60 | >60 |",
        "|---|---|---|---|",
    ]
    for kind in ("agree", "llrm-only", "llvm-only"):
        out.append(f"| {kind} | " + " | ".join(str(report["by_class_and_size"].get(f"{kind}|{band}", 0)) for band in ("<=20", "<=60", ">60")) + " |")
    for kind in ("llrm_only", "llvm_only"):
        shown = [r for r in report["rows"] if r[kind]][:examples]
        if shown:
            out += ["", f"{kind.replace('_', '-')} examples (caller <- callee: sites, callee/caller instructions, LLVM's verdict):"]
            for r in shown:
                out.append(f"- {r['caller']} <- {r['callee']}: {r[kind]} of {r['sites']}, {r['callee_instructions']}/{r['caller_instructions']}, once={r['callee_called_once']}, llvm={r['llvm']}")
    return "\n".join(out)


def stage_files(directory: Path) -> tuple[Path, Path] | None:
    """(before, remarks) of an `llrm-c --dump` directory: the second of three consecutive NN-pipeline.ll, and remarks.txt."""
    named = sorted(directory.glob("[0-9]*-*.ll"), key=lambda p: int(re.match(r"\d+", p.name)[0]))
    pipeline = [p for p in named if p.name.endswith("-pipeline.ll")]
    remarks = directory / "remarks.txt"
    return (pipeline[1], remarks) if len(pipeline) >= 2 and remarks.is_file() else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("paths", nargs="+")
    parser.add_argument("--pipeline", default="inline")
    parser.add_argument("--threshold", type=int)
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    if args.paths[0] == "dumps":
        found = [(d, stage_files(Path(d))) for d in args.paths[1:]]
        pairs = [(d, *files) for d, files in found if files]
        for d, files in found:
            if not files:
                print(f"inlinediff: {d}: no pipeline module, or no remarks.txt", file=sys.stderr)
    else:
        pairs = [(args.paths[0], Path(args.paths[0]), Path(args.paths[1]))]
    merged: dict = {}
    for name, before, remarks in pairs:
        report = compare(before.read_text(), remarks.read_text(), args.pipeline, args.threshold)
        merged[name] = report
        print(f"## {name}\n{table(report)}\n")
    if args.json:
        args.json.write_text(json.dumps(merged, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
