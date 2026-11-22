#!/usr/bin/env python3
"""Fit the z80 byte table (`crates/target/llrm-z80/src/size_fit.txt`) to the bytes the backend writes.

    tools/z80_size_fit.py [--levels 1,2,3,s] [--work DIR] [--write] [--weight-exp 0.5] [--splits 8]

Per level it makes the front-end IR of every z80 program of `tests/run/c`, runs `tests/size_calibration.rs` over it
(`LLRM_SIZE_DIR`, `LLRM_SIZE_LEVEL`: one row a function, its operations by class and the bytes it is written as) and fits
the `OperationCosts` fields `function_bytes` prices them with: non-negative least squares on the error relative to the
written bytes (weight 1 / bytes^weight-exp, a function under 20 B weighing as 20), coordinate descent.

The hold-out is by program: a random half of the programs fits, the other half is scored, `--splits` times, for the table
in force (`trial`, from the file as it is) and the refit. `--write` writes the refit of all rows, rounded, to the file.
Functions with float operations are left out: their prices are the sheet's.
"""
import argparse, collections, importlib.util, json, os, random, statistics, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRAMS = ROOT / "crates/target/llrm-z80/tests/programs"
FIT = ROOT / "crates/target/llrm-z80/src/size_fit.txt"
# a compile that does not finish at -O3 (known_loads_walks: a compile-time fault, rows Z80-COMPILE-TIME) has no row
SKIP = {("3", "known_loads_walks")}

# the OperationCosts field each class of `walk::operation_counts` is priced with by `function_bytes`
FIELDS = {
    "function": [],
    "add": ["add", "addc", "logic", "icmp", "icmpc"],
    "address": ["gep", "gepc"],
    "load": ["loadi"],
    "direct_load": ["loadd"],
    "store": ["storei"],
    "direct_store": ["stored"],
    "shift": ["shiftc", "shiftv"],
    "extend": ["ext"],
    "call": ["call"],
    "argument": ["callargc", "callargv"],
    "multiply": ["mul"],
    "divide": ["div"],
    "branch": ["br", "select", "switch2"],
    "move": ["select", "phi"],
    "return_": ["ret"],
}
LEFT_OUT = {"fp", "castfp", "fcmp"}


def load_run():
    spec = importlib.util.spec_from_file_location("run", PROGRAMS / "run.py")
    run = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(PROGRAMS))
    sys.path.insert(0, str(ROOT / "tools"))
    sys.modules["run"] = run
    spec.loader.exec_module(run)
    return run


def make_ir(run, level, out):
    import pin_clang

    clang = pin_clang.path()  # LLRM_PIN_CLANG, checked to be the pin's build
    out.mkdir(parents=True, exist_ok=True)
    flag = {"s": "-Os", "z": "-Oz"}.get(level, f"-O{level}")

    def one(path):
        name = f"t_{path.stem}"
        if (level, path.stem) in SKIP:
            return
        target = out / f"{name}.ll"
        done = subprocess.run(
            [clang, flag, *run.CLANG_FLAGS, *run.program_flags(path)[0], str(path), "-o", str(target)],
            capture_output=True, text=True, cwd=ROOT,
        )
        if done.returncode:
            return
        text = [line for line in target.read_text().splitlines() if not line.startswith("target datalayout")]
        target.write_text(run.DATALAYOUT + "\n" + "\n".join(text).replace(str(ROOT) + "/", "") + "\n")

    with ThreadPoolExecutor(8) as pool:
        list(pool.map(one, run.selected(ROOT / "tests/run/c")))


def rows_of(level, directory):
    env = {**os.environ, "LLRM_SIZE_FIT": "1", "LLRM_SIZE_DIR": str(directory), "LLRM_SIZE_LEVEL": level}
    done = subprocess.run(
        ["cargo", "test", "--release", "-p", "llrm-z80", "--test", "size_calibration", "--", "--nocapture"],
        cwd=ROOT, env=env, capture_output=True, text=True,
    )
    rows = []
    for line in done.stdout.splitlines():
        parts = line.split()
        if len(parts) < 5 or parts[0] != "row":
            continue
        counts = {}
        for item in (parts[5].split(",") if len(parts) > 5 else []):
            name, value = item.split("=")
            counts[name] = int(value)
        if LEFT_OUT & set(counts):
            continue
        counts["switch2"] = 2 * counts.get("switch", 0)
        x = {"function": 1}
        for field, classes in FIELDS.items():
            if classes:
                x[field] = sum(counts.get(c, 0) for c in classes)
        rows.append((parts[1], int(parts[3]), int(parts[4]), x))
    return rows


def fit(rows, weight_exp):
    fields = list(FIELDS)
    y = [r[2] for r in rows]
    w = [1 / max(v, 20) ** weight_exp for v in y]
    price = dict.fromkeys(fields, 0.0)
    for _ in range(400):
        for f in fields:
            num = den = 0.0
            for i, r in enumerate(rows):
                xf = r[3].get(f, 0)
                if xf:
                    rest = y[i] - sum(price[g] * r[3].get(g, 0) for g in fields if g != f)
                    num += w[i] ** 2 * xf * rest
                    den += w[i] ** 2 * xf * xf
            price[f] = max(0.0, num / den) if den else 0.0
    return price


def estimate(price, row):
    return sum(price[f] * n for f, n in row[3].items())


def score(rows, estimates):
    y = [r[2] for r in rows]
    rel = [abs(e - v) / max(v, 20) for e, v in zip(estimates, y)]
    ratio = [e / v for e, v in zip(estimates, y) if v]
    return (sum(estimates) / sum(y), statistics.median(ratio), statistics.mean(rel), sum(r < 0.25 for r in rel) / len(rel))


def current():
    table = {}
    if FIT.exists():
        for line in FIT.read_text().splitlines():
            if line.strip() and not line.startswith("#"):
                name, value = line.split("=")
                table[name.strip()] = float(value)
    return table


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--levels", default="1,2,3,s")
    ap.add_argument("--work", default=None)
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--weight-exp", type=float, default=0.5)
    ap.add_argument("--splits", type=int, default=8)
    ap.add_argument("--fresh", action="store_true", help="rebuild the rows even where the work directory has them")
    args = ap.parse_args()
    run = load_run()
    work = Path(args.work or tempfile.mkdtemp(prefix="z80-size-fit-"))
    cache = work / "rows.json"
    rows = []
    if cache.exists() and not args.fresh:
        rows = [(a, b, c, d) for a, b, c, d in json.loads(cache.read_text())]
    else:
        for level in args.levels.split(","):
            directory = work / f"O{level}"
            make_ir(run, level, directory)
            got = rows_of(level, directory)
            print(f"-O{level}: {len(got)} functions", file=sys.stderr)
            rows += got
        work.mkdir(parents=True, exist_ok=True)
        cache.write_text(json.dumps(rows))
    by_program = [(r[0], r) for r in rows]
    names = sorted({p for p, _ in by_program})
    held = current()
    scores = collections.defaultdict(list)
    for seed in range(1, args.splits + 1):
        half = set(random.Random(seed).sample(names, len(names) // 2))
        train = [r for p, r in by_program if p in half]
        test = [r for p, r in by_program if p not in half]
        price = fit(train, args.weight_exp)
        rounded = {f: round(v) for f, v in price.items()}
        scores["refit"].append(score(test, [estimate(rounded, r) for r in test]))
        scores["trial"].append(score(test, [r[1] for r in test]))
        if held:
            scores["file"].append(score(test, [estimate({f: held.get(f, 0) for f in FIELDS}, r) for r in test]))
    print(f"{len(rows)} functions, {len(names)} programs; hold-out by program, mean of {args.splits} splits")
    print("                aggregate  median  mean|rel err|  within 25%")
    for label, runs in scores.items():
        m = [statistics.mean(s[i] for s in runs) for i in range(4)]
        print(f"  {label:8}      {m[0]:.2f}      {m[1]:.2f}      {m[2]:.2f}          {m[3]:.2f}   (worst split {max(s[2] for s in runs):.2f})")
    price = {f: round(v) for f, v in fit([r for _, r in by_program], args.weight_exp).items()}
    print("refit of all rows:", price)
    if args.write:
        lines = [
            "# Bytes a z80 operation is written as, by `OperationCosts` field: `tools/z80_size_fit.py --write` fits them to",
            "# the bytes the backend writes (`tests/size_calibration.rs`). Generated; do not edit by hand.",
        ]
        lines += [f"{f} = {v}" for f, v in price.items()]
        FIT.write_text("\n".join(lines) + "\n")
        print("wrote", FIT)


if __name__ == "__main__":
    main()
