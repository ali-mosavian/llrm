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

INPUTS = Path(os.environ.get("LLRM_DIFFTEST_INPUTS") or Path.home() / "scratch/replay/inputs")
LLVM_BIN = Path(os.environ.get("LLVM_BIN") or (Path.home() / "scratch/llvm-build/bin" if (Path.home() / "scratch/llvm-build/bin/opt").is_file() else "/usr/lib/llvm-20/bin"))

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
    "trivialunswitch": "loop-mssa(simple-loop-unswitch<no-nontrivial>)",
    "dead": "adce",
    "tailrec": "tailcallelim",
    "memcpyopt": "memcpyopt",
}


# The C++ each counterpart is, under llvm/lib at d1106deb71cc: a difference belongs to the function of that file that does the work.
SOURCE = {
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
    "tailcallelim": "Transforms/Scalar/TailRecursionElimination.cpp",
    "memcpyopt": "Transforms/Scalar/MemCpyOptimizer.cpp",
}
LLVM_COMMIT = "d1106deb71cc"


def source_link(passes: str) -> str:
    path = SOURCE.get(passes)
    return f"[{path.rsplit('/', 1)[1]}](https://github.com/llvm/llvm-project/blob/{LLVM_COMMIT}/llvm/lib/{path})" if path else "-"


LLRM_CONVENTIONS = re.compile(r"(?<![\w@%$.\"-])(?:watcallcc|sysvcc|ia16cc|regparm3cc|regparm3popcc|cc[0-9]+)(?![\w$.\"-]) ?")
DECLARED_INTERNAL = re.compile(r"^declare internal ")


def normalise(text: str) -> str:
    """llrm's text in the dialect LLVM reads: its calling-convention words (`watcallcc`, `sysvcc`, `cc1000`) and `internal` on a declaration. Only
    those tokens, in code: not a name that contains one, not a string, not a comment."""
    out = []
    for line in text.splitlines(keepends=True):
        parts = re.split(r'("(?:[^"\\]|\\.)*"|;.*$)', line.rstrip("\n"), flags=re.M)
        code = [p if i % 2 else LLRM_CONVENTIONS.sub("", p) for i, p in enumerate(parts)]
        line = "".join(code) + ("\n" if line.endswith("\n") else "")
        out.append(DECLARED_INTERNAL.sub("declare ", line))
    return "".join(out)


def opt(passes: str, text: str) -> subprocess.CompletedProcess:
    return subprocess.run([str(LLVM_BIN / "opt"), f"-passes={passes}", "-S", "-"], input=text, capture_output=True, text=True)


def functions(text: str) -> dict[str, list[str]]:
    """name -> the lines of each defined function."""
    found: dict[str, list[str]] = {}
    current = None
    for line in text.splitlines():
        if line.startswith("define "):
            current = re.search(r"@([\w.$-]+)\(", line)[1]
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


def canonical(lines: list[str]) -> list[str]:
    """The function with its blocks in control-flow order, its local names numbered by first appearance, and what is not the code left out
    (comments, function and parameter attributes, `align`, metadata attachments, the order of a phi's incoming pairs). Two bodies that differ
    only in those read alike."""
    lines = trimmed(lines)
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


def manifest(root: Path, configs: list[str] | None) -> list[dict]:
    rows = []
    for config in sorted(p.name for p in root.iterdir() if (p / "MANIFEST.tsv").is_file()):
        if configs and config not in configs:
            continue
        for line in (root / config / "MANIFEST.tsv").read_text().splitlines():
            c, fn, name, run, file, instructions = line.split("\t")
            rows.append({"config": c, "fn": fn, "pass": name, "run": int(run), "file": file, "instructions": int(instructions)})
    return rows


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
        for number, slot, step, here in steps:
            before = [(abs(s - slot), f) for n, s, _, f in steps if n == number - 1]
            if step == name and before:
                out.append(({"config": directory.name, "fn": body.name, "pass": name, "run": number, "file": str(min(before)[1])}, here))
    return out


def level_of(config: str) -> str:
    """The -O level a config name ends in (d_faces-O1), -O2 for the ladders."""
    found = re.search(r"-(O\d|Os|Oz|Omax)$", config)
    return "-" + (found[1] if found else "O2")


def isolated(replay: Path, row: dict, source: Path) -> str:
    """llrm's answer to the input alone: `llrm-pass` runs the one pass PASS once on `source`."""
    try:
        done = subprocess.run([str(replay), level_of(row["config"]), row["pass"], str(source)], capture_output=True, text=True)
    except OSError as error:
        raise RuntimeError(f"{replay}: {error}") from error
    if done.returncode:
        raise RuntimeError("llrm-pass " + row["pass"] + ": " + (done.stderr.strip().splitlines() or ["failed"])[0])
    return done.stdout


def analyse(todo: list[tuple[dict, Path]], root: Path, name: str, jobs: int = 8, replay: Path | None = None) -> dict:
    """The report of pass `name` over `todo`. With `replay` (llrm-pass) llrm's answer is the pass run alone on the input; without, the next step's
    input, which is the pass's answer only if nothing but the pass ran between (the report counts, with `replay`, where the two differ)."""
    passes = COUNTERPART[name]

    def one(item: tuple[dict, Path]) -> tuple[dict, dict, bool]:
        row, answer = item
        source = Path(row["file"]) if Path(row["file"]).is_absolute() else root / row["file"]
        proxy = answer.read_text()
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
    llrm_only, llvm_only = collections.Counter(), collections.Counter()
    for _, _, v in changed:
        llrm_only.update(v["llrm_only"])
        llvm_only.update(v["llvm_only"])
    return {
        "pass": name,
        "counterpart": passes,
        "source": source_link(passes),
        "answer": "the pass run alone (llrm-pass)" if replay else "next-input proxy",
        "proxy_equals_isolated": sum(1 for _, c, same in done if same and "error" not in c) if replay else None,
        "inputs": len(todo),
        "parse_failures": len(failures),
        "first_failures": [{"file": r["file"], "error": e} for r, e in failures[:5]],
        "functions": len(judged),
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
        f"where either side changed the function: {report['acted_on']}, identical {report['identical']}",
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
    pair = sub.add_parser("pair")
    pair.add_argument("input", type=Path)
    pair.add_argument("llrm_out", type=Path)
    pair.add_argument("--pass", dest="pass_name", required=True)
    args = parser.parse_args()
    if args.command != "pair":
        envguard.require_clean("difftest", allow=args.allow_env)
    if args.pass_name not in COUNTERPART:
        print(f"difftest: {args.pass_name} has no counterpart in LLVM; the passes with one: {' '.join(sorted(COUNTERPART))}", file=sys.stderr)
        return 2
    if args.command == "pair":
        result = compare(args.input.read_text(), args.llrm_out.read_text(), COUNTERPART[args.pass_name])
        print(json.dumps(result, indent=1))
        return 0
    root = args.inputs if args.command == "run" else Path("/")
    todo = pairs(manifest(root, args.config), args.pass_name, root) if args.command == "run" else [x for d in args.directory for x in dump_pairs(d, args.pass_name)]
    if args.limit:
        todo = todo[: args.limit]
    report = analyse(todo, root, args.pass_name, replay=args.llrm_pass)
    version = subprocess.run([str(LLVM_BIN / "opt"), "--version"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    print(f"opt: {LLVM_BIN / 'opt'} ({version})")
    print(table(report))
    if getattr(args, "json", None):
        args.json.write_text(json.dumps(report, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
