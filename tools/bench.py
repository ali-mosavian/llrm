"""
A real before/after: build, link and run a bench program twice -- once as BC
left it, once with qbopt's rewrite applied -- timed by RDTSC in milliseconds.

The PIT reader tore by whole BIOS ticks between identical runs. conf/pinned.conf
fixes the emulated CPU rate, and DOSBox-X scales RDTSC by it, so a count is
emulated time. Every repetition and its program answers are kept.
"""

import re
import sys
import shutil
import hashlib
import argparse
import statistics
import subprocess
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
NATIVE = Path.home() / "work/other/d32x/toolchains/native/bin"
STAMPS = ("TSC0=", "TSC1=")


def _counts_per_ms() -> int:
    """DOSBox-X's RDTSC advances by the fixed cycle rate per emulated millisecond."""
    return int(re.search(r"^cycles=(\d+)$", PINNED.read_text(), re.MULTILINE).group(1))


def elapsed_ms(text: str) -> float | None:
    stamps = {}
    for line in text.splitlines():
        if line.startswith(STAMPS):
            hi, lo = (int(one) & 0xFFFFFFFF for one in line.split("=", 1)[1].split())
            stamps[line[:4]] = hi << 32 | lo
    if len(stamps) != 2:
        return None
    return (stamps["TSC1"] - stamps["TSC0"]) / _counts_per_ms()


def optimized(
    data: bytes,
    native_fpu: bool = True,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> bytes:
    result = wholeseg.emitted(
        data, native_fpu=native_fpu, cpu=cpu, basic_semantics=basic_semantics, bounds_checks=bounds_checks
    )
    if result.outcome is not wholeseg.Emission.LIR:
        raise SystemExit(f"benchmark optimization refused: {result.reason}")
    return result.data


def answers(text: str) -> tuple[str, ...]:
    lines = tuple(line.strip() for line in text.splitlines() if line.strip() and not line.startswith(STAMPS))
    if len(lines) < 2 or lines[-1] != "DONE":
        raise SystemExit("benchmark did not produce an answer followed by DONE")
    return lines


def output_name(exe: Path, repetition: int) -> str:
    return f"{exe.stem[:4]}{repetition}.TXT"


def build(
    tag: str,
    prog: str = "nbody",
    native_fpu: bool = True,
    transform=None,
    cpu: str = "386",
    basic_semantics: bool = False,
    bounds_checks: bool = False,
) -> tuple[Path, Path]:
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
    change = transform or (lambda data: optimized(data, native_fpu, cpu, basic_semantics, bounds_checks))
    (work / f"{name}Q.OBJ").write_bytes(change(obj.read_bytes()))

    assembled = subprocess.run(
        [str(NATIVE / "jwasm"), "-c", "-Cp", "-Zg", "-omf", f"-Fo{work / 'TSCSNAP.OBJ'}", str(BENCH / "tscsnap.asm")],
        capture_output=True,
        text=True,
    )
    if assembled.returncode != 0:
        raise SystemExit(f"jwasm failed on tscsnap.asm:\n{assembled.stdout}{assembled.stderr}")
    linking = launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} {name}.OBJ+TSCSNAP.OBJ, BASE.EXE,, {cfg.runtime}; >> LINK.OUT",
            f"{cfg.link} {name}Q.OBJ+TSCSNAP.OBJ, OPT.EXE,, {cfg.runtime}; >> LINK.OUT",
        ],
        env={"LIB": r"V:\LIB"},
    )
    report = read_dos(work, "LINK.OUT")
    if (
        not linking.finished
        or linking.timed_out
        or report.count("Microsoft (R) Segmented Executable Linker") != 2
        or re.search(r"unresolved external|error\s+L\d+", report, re.IGNORECASE)
    ):
        raise SystemExit(f"LINK did not complete both builds without errors; see {work / 'LINK.OUT'}")
    base, opt = work / "BASE.EXE", work / "OPT.EXE"
    if not base.is_file() or not opt.is_file():
        raise SystemExit(f"LINK did not produce both .EXEs; see {work / 'LINK.OUT'}")
    return base, opt


def run(
    tag: str, exe: Path, steps: int, reps: int, prog: str = "nbody", *, expected: tuple[str, ...] | None = None
) -> list[float]:
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
        got = elapsed_ms(out)
        if got is None or got <= 0:
            raise SystemExit(f"no positive RDTSC reading in {work / name}")
        readings.append(got)
    return readings


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="bench")
    ap.add_argument("--config", default="v-g3", choices=list(CONFIGS))
    ap.add_argument("--prog", default="nbody")
    from qbopt.backend import cpu as targets

    ap.add_argument("--cpu", choices=targets.names(), default="386")
    # Kept as an accepted compatibility spelling. Native x87 is now the only
    # production path, and an ordinary benchmark must exercise that path too.
    ap.add_argument("--native-fpu", action="store_true", default=True, help=argparse.SUPPRESS)
    ap.add_argument("--basic-semantics", action="store_true")
    ap.add_argument("--bounds-checks", action="store_true")
    ap.add_argument("--steps", type=int, default=2000)
    ap.add_argument("--reps", type=int, default=5)
    args = ap.parse_args(argv)
    if args.steps <= 0 or args.reps <= 0:
        ap.error("steps and reps must be positive")
    cfg = CONFIGS[args.config]
    base_exe, opt_exe = build(
        args.config,
        args.prog,
        args.native_fpu,
        cpu=args.cpu,
        basic_semantics=args.basic_semantics,
        bounds_checks=args.bounds_checks,
    )
    base_ms = run(args.config, base_exe, args.steps, args.reps, args.prog)
    expected = answers(read_dos(BUILD / args.config / args.prog, output_name(base_exe, 0)))
    opt_ms = run(args.config, opt_exe, args.steps, args.reps, args.prog, expected=expected)

    for label, readings in (("base", base_ms), ("opt", opt_ms)):
        print(f"{label}: {[round(one, 3) for one in readings]} ms, spread {max(readings) - min(readings):.3f} ms")

    b, o = statistics.median(base_ms), statistics.median(opt_ms)
    print(f"\nmedian: base {b:.3f} ms, opt {o:.3f} ms")
    print(f"ratio: base/opt = {b / o:.4f}")

    print("\nquotable stamp:")
    print(f"  dosbox-x: {dosbox_bin()}")
    print(f"  conf sha256: {sha256(PINNED)}")
    print(f"  config: {args.config}, steps: {args.steps}, reps: {args.reps}")
    print(f"  tuning CPU: {args.cpu} (does not change DOSBox timing model)")
    print(f"  numeric semantics: {'basic' if args.basic_semantics else 'native'}")
    print(f"  bounds checks: {args.bounds_checks}")
    print(f"  BC.EXE sha256: {sha256(host_path(cfg.mount, cfg.bc))}")
    print(f"  jwasm sha256: {sha256(NATIVE / 'jwasm')}")
    print(f"  LINK.EXE sha256: {sha256(host_path(cfg.mount, cfg.link))}")
    print(f"  runtime sha256: {sha256(host_path(cfg.mount, cfg.runtime))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
