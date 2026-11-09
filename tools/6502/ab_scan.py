#!/usr/bin/env python3
"""The 6502 corpus A/B of two dists: which programs of tests/run/c the 6502 harness applies to, which of those compile to different
assembly, and their bytes and clocks (4 pads, sim65) under each dist.
usage: ab_scan.py NEW_DIST BASE_DIST OUTDIR [--levels O2,Os] [--programs a,b] [--jobs N]
A dist is a directory with llrm-cc and mos_rt.s (and a COMMIT file naming its tree). The report starts with a stamp naming this script's
tree and both dists, so a table that cites it names what ran. A dist with an ARGS_AX file selects the A:X `report` glue, per dist.
Two defects made earlier tables wrong: programs for other targets were scanned (x86 -m32 programs read WRONG, 2026-11-09) and the
runtime linked was cut short (see test_ours_cr.py ReachableRuntime). Both are tests in test_ab_scan.py."""
import os, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CORPUS = ROOT / "tests/run/c"
sys.path.insert(0, str(ROOT / "tools"))
import corpus_targets  # noqa: E402

TARGET = "6502-sim65"


def applicable(names=None, corpus=CORPUS):
    """The programs the 6502 harness runs: the corpus files `corpus_targets.applies` accepts for TARGET."""
    paths = [corpus / f"{n}.c" for n in names] if names else sorted(corpus.glob("*.c"))
    return [p.stem for p in paths if corpus_targets.applies(TARGET, p.read_text())]


def header_flags(path):
    """The program's `// flags:` -f/-D options, as ours_cr.py reads them."""
    src = (HERE / "ours_cr.py").read_text()
    ns = {}
    exec("import re\n" + src[src.index("X86_ONLY"):src.index("def one(job):")], ns)
    return ns["header_flags"](path)


def assembly(dist, level, name, out):
    """Compile `name` to assembly under `dist`; whether it compiled."""
    cmd = [f"{dist}/llrm-cc", f"--target={TARGET}", f"-{level}", "-S", "-w", "-fsigned-char", *header_flags(CORPUS / f"{name}.c"),
           "-I", str(HERE / "inc"), str(CORPUS / f"{name}.c"), "-o", str(out)]
    return subprocess.run(["nice", "-n", "10", *cmd], capture_output=True, timeout=300).returncode == 0 and out.exists()


def changed(new, base, level, names, jobs=4):
    """The programs whose assembly differs between the dists (one that does not compile under either differs)."""
    with tempfile.TemporaryDirectory(prefix="ab-scan-") as tmp:
        def one(name):
            a, b = Path(tmp) / f"{name}.a.s", Path(tmp) / f"{name}.b.s"
            ok = (assembly(base, level, name, a), assembly(new, level, name, b))
            return name, ok != (True, True) or a.read_bytes() != b.read_bytes()
        with ThreadPoolExecutor(jobs) as pool:
            return [n for n, differs in pool.map(one, names) if differs]


def args_ax(dist):
    """`1` where the dist passes words in a:x (it has an ARGS_AX file), else `0`: `report`'s glue differs per dist, and one environment
    variable for both made the base dist's long arguments read from the wrong registers."""
    return "1" if (Path(dist) / "ARGS_AX").exists() else "0"


def cell(dist, level, name, out):
    """(verdict, data) of one program under one dist, by ours_cr.py: 4 pads, sim65."""
    env = {**os.environ, "OURS_RT": f"{dist}/mos_rt.s", "OURS_PADS": os.environ.get("OURS_PADS", "0,37,101,200"),
           "OURS_CORPUS": str(CORPUS), "OURS_FLAGS": "honor",
           "OURS_ARGS_AX": args_ax(dist)}
    run = subprocess.run([sys.executable, str(HERE / "ours_cr.py"), f"{dist}/llrm-cc", level, str(out), name], env=env, capture_output=True, text=True, timeout=900)
    import json
    try:
        (_, verdict, data), = json.load(open(str(out) + ".json"))
    except (OSError, ValueError):
        return "FAIL", run.stderr[-200:]
    return verdict, data


def stamp(new, base):
    def commit(d):
        f = Path(d) / "COMMIT"
        return f.read_text().split()[0] if f.exists() else "no COMMIT file"
    head = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    return f"ab_scan.py @ {head}: new {new} ({commit(new)}), base {base} ({commit(base)})"


def main(argv):
    new, base, out = argv[:3]
    levels = (argv[argv.index("--levels") + 1] if "--levels" in argv else "O2,Os").split(",")
    names = argv[argv.index("--programs") + 1].split(",") if "--programs" in argv else None
    jobs = int(argv[argv.index("--jobs") + 1]) if "--jobs" in argv else 2
    Path(out).mkdir(parents=True, exist_ok=True)
    print(stamp(new, base))
    pool_names = applicable(names)
    for level in levels:
        diff = changed(new, base, level, pool_names)
        print(f"== {level}: {len(pool_names)} applicable programs, {len(diff)} differ")
        def both(name):
            return name, cell(base, level, name, f"{out}/{name}.{level}.base.tsv"), cell(new, level, name, f"{out}/{name}.{level}.new.tsv")
        with ThreadPoolExecutor(jobs) as pool:
            rows = list(pool.map(both, diff))
        tb = tc = 0
        for name, (bv, bd), (nv, nd) in sorted(rows):
            if bv != "PASS" or nv != "PASS":
                print(f"  {name:32} base {bv} new {nv}")
                continue
            db, dc = nd["bytes"] - bd["bytes"], nd["clks"] - bd["clks"]
            tb += db; tc += dc
            print(f"  {name:32} {bd['bytes']:>6} {bd['clks']:>10} -> {nd['bytes']:>6} {nd['clks']:>10}  dB {db:+d} dClk {dc:+d}")
        print(f"  total dB {tb:+d} dClk {tc:+d}")


if __name__ == "__main__":
    main(sys.argv[1:])
