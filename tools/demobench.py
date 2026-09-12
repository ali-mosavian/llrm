"""
Matched BC and qbopt runs of the phatcode Spring 2002 QuickBASIC demos.

Each demo is compiled from one instrumented source and linked twice: as BC
left it, and after `qbopt.rewrite` over the object and BCOM45.LIB. Both
builds run the same frames under a fixed-cycle DOSBox, so emulated PIT time
is the measure. Each mark records the PIT ticks since the previous mark and
an Adler-32 of video memory plus the DAC palette; the two builds must agree
on every checksum before any timing is reported.

The instrumentation is measurement, not optimization: vertical-retrace
WAITs would hide CPU cost behind the 70 Hz refresh, TIMER-driven work and
RANDOMIZE TIMER would make the two builds do different work, and a key
wait would never return. Everything else in the demo is left as written.

The archives are not in the repository; pass the directory they were
unpacked into (one subdirectory per demo, as named in DEMOS).
"""

import re
import sys
import shutil
import struct
import argparse
from pathlib import Path
from dataclasses import dataclass

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos

from qbopt import rewrite

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "build" / "demobench"

MEASURE = """\
[sdl]
priority=higher,normal
output=surface
[dosbox]
memsize=32
startquiet=true
startbanner=false
[cpu]
core=dynamic
cputype=pentium
cycles=fixed 75000
[dos]
xms=true
ems=true
dpmi=false
[mixer]
nosound=true
"""

PRELUDE = """\
DECLARE SUB BenchMark (phase AS INTEGER)
DECLARE SUB BenchSnap (tick AS LONG, count AS LONG)
DIM SHARED benchTick AS LONG, benchCount AS LONG, benchFrame AS LONG
"""

# Types spelled out: the demos' own DEFxxx statements reach this far.
EPILOGUE = """
REM $DYNAMIC
SUB BenchMark (phase AS INTEGER)
    ' Far, so a demo whose DGROUP is nearly full still links.
    DIM tint(767) AS INTEGER, taken(2) AS LONG
    DIM tick AS LONG, count AS LONG, elapsed AS LONG, offset AS LONG
    DIM low AS LONG, high AS LONG, entry AS INTEGER
    CALL BenchSnap(tick, count)
    elapsed = (tick - benchTick) * 65536& + (benchCount - count)
    low = 1: high = 0
    DEF SEG = &HA000
    FOR offset = 0 TO 63999
        low = (low + PEEK(offset)) MOD 65521
        high = (high + low) MOD 65521
    NEXT
    BSAVE "V" + LTRIM$(STR$(phase)) + ".BIN", 0, 64000
    DEF SEG
    OUT &H3C7, 0
    FOR entry = 0 TO 767
        tint(entry) = INP(&H3C9)
    NEXT
    DEF SEG = VARSEG(tint(0))
    BSAVE "P" + LTRIM$(STR$(phase)) + ".BIN", VARPTR(tint(0)), 1536
    ' BSAVE, not OPEN: a file buffer comes out of DGROUP string space, and
    ' deedlines has too little left -- its text$ assignments hit error 14.
    taken(0) = elapsed: taken(1) = high: taken(2) = low
    DEF SEG = VARSEG(taken(0))
    BSAVE "T" + LTRIM$(STR$(phase)) + ".BIN", VARPTR(taken(0)), 12
    DEF SEG
    CALL BenchSnap(benchTick, benchCount)
END SUB

SUB BenchSnap (tick AS LONG, count AS LONG)
    ' bench/nbody.bas's PitSnap, without the mask. Masking IRQ0 across the
    ' read stops the BIOS tick from moving, which is not the same as making
    ' it right: a tick already pending is not counted, and the reading is a
    ' whole 65536 short with the down-counter nowhere near an edge, where no
    ' edge guard can see it. qbdemo's mark 4 read 305304 against 370838 for
    ' two programs whose mark-4 code is the same instructions.
    ' Reading the tick either side and retrying while they differ is the
    ' fact wanted: no tick landed during this reading.
    ' One byte either side, not the whole count: `tick` is a reference
    ' parameter, and reading it back to compare made the guard a question
    ' about a far load rather than about the clock. The low byte of the
    ' BIOS tick changes on every tick and wraps in 14 seconds, which is
    ' all "did a tick land during this reading" needs.
    DIM low AS INTEGER, high AS INTEGER, raw AS LONG
    DIM first AS INTEGER, second AS INTEGER
    DO
        DEF SEG = &H40
        first = PEEK(&H6C)
        OUT &H43, &H0
        low = INP(&H40)
        high = INP(&H40)
        second = PEEK(&H6C)
        tick = CLNG(PEEK(&H6C)) + CLNG(PEEK(&H6D)) * 256& + CLNG(PEEK(&H6E)) * 65536&
        DEF SEG
        raw = CLNG(low) + CLNG(high) * 256&
    LOOP WHILE first <> second OR raw > 65536& - 12000 OR raw < 12000
    count = raw
END SUB
"""

# q-O without /Zi: CodeView data costs ~900 bytes of DGROUP, and deedlines,
# built as shipped, runs out of string space (error 14) with it.
SWITCHES = "/O /FPi"

RETRACE = (r"WAIT\s+&H3DA\s*,\s*\d+(\s*,\s*\d+)?", "benchFrame = benchFrame")


@dataclass(frozen=True, slots=True)
class Demo:
    folder: str
    source: str
    assets: tuple[str, ...]
    # (pattern, replacement): each must match at least once, so a spec that
    # has drifted from its source fails rather than measuring something else
    edits: tuple[tuple[str, str], ...]
    arguments: str = ""


DEMOS = {
    "qbdemo": Demo(
        "qbdemo",
        "QBDEMO.BAS",
        ("CANADA.BSV",),
        (
            RETRACE,
            (r"^ti1! = TIMER$", "BenchMark 0\nti1! = TIMER"),
            (r"^fractaleffect 2250$", "fractaleffect 150\nBenchMark 1"),
            (r"^shadebobeffect 8192$", "shadebobeffect 1024\nBenchMark 2"),
            (r"^plasma 1024$", "plasma 160\nBenchMark 3"),
            (r"^ohcanada 512.*$", "ohcanada 96\nBenchMark 4"),
            (r"^a\$ = INPUT\$\(1\)$", "SYSTEM"),
        ),
    ),
    "oimad": Demo(
        "oimad",
        "OIMAD.BAS",
        ("DEM1.RAW", "MASK.BSV", "PAL.DAT", "TEXT.BSV"),
        (
            RETRACE,
            (r"^Init$", "Init\nBenchMark 0"),
            (r"IF TIMER - oldtimer# > \.1 THEN", "IF benchFrame MOD 8 = 0 THEN"),
            (r"IF TIMER - oldtimer2# > 5 THEN", "IF benchFrame > 1500 THEN"),
            (
                r"^LOOP UNTIL INKEY\$ = CHR\$\(27\)$",
                "benchFrame = benchFrame + 1\nLOOP UNTIL benchFrame >= 3000\nBenchMark 1\nSYSTEM",
            ),
        ),
        arguments="-NOSOUND",
    ),
    "deedlines": Demo(
        "deedlines",
        "DEEDSAKS.BAS",
        (
            "3DREC.001",
            "3RDREC.REC",
            "COW.3DO",
            "FONTS16.FNT",
            "HITEAPOT.3DO",
            "QBINSIDE.SPR",
            "SPHRPREC.DAT",
            "WOLFMAP0.WAD",
        ),
        (
            RETRACE,
            # A retrace-paced pause; without the WAIT it only spins on the keyboard.
            (r"^(SUB delay \(t%\)\s+SHARED xit%)$", r"\1\nEXIT SUB"),
            (r"^RANDOMIZE TIMER$", "RANDOMIZE 1"),
            (r"^ax = TIMER$", ""),
            # Every effect's timeline at a tenth: fades, phase switches, exits.
            (r"^s% = 0: t% = 256$", "s% = 0: t% = 26"),
            (r"^s1% = 0: t1% = 256$", "s1% = 0: t1% = 26"),
            (r"^IF fps% = 256 THEN fs", "IF fps% = 26 THEN fs"),
            (r"^IF fps% = 256 \+ 768 THEN", "IF fps% = 102 THEN"),
            (r"^IF fps% = 256 \+ 1024 THEN", "IF fps% = 128 THEN"),
            (r"^(FOR [ln]% = \d+ TO \d+)$", r"\1 STEP 10"),
            (
                r"^IF fps% > 1400 AND kxy0% > -100 THEN kxy0% = kxy0% - 100 ELSE "
                r"IF fps% <= 1000 AND kxy0% < 10000 THEN kxy0% = kxy0% \+ 100$",
                "IF fps% > 140 AND kxy0% > -100 THEN kxy0% = kxy0% - 1010 ELSE "
                "IF fps% <= 100 AND kxy0% < 10000 THEN kxy0% = kxy0% + 1000",
            ),
            (r"k% < 1024 THEN (pl[xy]) = \1 - \.025$", r"k% < 102 THEN \1 = \1 - .25"),
            (
                r"^IF k% > 1024 THEN plx = plx \+ \.01: ply = ply \+ \.01$",
                "IF k% > 102 THEN plx = plx + .1: ply = ply + .1",
            ),
            (r"^IF k% = 1220 THEN", "IF k% = 122 THEN"),
            (r"^IF filei% = 4446 THEN", "IF filei% = 444 THEN"),
            (r"^IF filei% = 194 THEN", "IF filei% = 20 THEN"),
            (r"^m% = 0: q% = 32:", "m% = 0: q% = 3:"),
            (r"q% = INT\(RND \* 255 \+ 5\)", "q% = INT(RND * 25 + 1)"),
            (r"^IF fps% < 1600 AND", "IF fps% < 160 AND"),
            (
                r"^IF fps% > 1600 AND fps% < 1855 AND m% = q% THEN sq% = 1: m% = 0: q% = 255:",
                "IF fps% > 160 AND fps% < 185 AND m% = q% THEN sq% = 1: m% = 0: q% = 25:",
            ),
            (r"^IF fps% > 1856 AND", "IF fps% > 185 AND"),
            (r"^IF fps% > 2100 THEN", "IF fps% > 210 THEN"),
            (r"^IF fps% = 512 THEN k0%", "IF fps% = 51 THEN k0%"),
            (r"^IF fps% = 1512 THEN", "IF fps% = 151 THEN"),
            (r"^IF fps% = 2500 THEN", "IF fps% = 250 THEN"),
            (r"^IF fps% = 2800 THEN", "IF fps% = 280 THEN"),
            (r"^prehistoricode$", "BenchMark 0"),
            (r"^spheremaplasma$", "BenchMark 1\nspheremaplasma\nBenchMark 2"),
            (r"^zoomdistort$", "zoomdistort\nBenchMark 3"),
            (r"^rgblights$", "rgblights\nBenchMark 4"),
            (r"^actions3d$", "actions3d\nBenchMark 5"),
            (r"^cycleblobs$", "cycleblobs\nBenchMark 6"),
            (r"^plasmablobs$", "plasmablobs\nBenchMark 7"),
            (r"^telos:$", "telos:\nBenchMark 8\nSYSTEM"),
        ),
    ),
}


def instrumented(demo: Demo, text: str) -> str:
    lines = text.replace("\r\n", "\n")
    for pattern, replacement in demo.edits:
        lines, count = re.subn(pattern, replacement, lines, flags=re.MULTILINE | re.IGNORECASE)
        if count == 0:
            raise SystemExit(f"{demo.source}: edit {pattern!r} matched nothing")
    return PRELUDE + lines.rstrip("\n") + "\n" + EPILOGUE


def build(name: str, demos: Path, cpu: str) -> Path:
    demo = DEMOS[name]
    cfg = CONFIGS["q-O"]
    work = BUILD / name
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    source = demos / demo.folder / demo.source
    (work / "DEMO.BAS").write_bytes(
        instrumented(demo, source.read_text("latin-1")).replace("\n", "\r\n").encode("latin-1")
    )
    for asset in demo.assets:
        shutil.copy(demos / demo.folder / asset, work / asset)

    compiled = launch(work, cfg.mount, [f"{cfg.bc} {SWITCHES} DEMO.BAS, DEMO.OBJ; > BC.OUT"], env={"LIB": r"V:\LIB"})
    if not compiled.finished or re.findall(r"(\d+)\s+Severe\s+Error", read_dos(work, "BC.OUT")) != ["0"]:
        raise SystemExit(f"BC failed; see {work / 'BC.OUT'}")
    runtime = cfg.mount / "LIB" / "BCOM45.LIB"
    if rewrite.main([str(work / "DEMO.OBJ"), str(runtime), "-o", str(work / "DEMOQ.OBJ"), "--cpu", cpu]):
        raise SystemExit(f"qbopt failed on {name}")

    linked = launch(
        work,
        cfg.mount,
        [
            f"{cfg.link} DEMO.OBJ, BASE.EXE,, {cfg.runtime}; > LINK.OUT",
            f"{cfg.link} DEMOQ.OBJ, OPT.EXE,, {cfg.runtime}; >> LINK.OUT",
        ],
        env={"LIB": r"V:\LIB"},
    )
    report = read_dos(work, "LINK.OUT")
    if not linked.finished or re.search(r"unresolved external|error\s+L\d+", report, re.IGNORECASE):
        raise SystemExit(f"LINK failed; see {work / 'LINK.OUT'}")
    # For driving the runs by hand through the DOSBox debugger, watching them.
    (work / "mcp.conf").write_text(f"{MEASURE}[autoexec]\n@echo off\nmount c {work}\nc:\n")
    return work


def adler(data: bytes) -> tuple[int, int]:
    low, high = 1, 0
    for byte in data:
        low = (low + byte) % 65521
        high = (high + low) % 65521
    return high, low


def collect(work: Path, build: str) -> Path:
    """Move one run's outputs -- each mark's T/V/P dumps -- aside."""
    into = work / build
    shutil.rmtree(into, ignore_errors=True)
    into.mkdir()
    for one in work.iterdir():
        if one.is_file() and re.fullmatch(r"[TVP]\d+\.BIN", one.name, re.IGNORECASE):
            one.rename(into / one.name.upper())
    return into


def marks(into: Path) -> dict[int, tuple[int, tuple[int, int], bytes | None]]:
    """Per mark: ticks, the checksum the program computed, and the dumped screen and palette."""
    found = {}
    for taken in sorted(into.glob("T*.BIN")):
        mark = int(taken.stem[1:])
        # BSAVE prefixes a seven-byte header.
        elapsed, high, low = struct.unpack("<3i", taken.read_bytes()[7:19])
        screen, palette = into / f"V{mark}.BIN", into / f"P{mark}.BIN"
        dumped = screen.read_bytes()[7:] + palette.read_bytes()[7:] if screen.is_file() and palette.is_file() else None
        found[mark] = (elapsed, (high, low), dumped)
    return found


def report(name: str, work: Path) -> bool:
    base, opt = marks(work / "base"), marks(work / "opt")
    print(name)
    same_everywhere = True
    total_base = total_opt = 0
    for mark in sorted(base):
        base_ticks, base_probe, base_dump = base[mark]
        opt_ticks, opt_probe, opt_dump = opt.get(mark, (0, None, None))
        truth = adler(base_dump[:64000]) if base_dump else None
        # The probe is the program's own checksum, compiled by BC and by us:
        # it must agree with the host's reading of the same bytes.
        probes = (
            f"probe BC {'ok' if base_probe == truth else base_probe} / qbopt {'ok' if adler(opt_dump[:64000]) == opt_probe else opt_probe}"
            if opt_dump
            else ""
        )
        same = base_dump is not None and base_dump == opt_dump
        if mark == 0:
            print(f"  mark 0: {'same output' if same else 'OUTPUT DIFFERS'}  {probes}")
            continue
        same_everywhere &= same
        total_base, total_opt = total_base + base_ticks, total_opt + opt_ticks
        ratio = base_ticks / opt_ticks if opt_ticks > 0 else 0
        print(
            f"  mark {mark}: {base_ticks:>10d} -> {opt_ticks:>10d}  {ratio:5.2f}x  "
            f"{'same output' if same else 'OUTPUT DIFFERS'}  {probes}"
        )
    if total_opt:
        print(f"  total : {total_base:>10d} -> {total_opt:>10d}  {total_base / total_opt:5.2f}x")
    return same_everywhere


def run(name: str, work: Path, exe: str, timeout: int, visible: bool) -> None:
    result = launch(
        work, CONFIGS["q-O"].mount, [f"{exe} {DEMOS[name].arguments}"], conf=MEASURE, timeout=timeout, visible=visible
    )
    if not result.finished or result.timed_out:
        raise SystemExit(f"{name} {exe} did not finish")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="demobench")
    ap.add_argument("demos", type=Path, help="directory holding the unpacked demo archives")
    ap.add_argument("names", nargs="*", default=sorted(DEMOS))
    ap.add_argument("--cpu", default="386")
    ap.add_argument("--timeout", type=int, default=1800)
    ap.add_argument("--visible", action="store_true", help="show the DOSBox window while the demos run")
    ap.add_argument(
        "--build-only",
        action="store_true",
        help="build both EXEs and stop; run them by hand, then --collect and --report",
    )
    ap.add_argument("--collect", choices=("base", "opt"), help="move a finished run's outputs aside")
    ap.add_argument("--report", action="store_true", help="compare collected base and opt runs")
    args = ap.parse_args(argv)
    failed = False
    for name in args.names:
        work = BUILD / name
        if args.collect:
            collect(work, args.collect)
            continue
        if args.report:
            failed |= not report(name, work)
            continue
        work = build(name, args.demos, args.cpu)
        if args.build_only:
            continue
        run(name, work, "BASE.EXE", args.timeout, args.visible)
        collect(work, "base")
        run(name, work, "OPT.EXE", args.timeout, args.visible)
        collect(work, "opt")
        failed |= not report(name, work)
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
