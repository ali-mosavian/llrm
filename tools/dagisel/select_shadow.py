#!/usr/bin/env python3
"""Per function, what the DAG selector makes against the pin's llc, bucketed by cause.

    tools/dagisel/select_shadow.py LLRM_C SELECT_SHADOW [--src DIR] [--programs a,b] [--jobs N] [--out FILE]

SELECT_SHADOW is `cargo build --release -p llrm-core --example select_shadow`. Each program is compiled
by `LLRM_C -m32 -march=i486 -O2 --dump`; the last IR the dump writes goes to the example (the DAG path:
builder, legalizers, `select_node`) and to the pin's `llc -O2 -stop-after=finalize-isel` (through
pin.py). A function lands in the first bucket that fits: a block the builder or a legalizer refuses
(by reason), a node the selector could not select (by node), machine opcodes that differ from the pin's
(by the opcode), or equal. Prints the bucket table; `--out` writes every function.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "pairs"))
import pin  # noqa: E402

FLAGS = ["-m32", "-mabi=sysv", "-march=i486", "-O2"]
# Opcodes the pin's selection makes that are not instructions we compare: copies and bookkeeping.
FLAG_WORDS = {"nuw", "nsw", "exact", "nnan", "ninf", "nsz", "arcp", "contract", "afn", "reassoc", "nofpexcept", "samesign", "nneg", "disjoint", "inbounds", "fmf"}
BOOKKEEPING = {"COPY", "PHI", "IMPLICIT_DEF", "ADJCALLSTACKDOWN32", "ADJCALLSTACKUP32", "KILL"}


def last_ir(dump: Path) -> Path | None:
    files = sorted(p for p in dump.glob("[0-9]*.ll"))
    return files[-1] if files else None


def pin_opcodes(mir: str) -> dict[str, Counter]:
    """{function: Counter of machine opcodes} from `-stop-after=finalize-isel` output."""
    out: dict[str, Counter] = {}
    current = None
    in_body = False
    for line in mir.splitlines():
        if line.startswith("name:"):
            current = line.split(":", 1)[1].strip()
            out[current] = Counter()
            in_body = False
        elif line.startswith("body:"):
            in_body = True
        elif line.startswith("...") or line.startswith("---"):
            in_body = False
        elif in_body and current and line.startswith("    ") and not line.strip().startswith(("successors:", "liveins:", "bb.")):
            text = line.strip()
            if "=" in text.split(" ::")[0]:
                text = text.split("=", 1)[1].strip()
            words = text.split(" ")
            while words and words[0] in FLAG_WORDS:
                words = words[1:]
            op = (words[0] if words else "").rstrip(",")
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", op):
                out[current][op] += 1
    return out


def parse_counts(text: str) -> Counter:
    counts: Counter = Counter()
    for part in filter(None, text.split(",")):
        name, n = part.rsplit(":", 1)
        counts[name] = int(n)
    return counts


def ours(stdout: str) -> tuple[dict[str, dict], dict[str, list[str]]]:
    funcs, refused = {}, {}
    for line in stdout.splitlines():
        if line.startswith("FUNC "):
            m = re.match(r"FUNC (\S+) blocks=(\d+) built=(\d+) machine=(\S*) unselected=(\S*)", line)
            funcs[m[1]] = {"blocks": int(m[2]), "built": int(m[3]), "machine": parse_counts(m[4]), "unselected": parse_counts(m[5])}
        elif line.startswith("REFUSED "):
            head, reason = line[len("REFUSED "):].split(" ", 1)
            refused.setdefault(head.split(":")[0], []).append(reason)
    return funcs, refused


def short(reason: str) -> str:
    reason = re.sub(r"Unsupported \{ op: (\w+), ty: [^,]*, what: \"([^\"]*)\" \}", r"\1: \2", reason)
    return reason[:70]


def one(llrm_c: str, shadow: str, source: Path) -> list[tuple[str, str, str]]:
    """[(program, function, bucket)]."""
    with tempfile.TemporaryDirectory() as tmp:
        dump = Path(tmp) / "d"
        dump.mkdir()
        done = subprocess.run([llrm_c, *FLAGS, "--dump", str(dump), "-S", "-o", str(Path(tmp) / "o.s"), str(source)], capture_output=True, text=True, timeout=600)
        ir = last_ir(dump)
        if done.returncode != 0 or ir is None:
            return [(source.stem, "*", "compile failed: " + (done.stderr.strip().splitlines() or ["?"])[-1][:60])]
        # llrm's own calling convention for internal functions is not LLVM's: both sides see the default one.
        clean = Path(tmp) / "clean.ll"
        clean.write_text(re.sub(r"\b(?:watcallcc|sysvcc|regparm3cc|cdeclcc|pascalcc|ia16cc) ", "", ir.read_text()))
        ir = clean
        mine = subprocess.run([shadow, str(ir)], capture_output=True, text=True, timeout=600)
        if mine.returncode != 0:
            return [(source.stem, "*", "shadow crashed: " + (mine.stderr.strip().splitlines() or ["?"])[-1][:60])]
        funcs, refused = ours(mine.stdout)
        theirs = pin.llc(["-O2", "-stop-after=finalize-isel", "-o", "-"], ir.read_text(), pin.target("m32"))
        if theirs.returncode != 0:
            return [(source.stem, "*", "pin llc failed: " + (theirs.stderr.strip().splitlines() or ["?"])[-1][:60])]
        pinned = pin_opcodes(theirs.stdout)
        rows = []
        for name, f in funcs.items():
            reasons = refused.get(name, [])
            if f["blocks"] == 0:
                continue  # a declaration
            if reasons:
                top = Counter(reasons).most_common(1)[0][0]
                kind, _, why = top.partition(": ")
                bucket = f"refused {kind}: " + short(why)
            elif f["unselected"]:
                bucket = "unselected " + max(f["unselected"], key=f["unselected"].get)
            else:
                want = Counter({k: v for k, v in pinned.get(name, Counter()).items() if k not in BOOKKEEPING})
                have = Counter({k: v for k, v in f["machine"].items() if k not in BOOKKEEPING})
                if want == have:
                    bucket = "equal"
                else:
                    pin_only, ours_only = want - have, have - want
                    top = max((pin_only + ours_only).items(), key=lambda kv: kv[1])[0]
                    bucket = f"differs: {'pin only' if top in pin_only else 'ours only'} {top}"
            rows.append((source.stem, name, bucket))
        return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm_c")
    parser.add_argument("shadow")
    parser.add_argument("--src", default=str(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")) / "base/b"))
    parser.add_argument("--programs", default="")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    parser.add_argument("--out", default="")
    args = parser.parse_args()
    sources = sorted(Path(args.src).glob("*.c"))
    if args.programs:
        want = set(args.programs.split(","))
        sources = [s for s in sources if s.stem in want]
    with ThreadPoolExecutor(args.jobs) as pool:
        rows = [row for part in pool.map(lambda s: one(args.llrm_c, args.shadow, s), sources) for row in part]
    buckets = Counter(bucket for _, _, bucket in rows)
    print(f"{len(rows)} functions in {len(sources)} programs: {buckets['equal']} equal")
    for bucket, n in buckets.most_common():
        print(f"{n:6d}  {bucket}")
    if args.out:
        Path(args.out).write_text("".join(f"{p}\t{f}\t{b}\n" for p, f, b in rows))
    return 0 if buckets["equal"] == len(rows) else 1


if __name__ == "__main__":
    sys.exit(main())
