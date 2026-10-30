"""NIBBLES at m16 and BCOM45's own build of it, played to "Sammy Dies" under a matrix of emulator confs: for each, the pixels the death
screen differs from BCOM45's (base conf) and the emulated time and cycles it came at. A hosted run that dies at a pixel count a dev box
does not says, by which confs move it (and whether the BCOM45 control moves too), whether the emulator or the compiler is the cause.

    python3 tools/dosbatch/nibprobe.py        # one line per conf and subject; always exits 0"""
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import qb32  # noqa: E402
import qb32demos  # noqa: E402
import qbbuild  # noqa: E402
import qbplay  # noqa: E402
import qbruntime  # noqa: E402

DEMO = "NIBBLES"
CHECKPOINT = "dies"
# (label, conf edit, keys typed without waiting for the program's read). Virtual time is fixed cycles per emulated ms: `cycles=max`
# has no lockstep, so the rate is what varies.
def same(conf: str) -> str:
    return conf


VARIANTS = [
    ("base", same, False),
    ("core=simple", lambda conf: conf.replace("core=normal", "core=simple"), False),
    ("fpu=false", lambda conf: conf.replace("cputype=pentium", "cputype=pentium\nfpu=false"), False),
    ("rate=500", lambda conf: conf.replace("virtual time rate=2000", "virtual time rate=500"), False),
    ("rate=20000", lambda conf: conf.replace("virtual time rate=2000", "virtual time rate=20000"), False),
    ("keys=timed", same, True),
]


def edited(label: str, edit, conf: str) -> str:
    """`conf` with the variant's edit, which must change it: a variant that edits nothing would print a second base."""
    new = edit(conf)
    assert edit is same or new != conf, f"{label}: the conf has nothing to edit"
    return new


def line(label: str, subject: str, exe: Path, beside: tuple[Path, ...], want: Path, work: Path, edit, timed: bool) -> str:
    """The probe's line for one conf and subject."""
    work.mkdir(parents=True)
    for one in beside:
        (work / one.name).write_bytes(one.read_bytes())
    ended: dict = {}
    try:
        shots = qb32demos.play(exe, work, qb32demos.script(DEMO), DEMO, conf=edited(label, edit, qb32.DEMO_CONF), timed=timed, ended=ended)
        pixels = qbplay.differing_pixels(want, shots[CHECKPOINT])
    except Exception as why:
        return f"nibprobe {label:<12} {subject:<7} {type(why).__name__}: {str(why)[:100]}"
    cycles = ended["us"] * ended["cycles_per_ms"] // 1000
    return f"nibprobe {label:<12} {subject:<7} {pixels:>6} pixels at {CHECKPOINT}, {ended['us']} emulated us, {cycles} cycles"


def main() -> int:
    found, reason = qbruntime.demo_sources()
    sources = {path.stem.upper(): path for path in found.values()}
    if DEMO not in sources:
        print(f"nibprobe: skipped, {reason or 'no ' + DEMO}")
        return 0
    with tempfile.TemporaryDirectory() as where:
        work = Path(where)
        try:
            built = qbbuild.build(sources[DEMO], work / "build", qbbuild.runtimes(work))[qbbuild.REAL]
            (work / "ref").mkdir()
            control = qbplay.reference({DEMO: sources[DEMO]}, work / "ref")[DEMO]
            want = qb32demos.reference_shots(DEMO, sources[DEMO], work / "refshots")[CHECKPOINT]
        except Exception as why:
            print(f"nibprobe: could not build: {type(why).__name__}: {str(why)[:200]}")
            return 0
        subjects = {"m16": (built.exe, built.files), "bcom45": (control, ())}
        with ThreadPoolExecutor(4) as pool:
            futures = [
                pool.submit(line, label, subject, exe, beside, want, work / "play" / f"{label}-{subject}", edit, timed)
                for label, edit, timed in VARIANTS
                for subject, (exe, beside) in subjects.items()
            ]
            for future in futures:
                print(future.result(), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
