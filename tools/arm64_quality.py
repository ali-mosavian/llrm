#!/usr/bin/env python3
"""arm64 code-quality metrics: ours (llrm-arm64-llc) vs the pinned clang and gcc, per program and level.

  tools/arm64_quality.py [-O O2,Os] [--bench] [--json FILE] [FILE.c|FILE.ll ...]

Objects come from arm64_parity.build_variants (env: LLRM_PIN_BUILD, LLRM_BIN, LLRM_GCC). Metrics, over all of __text:
  insns   static instruction count.
  mem     loads + stores; ldp/stp/ld1.. count as one; atomics (ldadd, swp, cas..) count as one.
  spills  sp- or x29-relative loads/stores that are not saves.
  saves   sp/x29-relative accesses whose data registers are all callee-saved (x19-x30, d8-d15) AND that sit in a
          prologue run (contiguous sp adjusts / frame setup / such stores, in the entry block, or elsewhere if the run
          adjusts sp: shrink-wrapping) or an epilogue run (the same kind of run of loads ending at ret or a tail branch).
          A callee-saved access anywhere else is a spill.
  exec    ESTIMATED executed instructions: sum over basic blocks of insns * 10**loop_depth. Loops are natural loops of
          back-edges u->v where v dominates u (loops with one header are merged); depth = loops whose body holds the block, capped at 4
          (gcc turns recursion into depth-10 loop nests; uncapped, one such program swamps the means).
          A call (bl, or b by relocation) to a function of the same object adds the callee's exec once per call, times
          the call block's weight; a callee in the same call cycle (recursion, mutual or self) adds 0: its work is not countable statically, and chains of
          clones would otherwise multiply the site weights. Program exec = sum over functions no other
          function calls. Indirect branches end a block with no successors.
Ratio is ours/min(clang, gcc); for spills and saves it is (ours+1)/(best+1), since best is often 0.
Self-check: parsed instruction lines must equal the code sections' size/4. Data words in text (.long, .word...)
make a program 'DATA' and fail the equality check otherwise ('PARSE'); both leave it out of the means.
"""
import argparse, importlib.util, json, math, re, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
LEVELS_DEFAULT = "O2,Os"
METRICS = ["insns", "mem", "spills", "saves", "exec"]
SMOOTHED = {"spills", "saves"}
SIDES = ["clang", "ours", "gcc"]
MAX_DEPTH = 4

INSN = re.compile(r"^\s*([0-9a-f]+):\s+(\S+)\s*(.*?)\s*$")
RELOC = re.compile(r"^\s+[0-9a-f]+:\s+ARM64_RELOC_BRANCH26\s+(\S+)")
SYM = re.compile(r"^([0-9a-f]+) .* F __TEXT,(__text\w*) (\S+)$")
HEX = re.compile(r"0x([0-9a-f]+)")
CALLEE = re.compile(r"^(?:[xw](?:19|2[0-9]|30)|d(?:[89]|1[0-5]))$")
FRAMEBASE = re.compile(r"\[\s*(sp|x29|fp)\b")
COND = re.compile(r"^(b\.\w+|bc\.\w+|cbn?z|tbn?z)$")


class Insn:
    __slots__ = ("addr", "mn", "ops", "reloc")

    def __init__(self, addr, mn, ops, reloc=None):
        self.addr, self.mn, self.ops, self.reloc = addr, mn, ops, reloc

    @property
    def is_data(self):
        return self.mn.startswith(".") or self.mn.startswith("<")


def parse_disasm(text):
    """Instructions in order, with a following BRANCH26 relocation line attached to the one before it."""
    out = []
    for line in text.splitlines():
        r = RELOC.match(line)
        if r:
            if out:
                out[-1].reloc = r[1]
            continue
        m = INSN.match(line)
        if m and not m[2].startswith("ARM64_"):
            out.append(Insn(int(m[1], 16), m[2], m[3].split(";")[0].strip()))
    return out


def parse_symbols(text):
    """[(addr, name)] of the code symbols, sorted; ltmp* assembler temporaries are not functions."""
    syms = []
    for line in text.splitlines():
        m = SYM.match(line.strip())
        if m and not m[3].startswith(("ltmp", "L")):
            syms.append((int(m[1], 16), m[3]))
    return sorted(set(syms))


def data_regs(ops):
    """Registers named before the memory operand."""
    head = ops.split("[")[0]
    return [r.strip() for r in head.split(",") if r.strip()]


def is_load(mn):
    return mn.startswith("ld") or mn.startswith(("swp", "cas"))


def is_store(mn):
    return mn.startswith("st")


def is_mem(i):
    return not i.is_data and (is_load(i.mn) or is_store(i.mn))


def frame_access(i):
    return is_mem(i) and FRAMEBASE.search(i.ops) is not None


def callee_only(i):
    regs = data_regs(i.ops)
    return bool(regs) and all(CALLEE.match(r) for r in regs)


def is_sp_adjust(i):
    return (i.ops.startswith("sp,") and i.mn in ("sub", "add", "mov")) or i.mn in ("pacibsp", "paciasp", "autibsp", "autiasp")


def is_frame_setup(i):
    return i.mn in ("mov", "add") and i.ops.startswith("x29, sp")


def is_branch(i):
    return i.mn == "b" or i.mn in ("br", "ret", "bl", "blr") or bool(COND.match(i.mn))


def target(i):
    m = HEX.search(i.ops)
    return int(m[1], 16) if m else None


def blocks_of(insns, lo, hi, names):
    """Basic blocks of insns[lo:hi) as (start_index, end_index_exclusive, succ_block_ids, calls[names]) lists."""
    addrs = {insns[k].addr: k for k in range(lo, hi)}
    lead = {lo}
    for k in range(lo, hi):
        i = insns[k]
        if not is_branch(i):
            continue
        if i.mn != "bl" and i.mn != "blr":
            if k + 1 < hi:
                lead.add(k + 1)
            if i.reloc is None and target(i) in addrs:
                lead.add(addrs[target(i)])
    starts = sorted(lead)
    bid = {s: n for n, s in enumerate(starts)}
    blocks = []
    for n, s in enumerate(starts):
        e = starts[n + 1] if n + 1 < len(starts) else hi
        last = insns[e - 1]
        succ, calls = [], []
        for k in range(s, e):
            i = insns[k]
            if i.mn == "bl" or (i.mn == "b" and (i.reloc or target(i) not in addrs)):
                calls.append(i.reloc or target(i))
        fall = e < hi
        if last.mn == "b":
            if last.reloc is None and target(last) in addrs:
                succ.append(bid[addrs[target(last)]])
            fall = False
        elif COND.match(last.mn):
            if last.reloc is None and target(last) in addrs:
                succ.append(bid[addrs[target(last)]])
        elif last.mn in ("ret", "br"):
            fall = False
        if fall:
            succ.append(bid[e])
        blocks.append((s, e, succ, calls))
    return blocks


def loop_depths(blocks):
    """Per-block loop depth from natural loops of dominance back-edges; unreachable blocks get 0."""
    n = len(blocks)
    reach = set()
    stack = [0]
    while stack:
        b = stack.pop()
        if b in reach:
            continue
        reach.add(b)
        stack.extend(blocks[b][2])
    preds = {b: [] for b in reach}
    for b in reach:
        for s in blocks[b][2]:
            preds[s].append(b)
    dom = {b: set(reach) for b in reach}
    dom[0] = {0}
    changed = True
    while changed:
        changed = False
        for b in sorted(reach - {0}):
            new = set.intersection(*(dom[p] for p in preds[b])) | {b} if preds[b] else {b}
            if new != dom[b]:
                dom[b], changed = new, True
    bodies = {}
    for u in reach:
        for v in blocks[u][2]:
            if v in dom[u]:
                body = bodies.setdefault(v, {v})
                work = [u]
                while work:
                    x = work.pop()
                    if x not in body:
                        body.add(x)
                        work.extend(preds[x])
    return [sum(b in body for body in bodies.values()) for b in range(n)]


def classify_frame(insns, blocks):
    """(spills, saves) over one function's frame accesses; see the module docstring."""
    spills = saves = 0
    for bi, (s, e, _, _) in enumerate(blocks):
        marked = set()
        run = []
        for k in range(s, e + 1):
            i = insns[k] if k < e else None
            ok = i is not None and (is_sp_adjust(i) or is_frame_setup(i) or (frame_access(i) and is_store(i.mn) and callee_only(i)))
            if ok:
                run.append(k)
                continue
            if run and (bi == 0 and run[0] == s or any(is_sp_adjust(insns[x]) for x in run)):
                marked.update(x for x in run if frame_access(insns[x]))
            run = []
        # epilogue: contiguous loads/sp restores that end the block in ret or a tail branch
        last = insns[e - 1]
        if last.mn == "ret" or (last.mn == "b" and (last.reloc or target(last) is None)):
            k = e - 2
            while k >= s:
                i = insns[k]
                if is_sp_adjust(i) or i.mn == "mov" and i.ops.startswith("sp,") or (frame_access(i) and is_load(i.mn) and callee_only(i)):
                    if frame_access(i):
                        marked.add(k)
                    k -= 1
                else:
                    break
        for k in range(s, e):
            if frame_access(insns[k]):
                if k in marked:
                    saves += 1
                else:
                    spills += 1
    return spills, saves


def measure(insns, syms):
    """The five metrics of one object's parsed instructions and code symbols."""
    insns = [i for i in insns]
    if not syms and insns:
        syms = [(insns[0].addr, "?")]
    elif insns and syms[0][0] > insns[0].addr:
        syms = [(insns[0].addr, "?")] + syms
    bounds = [(a, syms[n + 1][0] if n + 1 < len(syms) else 1 << 62, name) for n, (a, name) in enumerate(syms)]
    byname = {name: n for n, (_, _, name) in enumerate(bounds)}
    byaddr = {a: n for n, (a, _, _) in enumerate(bounds)}
    fns = []
    mem = spills = saves = 0
    for a, b, name in bounds:
        idx = [k for k, i in enumerate(insns) if a <= i.addr < b]
        if not idx:
            fns.append(None)
            continue
        lo, hi = idx[0], idx[-1] + 1
        blocks = blocks_of(insns, lo, hi, byname)
        depth = loop_depths(blocks)
        sp, sv = classify_frame(insns, blocks)
        spills += sp
        saves += sv
        mem += sum(is_mem(insns[k]) for k in range(lo, hi))
        callees = []
        base = 0
        for bl, (s, e, _, calls) in zip(depth, blocks):
            w = 10 ** min(bl, MAX_DEPTH)
            base += (e - s) * w
            for c in calls:
                n = byname.get(c) if isinstance(c, str) else byaddr.get(c)
                if n is not None:
                    callees.append((n, w))
        fns.append((base, callees))
    reach = []
    for n in range(len(fns)):
        seen, work = set(), [c for c, _ in fns[n][1]] if fns[n] else []
        while work:
            x = work.pop()
            if x not in seen:
                seen.add(x)
                work.extend(c for c, _ in fns[x][1]) if fns[x] else None
        reach.append(seen)
    memo = {}

    def est(n):
        if n not in memo:
            base, callees = fns[n]
            memo[n] = base + sum(w * est(c) for c, w in callees if fns[c] and not (c in reach[n] and n in reach[c]) and c != n)
        return memo[n]

    called = {c for f in fns if f for c, _ in f[1]}
    roots = [n for n in range(len(fns)) if fns[n] is not None and n not in called] or [n for n in range(len(fns)) if fns[n]]
    return {"insns": sum(not i.is_data for i in insns), "mem": mem, "spills": spills, "saves": saves,
            "exec": sum(est(n) for n in roots if fns[n])}


def objdump(obj):
    d = subprocess.run(["xcrun", "objdump", "-d", "-r", "--no-show-raw-insn", str(obj)], capture_output=True, text=True)
    t = subprocess.run(["xcrun", "objdump", "-t", str(obj)], capture_output=True, text=True)
    return d.stdout, t.stdout


class Unmeasurable(Exception):
    def __init__(self, kind, why):
        super().__init__(why)
        self.kind = kind


def check_and_measure(insns, syms, text_bytes):
    """Rule 2: refuse a count that is not exactly the code sections' size/4."""
    data = sum(i.is_data for i in insns)
    if data:
        raise Unmeasurable("DATA", f"{data} data words in text")
    if not insns or len(insns) * 4 != text_bytes:
        raise Unmeasurable("PARSE", f"parsed {len(insns)} insns, text is {text_bytes} bytes")
    return measure(insns, syms)


def measure_obj(par, obj):
    dis, st = objdump(obj)
    return check_and_measure(parse_disasm(dis), parse_symbols(st), par.text_size(obj) or 0)


def load_parity():
    """tools/arm64_parity.py calls main() on import; run its source without that last call."""
    src = (HERE / "arm64_parity.py").read_text().rstrip()
    assert src.endswith("\nmain()"), "arm64_parity.py no longer ends in main()"
    mod = importlib.util.module_from_spec(importlib.util.spec_from_loader("arm64_parity", loader=None))
    mod.__file__ = str(HERE / "arm64_parity.py")
    exec(compile(src[: -len("main()")], str(HERE / "arm64_parity.py"), "exec"), mod.__dict__)
    return mod


def ratio(metric, ours, a, b):
    best = min(a, b)
    return (ours + 1) / (best + 1) if metric in SMOOTHED else (ours / best if best else (1.0 if ours == 0 else math.inf))


def geo(xs):
    xs = [x for x in xs if math.isfinite(x) and x > 0]
    return math.exp(sum(map(math.log, xs)) / len(xs)) if xs else float("nan")


def run_program(par, src, lvl, tmp):
    """{side: metrics} or a status string ('FAIL: ...', 'DATA: ...', 'PARSE: ...')."""
    v = par.build_variants(src, lvl, tmp, "")
    res = {}
    for s in SIDES:
        o = v.get(s)
        if not isinstance(o, Path):
            return f"FAIL: {s}: {o or 'not built'}"[:90]
        try:
            res[s] = measure_obj(par, o)
        except Unmeasurable as e:
            return f"{e.kind}: {s}: {e}"
    return res


def table(lvl, rows):
    ok = {p: r for p, r in rows.items() if isinstance(r, dict)}
    out = [f"== {lvl} =="]
    head = f"{'prog':<14}" + "".join(f" | {m:<6}{'clang':>10}{'ours':>10}{'gcc':>10}{'o/best':>7}" for m in METRICS)
    out.append(head)
    ratios = {m: [] for m in METRICS}
    per = {m: {} for m in METRICS}
    for p, r in rows.items():
        if not isinstance(r, dict):
            out.append(f"{p:<14} {r}")
            continue
        cells = ""
        for m in METRICS:
            c, o, g = (r[s][m] for s in SIDES)
            q = ratio(m, o, c, g)
            ratios[m].append(q)
            per[m][p] = q
            cells += f" | {'':<6}{c:>10}{o:>10}{g:>10}{q:>7.3f}"
        out.append(f"{p:<14}{cells}")
    out.append(f"{'GEOMEAN':<14}" + "".join(f" | {'':<6}{'':>10}{'':>10}{'':>10}{geo(ratios[m]):>7.3f}" for m in METRICS) + f"   ({len(ok)} programs)")
    return out, per


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("-O", default=LEVELS_DEFAULT)
    ap.add_argument("--bench", action="store_true", help="bench/*/*.c from the current directory, minus lru")
    ap.add_argument("--json")
    a = ap.parse_args(argv)
    par = load_parity()
    files = [Path(f) for f in a.files]
    if a.bench:
        sys.path.insert(0, str(HERE))
        import benchlist
        files += [p for p in benchlist.sources(".c", Path.cwd() / "bench") if p.stem != "lru"]
    if not files:
        ap.error("no programs")
    tmp = Path(tempfile.mkdtemp(prefix="quality."))
    levels = a.O.split(",")
    report, worst, bad = {}, [], 0
    for lvl in levels:
        rows = {}
        for f in files:
            if lvl != "O2" and f.suffix != ".c":
                continue
            rows[f.stem] = run_program(par, f.resolve(), lvl, tmp)
            bad += not isinstance(rows[f.stem], dict)
        lines, per = table(lvl, rows)
        print("\n".join(lines) + "\n")
        for m in METRICS:
            top = sorted(per[m].items(), key=lambda kv: -kv[1])[:5]
            worst.append(f"worst {lvl} {m}: " + ", ".join(f"{p} {q:.2f}" for p, q in top))
        report[lvl] = rows
    print("\n".join(worst))
    print(f"{bad} program-levels not measured (FAIL/DATA/PARSE), excluded from the means")
    if a.json:
        Path(a.json).write_text(json.dumps(report, indent=1))


if __name__ == "__main__":
    main()
