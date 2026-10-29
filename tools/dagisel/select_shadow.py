#!/usr/bin/env python3
"""Per function, what the DAG selector makes against the pin's llc, bucketed by cause.

    tools/dagisel/select_shadow.py LLRM_CC SELECT_SHADOW [--src DIR] [--programs a,b] [--jobs N] [--out FILE]

SELECT_SHADOW is `cargo build --release -p llrm-codegen --example select_shadow`. Each program is compiled
by `LLRM_CC -m32 -march=i486 -O2 --dump`; the last IR the dump writes goes to the example (the DAG path:
builder, legalizers, `select_node`) and to the pin's `llc -O2 -stop-after=finalize-isel` (through
pin.py). Every defined function of the IR lands in exactly one bucket, so the total is fixed (a whole-program failure puts all its functions in one bucket; a program with no IR counts as one row). The first bucket that fits: a block the builder or a legalizer refuses
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

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "pairs"))
import pin  # noqa: E402
import llrmbin  # noqa: E402

FLAGS = ["-c", *llrmbin.c_args(32), "-mabi=sysv", "-O2"]
M16_FLAGS = ["-c", *llrmbin.c_args(16), "-O2"]
# Opcodes the pin's selection makes that are not instructions we compare: copies and bookkeeping.
FLAG_WORDS = {"nuw", "nsw", "exact", "nnan", "ninf", "nsz", "arcp", "contract", "afn", "reassoc", "nofpexcept", "samesign", "nneg", "disjoint", "inbounds", "fmf"}
# The pin's finalize-isel runs `EmitLoweredSelect`: its output has a JCC_1 diamond where our DAG selection still has the
# CMOV_GR* pseudo (the emitter expands it to the same diamond, so there is no cmov: illegal on a 486).
SELECT_PSEUDOS = {"CMOV_GR8", "CMOV_GR16", "CMOV_GR32"}
# The pin's finalize-isel runs the custom inserter of FP_TO_INT*_IN_MEM: the control-word switch around the store. Ours
# keeps the pseudo (`floatalloc` writes the switch, shared by a run of them).
EQUAL_FP = "equal through the FP_TO_INT rule"
FP_TO_INT = re.compile(r"FP(\d+)_TO_INT(\d+)_IN_MEM")
BOOKKEEPING = {"COPY", "EXTRACT_SUBREG", "PHI", "IMPLICIT_DEF", "ADJCALLSTACKDOWN32", "ADJCALLSTACKUP32", "KILL", "COPY_TO_REGCLASS", "LIFETIME_START", "LIFETIME_END"}


def defined(ir: str) -> list[str]:
    """The functions the IR defines: the fixed total, whatever either side later does with them."""
    return [name.strip('"') for name in re.findall(r'^define\b[^@\n]*@("[^"\n]+"|[^\s(]+)\(', ir, re.M)]


def fp80(match: re.Match) -> str:
    """`x86_fp80 0x<double bits>` (how llrm writes it) as LLVM's `0xK<sign+exponent><mantissa>`."""
    bits = int(match.group(1), 16)
    sign, exponent, fraction = bits >> 63, (bits >> 52) & 0x7FF, bits & ((1 << 52) - 1)
    if exponent == 0x7FF:
        exponent, mantissa = 0x7FFF, (1 << 63) | (fraction << 11)
    elif exponent == 0:
        exponent, mantissa = 0, fraction << 11
        if fraction:
            shift = 64 - mantissa.bit_length()
            exponent, mantissa = 16383 - 1022 - shift, mantissa << shift
    else:
        exponent, mantissa = exponent - 1023 + 16383, (1 << 63) | (fraction << 11)
    return f"x86_fp80 0xK{sign << 15 | exponent:04X}{mantissa:016X}"


def pin_ir(text: str) -> str:
    """llrm's IR as the pin reads it: llrm's calling conventions are not LLVM's, `switch table` is llrm's own form of a switch, and an
    fp80 constant is spelled with the bits of a double."""
    text = re.sub(r"\b(?:watcallcc|sysvcc|regparm3cc|cdeclcc|pascalcc|ia16cc) ", "", text)
    text = text.replace("switch table ", "switch ")
    return re.sub(r"x86_fp80 0x([0-9A-Fa-f]{16})\b", fp80, text)


def first_error(stderr: str) -> str:
    """The first line that says `error`, else the last line: llc's last line is the caret under it."""
    lines = stderr.strip().splitlines() or ["?"]
    return next((line for line in lines if "error" in line), lines[-1]).strip()


def whole_program(program: str, ir: str, bucket: str) -> list[tuple[str, str, str]]:
    """A failure of the whole run: every defined function lands in `bucket`, so the total holds."""
    return [(program, f, bucket) for f in defined(ir)] or [(program, "*", bucket)]


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


def expanded_selects(have: Counter, want: Counter) -> Counter:
    """`have` with each select pseudo as the JCC_1 the pin's `EmitLoweredSelect` makes. A cascade of selects on one
    compare is one diamond: the pin's count may be lower than the pseudo count, by at most one JCC_1 per extra pseudo."""
    selects = sum(have.pop(name, 0) for name in list(have) if name in SELECT_PSEUDOS)
    if not selects:
        return have
    ours = have["JCC_1"] + selects
    cascaded = max(0, min(ours - want["JCC_1"], selects - 1))
    have["JCC_1"] = ours - cascaded
    return have


def expanded_fp_to_int(have: Counter) -> Counter:
    """`have` with each FP_TO_INT*_IN_MEM pseudo as the instructions the pin's inserter makes."""
    for name in [name for name in have if FP_TO_INT.fullmatch(name)]:
        count = have.pop(name)
        source, target = FP_TO_INT.fullmatch(name).groups()
        for made, times in {"FNSTCW16m": 1, "MOVZX32rm16": 1, "OR32ri": 1, "MOV16mr": 1, "FLDCW16m": 2, f"IST_Fp{target}m{source}": 1}.items():
            have[made] += count * times
    return have


def rows_of(program, funcs, refused, pinned, ir=""):
    rows = []
    for name in defined(ir):
        if name not in funcs:
            rows.append((program, name, "missing from the shadow's output"))
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
        elif pinned is None:
            bucket = "selects"  # m16: the pin has no oracle, the old selector's output is compared later
        else:
            want = Counter({k: v for k, v in pinned.get(name, Counter()).items() if k not in BOOKKEEPING})
            have = Counter({k: v for k, v in f["machine"].items() if k not in BOOKKEEPING})
            plain = expanded_selects(Counter(have), want)
            have = expanded_selects(expanded_fp_to_int(have), want)
            if want == have:
                bucket = "equal" if want == plain else EQUAL_FP
            else:
                pin_only, ours_only = want - have, have - want
                top = max((pin_only + ours_only).items(), key=lambda kv: kv[1])[0]
                bucket = f"differs: {'pin only' if top in pin_only else 'ours only'} {top}"
        rows.append((program, name, bucket))
    return rows


def compare(shadow: str, ir: Path, program: str) -> list[tuple[str, str, str]]:
    """The rows for one IR file the pin's llc and the example both read."""
    mine = subprocess.run([shadow, str(ir)], capture_output=True, text=True, timeout=600)
    if mine.returncode != 0:
        return whole_program(program, ir.read_text(), "shadow crashed: " + (mine.stderr.strip().splitlines() or ["?"])[-1][:60])
    funcs, refused = ours(mine.stdout)
    theirs = pin.llc(["-O2", "-stop-after=finalize-isel", "-o", "-"], ir.read_text(), pin.target("m32"))
    if theirs.returncode != 0:
        return whole_program(program, ir.read_text(), "pin llc failed: " + first_error(theirs.stderr)[-60:])
    return rows_of(program, funcs, refused, pin_opcodes(theirs.stdout), ir.read_text())


def one(llrm_cc: str, shadow: str, source: Path, m16: bool = False, level: str = "-O2") -> list[tuple[str, str, str]]:
    """[(program, function, bucket)]."""
    with tempfile.TemporaryDirectory() as tmp:
        dump = Path(tmp) / "d"
        dump.mkdir()
        flags = [*(M16_FLAGS if m16 else FLAGS)[:-1], level]
        done = subprocess.run([llrm_cc, *flags, "--dump", str(dump), "-S", "-o", str(Path(tmp) / "o.s"), str(source)], capture_output=True, text=True, timeout=600, env={**os.environ, **llrmbin.cc_environment()})
        ir = last_ir(dump)
        if done.returncode != 0 or ir is None:
            return whole_program(source.stem, ir.read_text() if ir else "", "compile failed: " + (done.stderr.strip().splitlines() or ["?"])[-1][:60])
        # llrm's own calling convention for internal functions is not LLVM's: both sides see the default one.
        clean = Path(tmp) / "clean.ll"
        clean.write_text(re.sub(r"\b(?:watcallcc|sysvcc|regparm3cc|cdeclcc|pascalcc|ia16cc) ", "", ir.read_text()))
        ir = clean
        env = {**os.environ, **({"M16": "1"} if m16 else {}), **({"OPTSIZE": "1"} if level == "-Os" else {})}
        mine = subprocess.run([shadow, str(ir)], capture_output=True, text=True, timeout=600, env=env)
        if mine.returncode != 0:
            return whole_program(source.stem, ir.read_text(), "shadow crashed: " + (mine.stderr.strip().splitlines() or ["?"])[-1][:60])
        funcs, refused = ours(mine.stdout)
        if m16:
            return rows_of(source.stem, funcs, refused, None, ir.read_text())
        theirs = pin.llc(["-O2", "-stop-after=finalize-isel", "-o", "-"], pin_ir(ir.read_text()), pin.target("m32", optsize=level == "-Os"))
        if theirs.returncode != 0:
            return whole_program(source.stem, ir.read_text(), "pin llc failed: " + first_error(theirs.stderr)[-60:])
        pinned = pin_opcodes(theirs.stdout)
        return rows_of(source.stem, funcs, refused, pinned, ir.read_text())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm_cc")
    parser.add_argument("shadow")
    parser.add_argument("--src", default=str(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")) / "base/b"))
    parser.add_argument("--programs", default="")
    parser.add_argument("--ir", default="", help="compare one IR file the pin and the example both read (no compiler)")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    parser.add_argument("--out", default="")
    parser.add_argument("--level", default="-O2", choices=["-O2", "-Os"])
    parser.add_argument("--m16", action="store_true", help="real mode: no pin; a function that builds and selects is `selects`")
    args = parser.parse_args()
    if args.ir:
        rows = compare(args.shadow, Path(args.ir), Path(args.ir).stem)
        for _, f, b in rows:
            print(f"{b:60s} {f}")
        return 0
    sources = sorted(Path(args.src).glob("*.c"))
    if args.programs:
        want = set(args.programs.split(","))
        sources = [s for s in sources if s.stem in want]
    with ThreadPoolExecutor(args.jobs) as pool:
        rows = [row for part in pool.map(lambda s: one(args.llrm_cc, args.shadow, s, args.m16, args.level), sources) for row in part]
    buckets = Counter(bucket for _, _, bucket in rows)
    good = "selects" if args.m16 else "equal"
    buckets[good] += buckets[EQUAL_FP]
    print(f"{buckets[good]} {good} of {len(rows)} defined functions in {len(sources)} programs")
    for bucket, n in buckets.most_common():
        print(f"{n:6d}  {bucket}")
    skipped = sum(n for bucket, n in buckets.items() if "failed" in bucket or "crashed" in bucket)
    print(f"{skipped} of {len(rows)} functions were not compared (a tool failed on their program)")
    if args.out:
        Path(args.out).write_text("".join(f"{p}\t{f}\t{b}\n" for p, f, b in rows))
    return 0 if buckets[good] == len(rows) else 1


if __name__ == "__main__":
    sys.exit(main())
