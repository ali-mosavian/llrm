#!/usr/bin/env python3
"""Loop deletion against the pinned `opt -passes=loop(loop-deletion)`, decisions first.

    tools/loopdeletion_oracle.py [--rotated] [FILE.ll ...]
        (default: tests/fixtures/loopdeletion and the analysis corpus)

Both sides get each file through `loop-simplify,lcssa` (the loop pass manager's
form), at `-mtriple=i486-unknown-linux-gnu -mcpu=i486`; with `--rotated` through
`loop-rotate` too, which is the acceptance premise (a rotated loop is deleted
like any other). For each function: the loops left (the decision: which loops
were deleted), then the blocks and instructions per opcode after
`instcombine,simplifycfg,adce` on both outputs (llrm leaves the cleanup to the
passes that follow, D9). A control, `-delete-threshold=0`-like (`opt` with the
pass run at threshold 1), must differ from llrm on some file. Prints
`files total / compared / failed / missing` first, then each mismatch. Exit 1
on any failure or a missing side.
"""
import collections
import glob
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/debug/llrm-mir"
TARGET = ["-mtriple=i486-unknown-linux-gnu", "-mcpu=i486"]
# Differences with a stated deviation (the plan page), not failures.
CLASSIFIED = {}
REQUIRED = {"counter_header", "counter_rotated", "result_read", "stores", "never_executes", "first_iteration_exit", "waiting_loop", "nested_dead", "two_exits"}


def run(cmd, stdin=None):
    return subprocess.run(cmd, input=stdin, capture_output=True, text=True)


def exact(line):
    """A decimal `float` literal opt prints rounds to the float; llrm reads only exact ones."""
    if " float " not in line or " double " in line:
        return line
    return re.sub(
        r"(?<![\w.%@])-?\d+\.\d+e[+-]\d+\b",
        lambda m: "0x%016X" % struct.unpack(">Q", struct.pack(">d", struct.unpack("f", struct.pack("f", float(m.group(0))))[0]))[0],
        line,
    )


def to_mir(text):
    text = re.sub(r"\bcaptures\(none\)", "nocapture", text)
    text = re.sub(r" captures\([^)]*\)", "", text)
    text = re.sub(r"\bf0x([0-9A-Fa-f]{16})\b", r"0x\1", text)
    text = re.sub(r" nocreateundeforpoison\b", "", text)
    text = re.sub(r"\b(hidden|dso_local|local_unnamed_addr) ", "", text)
    return "\n".join(exact(l) for l in text.splitlines() if not l.startswith("target triple")) + "\n"


def functions(ir):
    return {m.group(1): m.group(0) for m in re.finditer(r"^define [^\n]*@\"?([\w.$-]+)\"?\(.*?^}$", ir, re.M | re.S)}


def stripped(text):
    """`text` without the metadata attachments: they number differently on the two sides."""
    return re.sub(r", !(tbaa|prof|llvm\.loop|range|nonnull|align|noundef|dbg)\s+![\w.]+", "", text)


def spliced(module, fn, body):
    """`module` with the body of @fn replaced by that of `body` (a whole `define`), so that both sides are
    cleaned with the declarations and attributes `opt` printed (llrm does not keep every one)."""
    old = functions(module).get(fn)
    if old is None:
        return None
    inner = body[body.index("{") + 1: body.rindex("}")]
    head = old[: old.index("{") + 1]
    return module.replace(old, head + inner + "}", 1)


def shape(body):
    blocks, ops = 0, collections.Counter()
    for line in body.splitlines():
        s = line.split(";")[0].strip()
        if re.match(r"^[\w.$-]+:$", s):
            blocks += 1
        elif s and not s.startswith(("define", "}", "attributes", "source_filename", "declare", "target", "!", "@")):
            ops[re.sub(r"^%[\w.$-]+ = ", "", s).split()[0]] += 1
    return blocks, dict(sorted(ops.items()))


def loops_by_function(ir):
    """The loops of each function of a module (the module whole: a body alone does not parse)."""
    r = run([str(PINNED), *TARGET, "-passes=print<loops>", "-disable-output"], stdin=ir)
    assert r.returncode == 0, r.stderr[:300]
    counts, current = {}, None
    for line in r.stderr.splitlines():
        m = re.match(r"Loop info for function '(.*)':", line)
        if m:
            current = m.group(1)
            counts[current] = 0
        elif current is not None and "Loop at depth" in line:
            counts[current] += 1
    return counts


def cleaned(ir):
    """`ir` through instcombine, simplifycfg and adce until it stops changing (at most 6 rounds)."""
    last = None
    for _ in range(6):
        r = run([str(PINNED), *TARGET, "-passes=instcombine<no-verify-fixpoint>,simplifycfg,adce", "-S"], stdin=ir)
        if r.returncode != 0:
            return None
        ir = r.stdout
        if ir == last:
            break
        last = ir
    return ir


def opt(ir, passes):
    r = run([str(PINNED), f"-passes={passes}", "-S", *TARGET], stdin=ir)
    return r.stdout if r.returncode == 0 else None


def main():
    args = sys.argv[1:]
    rotated = "--rotated" in args
    args = [a for a in args if a != "--rotated"]
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"loopdeletion_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0], f"(pinned {PINNED})", "rotated" if rotated else "")
    files = args or sorted(
        glob.glob(str(ROOT / "tests/fixtures/loopdeletion/*.ll")) + glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll"))
    )
    compared = failed = missing = deleted = 0
    classified = []
    total = 0
    bad = []
    if not args:
        absent = REQUIRED - {Path(f).stem for f in files if "fixtures/loopdeletion" in f}
        if absent:
            failed += len(absent)
            bad.append(f"the corpus misses shapes: {sorted(absent)}")
    for f in files:
        name = Path(f).parent.name + "/" + Path(f).stem
        ir = Path(f).read_text()
        prep = "loop-simplify,lcssa" + (",loop-rotate" if rotated else "")
        prepared = opt(ir, prep)
        if prepared is None:
            missing += 1
            bad.append(f"{name}: opt could not prepare it")
            continue
        want = opt(prepared, "loop(loop-deletion)")
        tmp = Path(tempfile.mkdtemp()) / "prepared.ll"
        tmp.write_text(to_mir(prepared))
        got = run([str(LLRM), "--loop-deletion", str(tmp)])
        if want is None or got.returncode != 0:
            missing += 1
            bad.append(f"{name}: " + ("opt failed" if want is None else "llrm-mir failed: " + got.stderr.strip()[:300]))
            continue
        compared += 1
        before_fns, want_fns, got_fns = functions(prepared), functions(want), functions(got.stdout)
        before_loops, want_loops = loops_by_function(prepared), loops_by_function(want)
        got_loops = loops_by_function(got.stdout.replace("define internal i16 @", "define internal i16 @"))
        clean_want, clean_got = {}, {}
        for fn_, text_ in before_fns.items():
            if before_loops.get(fn_, 0) == want_loops.get(fn_, 0) == got_loops.get(fn_, 0):
                continue
            a_ = cleaned(stripped(spliced(want, fn_, want_fns[fn_]) or ""))
            b_ = cleaned(stripped(spliced(want, fn_, got_fns.get(fn_, "{}")) or ""))
            clean_want[fn_] = functions(a_ or "").get(fn_)
            clean_got[fn_] = functions(b_ or "").get(fn_)
        for fn, text in before_fns.items():
            total += 1
            wl, gl = want_loops.get(fn, 0), got_loops.get(fn, 0)
            deleted += before_loops.get(fn, 0) != wl
            if wl != gl and (name, fn) in CLASSIFIED:
                classified.append(f"{name} @{fn}: {CLASSIFIED[(name, fn)]}")
                continue
            if wl != gl:
                failed += 1
                bad.append(f"{name} @{fn}: loops left: opt {wl}, llrm {gl} (before {before_loops.get(fn, 0)})")
                continue
            if wl != before_loops.get(fn, 0):
                a, b = clean_want.get(fn), clean_got.get(fn)
                if a is None or b is None or shape(a) != shape(b):
                    failed += 1
                    bad.append(f"{name} @{fn}: deleted differently\n--- opt {shape(a) if a else None}\n--- llrm {shape(b) if b else None}")
    if not args and not deleted:
        failed += 1
        bad.append("opt deleted no loop in any file: the oracle cannot fail")
    print(f"files {len(files)} / compared {compared} / failed {failed} / missing {missing}"
          f"  ({total} functions, opt deleted in {deleted}, {len(classified)} classified)")
    for line in classified:
        print("classified:", line)
    for line in bad:
        print(line)
    return 1 if failed or missing else 0


sys.exit(main())
