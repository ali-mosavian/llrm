#!/usr/bin/env python3
"""Where llrm's compile time goes on one file, and what gcc and clang spend in the same places.

    uv run --project tools python tools/compile_map.py [--source render/d_faces.c] [--level -O1] [--json OUT] [--diff OLD.json]

llrm: `perf stat -e instructions:u` for the total, `LLRM_DEBUG=time` for each step's own instructions (the same thread's
user-space count). The rows must add up to the total; the map says how far they do not. gcc: `-ftime-report` and clang:
`-ftime-trace` on a file holding 24 copies of the source (their timers are too coarse for one), each category's share of the
run applied to the one-file `perf stat` total, so a row's target is in the same unit: millions of user instructions. Default flags
throughout, what users get; the command lines and the commit are in the header, so that two maps made under other flags are not
compared. Every row names its owner, and a row of 2% or more with none is flagged.

Cycles (`perf stat -e cycles:u`, best of 7 with the spread) are given for the whole compile of each compiler, because an edit can cut instructions and
lose cycles; per-step cycles are not measurable.

llrm's counts are exact and repeat to the instruction; gcc's and clang's shares are times, summed over three runs, and move by
about 15% on a small category between runs, so read their columns as a target, not a measurement of the last digit.

The source is QCport's, read from $QCPORT (the directory that holds render/d_faces.c).
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import corpora  # noqa: E402
import pin_clang  # noqa: E402
import llrmbin  # noqa: E402

QCPORT = Path(os.environ.get("QCPORT", Path.home() / "scratch/qbq-llrm/src"))
KEYWORDS = "-Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near=".split()
INCLUDES = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(QCPORT / d))]
FOREIGN = [*KEYWORDS, "-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-w", *INCLUDES]
COPIES = 24
REPEATS = 3
CYCLE_RUNS = 7   # cycles are noisy on a shared host
FLAG_AT = 2.0   # percent of the total: an unowned row at or above it is flagged

# (pattern on a step's name, category, owner). The first that matches. Owners are the sessions' current work; None is unowned.
ROWS = [
    (r"^(frontend|hir|data layout|global names|debug info|data placement|mir (verify|link|runtime|markers|window))", "frontend", None),
    (r"^(analysis (points-to|pointer-values|call-effects|writes|exposed-frames|through-memory|float-facts|annotated|published|terminal-sites|callbacks)|ssa |summaries (points-to|values|visit|callbacks|topology))", "analysis: pointers", "points-to"),
    (r"^(analysis accesses|mir dse|mir memcpyopt|mir loopmotion)", "analysis: memory", "alias-queries"),
    (r"^(analysis (globals-aa|summaries|call-graph|callee-effects|call-registers|global-sizes|declarations|type-ancestry)|summaries direct|mir (stamp|interprocedural|outer analyses|spares|homes))", "analysis: module", "analyses-carry"),
    (r"^analysis (bounded|dominated-edges|registers|counted|followers|demanded)", "analysis: ranges", "ranger"),
    (r"^analysis ", "analysis: other", None),
    (r"^mir ", "mir passes", None),
    (r"^isel|^lir (far|schedule|shrink|duplicated)", "isel", None),
    (r"^(facts (hints|sibling prices|fold prices)|siblings )", "regalloc: hints", "hint-slope"),
    (r"^(facts |classes |intervals |regalloc |split |spill |after |lir (regalloc|ssaspill)|candidate |ssa |siblings )", "regalloc", "allocator"),
    (r"^lir (peephole|jumps|floatassign|floatalloc|floatfold|coalesce|phielim|twoaddr|parcopy|loopslots|frame)", "lir passes", None),
    (r"^(assemble|machine|objbuild|emit|frame laid|masm|stack checks)", "emit", None),
]
# The same categories in gcc's timers and clang's trace events.
GCC = [
    (r"^(parser|lexical|preprocessing|name lookup|phase setup|phase parsing|tree gimplify)", "frontend"),
    (r"^(alias analysis|alias stmt|tree PTA|tree modref|tree SSA incremental|tree operand scan)", "analysis: pointers"),
    (r"^(dead store|tree DSE|tree PRE|tree FRE)", "analysis: memory"),
    (r"^(callgraph construction|ipa )", "analysis: module"),
    (r"^(tree VRP|tree SSA|ranger|dominance|tree CCP|tree conservative)", "analysis: ranges"),
    (r"^(tree |dominator|forward prop|PHI|scev|loop |complete unrolling|SSA |if-conversion early|ipa|expand vector)", "mir passes"),
    (r"^(expand|RTL|rtl expand)", "isel"),
    (r"^(integrated RA|LRA|reload|IRA|regmove|register scan|reg stack)", "regalloc"),
    (r"^(df |hard reg|register information|combiner|scheduling|peephole|jump|CSE|CPROP|thread pro|shorten|reorder|branch|final|rest of compilation|if-conversion|mode switching|var-tracking|dwarf|machine dep|compare elim|address taken|remove unused|DCE|TV)", "lir passes"),
    (r"^(out of ssa|cfg cleanup|expand vars)", "mir passes"),
]
CLANG = [
    (r"^(Frontend|Source|Parse|ParseDeclarationOrFunctionDefinition|ParseFunctionDefinition|PerformPendingInstantiations|Sema)", "frontend"),
    (r"^(AAManager|BasicAA|MemorySSA|AAEval|MemoryDependence|GlobalsAA|PointerAnalysis|TypeBasedAA|ScopedNoAlias|CaptureTracking)", "analysis: pointers"),
    (r"^(DSE|MemCpyOpt|DSEPass|MemCpyOptPass|LICM|LoopLoadElimination|GVN|NewGVN|MergedLoadStoreMotion)", "analysis: memory"),
    (r"^(ModuleInliner|Inliner|IPSCCP|GlobalOpt|FunctionAttrs|Argument|DeadArg|GlobalDCE|InferFunctionAttrs|CalledValue|ConstantMerge)", "analysis: module"),
    (r"^(ScalarEvolution|LazyValue|CorrelatedValue|DemandedBits|SCEV|VRP|Range|LoopInfo|DominatorTree|AssumptionCache|BlockFrequency|BranchProbability)", "analysis: ranges"),
    (r"Pass$|^(InstCombine|SimplifyCFG|SROA|Optimizer|PassManager|OptModule|OptFunction|OptimizerPass|Loop|LoopPass|RunLoopPass|RunPass$)", "mir passes"),
    (r"(DAG->DAG|SelectionDAG|Instruction Selection|ISel|Legalize|DAGCombine|Scheduler for Selection|CodeGen Function|DAG)", "isel"),
    (r"(Greedy Register|Register Coalescer|Live Interval|Virtual Register|Live Variables|Slot index|Spill|Register Allocator|Live Range|Machine Copy|Simple Register Coalescing|Eliminate PHI|Two-Address|Rename Disconnected|Machine Block Frequency|Stack Slot|Prologue|Prolog/Epilog)", "regalloc"),
    (r"(Machine|X86|Branch|Tail|Peephole|Post-RA|Expand|Insert|Finalize|BreakFalse|Fixup|Dead Machine|Assembly|Emit|Object|MC|Asm|PostRA)", "lir passes"),
]


def category(name: str, table) -> str | None:
    for entry in table:
        if re.search(entry[0], name):
            return entry[1]
    return None


def owner_of(name: str):
    for pattern, cat, owner in ROWS:
        if re.search(pattern, name):
            return cat, owner
    return "other", None


def run(cmd, **kw):
    done = subprocess.run([str(c) for c in cmd], capture_output=True, text=True, **kw)
    return done


def instructions(cmd, env=None):
    """User-space instructions of one run (perf stat, children included)."""
    done = run(["perf", "stat", "-e", "instructions:u", "-x,", *cmd], env=env)
    line = [one for one in done.stderr.splitlines() if "instructions" in one][-1]
    return int(line.split(",")[0])


def parse_counters(stderr: str) -> dict:
    """{event: count} from `perf stat -x,` output."""
    counts = {}
    for line in stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[0].isdigit():
            counts[fields[2].split(":")[0]] = int(fields[0])
    return counts


def cycles(cmd, env=None, runs=CYCLE_RUNS):
    """Cycles and instructions of `runs` runs. Instructions repeat; cycles move with the machine's load, so the best run is the
    figure and the spread (max - min over the best) says how far to trust it."""
    seen = [parse_counters(run(["perf", "stat", "-e", "cycles:u,instructions:u", "-x,", *cmd], env=env).stderr) for _ in range(runs)]
    best = min(one["cycles"] for one in seen)
    worst = max(one["cycles"] for one in seen)
    return {"cycles_M": best / 1e6, "spread_pct": 100 * (worst - best) / best,
            "ipc": min(one["instructions"] for one in seen) / best}


def parse_steps(stderr: str):
    """(count, own M, total M, name) of each `[instr]` row, and the run's own `total` line."""
    rows, total = [], None
    for line in stderr.splitlines():
        match = re.match(r"\[instr\]\s+([\d.]+) Minstr own\s+([\d.]+) Minstr total\s+(\d+)x (.+)$", line)
        if match:
            rows.append((int(match[3]), float(match[1]), float(match[2]), match[4].strip()))
        match = re.match(r"\[instr\] total ([\d.]+) Minstr, outermost steps ([\d.]+) Minstr, outside every step ([\d.]+)", line)
        if match:
            total = tuple(float(x) for x in match.groups())
    return rows, total


def parse_gcc(report: str):
    """(name, user seconds) of each timer of `-ftime-report`, and its TOTAL. The phase rows and the two callgraph rows that contain
    others are left out."""
    rows, total = [], None
    for line in report.splitlines():
        whole = re.match(r"^\s*TOTAL\s*:\s+([\d.]+)", line)
        if whole:
            total = float(whole[1])
            continue
        match = re.match(r"^\s*(.+?)\s*:\s+([\d.]+)\s+\(\s*\d+%\)\s+([\d.]+)", line)
        if match:
            name, user = match[1].strip(), float(match[2])
            if not name.startswith("phase ") and name not in ("callgraph functions expansion", "callgraph ipa passes"):
                rows.append((name, user))
    return rows, total


def self_times(events):
    """name -> microseconds spent in an event and not in the events nested in it (clang's `-ftime-trace`, complete events)."""
    done = sorted((e for e in events if e.get("ph") == "X" and not e["name"].startswith("Total ")), key=lambda e: (e["ts"], -e["dur"]))
    own = {id(e): e["dur"] for e in done}
    stack = []
    for event in done:
        while stack and stack[-1]["ts"] + stack[-1]["dur"] <= event["ts"]:
            stack.pop()
        if stack:
            own[id(stack[-1])] -= event["dur"]
        stack.append(event)
    out = collections.Counter()
    for event in done:
        detail = event.get("args", {}).get("detail", "") if event["name"] == "RunPass" else ""
        out[f'{event["name"]} {detail}'.strip()] += max(0, own[id(event)])
    return out


def copies(source: Path, work: Path) -> Path:
    """`source` included COPIES times, each with every name it defines renamed, so that one file takes long enough for a timer that
    reads in hundredths of a second."""
    names = set()
    obj = work / "names.o"
    done = run(["gcc", *FOREIGN, "-O0", "-c", source, "-o", obj])
    if done.returncode == 0:
        for line in run(["nm", obj]).stdout.splitlines():
            parts = line.split()
            if len(parts) == 3 and parts[1] not in "UrR" and not parts[2].startswith((".", "__x86")):
                names.add(parts[2])
    text = source.read_text()
    names |= set(re.findall(r"static\s+[\w\s\*]*?\b(\w+)\s*\(", text)) | set(re.findall(r"typedef\s+struct\s*\{[^}]*\}\s*(\w+);", text))
    out = []
    for k in range(COPIES):
        out += [f"#define {n} {n}_{k}" for n in sorted(names)] + [f'#include "{source}"'] + [f"#undef {n}" for n in sorted(names)]
    path = work / "copies.c"
    path.write_text("\n".join(out) + "\n")
    return path


def commit() -> str:
    return run(["git", "-C", ROOT, "log", "-1", "--format=%h %s"]).stdout.strip()


def output_args(kind: str, directory: Path) -> tuple[list[str], list[str]]:
    """(llrm's, gcc's and clang's) output arguments for one kind of output: `asm` is `-S -o /dev/null` for all three (the listing is written, 15.6 M of text for llrm
    at d_faces -O2); `obj` is an object file for all three (what users build). Both columns of a ratio use the same kind."""
    if kind == "asm":
        return ["-S", "-o", "/dev/null"], ["-S", "-o", "/dev/null"]
    return ["-o", str(directory / "o.obj")], ["-c", "-o", str(directory / "o.o")]


def build_map(args) -> dict:
    llrm = llrmbin.compiler(llrmbin.bin_dir())
    source = args.source if args.source.is_absolute() else QCPORT / args.source
    outdir = Path(tempfile.mkdtemp(prefix="compile_map_out"))
    import atexit, shutil
    atexit.register(shutil.rmtree, outdir, True)
    llrm_out, foreign_out = output_args(args.output, outdir)
    base = [str(llrm), "-c", *llrmbin.c_args(32), args.level, *INCLUDES, *llrm_out, str(source)]
    env = {"PATH": os.environ["PATH"], **llrmbin.cc_environment()}
    total = instructions(base, env)
    timing = {"llrm": cycles(base, env)}
    timed_env = {**env, "LLRM_DEBUG": "time", "LLRM_TIME_TOP": "1000"}
    timed = run(base, env=timed_env)
    instrumented = instructions(base, timed_env)
    with tempfile.TemporaryDirectory() as empty_dir:  # what a compile of nothing costs: loader, runtime start, exit
        empty = Path(empty_dir) / "empty.c"
        empty.write_text("int f(void){return 1;}\n")
        startup = instructions([*base[:-1], str(empty)], env) / 1e6 if base[-1] == str(source) else None
    rows, step_total = parse_steps(timed.stderr)
    own_sum = sum(r[1] for r in rows)
    result = {
        "header": {"commit": commit(), "llrm": " ".join(base), "output": args.output, "gcc": f"gcc {' '.join(FOREIGN)} {args.level} {' '.join(foreign_out)} {source}",
                   "clang": f"{pin_clang.path()} {' '.join(FOREIGN)} {args.level} {' '.join(foreign_out)} {source}"},
        "total_M": total / 1e6, "instrumented_M": instrumented / 1e6, "rows_sum_M": own_sum, "debug_total_M": step_total[0] if step_total else None, "startup_M": startup,
        "outside_every_step_M": step_total[2] if step_total else None,
        "load_average": list(os.getloadavg()),
        "rows": [], "categories": {},
    }
    for count, own, whole, name in sorted(rows, key=lambda r: -r[1]):
        cat, owner = owner_of(name)
        result["rows"].append({"name": name, "count": count, "own_M": own, "total_M": whole, "category": cat, "owner": owner})
    cats = collections.defaultdict(float)
    scale = total / instrumented
    result["scale"] = scale
    for r in result["rows"]:
        cats[r["category"]] += r["own_M"] * scale
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        many = copies(source, work)
        gcc_one = instructions(["gcc", *FOREIGN, args.level, *foreign_out, str(source)], env) / 1e6
        clang_one = instructions([pin_clang.path(), *FOREIGN, args.level, *foreign_out, str(source)], env) / 1e6
        for name in ("gcc", "clang"):
            timing[name] = cycles([pin_clang.path() if name == "clang" else name, *FOREIGN, args.level, *foreign_out, str(source)], env)
        # Their timers are time, not instructions: three runs, summed.
        sums, gtotal = collections.Counter(), 0.0
        for _ in range(REPEATS):
            part, whole = parse_gcc(run(["gcc", *FOREIGN, args.level, *foreign_out, "-ftime-report", str(many)]).stderr)
            for name, user in part:
                sums[name] += user
            gtotal += whole or 0.0
        grows = list(sums.items())
        gcat = collections.defaultdict(float)
        for name, user in grows:
            gcat[category(name, GCC) or "other"] += user
        gcat["other"] += max(0.0, (gtotal or 0) - sum(u for _, u in grows))
        selfs = collections.Counter()
        for k in range(REPEATS):
            trace = work / f"trace{k}"
            trace.mkdir()
            run([pin_clang.path(), *FOREIGN, args.level, *(["-S", "-o", trace / "o.s"] if args.output == "asm" else ["-c", "-o", trace / "o.o"]), "-ftime-trace", "-ftime-trace-granularity=20", str(many)])
            selfs.update(self_times(json.loads(next(trace.glob("*.json")).read_text())["traceEvents"]))
        ccat = collections.defaultdict(float)
        for name, micro in selfs.items():
            ccat[category(name, CLANG) or "other"] += micro
        ctotal = sum(ccat.values())
    for cat in sorted(set(cats) | set(gcat) | set(ccat)):
        result["categories"][cat] = {
            "llrm_M": cats.get(cat, 0.0),
            "gcc_M": gcat.get(cat, 0.0) / (gtotal or 1) * gcc_one,
            "clang_M": ccat.get(cat, 0.0) / (ctotal or 1) * clang_one,
        }
    result["gcc_total_M"], result["clang_total_M"] = gcc_one, clang_one
    result["cycles"] = timing
    result["detail"] = {"gcc": {cat: sorted(((u / (gtotal or 1) * gcc_one, n) for n, u in grows if (category(n, GCC) or "other") == cat), reverse=True)[:5]
                                for cat in gcat},
                        "clang": {cat: sorted(((m / (ctotal or 1) * clang_one, n) for n, m in selfs.items() if (category(n, CLANG) or "other") == cat), reverse=True)[:5]
                                  for cat in ccat}}
    return result


def accounting(result: dict) -> list[tuple[str, float]]:
    """The lines of the accounting block, (label, M) in order, and the identities they satisfy:
    instrumented = timed thread + outside the thread; timed thread = rows + outside every step + unexplained. Every figure of a report is quoted with this block."""
    plain, inst = result["total_M"], result["instrumented_M"]
    thread = result["debug_total_M"] if result.get("debug_total_M") is not None else result["rows_sum_M"] + (result["outside_every_step_M"] or 0.0)
    rows, outside = result["rows_sum_M"], result["outside_every_step_M"] or 0.0
    process = inst - thread
    startup = min(result.get("startup_M") or 0.0, process)
    return [("plain perf total (instructions:u, process)", plain), ("instrumented perf total (LLRM_DEBUG=time on)", inst),
            ("instrument overhead (instrumented - plain)", inst - plain), ("timed thread total", thread), ("rows sum (own M of every step)", rows),
            ("outside every step, inside the thread", outside), ("unexplained in the thread (thread - rows - outside)", thread - rows - outside),
            ("outside the timed thread (instrumented - thread)", process), ("  of which start-up and exit (empty-file compile)", startup),
            ("  of which the rest (main-thread work, instrument)", process - startup)]


def show(result: dict, old: dict | None = None) -> str:
    out = []
    head = result["header"]
    out += [f"commit  {head['commit']}", f"llrm    {head['llrm']}", f"gcc     {head['gcc']}", f"clang   {head['clang']}", ""]
    total = result["total_M"]
    inst = result["instrumented_M"]
    thread = result["debug_total_M"] if result.get("debug_total_M") is not None else inst  # an old map without the thread total: the instrumented total
    gap = thread - result["rows_sum_M"]  # the rows add up to the timed thread; the instrumented total also holds the process around it
    load = result.get("load_average")
    out += [f"Host load average at the run (1/5/15 min): {' / '.join(f'{one:.1f}' for one in load)}; instruction counts do not depend on it, a cycles figure does." if load else "Host load average: not recorded."]
    out += [f"Output kind: {result['header'].get('output', 'asm')} for llrm, gcc and clang alike (asm = -S, the listing is written; obj = an object file).",
            "Accounting (M instructions; quote it with any figure of this map):"] + [f"  {label:56}{value:>9,.1f}" for label, value in accounting(result)]
    out.append(f"The rows sum to {100 * result['rows_sum_M'] / thread:.1f}% of the timed thread and {100 * result['rows_sum_M'] / inst:.1f}% of the instrumented total (the instrument's own cost and the process around the thread are in the second only). Tolerance: rows within 2% of the timed thread, else the map is wrong.")
    if abs(gap) > 0.02 * thread:
        out.append("WARNING: the rows are not within 2% of the timed thread; read nothing off this map until that is explained.")
    out.append(f"gcc {result['gcc_total_M']:,.0f} M, clang {result['clang_total_M']:,.0f} M (perf total, one file). llrm/gcc {total / result['gcc_total_M']:.1f}x, llrm/clang {total / result['clang_total_M']:.1f}x.")
    if "cycles" in result:
        out += ["", f"Cycles (best of {CYCLE_RUNS}; spread = worst over best, how far the host's load moved them; per-step cycles are not available)"]
        for name, one in result["cycles"].items():
            ratio = f", {one['cycles_M'] / result['cycles']['gcc']['cycles_M']:.1f}x gcc" if name != "gcc" else ""
            out.append(f"  {name:6}{one['cycles_M']:>9,.0f} M cycles, IPC {one['ipc']:.2f}, spread {one['spread_pct']:.0f}%{ratio}")
    out += ["", f"By category (llrm's rows scaled by {result['scale']:.3f} to the plain total; gcc and clang: their time-share applied to their totals)",
            f"{'category':22}{'llrm':>9}{'%':>6}{'gcc':>9}{'clang':>9}{'llrm/gcc':>10}"]
    for cat, v in sorted(result["categories"].items(), key=lambda kv: -kv[1]["llrm_M"]):
        ratio = f"{v['llrm_M'] / v['gcc_M']:.1f}x" if v["gcc_M"] > 1 else ""
        out.append(f"{cat:22}{v['llrm_M']:>9.0f}{100 * v['llrm_M'] / total:>6.1f}{v['gcc_M']:>9.0f}{v['clang_M']:>9.0f}{ratio:>10}")
    coarse = {"frontend": "front end", "mir passes": "mid-end (passes and their analyses)", "isel": "instruction selection",
              "regalloc": "register allocation", "regalloc: hints": "register allocation", "lir passes": "late machine passes and emit",
              "emit": "late machine passes and emit", "other": "unmatched"}
    buckets = collections.defaultdict(lambda: [0.0, 0.0, 0.0])
    for cat, v in result["categories"].items():
        key = coarse.get(cat, "mid-end (passes and their analyses)" if cat.startswith("analysis") else "unmatched")
        for k, name in enumerate(("llrm_M", "gcc_M", "clang_M")):
            buckets[key][k] += v[name]
    out += ["", "The same in five buckets (the matching of single passes between three compilers is coarse; these are the safer comparison)",
            f"{'bucket':38}{'llrm':>8}{'gcc':>8}{'clang':>8}{'llrm/gcc':>10}{'llrm/clang':>12}"]
    for key, (l, g, c) in sorted(buckets.items(), key=lambda kv: -kv[1][0]):
        out.append(f"{key:38}{l:>8.0f}{g:>8.0f}{c:>8.0f}{(f'{l / g:.1f}x' if g > 1 else ''):>10}{(f'{l / c:.1f}x' if c > 1 else ''):>12}")
    before = {r["name"]: r["own_M"] for r in (old or {}).get("rows", [])}
    out += ["", f"By step (own M as measured under the instrument; % of the instrumented total), rows of 0.5% and more, then one line for the rest{', change against the other map' if old else ''}",
            f"{'step':34}{'n':>6}{'own M':>9}{'%':>6}{'cum %':>7}  {'category':20}{'owner':16}" + ("   change M" if old else "")]
    cum = 0.0
    rest_n, rest_rows, rest_own = 0, 0, 0.0
    for r in result["rows"]:
        share = 100 * r["own_M"] / inst
        if share < 0.5:
            rest_n += r["count"]; rest_rows += 1; rest_own += r["own_M"]
            continue
        cum += share
        flag = "UNOWNED" if r["owner"] is None and share >= FLAG_AT else (r["owner"] or "-")
        delta = f"{r['own_M'] - before.get(r['name'], 0.0):>+10.0f}" if old else ""
        out.append(f"{r['name'][:33]:34}{r['count']:>6}{r['own_M']:>9.0f}{share:>6.1f}{cum:>7.1f}  {r['category']:20}{flag:16}{delta}")
    if rest_rows:  # the table sums to the instrumented total: the rows under 0.5% were ~19 M of it, listed nowhere
        rest_share = 100 * rest_own / inst
        out.append(f"{f'other ({rest_rows} rows under 0.5%)':34}{rest_n:>6}{rest_own:>9.0f}{rest_share:>6.1f}{cum + rest_share:>7.1f}  {'-':20}{'-':16}")
    unowned = [r for r in result["rows"] if r["owner"] is None and 100 * r["own_M"] / inst >= FLAG_AT]
    out += ["", f"{len(unowned)} unowned row(s) at {FLAG_AT}% or more (of the instrumented total): "
            + (", ".join(f"{r['name']} ({100 * r['own_M'] / inst:.1f}%)" for r in unowned) or "none")]
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", type=Path, default=Path("render/d_faces.c"))
    parser.add_argument("--level", default="-O1")
    parser.add_argument("--output", choices=("asm", "obj"), default="asm", help="the output of all three compilers: assembly text (-S) or an object file; ratios are only between columns of one kind")
    parser.add_argument("--json", type=Path)
    parser.add_argument("--detail", action="store_true", help="the largest gcc and clang timers of each category, to check the matching")
    parser.add_argument("--diff", type=Path, help="a map made earlier (--json): the table shows each step's change")
    parser.add_argument("--allow-other-corpus", action="store_true", help="read a d_faces tree whose hash is not the registered one (tools/corpora.py)")
    args = parser.parse_args()
    global QCPORT, INCLUDES
    QCPORT = corpora.resolve("dfaces", os.environ.get("DFACES_SRC") or os.environ.get("QCPORT"), args.allow_other_corpus)
    INCLUDES = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(QCPORT / d))]
    print(corpora.header("dfaces", QCPORT))
    result = build_map(args)
    old = json.loads(args.diff.read_text()) if args.diff else None
    print(show(result, old))
    if args.detail:
        for who in ("gcc", "clang"):
            print(f"\n{who}: largest timers by category (M)")
            for cat, items in sorted(result["detail"][who].items()):
                print(f"  {cat}: " + ", ".join(f"{name[:34]} {m:.1f}" for m, name in items))
    if args.json:
        args.json.write_text(json.dumps(result, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
