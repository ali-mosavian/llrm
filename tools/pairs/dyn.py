"""Executed instructions and 486 clocks of ours and the pin's, per program, in the vsgcc harness (m32 only).

Each kernel is wrapped (`wrap.py`: called through a volatile pointer) so no compiler inlines it into main; `pindiff.py --dyn` adds the figures
to its table. Needs iced_x86 and unicorn on the Python path (`harness.py`)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
VSGCC = ROOT / "crates/target/llrm-x86-m32/vsgcc"


def stub(work: Path) -> None:
    """The harness' stub.elf, built from stub.s into `work` and stamped with the digest `harness.fresh_stub` asks for."""
    sys.path.insert(0, str(VSGCC))
    import harness

    emulation = subprocess.run(["python3", str(ROOT / "tools/linkrecipe.py"), "x86-m32", "ld-emulation"], capture_output=True, text=True, check=True).stdout.strip()
    subprocess.run(["gcc", "-m32", "-c", str(VSGCC / "stub.s"), "-o", str(work / "stub.o")], check=True)
    subprocess.run(["ld", "-m", emulation, "-static", "-e", "0", "-Ttext=0x8000", "-o", str(work / "stub.elf"), str(work / "stub.o")], check=True)
    harness.record_stub(work)


def ours_command(llc: str, level: str, march: str, ll: Path, out: Path) -> list[str]:
    """Ours at the level asked: -Os is -Os (the static columns compile it so; -O2 here gave Os a `movzx` load the pin's optsize code lacks)."""
    return [llc, "-m32", f"-{level}", f"-march={march}", "--isel=dag", "-mabi=sysv", "-fobject-format=elf", "-o", str(out), str(ll)]


def measure(names: list[str], level: str, march: str, llc: str, work: Path, one) -> dict[str, dict | str]:
    """name -> {ins, pin_ins, clk, pin_clk, same} or why there is none. `one` is pindiff's `one`."""
    work.mkdir(parents=True, exist_ok=True)
    os.environ["VSGCC_WORK"] = str(work)
    stub(work)
    sys.path.insert(0, str(VSGCC))
    import harness
    import programs
    import wrap

    (work / "b").mkdir(exist_ok=True)
    sources = programs.sources()
    out: dict[str, dict | str] = {}
    for name in names:
        if name not in sources:
            out[name] = "not a vsgcc program"
            continue
        src = work / "src" / f"{name}.c"
        src.parent.mkdir(exist_ok=True)
        src.write_text(wrap.wrapped(name, sources[name].read_text()))
        built = one(src, "m32", level, llc, work, False)
        if isinstance(built, str):
            out[name] = built
            continue
        ours = work / f"{name}.dyn.o"
        ran = subprocess.run(ours_command(llc, level, march, work / f"{name}.ll", ours), capture_output=True, text=True)
        if ran.returncode:
            out[name] = "ours elf: " + (ran.stderr.strip().splitlines() or ["?"])[0][:100]
            continue
        failed = None
        for variant, obj in (("ours", ours), ("pin", work / f"{name}.pin.o")):
            link = subprocess.run(["ld", "-m", "elf_i386", "-static", "-e", "0", f"-Ttext={harness.BASE:#x}", f"--just-symbols={work}/stub.elf", "-o", str(work / "b" / f"{name}.{variant}{level}.elf"), str(obj)], capture_output=True, text=True)
            if link.returncode:
                failed = f"link {variant}: " + (link.stderr.strip().splitlines() or ["?"])[-1][:100]
        if failed:
            out[name] = failed
            continue
        try:
            a, b = harness.run(name, f"ours{level}"), harness.run(name, f"pin{level}")
        except Exception as error:  # the emulator's own failures name the program
            out[name] = f"run: {str(error)[:100]}"
            continue
        out[name] = {"ins": a["ins"], "pin_ins": b["ins"], "clk": a["clocks"], "pin_clk": b["clocks"], "same": a["reports"] == b["reports"]}
    return out
