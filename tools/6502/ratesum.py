#!/usr/bin/env python3
"""Gate number for 6502: rate-sum (clk + 3 B) of a head dist against a parent dist.

    ratesum.py --head DIST --base DIST [--out DIR] [--cpus 6502,65c02] [--levels O1,Os] [--no-run]

A DIST holds llrm-cc, llrm-6502-llc and mos_rt.s. Each (cpu, level, side) runs ours_cr.py over the 15 gate
programs at 256 pads (every code-address residue mod 256). Rate = mean clocks over the pads + 3 * bytes; the
reported delta is head - base over the programs that PASS on both sides, with and without idioms. --no-run
reads the results already in --out. Prints the md5 of every binary, so equal binaries are visible.
"""
import argparse, builtins, hashlib, json, os, shutil, statistics, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROGRAMS = ("long_compare_chain known_loads_walks bit_test memcpy_libcall phi_slot_unknown_arm int64_division "
            "overflow_builtins idioms long_shift_by_a_call_result double_to_uint64 data_int64 "
            "frame_address_stack_args huffman_decode word_compare_branches sat_builtins").split()
BINARIES = ("llrm-cc", "llrm-llc", "llrm-6502-llc", "mos_rt.s")  # llrm-llc is the mid-end: a stale one fakes a delta
SUBSET = "idioms long_compare_chain phi_slot_unknown_arm known_loads_walks int64_division overflow_builtins".split()
UNRELATED = ("phi_slot_unknown_arm", "int64_division")  # wins from other PRs, excluded to show the PR's own effect
BYTE_WEIGHT = 3
PADS = ",".join(map(str, range(256)))
GUARD = Path.home() / "scratch" / "lg" / "load_guard.sh"
GUARD_DEFAULTS = dict(LG_SLOTS="12", LG_TIER="1", LG_START_1M="48", LG_STOP_5M="50", LG_STOP_1M="60",
                      LG_RESUME_5M="44", LG_RESUME_1M="40")


def md5(path):
    return hashlib.md5(Path(path).read_bytes()).hexdigest()[:8]


def result_path(out, dist, cpu, level):
    """Named by the binaries, so one --out holds many runs and a result is never read for other binaries."""
    ids = "-".join(md5(dist / f) for f in BINARIES)
    return out / f"{cpu}.{ids}.{level}.tsv"


def load(path):
    """name -> (status, detail); None when the run left no result."""
    try:
        return {r[0]: (r[1], r[2]) for r in json.load(open(path))}
    except (OSError, ValueError):
        return None


def compare(base, head, names, skip=()):
    """Rate delta head - base. Programs not PASS on both sides are listed, never summed.
    Returns (delta, mean_clock_delta, byte_delta, per-program rows, unmeasured names)."""
    rows, unmeasured = [], []
    for name in names:
        (sa, da), (sb, db) = base.get(name, ("missing", None)), head.get(name, ("missing", None))
        if sa != "PASS" or sb != "PASS":
            unmeasured.append((name, sa, sb))
            continue
        d_bytes = db["bytes"] - da["bytes"]
        d_clk = statistics.mean(db["layouts"]) - statistics.mean(da["layouts"])
        rows.append((name, d_bytes, d_clk, d_clk + BYTE_WEIGHT * d_bytes))
    kept = [r for r in rows if r[0] not in skip]
    return (sum(r[3] for r in kept), sum(r[2] for r in kept), sum(r[1] for r in kept),
            sorted(rows, key=lambda r: -r[3]), unmeasured)


def counts(side, names):
    tally = {}
    for name in names:
        status = side.get(name, ("missing", None))[0]
        tally[status] = tally.get(status, 0) + 1
    return tally


def run_side(dist, cpu, level, out, names, tmp):
    """One ours_cr.py run; the 'tsv' it writes is the id of a finished run."""
    env = dict(os.environ, OURS_CPU=cpu, OURS_PADS=PADS, OURS_RT=str(dist / "mos_rt.s"), LLRM_CC=str(dist / "llrm-cc"),
               OURS_FLAGS="honor", TMPDIR=str(tmp))
    env.setdefault("SIM65_INSNS", str(Path.home() / "scratch" / "s4" / "sim65-insns"))
    for k, v in GUARD_DEFAULTS.items():
        env.setdefault(k, v)
    cmd = ["python3", str(HERE / "ours_cr.py"), str(dist / "llrm-cc"), level, str(out), *names]
    if GUARD.exists():
        cmd = [str(GUARD), *cmd]
    with open(f"{out}.log", "w") as log:
        subprocess.run(cmd, env=env, stdout=log, stderr=subprocess.STDOUT, check=False, cwd=HERE)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--head", required=True, type=Path)
    ap.add_argument("--base", required=True, type=Path, help="parent dist, e.g. the build of 720b2e8c90")
    ap.add_argument("--out", type=Path, default=Path.home() / "scratch" / "ratesum")
    ap.add_argument("--cpus", default="6502,65c02")
    ap.add_argument("--levels", default="O1,Os")
    ap.add_argument("--names", default=" ".join(PROGRAMS))
    ap.add_argument("--no-run", action="store_true", help="only read the results already in --out")
    ap.add_argument("--allow-same", action="store_true", help="measure although the two llc binaries are equal")
    ap.add_argument("--base-label", default="720b2e8c90", help="commit of the base dist, printed on every line")
    ap.add_argument("--head-label", default="head", help="commit of the head dist, printed on every line")
    a = ap.parse_args(argv)
    names, sides = a.names.split(), {"base": a.base, "head": a.head}
    for tag, d in sides.items():
        for f in ("llrm-cc", "llrm-llc", "llrm-6502-llc", "mos_rt.s"):
            if not (d / f).is_file():
                sys.exit(f"ratesum: {d / f} is missing ({tag})")
    ids = lambda d: f"llc {md5(d / 'llrm-llc')}/{md5(d / 'llrm-6502-llc')}"
    tag = f"[base {a.base_label} {ids(a.base)} | head {a.head_label} {ids(a.head)}]"
    print = lambda *x, **k: builtins.print(tag, *x, **k)  # one baseline on every line
    print("md5 (cc, llrm-llc, 6502-llc, mos_rt):", {t: tuple(md5(d / f) for f in BINARIES) for t, d in sides.items()})
    if md5(a.base / "llrm-6502-llc") == md5(a.head / "llrm-6502-llc") and not a.allow_same:
        sys.exit("ratesum: base and head llrm-6502-llc are the same binary; the delta would be noise (--allow-same to measure anyway)")
    a.out.mkdir(parents=True, exist_ok=True)
    tmp = a.out / "tmp"
    for cpu in a.cpus.split(","):
        for level in a.levels.split(","):
            for tag, d in sides.items():
                out = result_path(a.out, d, cpu, level)
                done = Path(f"{out}.done")
                if not a.no_run and not (done.exists() and done.read_text() == " ".join(names)):
                    tmp.mkdir(exist_ok=True)
                    done.unlink(missing_ok=True)
                    run_side(d, cpu, level, out, names, tmp)
                    if Path(f"{out}.json").exists():  # reused only for the same binaries and programs
                        done.write_text(" ".join(names))
    shutil.rmtree(tmp, True)
    for cpu in a.cpus.split(","):
        for level in a.levels.split(","):
            base, head = (load(f"{result_path(a.out, d, cpu, level)}.json") for d in (a.base, a.head))
            print(f"== {cpu} {level}")
            if base is None or head is None:
                print("  UNMEASURED: no result file for", "base" if base is None else "head")
                continue
            total, clk, size, rows, unmeasured = compare(base, head, names)
            if len(unmeasured) == len(names):
                print("  UNMEASURED: no program passes on both sides")
                continue
            without = compare(base, head, names, skip=("idioms",))[0]
            print(f"  compiled+ran: base {counts(base, names)} head {counts(head, names)} of {len(names)}")
            for n, sa, sb in unmeasured:
                print(f"  not summed: {n} base {sa} head {sb}")
            for n, db, dc, rate in rows:
                if db or round(dc):
                    print(f"  {n:30s} dB {db:+5d} dClk {dc:+10.0f} rate {rate:+10.0f}")
            worse = [r for r in rows if r[3] > 0]
            subset = sum(r[3] for r in rows if r[0] in SUBSET)
            print(f"  RATE SUM (clk+3B) head-base: {total:+.0f}   without idioms: {without:+.0f}   (dClk {clk:+.0f}, dB {size:+d})")
            excl2 = compare(base, head, names, skip=UNRELATED)[0]
            print(f"  SUMMARY {cpu} {level} sum15 {total:+.0f} sum15-excl2 {excl2:+.0f} sum6 {subset:+.0f} "
                  f"worse [{' '.join(f'{r[0]}:{r[3]:+.0f}' for r in worse)}] mos_rt {md5(a.head / 'mos_rt.s')}/{md5(a.base / 'mos_rt.s')}")
            print(f"  worse cells ({len(worse)}): {sum(r[3] for r in worse):+.0f}   [{' '.join(r[0] for r in worse)}]   the 6-program subset: {subset:+.0f}")


if __name__ == "__main__":
    main()
