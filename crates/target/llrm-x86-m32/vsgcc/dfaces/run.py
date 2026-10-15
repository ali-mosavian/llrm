"""d_faces.c executed under the 486 model: the code-quality scoreboard for the one file the user's goal is judged on.

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/dfaces/run.py [--levels O1 O2 O3 Os] [--json OUT]

QCport's render/d_faces.c is built by llrm (m32, the System V convention), gcc and clang at each level, and by gcc -O0 for the
reference. Each object is linked with env.c (see there), run in unicorn through `bench_dfaces`, and what executes inside d_faces.o
(and the runtime library it pulls in) is counted: instructions, memory operands, operands across an alignment boundary and 486
clocks (harness.py's model). A variant is right when the hash env.c reports equals the -O0 build's.
Products go to $VSGCC_WORK (default ~/scratch/vsgcc-work), the QCport sources are read from $QCPORT (default ~/scratch/qbq-llrm/src).
"""
import argparse
import json
import math
import os
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import harness  # noqa: E402
from harness import MEMORY, OUT, cost, elf_segments, misaligned_operands, multiplier_of, nm, omf_link  # noqa: E402
from iced_x86 import Decoder, Mnemonic  # noqa: E402
from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, Uc  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_ESP  # noqa: E402

QCPORT = Path(os.environ.get("QCPORT", Path.home() / "scratch/qbq-llrm/src"))
D_FACES = QCPORT / "render" / "d_faces.c"
SOURCE = OUT / "dfaces" / "d_faces_flat.c"
ENV_BASE = 0x300000                       # env.elf is loaded here; everything below it and above BASE is the code under test
LEVELS = ("O1", "O2", "O3", "Os")
# The Borland-era keywords mean nothing flat; every compiler reads the file with the same convention and the same sizes.
KEYWORDS = "far huge near pascal cdecl __far _far __near _near"
INCLUDES = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(QCPORT / d))]
GCC_FLAGS = ["-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-fno-asynchronous-unwind-tables", "-w"]
COVERAGE = ["flat", "tex", "ptex", "wire", "refused", "sc_hit", "sc_miss", "alloc", "alloc_fail", "build", "noz", "nodst"]


def sh(cmd, **kw):
    done = subprocess.run([str(c) for c in cmd], capture_output=True, text=True, **kw)
    assert done.returncode == 0, f"{' '.join(map(str, cmd))}\n{done.stderr[-2000:]}"
    return done


def stub():
    """stub.elf as run.sh builds it, made here when absent."""
    built = OUT / "stub.elf"
    if not (OUT / "stub.elf.src").exists() or not built.exists():
        OUT.mkdir(parents=True, exist_ok=True)
        sys.path.insert(0, str(harness.REPO / "tools"))
        import linkrecipe
        sh(["gcc", "-m32", "-c", HERE.parent / "stub.s", "-o", OUT / "stub.o"])
        sh(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x8000", "-o", built, OUT / "stub.o"])
        harness.record_stub(OUT)
    return harness.fresh_stub()


def emulation():
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    return linkrecipe.ld_emulation("x86-m32")


def write_source():
    """The file every compiler reads: d_faces.c with its keywords of the segmented models emptied, so that they all take the flat
    model and one calling convention (llrm-c has no -D)."""
    SOURCE.parent.mkdir(parents=True, exist_ok=True)
    SOURCE.write_text("".join(f"#define {word}\n" for word in KEYWORDS.split()) + f'#include "{D_FACES}"\n')


def build_env():
    """env.c, once per change of the file or the scene's headers: gcc -O0, linked at ENV_BASE against the stub."""
    work = OUT / "dfaces"
    work.mkdir(parents=True, exist_ok=True)
    elf = work / "env.elf"
    if not elf.exists() or elf.stat().st_mtime < max((HERE / "env.c").stat().st_mtime, (QCPORT / "render/qcshared.h").stat().st_mtime):
        sh(["gcc", *GCC_FLAGS, "-O0", "-I", QCPORT / "render", "-c", HERE / "env.c", "-o", work / "env.o"])
        sh(["ld", "-m", emulation(), "-static", "-e", "0", f"-Ttext={ENV_BASE:#x}", f"--just-symbols={stub()}", "-o", elf, work / "env.o"])
    return elf


def build_variant(variant):
    """d_faces.c as `variant` makes it: OMF for llrm, ELF linked at BASE for gcc and clang. Names as build.sh gives them, so that
    harness.code_bytes reads the sizes."""
    (OUT / "o").mkdir(parents=True, exist_ok=True)
    (OUT / "b").mkdir(parents=True, exist_ok=True)
    level = variant[-2:] if variant[-2:] in ("O0", "O1", "O2", "O3", "Os") else "O2"
    if variant.startswith("llrm"):
        level = variant[4:] or "O2"
        obj = OUT / "o" / f"dfaces.{variant}.obj"
        sh([harness.LLRM, "-m32", "-mabi=sysv", f"-{level}", "-march=i486", *INCLUDES, "-o", obj, SOURCE])
        return obj
    compiler = "gcc" if variant.startswith("gcc") else "clang"
    level = variant[len(compiler):]
    obj = OUT / "b" / f"dfaces.{variant}.o"
    sh([compiler, *GCC_FLAGS, f"-{level}", *INCLUDES, "-c", SOURCE, "-o", obj])
    runtime = []
    if level != "O0":
        runtime = [sh(["gcc", "-m32", "--print-libgcc-file-name"]).stdout.strip()] if compiler == "gcc" else [sh(["clang", "-m32", "--rtlib=compiler-rt", "-print-libgcc-file-name"]).stdout.strip()]
    elf = OUT / "b" / f"dfaces.{variant}.elf"
    sh(["ld", "-m", emulation(), "-static", "-e", "0", f"-Ttext={harness.BASE:#x}", f"--just-symbols={stub()}", f"--just-symbols={build_env()}",
        f"-Map={OUT / 'b' / f'dfaces.{variant}.map'}", "-o", elf, obj, *runtime])
    return elf


def pick(symbols, name):
    for spelled in (name, "_" + name, name + "_"):
        if spelled in symbols:
            return symbols[spelled]
    raise KeyError(name)


def run(variant, limit=400_000_000):
    write_source()
    stub_elf, env_elf = stub(), build_env()
    stub_symbols, env_symbols = nm(stub_elf), nm(env_elf)
    built = build_variant(variant)
    if variant.startswith("llrm"):
        segments, symbols = omf_link(built, {**stub_symbols, **env_symbols})
    else:
        segments, symbols = elf_segments(built), nm(built)
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(0, harness.MEM)
    for va, blob, _ in segments + elf_segments(stub_elf) + elf_segments(env_elf):
        uc.mem_write(va, blob)
    uc.mem_write(harness.SENT, b"\xf4")
    esp = harness.STACK_TOP - 8
    uc.mem_write(esp, struct.pack("<III", harness.SENT, 0, 0))
    uc.reg_write(UC_X86_REG_ESP, esp)
    for slot, name in (("entry_draw", "d_draw_faces"), ("entry_layout", "d_faces_layout_ok")):
        uc.mem_write(env_symbols[slot], struct.pack("<I", pick(symbols, name)))
    report_at = {stub_symbols["report"]}
    total, cache, reports = Counter(), {}, []
    st = dict(prev=None, firstrep=True, multiplier=None)

    def hook(uc, address, size, _):
        if address == harness.SENT:
            uc.emu_stop()
            return
        if address in report_at:
            reports.append(struct.unpack("<i", uc.mem_read(uc.reg_read(UC_X86_REG_ESP) + 4, 4))[0])
        if not harness.BASE <= address < ENV_BASE:
            # Out of d_faces: the call that left it, or its return, is the last instruction charged.
            if st["prev"] is not None:
                ins, mem = cache[st["prev"]][:2]
                total["clocks"] += cost(ins, True, True, mem, st["multiplier"])
                st["prev"] = None
            return
        info = cache.get(address)
        if info is None:
            ins = Decoder(32, bytes(uc.mem_read(address, min(size, 15))), ip=address).decode()
            mem = 0 if ins.mnemonic == Mnemonic.LEA else sum(1 for i in range(ins.op_count) if ins.op_kind(i) in MEMORY)
            rep = ins.has_rep_prefix or ins.has_repe_prefix or ins.has_repne_prefix
            info = cache[address] = (ins, mem, rep)
        ins, mem, rep = info
        if rep and uc.reg_read(UC_X86_REG_ECX) == 0:
            return
        prev = st["prev"]
        if prev is not None and cache[prev][0].mnemonic != Mnemonic.NOP:
            pins = cache[prev][0]
            total["clocks"] += cost(pins, address != prev + pins.len, st["firstrep"], cache[prev][1], st["multiplier"])
        st["firstrep"] = not (rep and prev == address)
        st["prev"] = address
        if ins.mnemonic == Mnemonic.NOP:
            total["nops"] += 1
            return
        total["ins"] += 1
        total["mem"] += mem
        if mem:
            total["misaligned"] += misaligned_operands(uc, ins)
        st["multiplier"] = multiplier_of(uc, ins) if ins.mnemonic in (Mnemonic.MUL, Mnemonic.IMUL) else None

    uc.hook_add(UC_HOOK_CODE, hook)
    uc.emu_start(env_symbols["main"], harness.SENT, count=limit)
    assert total["ins"], f"{variant}: d_faces was never entered"
    covered = struct.unpack(f"<{len(COVERAGE)}I", bytes(uc.mem_read(env_symbols["coverage"], 4 * len(COVERAGE))))
    return dict(prog="dfaces", variant=variant, ins=total["ins"], mem=total["mem"], misaligned=total["misaligned"], nops=total["nops"],
                clocks=total["clocks"], code=harness.code_bytes("dfaces", variant), reports=reports, coverage=dict(zip(COVERAGE, covered)))


def geomean(values):
    return math.exp(sum(math.log(v) for v in values) / len(values))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--levels", nargs="*", default=list(LEVELS))
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    reference = run("gccO0")
    assert len(reference["reports"]) == 1, reference["reports"]
    rows = {"gccO0": reference}
    missing = [name for name, count in reference["coverage"].items() if not count and name not in ("flat",)]
    assert not missing, f"the scene reaches none of: {missing}"
    for level in args.levels:
        for variant in (f"llrm{'' if level == 'O2' else level}", f"gcc{level}", f"clang{level}"):
            rows[variant] = run(variant)
    keys = ("ins", "mem", "misaligned", "clocks", "code")
    print("scene reaches: " + ", ".join(f"{name} {count}" for name, count in reference["coverage"].items()))
    print(f"{'variant':10}" + "".join(f"{k:>11}" for k in keys) + "   ok")
    for variant, r in rows.items():
        r["ok"] = r["reports"] == reference["reports"]
        print(f"{variant:10}" + "".join(f"{r[k]:>11}" for k in keys) + f"   {'yes' if r['ok'] else 'WRONG'}")
    print()
    for level in args.levels:
        llrm = rows[f"llrm{'' if level == 'O2' else level}"]
        for other in ("gcc", "clang"):
            ref = rows[f"{other}{level}"]
            print(f"-{level} llrm/{other}: " + "  ".join(f"{k} {llrm[k] / max(1, ref[k]):.2f}" for k in keys))
    if args.json:
        args.json.write_text("".join(json.dumps({k: v for k, v in r.items() if k != "reports"}) + "\n" for r in rows.values()))
    return 0 if all(r["ok"] for r in rows.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
