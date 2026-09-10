"""
A real before/after: build, link and run bench/nbody.bas twice -- once as BC
left it, once with qbopt's rewrite applied -- and read the 8253 PIT the way
docs/measurement.md prescribes rather than TIMER, which would put 0.2 of
error into a ratio this small.

conf/pinned.conf fixes the emulated CPU rate. Every repetition and its
program answers are kept; PIT scheduling noise is reported as a spread,
not mistaken for evidence that the configuration changed.
"""

import sys
import shutil
import hashlib
import argparse
import statistics
import re
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import host_path
from dosbox import dosbox_bin
from configs import switches_for

from qbopt import wholeseg

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "bench"
BUILD = ROOT / "build" / "bench"
PINNED = ROOT / "conf" / "pinned.conf"


def ticks(text: str) -> int | None:
    for line in text.splitlines():
        if line.startswith("TICKS="):
            return int(line.split("=", 1)[1])
    return None


def optimized(data: bytes, native_fpu: bool, cpu: str) -> bytes:
    result = wholeseg.emitted(data, native_fpu=native_fpu, cpu=cpu)
    if result.outcome is not wholeseg.Emission.LIR:
        raise SystemExit(f"benchmark optimization refused: {result.reason}")
    return result.data


def answers(text: str) -> tuple[str, ...]:
    lines = tuple(line.strip() for line in text.splitlines() if line.strip() and not line.startswith("TICKS="))
    if len(lines) < 2 or lines[-1] != "DONE":
        raise SystemExit("benchmark did not produce an answer followed by DONE")
    return lines


def output_name(exe: Path, repetition: int) -> str:
    return f"{exe.stem[:4]}{repetition}.TXT"


def build(tag: str, prog: str = "nbody", native_fpu: bool = False, transform=None, cpu: str = "386") -> tuple[Path, Path]:
    cfg = CONFIGS[tag]
    if not cfg.available:
        raise SystemExit(f"no toolchain at {cfg.mount}; see docs/testing.md")

    name = prog.upper()
    work = BUILD / tag / prog
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(BENCH / f"{prog}.bas", work / f"{name}.BAS")

    compilation = launch(
        work,
        cfg.mount,
        [f"{cfg.bc} {switches_for(cfg, prog)} {name}.BAS, {name}.OBJ; >> BC.OUT"],
        env={"LIB": r"V:\LIB"},
    )
    severe = re.findall(r"(\d+)\s+Severe\s+Error\(s\)", read_dos(work, "BC.OUT"), re.IGNORECASE)
    if not compilation.finished or compilation.timed_out or severe != ["0"]:
        raise SystemExit(f"BC did not complete with zero severe errors; see {work / 'BC.OUT'}")
    obj = work / f"{name}.OBJ"
    if not obj.is_file():
        raise SystemExit(f"BC did not produce {name}.OBJ; see {work / 'BC.OUT'}")
    change = transform or (lambda data: optimized(data, native_fpu, cpu))
    (work / f"{name}Q.OBJ").write_bytes(change(obj.read_bytes()))

    linking = launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} {name}.OBJ, BASE.EXE,, {cfg.runtime}; >> LINK.OUT",
            f"{cfg.link} {name}Q.OBJ, OPT.EXE,, {cfg.runtime}; >> LINK.OUT",
        ],
        env={"LIB": r"V:\LIB"},
    )
    report = read_dos(work, "LINK.OUT")
    if (not linking.finished or linking.timed_out
        or report.count("Microsoft (R) Segmented Executable Linker") != 2
        or re.search(r"unresolved external|error\s+L\d+", report, re.IGNORECASE)):
        raise SystemExit(f"LINK did not complete both builds without errors; see {work / 'LINK.OUT'}")
    base, opt = work / "BASE.EXE", work / "OPT.EXE"
    if not base.is_file() or not opt.is_file():
        raise SystemExit(f"LINK did not produce both .EXEs; see {work / 'LINK.OUT'}")
    return base, opt


def run(tag: str, exe: Path, steps: int, reps: int, prog: str = "nbody", *,
        expected: tuple[str, ...] | None = None) -> list[int]:
    cfg = CONFIGS[tag]
    work = BUILD / tag / prog
    readings = []
    for i in range(reps):
        name = output_name(exe, i)
        result = launch(
            work,
            cfg.mount,
            [f"{exe.name} {steps} > {name}"],
            conf=PINNED.read_text(),
        )
        if not result.finished or result.timed_out:
            raise SystemExit(f"benchmark did not finish: {exe.name}, repetition {i}")
        out = read_dos(work, name)
        actual = answers(out)
        if expected is None:
            expected = actual
        if actual != expected:
            raise SystemExit(f"benchmark answer mismatch: {work / name}")
        got = ticks(out)
        if got is None or got <= 0:
            raise SystemExit(f"no positive TICKS= reading in {work / name}")
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
    if args.steps <= 0 or args.reps <= 0:
        ap.error("steps and reps must be positive")

    cfg = CONFIGS[args.config]
    base_exe, opt_exe = build(args.config, args.prog, args.native_fpu, cpu=args.cpu)
    base_ticks = run(args.config, base_exe, args.steps, args.reps, args.prog)
    expected = answers(read_dos(BUILD / args.config / args.prog, output_name(base_exe, 0)))
    opt_ticks = run(args.config, opt_exe, args.steps, args.reps, args.prog, expected=expected)

    for label, readings in (("base", base_ticks), ("opt", opt_ticks)):
        spread = max(readings) - min(readings)
        print(f"{label}: {readings} ticks, spread {spread}")
        if spread != 0:
            print(f"  observed spread: {spread / 1193.182:.3f} ms (includes emulator timer noise)")

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
