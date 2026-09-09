"""
A real before/after: build, link and run bench/nbody.bas twice -- once as BC
left it, once with qbopt's rewrite applied -- and read the 8253 PIT the way
docs/measurement.md prescribes rather than TIMER, which would put 0.2 of
error into a ratio this small.

conf/pinned.conf ties the PIT to emulated cycles, so a repeat run of the same
binary must read back the same tick count. Every repetition is kept and
compared for exactly that reason: a spread of more than zero means the
machine was not pinned, and no ratio from it would be quotable.
"""

import sys
import shutil
import hashlib
import argparse
import statistics
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import host_path
from dosbox import dosbox_bin
from configs import switches_for

from qbopt.rewrite import rewrite

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "bench"
BUILD = ROOT / "build" / "bench"
PINNED = ROOT / "conf" / "pinned.conf"


def ticks(text: str) -> int | None:
    for line in text.splitlines():
        if line.startswith("TICKS="):
            return int(line.split("=", 1)[1])
    return None


def build(tag: str, prog: str = "nbody", native_fpu: bool = False, transform=None, cpu: str = "386") -> tuple[Path, Path]:
    cfg = CONFIGS[tag]
    if not cfg.available:
        raise SystemExit(f"no toolchain at {cfg.mount}; see docs/testing.md")

    name = prog.upper()
    work = BUILD / tag / prog
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(BENCH / f"{prog}.bas", work / f"{name}.BAS")

    launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {switches_for(cfg, prog)} {name}.BAS, {name}.OBJ; >> BC.OUT"],
        env={"LIB": r"V:\LIB"},
    )
    obj = work / f"{name}.OBJ"
    if not obj.is_file():
        raise SystemExit(f"BC did not produce {name}.OBJ; see {work / 'BC.OUT'}")
    change = transform or (lambda data: rewrite(data, dry_run=False, native_fpu=native_fpu, cpu=cpu)[0])
    (work / f"{name}Q.OBJ").write_bytes(change(obj.read_bytes()))

    launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} {name}.OBJ, BASE.EXE,, {cfg.runtime}; >> LINK.OUT",
            f"{cfg.link} {name}Q.OBJ, OPT.EXE,, {cfg.runtime}; >> LINK.OUT",
        ],
        env={"LIB": r"V:\LIB"},
    )
    base, opt = work / "BASE.EXE", work / "OPT.EXE"
    if not base.is_file() or not opt.is_file():
        raise SystemExit(f"LINK did not produce both .EXEs; see {work / 'LINK.OUT'}")
    return base, opt


def run(tag: str, exe: Path, steps: int, reps: int, prog: str = "nbody") -> list[int]:
    cfg = CONFIGS[tag]
    work = BUILD / tag / prog
    readings = []
    for i in range(reps):
        launch(
            work,
            cfg.mount,
            [f"{exe.name} {steps} > OUT{i}.TXT"],
            conf=PINNED.read_text(),
        )
        out = read_dos(work, f"OUT{i}.TXT")
        got = ticks(out)
        if got is None:
            raise SystemExit(f"no TICKS= line in {work / f'OUT{i}.TXT'}")
        readings.append(got)
    return readings


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="bench")
    ap.add_argument("--config", default="v-g3", choices=list(CONFIGS))
    ap.add_argument("--prog", default="nbody")
    from qbopt.cycles.timings import ARCHS
    ap.add_argument("--cpu", choices=("386", *ARCHS), default="386")
    ap.add_argument("--native-fpu", action="store_true")
    ap.add_argument("--steps", type=int, default=2000)
    ap.add_argument("--reps", type=int, default=5)
    args = ap.parse_args(argv)

    cfg = CONFIGS[args.config]
    base_exe, opt_exe = build(args.config, args.prog, args.native_fpu, cpu=args.cpu)
    base_ticks = run(args.config, base_exe, args.steps, args.reps, args.prog)
    opt_ticks = run(args.config, opt_exe, args.steps, args.reps, args.prog)

    for label, readings in (("base", base_ticks), ("opt", opt_ticks)):
        spread = max(readings) - min(readings)
        print(f"{label}: {readings} ticks, spread {spread}")
        if spread != 0:
            print(f"  WARNING: {label} did not read back identically -- the machine was not pinned")

    b, o = statistics.median(base_ticks), statistics.median(opt_ticks)
    print(f"\nmedian: base {b} ticks ({b / 1193.182:.1f} ms), opt {o} ticks ({o / 1193.182:.1f} ms)")
    print(f"ratio: base/opt = {b / o:.4f}")

    print("\nquotable stamp:")
    print(f"  dosbox-x: {dosbox_bin()}")
    print(f"  conf sha256: {sha256(PINNED)}")
    print(f"  config: {args.config}, steps: {args.steps}, reps: {args.reps}")
    print(f"  tuning CPU: {args.cpu} (does not change DOSBox timing model)")
    print(f"  BC.EXE sha256: {sha256(host_path(cfg.mount, cfg.bc))}")
    print(f"  LINK.EXE sha256: {sha256(host_path(cfg.mount, cfg.link))}")
    print(f"  runtime sha256: {sha256(host_path(cfg.mount, cfg.runtime))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
