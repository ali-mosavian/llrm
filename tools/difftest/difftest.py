#!/usr/bin/env python3
"""A pass of llrm against LLVM's counterpart, on the same input: what the two write differently, by instruction and by line.

    tools/difftest/difftest.py run PASS [--inputs DIR] [--config NAME ...] [--limit N] [--json OUT]
    tools/difftest/difftest.py dumps PASS DIR [--limit N]        the step files of an `llrm-c --dump DIR` made on an order branch
    tools/difftest/difftest.py pair INPUT.ll LLRM_OUT.ll --pass PASS    one input, llrm's answer, LLVM's

The inputs are LLRM_DUMP_INPUTS's (tools/mkinputs.sh): per config a MANIFEST.tsv of `config fn pass run file instructions`. llrm's
answer to run N of a function is the input of run N+1 of it (the pipeline runs its passes in order, and a pass that changes nothing writes
its input as its output), so llrm is not run again. LLVM's answer is `opt -passes=COUNTERPART` of the same file, at the commit this
harness names in its output. Both sides are made readable to LLVM by the same two rewrites (`normalise`) and then named alike
(`canonical`): a function is identical when its text is, otherwise the opcode histograms and the first differing lines say how.
A dump LLVM cannot read after the rewrites is a finding, listed with its first error.
"""

from __future__ import annotations

import os
import re
import sys
import json
import difflib
import argparse
import subprocess
import collections
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import envguard  # noqa: E402
import targetdesc  # noqa: E402

INPUTS = Path(os.environ.get("LLRM_DIFFTEST_INPUTS") or Path.home() / "scratch/replay/inputs")
PINNED_BIN = Path.home() / "scratch/llvm-build/bin"  # LLVM 24.0.0git d1106deb71cc, the reference of every comparison
# A different LLVM only by name: the distro's opt (20.1.8) is not the compiler the plan is judged against.
LLVM_BIN = Path(os.environ.get("LLVM_BIN") or PINNED_BIN)

# llrm pass -> the opt pipeline that does its work. Many to one where llrm splits what LLVM does in one (instcombine = fold + algebraic + phiopt);
# a pass with no counterpart is listed, not compared. instcombine runs without its fixpoint check: an input llrm's passes left in a form it takes two
# iterations to settle is LLVM's to settle here, not an error of the harness.
COUNTERPART = {
    "sroa": "sroa",
    "promote": "mem2reg",
    "fold": "instcombine<no-verify-fixpoint>",
    "algebraic": "instcombine<no-verify-fixpoint>",
    "phiopt": "instcombine<no-verify-fixpoint>",
    "gvn": "gvn",
    "earlycse": "early-cse<memssa>",
    "dse": "dse",
    "hoist": "loop-mssa(licm)",
    "loopmotion": "loop-mssa(licm)",
    "lcssa": "lcssa",
    "loopsimplify": "loop-simplify",
    "indvars": "indvars",
    "unroll": "loop(loop-unroll-full)",
    "peel": "loop-unroll<peeling>",
    "decide": "simplifycfg",
    "simplifycfg": "simplifycfg",
    "trivialunswitch": "loop-mssa(simple-loop-unswitch<no-nontrivial>)",
    "dead": "adce",
    "sccp": "sccp",
    "bdce": "bdce",
    "tailrec": "tailcallelim",
    "float2int": "float2int",
    "aggressive-instcombine": "aggressive-instcombine",
    "tailcallelim": "tailcallelim",
    "memcpyopt": "memcpyopt",
    "cvp": "correlated-propagation",
    "jumpthreading": "jump-threading",
    "dfajumpthreading": "dfa-jump-threading",
    "instsimplify": "instsimplify",
    "loopidiom": "loop(loop-idiom)",
    "loopdeletion": "loop(loop-deletion)",
    "rotate": "loop-mssa(loop-rotate<header-duplication;no-prepare-for-lto>)",
    "licm": "loop-mssa(licm<allowspeculation>)",
}


# A slot whose pipeline differs from its pass key's: the first LICM of LPM1 does not speculate (PassBuilderPipelines.cpp#L724).
SLOT_COUNTERPART = {"licm@724": "loop-mssa(licm<no-allowspeculation>)"}


def counterpart(name: str) -> str:
    """The opt pipeline of a pass key or of a slot of LLVM's order (`sroa@1179` is `sroa`)."""
    return SLOT_COUNTERPART.get(name) or COUNTERPART[name.split("@")[0]]


# The C++ each counterpart is, under llvm/lib at d1106deb71cc: a difference belongs to the function of that file that does the work.
SOURCE = {
    "instsimplify": "Transforms/Scalar/InstSimplifyPass.cpp",
    "loop-mssa(loop-rotate<header-duplication;no-prepare-for-lto>)": "Transforms/Scalar/LoopRotation.cpp",
    "loop-mssa(licm<allowspeculation>)": "Transforms/Scalar/LICM.cpp",
    "loop-mssa(licm<no-allowspeculation>)": "Transforms/Scalar/LICM.cpp",
    "loop(loop-idiom)": "Transforms/Scalar/LoopIdiomRecognize.cpp",
    "loop(loop-deletion)": "Transforms/Scalar/LoopDeletion.cpp",
    "sroa": "Transforms/Scalar/SROA.cpp",
    "mem2reg": "Transforms/Utils/Mem2Reg.cpp",
    "instcombine<no-verify-fixpoint>": "Transforms/InstCombine/InstructionCombining.cpp",
    "gvn": "Transforms/Scalar/GVN.cpp",
    "early-cse<memssa>": "Transforms/Scalar/EarlyCSE.cpp",
    "dse": "Transforms/Scalar/DeadStoreElimination.cpp",
    "loop-mssa(licm)": "Transforms/Scalar/LICM.cpp",
    "lcssa": "Transforms/Utils/LCSSA.cpp",
    "loop-simplify": "Transforms/Utils/LoopSimplify.cpp",
    "indvars": "Transforms/Scalar/IndVarSimplify.cpp",
    "loop(loop-unroll-full)": "Transforms/Scalar/LoopUnrollPass.cpp",
    "loop-unroll<peeling>": "Transforms/Scalar/LoopUnrollPass.cpp",
    "simplifycfg": "Transforms/Utils/SimplifyCFG.cpp",
    "loop-mssa(simple-loop-unswitch<no-nontrivial>)": "Transforms/Scalar/SimpleLoopUnswitch.cpp",
    "adce": "Transforms/Scalar/ADCE.cpp",
    "sccp": "Transforms/Scalar/SCCP.cpp",
    "bdce": "Transforms/Scalar/BDCE.cpp",
    "tailcallelim": "Transforms/Scalar/TailRecursionElimination.cpp",
    "float2int": "Transforms/Scalar/Float2Int.cpp",
    "aggressive-instcombine": "Transforms/AggressiveInstCombine/TruncInstCombine.cpp",
    "memcpyopt": "Transforms/Scalar/MemCpyOptimizer.cpp",
    "correlated-propagation": "Transforms/Scalar/CorrelatedValuePropagation.cpp",
    "jump-threading": "Transforms/Scalar/JumpThreading.cpp",
    "dfa-jump-threading": "Transforms/Scalar/DFAJumpThreading.cpp",
}
LLVM_COMMIT = "d1106deb71cc"


def source_link(passes: str) -> str:
    path = SOURCE.get(passes)
    return f"[{path.rsplit('/', 1)[1]}](https://github.com/llvm/llvm-project/blob/{LLVM_COMMIT}/llvm/lib/{path})" if path else "-"


LLRM_CONVENTIONS = re.compile(r"(?<![\w@%$.\"-])(?:watcallcc|sysvcc|ia16cc|regparm3cc|regparm3popcc|cc ?[0-9]+)(?![\w$.\"-]) ?")
DECLARED_INTERNAL = re.compile(r"^declare internal ")
# llrm's own attributes (llrm-transforms local.rs); LLVM reads them as string attributes.
LLRM_ATTRIBUTES = re.compile(r"(?<![\w@%$.\"-])(noretain|releases|nearcode|threeway)(?![\w$.\"-])")


def normalise(text: str) -> str:
    """llrm's text in the dialect LLVM reads: its calling-convention words (`watcallcc`, `sysvcc`, `cc1000`), its own attributes (`threeway`) and `internal` on a declaration. Only
    those tokens, in code: not a name that contains one, not a string, not a comment."""
    out = []
    for line in text.splitlines(keepends=True):
        parts = re.split(r'("(?:[^"\\]|\\.)*"|;.*$)', line.rstrip("\n"), flags=re.M)
        code = [p if i % 2 else LLRM_ATTRIBUTES.sub(r'"\1"', LLRM_CONVENTIONS.sub("", p)) for i, p in enumerate(parts)]
        line = "".join(code) + ("\n" if line.endswith("\n") else "")
        out.append(DECLARED_INTERNAL.sub("declare ", line))
    return "".join(out)


# The target opt costs for: a dump carries no `target triple`, so opt used its host default (x86-64) and every TTI-dependent answer (unroll, LSR, inline cost, SimplifyCFG
# speculation, LoopIdiom ...) was priced for the wrong machine (30 i32 GEPs cost 5 each there, not the x86-64 figure: inline cost 370 vs 215). Nor does it carry a
# datalayout: opt used the triple's 32-bit pointers, where a 16-bit target has 16-bit near pointers (`--data-layout` below, read from the target's own datalayout.toml, not copied).
def datalayout(mode: int) -> str:
    """LLVM's datalayout string of the target whose `datalayout.toml` says `mode = N`."""
    for one in targetdesc.targets().values():
        if one["mode"] == mode:
            return one["datalayout"]
    raise SystemExit(f"no datalayout.toml with mode = {mode}")


def target_args() -> list[str]:
    """`-mtriple`, `-mcpu` and `--data-layout` of llrm's target (LLRM_DIFFTEST_TARGET: its `m<mode>`, or `host`)."""
    return targetdesc.opt_args()


def opt(passes: str, text: str) -> subprocess.CompletedProcess:
    return subprocess.run([str(LLVM_BIN / "opt"), *target_args(), f"-passes={passes}", "-S", "-"], input=text, capture_output=True, text=True)


def functions(text: str) -> dict[str, list[str]]:
    """name -> the lines of each defined function."""
    found: dict[str, list[str]] = {}
    current = None
    for line in text.splitlines():
        if line.startswith("define "):
            named = re.search(r'@(?:"([^"]+)"|([\w.$-]+))\(', line)  # a 16-bit target quotes its decorated names: @"_f@3"
            current = named[1] or named[2]
            found[current] = []
        if current is not None:
            found[current].append(line)
            if line == "}":
                current = None
    return found


def trimmed(lines: list[str]) -> list[str]:
    """The lines without what is not the code: comments, blank lines, `align`, metadata attachments (their numbers are the module's)."""
    out = []
    for line in lines:
        line = line.split(";", 1)[0].rstrip()
        if line.strip():
            out.append(re.sub(r", !\w+ !\d+", "", re.sub(r", align \d+", "", line)))
    return out


def blocks_in_order(body: list[str]) -> list[list[str]]:
    """The blocks of a function body, each as its lines (the label first), in the order a walk of the control flow from the entry meets them
    (successors in the order the terminator names them), the unreachable after in file order: two layouts of one CFG read alike."""
    blocks: dict[str, list[str]] = {}
    current = "entry"
    blocks[current] = [current + ":"]
    for line in body:
        label = re.match(r"^([\w.$-]+):", line)
        if label:
            current = label[1]
            blocks.setdefault(current, [])
            blocks[current] = [line]
        else:
            blocks[current].append(line)
    names = list(blocks)
    if len(blocks["entry"]) == 1 and len(names) > 1:  # an entry block that has a label of its own
        del blocks["entry"]
        names = list(blocks)
    seen: list[str] = []
    stack = [names[0]]
    while stack:
        name = stack.pop()
        if name in seen or name not in blocks:
            continue
        seen.append(name)
        stack.extend(reversed(re.findall(r"label %([\w.$-]+)", blocks[name][-1])))
    return [blocks[name] for name in seen + [n for n in names if n not in seen]]


HEX_DOUBLE = re.compile(r"(?<![\w.])0x[0-9A-Fa-f]{16}(?![\w.])")
HEX_FLOAT = re.compile(r"(?<![\w.])f0x([0-9A-Fa-f]{8})(?![\w.])")  # opt's spelling of a float constant that has no short decimal
HEX_DOUBLE_F = re.compile(r"(?<![\w.])f(0x[0-9A-Fa-f]{16})(?![\w.])")  # ... and of a double (floats-O2: `f0x3FF0040000000000`)


def floats_alike(line: str) -> str:
    """Every hexadecimal floating constant of the line in the spelling LLVM's printer gives it: `%.6e` when that reads back as the same value
    (as a float or as a double), else the hex. llrm prints `0x40445EB860000000` where opt prints `4.074000e+01`: one constant."""
    import struct

    def spelled(match: re.Match) -> str:
        value = struct.unpack(">d", bytes.fromhex(match[0][2:]))[0]
        text = "%.6e" % value
        try:
            single = struct.unpack("f", struct.pack("f", value))[0] == value
            back = float(text)
            if back == value or (single and struct.unpack("f", struct.pack("f", back))[0] == value):
                return text
        except (OverflowError, struct.error):
            pass
        return match[0]

    line = HEX_FLOAT.sub(lambda m: "0x%016X" % struct.unpack(">Q", struct.pack(">d", struct.unpack(">f", bytes.fromhex(m[1]))[0]))[0], line)
    line = HEX_DOUBLE_F.sub(lambda m: m[1], line)
    return HEX_DOUBLE.sub(spelled, line)


def lifetime_alike(line: str) -> str:
    """`llvm.lifetime.start/end` without the size operand: the pin's signature. llrm's text still gives the size first (`(i64 4, ptr %p)`), which
    opt's reader upgrades; the call and its declaration read alike on both sides."""
    line = re.sub(r"(@llvm\.lifetime\.(?:start|end)\.p\d+)\(i64 (?:-?\d+|%[\w.$-]+), ", r"\1(", line)
    return re.sub(r"(declare void @llvm\.lifetime\.(?:start|end)\.p\d+)\(i64, ", r"\1(", line)


def call_attributes_alike(line: str) -> str:
    """A call without the attributes after its arguments: llrm writes them (`) nounwind`), opt names a group (`) #4`); neither is the code."""
    if " call " not in line and not line.lstrip().startswith(("call ", "tail call ", "musttail call ", "notail call ")):
        return line
    return re.sub(r"\)((?: [a-z_]+(?:\([^)]*\))?| #\d+)+)$", ")", line)


def canonical(lines: list[str]) -> list[str]:
    """The function with its blocks in control-flow order, its local names numbered by first appearance, and what is not the code left out
    (comments, function and parameter attributes, `align`, metadata attachments, the order of a phi's incoming pairs). Two bodies that differ
    only in those read alike."""
    lines = [call_attributes_alike(lifetime_alike(floats_alike(re.sub(r"\bcc (\d+)", r"cc\1", line)))) for line in trimmed(lines)]
    names: dict[str, str] = {}
    out = []
    head = next((n for n, line in enumerate(lines) if line.startswith("define ")), None)
    if head is not None:
        line = lines[head]
        header = re.match(r"^define\s+.*?@([\w.$-]+)\((.*)\)[^()]*(?:\([^()]*\)[^()]*)*\{$", line)
        params = re.findall(r"%[\w.$-]+", header[2]) if header else []
        for one in params:
            names.setdefault(one[1:], f"v{len(names)}")
        out.append(f"define @{header[1]} params={len(params)} {{" if header else line)
        body = [x for x in lines[head + 1 :] if x != "}"]
    else:
        body = lines
    order = blocks_in_order(body)
    for number, block in enumerate(order):
        names.setdefault(re.match(r"^([\w.$-]+):", block[0])[1], f"b{number}")
    for block in order:
        for line in block:
            line = re.sub(r"%([\w.$-]+)", lambda m: "%" + names.setdefault(m[1], f"v{len(names)}"), line)
            label = re.match(r"^([\w.$-]+):", line)
            if label and not line.startswith("%"):
                line = names.get(label[1], label[1]) + ":"
            if " = phi " in line:
                head_, _, rest = line.partition(" = phi ")
                kind, _, pairs = rest.partition(" ")
                line = f"{head_} = phi {kind} " + ", ".join(sorted(re.findall(r"\[[^\]]*\]", pairs)))
            out.append(line)
    return out + (["}"] if head is not None else [])


def opcode(line: str) -> str | None:
    """The opcode of an instruction line, None for anything else."""
    line = line.strip()
    if not line or line.endswith(":") or line in ("}", "{") or line.startswith(("define ", "declare ", "@", "!", "attributes", "target", "source_filename")):
        return None
    m = re.match(r"^(?:%[\w.$-]+\s*=\s*)?(?:(?:tail|musttail|notail)\s+)?([a-z][\w.]*)", line)
    return m[1] if m else None


def histogram(lines: list[str]) -> collections.Counter:
    return collections.Counter(op for op in map(opcode, lines) if op)


def compare(source: str, llrm_out: str, passes: str) -> dict:
    """One input: LLVM's pipeline `passes` on `source` against llrm's `llrm_out`, function by function. {"error": first error} where LLVM cannot read
    or write one of them."""
    given, theirs, ours = normalise(source), None, normalise(llrm_out)
    run = opt(passes, given)
    if run.returncode:
        return {"error": "opt " + passes + ": " + (run.stderr.strip().splitlines() or ["failed"])[0]}
    theirs = run.stdout
    out = {}
    mine, llvm, before = functions(ours), functions(theirs), functions(given)
    for name in sorted(before.keys() | mine.keys() | llvm.keys()):
        a, b = canonical(mine.get(name, [])), canonical(llvm.get(name, []))
        delta = histogram(a) - histogram(b), histogram(b) - histogram(a)
        out[name] = {
            "identical": a == b,
            "llrm_only": dict(delta[0]),
            "llvm_only": dict(delta[1]),
            "llrm_lines": len(a),
            "llvm_lines": len(b),
            "unchanged_by_llrm": canonical(before.get(name, [])) == a,
            "unchanged_by_llvm": canonical(before.get(name, [])) == b,
            "hunk": [] if a == b else list(difflib.unified_diff(a, b, "llrm", "llvm", n=1, lineterm=""))[2:30],
        }
    return out


def export(todo: list[tuple[dict, Path | None]], name: str, replay: Path, dest: Path) -> int:
    """For every function whose canonical text differs between llrm's pass and opt's counterpart: `dest/<name>/<config>__<body>/{input,llrm,opt}.ll` and `diff.txt`
    (llrm's answer, opt's answer at llrm's target, the unified diff of their canonical forms). Owners who cannot run the oracle diagnose from these. Returns the count."""
    written = 0
    for row, _ in todo:
        source = Path(row["file"])
        try:
            ours = isolated(replay, row, source)
        except RuntimeError:
            continue
        given = normalise(source.read_text())
        run = opt(counterpart(name), given)
        if run.returncode:
            continue
        mine, llvm, before = functions(normalise(ours)), functions(run.stdout), functions(given)
        for fn in sorted(before.keys() | mine.keys() | llvm.keys()):
            a, b = canonical(mine.get(fn, [])), canonical(llvm.get(fn, []))
            if a == b:
                continue
            out = dest / name / f"{row['config']}__{row['fn']}__{fn}".replace("/", "_")
            out.mkdir(parents=True, exist_ok=True)
            (out / "input.ll").write_text("\n".join(before.get(fn, [])) + "\n")
            (out / "llrm.ll").write_text("\n".join(a) + "\n")
            (out / "opt.ll").write_text("\n".join(b) + "\n")
            (out / "diff.txt").write_text("\n".join(difflib.unified_diff(a, b, "llrm", "opt", n=3, lineterm="")) + "\n")
            written += 1
    return written


def manifest(root: Path, configs: list[str] | None) -> list[dict]:
    rows = []
    for config in sorted(p.name for p in root.iterdir() if (p / "MANIFEST.tsv").is_file()):
        if configs and config not in configs:
            continue
        for line in (root / config / "MANIFEST.tsv").read_text().splitlines():
            c, fn, name, run, file, instructions = line.split("\t")
            rows.append({"config": c, "fn": fn, "pass": name, "run": int(run), "file": file, "instructions": int(instructions)})
    return rows


def relabelled(rows: list[dict], name: str, borrowed: list[str] | None) -> list[dict]:
    """The manifest with the runs of the passes in `borrowed` read as runs of `name`: a pass the replay set has no rows for (bdce) is judged on the inputs of the passes that run
    beside it. The inputs are real bodies; only the label changes."""
    return [{**r, "pass": name} if borrowed and r["pass"] in borrowed else r for r in rows]


def pairs(rows: list[dict], name: str, root: Path) -> list[tuple[dict, Path]]:
    """(the row of every run of pass `name`, the file llrm's answer is in): the next run's input of the same function. The last run of a function
    has no answer here and is left out."""
    follow = {(r["config"], r["fn"], r["run"]): r for r in rows}
    return [(r, root / follow[(r["config"], r["fn"], r["run"] + 1)]["file"]) for r in rows if r["pass"] == name and (r["config"], r["fn"], r["run"] + 1) in follow]


STEP = re.compile(r"^(\d+)-r(\d+)-(.+)@(\d+)\.ll$")


def dump_pairs(directory: Path, name: str) -> list[tuple[dict, Path]]:
    """(the row of every step `name`, the file of its answer) for `llrm-c --dump DIR` step files: <body>/NNN-rRR-<pass>@<slot>.ll is the body
    after a step that changed it, so its answer is the file itself and its input is the file numbered one less in the same pass list. The lists of a
    pipeline each count from 001 into the one directory, so of the files numbered NNN-1 the input is the one whose slot is nearest. A first
    step (001) has no input here and is left out. (The row names the input.)"""
    out = []
    for body in sorted(p for p in directory.iterdir() if p.is_dir()):
        steps = [(int(m[1]), int(m[4]), m[3], f) for f in body.glob("*.ll") if (m := STEP.match(f.name))]
        base, _, at = name.partition("@")
        for number, slot, step, here in steps:
            before = [(abs(s - slot), f) for n, s, _, f in steps if n == number - 1]
            if step == base and (not at or slot == int(at)) and before:
                out.append(({"config": directory.name, "fn": body.name, "pass": name, "run": number, "file": str(min(before)[1])}, here))
    return out


def input_pairs(directory: Path, name: str) -> list[tuple[dict, None]]:
    """(the row of every run of `name` in `llrm-c --dump DIR` made with LLRM_DUMP_INPUTS, None): <body>/NNNN-in-<pass>@<slot>.ll is the body a run
    was about to get, whether the run then changed it or not, so llrm declining where LLVM acts is seen too. There is no answer file: llrm-pass makes it."""
    return [
        ({"config": directory.name, "fn": body.name, "pass": name, "run": int(f.name.split("-")[0]), "file": str(f)}, None)
        for body in sorted(p for p in directory.iterdir() if p.is_dir())
        for f in sorted(body.glob(f"*-in-{name}.ll"))
    ]


def level_of(config: str) -> str:
    """The -O level a config name ends in (d_faces-O1), -O2 for the ladders."""
    found = re.search(r"-(O\d|Os|Oz|Omax)$", config)
    return "-" + (found[1] if found else "O2")


def licm_counts(text: str) -> dict[str, collections.Counter]:
    """Per function, the LICM events of `opt -passes='loop-mssa(licm)'` on `text` by remark name (Hoisted, Sunk, PromoteLoopAccessesToScalar, ...).
    The pinned opt is built without LLVM_FORCE_ENABLE_STATS, so `-stats` prints nothing: the remarks are the counters that exist."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "remarks.yaml"
        done = subprocess.run(
            [str(LLVM_BIN / "opt"), *target_args(), "-passes=loop-mssa(licm)", "-disable-output", f"-pass-remarks-output={out}", "-"], input=normalise(text), capture_output=True, text=True
        )
        if done.returncode:
            raise RuntimeError("opt licm: " + (done.stderr.strip().splitlines() or ["failed"])[0])
        found: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
        if out.is_file():
            for block in out.read_text().split("--- "):
                name = re.search(r"^Name:\s+(\S+)", block, re.M)
                function = re.search(r"^Function:\s+(\S+)", block, re.M)
                if name and function and re.search(r"^Pass:\s+licm\b", block, re.M):
                    found[function[1]][name[1]] += 1
    return dict(found)


def isolated(replay: Path, row: dict, source: Path) -> str:
    """llrm's answer to the input alone: `llrm-pass` runs the one pass PASS once on `source`."""
    try:
        order = ["--order"] if "@" in row["pass"] else []
        mode = targetdesc.default()
        layout = ["--mode", mode[1:]] if mode != "host" else []
        done = subprocess.run([str(replay), level_of(row["config"]), *layout, *order, row["pass"], str(source)], capture_output=True, text=True)
    except OSError as error:
        raise RuntimeError(f"{replay}: {error}") from error
    if done.returncode:
        raise RuntimeError("llrm-pass " + row["pass"] + ": " + failure_line(done.stderr))
    SEEN.add(llrm_target(done.stderr, mode))
    if order:
        KINDS[row["pass"]] = next((line.split("slot ", 1)[1] for line in done.stderr.splitlines() if "slot " in line), "?")
    return done.stdout


def failure_line(stderr: str) -> str:
    """What llrm-pass failed with: the panic's message or the last line, not the `slot`/`target` header lines it prints first (two panics read as "slot indvars@738 Simplify Old")."""
    lines = [line for line in stderr.strip().splitlines() if line.strip()]
    for i, line in enumerate(lines):
        if "panicked at" in line and i + 1 < len(lines):
            return f"{line.split('panicked at ', 1)[1].rstrip(':')}: {lines[i + 1].strip()}"
    return next((line for line in reversed(lines) if not line.startswith("note: run with")), "failed")


SEEN: set[str] = set()  # the target line llrm-pass printed, one per distinct answer


def llrm_target(stderr: str, mode: str) -> str:
    """The `target ...` line llrm-pass printed, refused unless it is the machine of `mode`: Neutral priced unroll/licm/rotate/indvars/loopidiom with unit costs
    against opt priced for the target (matmul unrolled to 36 blocks, opt's 216), and every count taken that way was an instrument number."""
    line = next((l.split("target ", 1)[1] for l in stderr.splitlines() if l.startswith("llrm-pass: target ")), None)
    if mode == "host":
        return line or "neutral"
    want = f"x86-{mode}"
    if line is None or not line.startswith(want + " cpu "):
        raise SystemExit(f"difftest: llrm side is not {want} ({line or 'llrm-pass named no target'}): no count is emitted from a Neutral machine")
    tti = targetdesc.targets()[mode].get("tti", "none")
    if f" tti {tti} " not in line:
        raise SystemExit(f"difftest: llrm side {line} does not price with TTI {tti}")
    llrm_cpu = line.split(" cpu ", 1)[1].split()[0]
    opt_cpu = targetdesc.targets()[mode]["llvm_cpu"]
    if opt_cpu.removeprefix("i") != llrm_cpu.removeprefix("i"):
        raise SystemExit(f"difftest: llrm prices for cpu {llrm_cpu} but opt is told -mcpu={opt_cpu}: the two sides compare different machines")
    return line


KINDS: dict[str, str] = {}  # slot -> what llrm-pass said stands in it ("sroa@1179 Early Translated (LLVM: sroa<modify-cfg>)")


def premise(changed: int, functions: int, differing: int = 0) -> str:
    """"ok", or why the comparison proves nothing: it must be able to give more than one answer on these inputs. It can when LLVM changes some
    function and leaves some alone, or when it and llrm disagree somewhere (`differing`). If LLVM changed none, or all, of the functions and
    agrees everywhere, agreement shows nothing about llrm's pass."""
    if functions == 0:
        return "PREMISE FAILED: no function was compared, so nothing was shown"
    if functions >= 2 and differing == 0 and changed == 0:
        return f"PREMISE FAILED: LLVM changed none of {functions} functions and agrees everywhere: the reference never acted, so agreement proves nothing"
    if functions >= 2 and differing == 0 and changed == functions:
        return f"PREMISE FAILED: LLVM changed all {functions} functions and agrees everywhere: the reference never declined, so it was not shown to discriminate"
    return "ok"


def analyse(todo: list[tuple[dict, Path]], root: Path, name: str, jobs: int = 8, replay: Path | None = None) -> dict:
    """The report of pass `name` over `todo`. With `replay` (llrm-pass) llrm's answer is the pass run alone on the input; without, the next step's
    input, which is the pass's answer only if nothing but the pass ran between (the report counts, with `replay`, where the two differ)."""
    passes = counterpart(name)

    def one(item: tuple[dict, Path]) -> tuple[dict, dict, bool]:
        row, answer = item
        source = Path(row["file"]) if Path(row["file"]).is_absolute() else root / row["file"]
        proxy = answer.read_text() if answer else None
        try:
            ours = isolated(replay, row, source) if replay else proxy
        except RuntimeError as error:
            return row, {"error": str(error)}, False
        return row, compare(source.read_text(), ours, passes), ours == proxy

    with ThreadPoolExecutor(jobs) as pool:
        done = list(pool.map(one, todo))
    failures = [(r, c["error"]) for r, c, _ in done if "error" in c]
    compared = [(r, c) for r, c, _ in done if "error" not in c]
    judged = [(r, fn, v) for r, c in compared for fn, v in c.items()]
    changed = [x for x in judged if not x[2]["unchanged_by_llrm"] or not x[2]["unchanged_by_llvm"]]
    llvm_changed = sum(1 for _, _, v in judged if not v["unchanged_by_llvm"])
    llrm_only, llvm_only = collections.Counter(), collections.Counter()
    for _, _, v in changed:
        llrm_only.update(v["llrm_only"])
        llvm_only.update(v["llvm_only"])
    return {
        "pass": name,
        "counterpart": passes,
        "slot": KINDS.get(name),
        "source": source_link(passes),
        "answer": "the pass run alone (llrm-pass)" if replay else "next-input proxy",
        "proxy_equals_isolated": sum(1 for _, c, same in done if same and "error" not in c) if replay and all(answer for _, answer in todo) else None,
        "inputs": len(todo),
        "parse_failures": len(failures),
        "first_failures": [{"file": r["file"], "error": e} for r, e in failures[:5]],
        "functions": len(judged),
        "llvm_changed": llvm_changed,
        "premise": premise(llvm_changed, len(judged), sum(1 for _, _, v in changed if not v["identical"])),
        "acted_on": len(changed),
        "identical": sum(1 for _, _, v in changed if v["identical"]),
        "histogram_llrm_only": dict(llrm_only.most_common()),
        "histogram_llvm_only": dict(llvm_only.most_common()),
        "examples": [{"file": r["file"], "fn": fn, "hunk": v["hunk"]} for r, fn, v in changed if not v["identical"]][:3],
    }


def table(report: dict) -> str:
    """The report as the markdown a comment on #1994 holds."""
    rows = [
        f"### {report['pass']} against `opt -passes={report['counterpart']}` ({report['source']})",
        "",
        f"llrm's answer: {report['answer']}"
        + (f"; the next input equals it on {report['proxy_equals_isolated']} of {report['inputs']}" if report["proxy_equals_isolated"] is not None else "")
        + f"; inputs {report['inputs']}; failed (LLVM or llrm) {report['parse_failures']}; functions compared {report['functions']}; "
        f"where either side changed the function: {report['acted_on']}, identical {report['identical']}; LLVM changed {report['llvm_changed']} of "
        f"{report['functions']}; premise {report['premise']}",
        "",
        "| opcode | only in llrm's answer | only in LLVM's |",
        "|---|---|---|",
    ]
    ops = sorted(set(report["histogram_llrm_only"]) | set(report["histogram_llvm_only"]), key=lambda o: -(report["histogram_llrm_only"].get(o, 0) + report["histogram_llvm_only"].get(o, 0)))
    rows += [f"| {op} | {report['histogram_llrm_only'].get(op, 0)} | {report['histogram_llvm_only'].get(op, 0)} |" for op in ops[:12]]
    for failure in report["first_failures"][:3]:
        rows.append(f"\nunreadable: `{failure['file']}`: {failure['error']}")
    for example in report["examples"]:
        rows += ["", f"`{example['file']}` @{example['fn']}:", "```diff", *example["hunk"], "```"]
    return "\n".join(rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run")
    run.add_argument("pass_name", metavar="PASS")
    run.add_argument("--inputs", type=Path, default=INPUTS)
    run.add_argument("--config", nargs="*")
    run.add_argument("--limit", type=int, default=0)
    run.add_argument("--inputs-of", nargs="+", metavar="PASS", help="the manifest has no rows of PASS: take the inputs of these passes (fold, algebraic ...) as PASS's inputs")
    run.add_argument("--json", type=Path)
    run.add_argument("--allow-env", nargs="*", default=[], metavar="NAME", help="the LLRM_CHECK_*/LLRM_EXP_* switch under test; any other in the environment is refused")
    run.add_argument("--llrm-pass", type=Path, help="llrm-pass (src/bin/llrm-pass.rs): llrm's answer is the pass run alone on the input, not the next input")
    dumps = sub.add_parser("dumps")
    dumps.add_argument("pass_name", metavar="PASS")
    dumps.add_argument("directory", type=Path, nargs="+")
    dumps.add_argument("--limit", type=int, default=0)
    dumps.add_argument("--json", type=Path)
    dumps.add_argument("--allow-env", nargs="*", default=[], metavar="NAME", help="the LLRM_CHECK_*/LLRM_EXP_* switch under test; any other in the environment is refused")
    dumps.add_argument("--llrm-pass", type=Path, help="the llrm-pass of the tree that made the dumps")
    inputs = sub.add_parser("inputs", help="the pass or slot on the inputs `llrm-c --dump DIR` made with LLRM_DUMP_INPUTS=1: every run, changed or not")
    inputs.add_argument("pass_name", metavar="PASS")
    inputs.add_argument("directory", type=Path, nargs="+")
    inputs.add_argument("--limit", type=int, default=0)
    inputs.add_argument("--json", type=Path)
    inputs.add_argument("--allow-env", nargs="*", default=[], metavar="NAME", help="the LLRM_CHECK_*/LLRM_EXP_* switch under test; any other in the environment is refused")
    inputs.add_argument("--llrm-pass", type=Path, required=True, help="llrm-pass: llrm's answer is the pass run alone on each input")
    inputs.add_argument("--export", type=Path, metavar="DIR", help="write input, llrm and opt text and the diff of every differing function under DIR/<slot>/")
    licm = sub.add_parser("licm", help="the LICM remarks of opt on FILE, per function")
    licm.add_argument("input", type=Path)
    pair = sub.add_parser("pair")
    pair.add_argument("input", type=Path)
    pair.add_argument("llrm_out", type=Path)
    pair.add_argument("--pass", dest="pass_name", required=True)
    args = parser.parse_args()
    if args.command == "licm":
        print(f"opt: {LLVM_BIN / 'opt'}")
        for function, events in sorted(licm_counts(args.input.read_text()).items()):
            print(f"{function}: " + ", ".join(f"{name} {n}" for name, n in sorted(events.items())))
        return 0
    if args.command != "pair":
        envguard.require_clean("difftest", allow=args.allow_env)
    if args.pass_name.split("@")[0] not in COUNTERPART:
        print(f"difftest: {args.pass_name} has no counterpart in LLVM; the passes with one: {' '.join(sorted(COUNTERPART))}", file=sys.stderr)
        return 2
    if args.command == "pair":
        result = compare(args.input.read_text(), args.llrm_out.read_text(), counterpart(args.pass_name))
        print(json.dumps(result, indent=1))
        return 0
    root = args.inputs if args.command == "run" else Path("/")
    todo = pairs(relabelled(manifest(root, args.config), args.pass_name, args.inputs_of), args.pass_name, root) if args.command == "run" else [x for d in args.directory for x in (input_pairs if args.command == "inputs" else dump_pairs)(d, args.pass_name)]
    if args.limit:
        todo = todo[: args.limit]
    if getattr(args, "export", None):
        print(f"exported {export(todo, args.pass_name, args.llrm_pass, args.export)} differing function(s) to {args.export / args.pass_name}")
    report = analyse(todo, root, args.pass_name, replay=args.llrm_pass)
    version = subprocess.run([str(LLVM_BIN / "opt"), "--version"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    print(f"opt: {LLVM_BIN / 'opt'} ({version}); target {' '.join(target_args()) or 'host default'}")
    for seen in sorted(SEEN):
        print(f"llrm-pass: target {seen}")
    print(table(report))
    if getattr(args, "json", None):
        args.json.write_text(json.dumps(report, indent=1))
    return 0 if report["premise"] == "ok" else 3


if __name__ == "__main__":
    sys.exit(main())
