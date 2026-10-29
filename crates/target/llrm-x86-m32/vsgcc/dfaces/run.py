"""d_faces.c executed under the 486 model: the code-quality scoreboard for the one file the user's goal is judged on.

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/dfaces/run.py [--levels O1 O2 O3 Os] [--json OUT]

QCport's render/d_faces.c is built by llrm (m32, the System V convention), gcc and clang at each level, and by gcc -O0 for the
reference. Each object is linked with env.c (see there), run in unicorn through `bench_dfaces`, and what executes inside d_faces.o
(and the runtime library it pulls in) is counted: instructions, memory operands, operands across an alignment boundary and 486
clocks (harness.py's model). A variant is right when the hash env.c reports equals the -O0 build's.
Products go to $VSGCC_WORK, as for the rest of vsgcc; the QCport sources are read from $DFACES_SRC, else $QCPORT: the directory that holds render/d_faces.c (the qb-quake variant, which
is not the QCport the QCPORT steps build).
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
import pin_clang  # noqa: E402
from harness import MEMORY, MISALIGNED, OUT, cost, elf_segments, misaligned_operands, multiplier_of, nm, omf_link  # noqa: E402
from iced_x86 import Decoder, Mnemonic  # noqa: E402
from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, Uc  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_ESP  # noqa: E402

QCPORT = Path(os.environ.get("DFACES_SRC") or os.environ.get("QCPORT") or Path.home() / "scratch/qbq-llrm/src")
D_FACES = QCPORT / "render" / "d_faces.c"
SOURCE = OUT / "dfaces" / "d_faces_flat.c"
ENV_BASE = 0x300000                       # env.elf is loaded here; everything below it and above BASE is the code under test
LEVELS = ("O1", "O2", "O3", "Os")
import wrapper  # noqa: E402

KEYWORDS = wrapper.KEYWORDS
INCLUDES = wrapper.includes(QCPORT)
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
    model and one calling convention."""
    wrapper.write_source(D_FACES, SOURCE)


def write_env_source():
    """env.c for the flat build: `FLAT` empties the segmented keywords."""
    path = OUT / "dfaces" / "env_flat.c"
    path.parent.mkdir(parents=True, exist_ok=True)
    trace = {"1": "#define TRACE 1\n", "all": "#define TRACE 1\n#define TRACE_ALL 1\n"}.get(os.environ.get("DFACES_TRACE", ""), "")
    path.write_text(f'#define FLAT 1\n{trace}#include "{HERE / "env.c"}"\n')
    return path


def build_env():
    """env.c, once per change of the file or the scene's headers: gcc -O0, linked at ENV_BASE against the stub."""
    work = OUT / "dfaces"
    work.mkdir(parents=True, exist_ok=True)
    elf = work / "env.elf"
    if not elf.exists() or elf.stat().st_mtime < max((HERE / "env.c").stat().st_mtime, (QCPORT / "render/qcshared.h").stat().st_mtime):
        sh(["gcc", *GCC_FLAGS, "-O0", "-I", QCPORT / "render", "-c", write_env_source(), "-o", work / "env.o"])
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
        sh(wrapper.llrm_command(harness.LLRM, level, QCPORT, SOURCE, obj))
        return obj
    compiler = "gcc" if variant.startswith("gcc") else "clang"
    level = variant[len(compiler):]
    command = compiler if compiler == "gcc" else pin_clang.path()  # not PATH's clang: the distro's 20.1.8 is not the pin
    obj = OUT / "b" / f"dfaces.{variant}.o"
    sh([command, *GCC_FLAGS, f"-{level}", *INCLUDES, "-c", SOURCE, "-o", obj])
    runtime = []
    if level != "O0":
        libgcc = sh(["gcc", "-m32", "--print-libgcc-file-name"]).stdout.strip()
        runtime = [libgcc]
        if compiler != "gcc":
            builtins = sh([command, "-m32", "--rtlib=compiler-rt", "-print-libgcc-file-name"]).stdout.strip()
            runtime = [builtins if Path(builtins).exists() else libgcc]  # the pin has no 32-bit compiler-rt here: the host's libgcc serves the 64-bit divides
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
                ins, mem, _, prefixed = cache[st["prev"]]
                total["clocks"] += cost(ins, True, True, mem, st["multiplier"], prefixed)
                st["prev"] = None
            return
        info = cache.get(address)
        if info is None:
            raw = bytes(uc.mem_read(address, min(size, 15)))
            ins = Decoder(32, raw, ip=address).decode()
            mem = 0 if ins.mnemonic == Mnemonic.LEA else sum(1 for i in range(ins.op_count) if ins.op_kind(i) in MEMORY)
            rep = ins.has_rep_prefix or ins.has_repe_prefix or ins.has_repne_prefix
            info = cache[address] = (ins, mem, rep, harness.clocks.prefixes(raw))
        ins, mem, rep, prefixed = info
        if rep and uc.reg_read(UC_X86_REG_ECX) == 0:
            return
        prev = st["prev"]
        if prev is not None and cache[prev][0].mnemonic != Mnemonic.NOP:
            pins = cache[prev][0]
            total["clocks"] += cost(pins, address != prev + pins.len, st["firstrep"], cache[prev][1], st["multiplier"], cache[prev][3])
        st["firstrep"] = not (rep and prev == address)
        st["prev"] = address
        if ins.mnemonic == Mnemonic.NOP:
            total["nops"] += 1
            return
        total["ins"] += 1
        total["mem"] += mem
        if mem and ins.mnemonic != Mnemonic.LEA:
            late = misaligned_operands(uc, ins)
            total["misaligned"] += late
            total["clocks"] += MISALIGNED * late
        st["multiplier"] = multiplier_of(uc, ins) if ins.mnemonic in (Mnemonic.MUL, Mnemonic.IMUL) else None

    uc.hook_add(UC_HOOK_CODE, hook)
    uc.emu_start(env_symbols["main"], harness.SENT, count=limit)
    assert total["ins"], f"{variant}: d_faces was never entered"
    covered = struct.unpack(f"<{len(COVERAGE)}I", bytes(uc.mem_read(env_symbols["coverage"], 4 * len(COVERAGE))))
    return dict(prog="dfaces", variant=variant, ins=total["ins"], mem=total["mem"], misaligned=total["misaligned"], nops=total["nops"],
                clocks=total["clocks"], code=harness.code_bytes("dfaces", variant), reports=reports, coverage=dict(zip(COVERAGE, covered)))


def run_m16(level):
    """d_faces.c as llrm builds it for the 16-bit target, as it is written (`far`, `pascal`), linked with env.c built the same way
    and run in icount's real-mode emulator; only what executes in d_faces' own code segment is counted. Its answer is judged
    against the flat reference: the 16-bit build is to do what gcc and clang do in 32 bits, not less."""
    for path in (harness.REPO / "tools" / "dosbatch", harness.REPO / "tools" / "bench", harness.REPO / "crates/target/llrm-x86-m16/bench"):
        if str(path) not in sys.path:
            sys.path.insert(0, str(path))
    import dosbatch
    import icount
    import maps

    work = OUT / "dfaces" / f"m16-{level}"
    work.mkdir(parents=True, exist_ok=True)
    compiled = {}
    for name, source, flag in (("d", D_FACES, f"-{level}"), ("e", HERE / "env.c", "-O1")):
        obj = work / f"{name}.obj"
        sh([dosbatch.BIN / "llrm-cc", *harness.llrmbin.c_args(16), flag, *INCLUDES, "-c", "-o", obj, source], env={**os.environ, **harness.llrmbin.cc_environment()})
        compiled[name] = obj
    exe, listing = work / "d.exe", work / "d.map"
    dosbatch.link_c(compiled["d"], exe, work, listing, after=("file", str(compiled["e"])))
    entry = maps.locate(exe, listing, "bench_dfaces")
    code = [one for one in maps.segments(listing) if one[0] == "D_FACES_TEXT"][0]
    done = icount.run(exe, entry, work, only=(code[1], code[1] + code[2]))
    assert not done.error, done.error
    reports = [int(word) for word in done.output.decode().split()]
    return dict(prog="dfaces", variant=f"m16{level}", ins=done.region.instructions, mem=done.region.memory_operands, misaligned=0,
                clocks=done.region.clocks, code=code[2], reports=reports, coverage={}, functions={})


def agree(reports, reference):
    """Whether a run did what the reference did: the hash of what was drawn exactly, and the sums of the floats handed to the
    rasteriser to a tolerance (a float on a rounding boundary differs between compilers; see env.c)."""
    if len(reports) != len(reference) or reports[:1] != reference[:1]:
        return False
    return all(abs(a - b) <= 2 + 1e-4 * abs(b) for a, b in zip(reports[1:], reference[1:]))


def geomean(values):
    return math.exp(sum(math.log(v) for v in values) / len(values))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--levels", nargs="*", default=list(LEVELS))
    parser.add_argument("--json", type=Path)
    parser.add_argument("--m16", action="store_true", help="llrm's 16-bit build as well, run in the real-mode emulator (misaligned operands are not counted there)")
    parser.add_argument("--functions", action="store_true", help="per function: executed instructions and clocks, llrm built as ELF for its symbols")
    args = parser.parse_args()
    reference = run("gccO0")
    assert len(reference["reports"]) == 6, reference["reports"]
    rows = {"gccO0": reference}
    missing = [name for name, count in reference["coverage"].items() if not count and name not in ("flat",)]
    assert not missing, f"the scene reaches none of: {missing}"
    for level in args.levels:
        for variant in (f"llrm{'' if level == 'O2' else level}", f"gcc{level}", f"clang{level}"):
            rows[variant] = run(variant)
    if args.m16:
        for level in args.levels:
            rows[f"m16{level}"] = run_m16(level)
    if args.functions:
        for level in args.levels:
            rows[f"llrmElf{level}"] = run(f"llrmElf{level}")
    keys = ("ins", "mem", "misaligned", "clocks", "code")
    print("scene reaches: " + ", ".join(f"{name} {count}" for name, count in reference["coverage"].items()))
    print(f"{'variant':10}" + "".join(f"{k:>11}" for k in keys) + "   ok")
    for variant, r in rows.items():
        r["ok"] = agree(r["reports"], reference["reports"])
        print(f"{variant:10}" + "".join(f"{r[k]:>11}" for k in keys) + f"   {'yes' if r['ok'] else 'WRONG'}")
    print()
    for level in args.levels:
        llrm = rows[f"llrm{'' if level == 'O2' else level}"]
        for other in ("gcc", "clang"):
            ref = rows[f"{other}{level}"]
            print(f"-{level} llrm/{other}: " + "  ".join(f"{k} {llrm[k] / max(1, ref[k]):.2f}" for k in keys))
    if args.m16:
        print("\nllrm -m16 against gcc and clang -m32 (the reference the 16-bit target is held to)")
        for level in args.levels:
            small = rows[f"m16{level}"]
            for other in ("gcc", "clang"):
                ref = rows[f"{other}{level}"]
                print(f"-{level} m16/{other}: " + "  ".join(f"{k} {small[k] / max(1, ref[k]):.2f}" for k in ("ins", "mem", "clocks", "code")))
    if args.functions:
        for level in args.levels:
            llrm_elf = rows[f"llrmElf{level}"]
            omf = rows[f"llrm{'' if level == 'O2' else level}"]
            # The same compiler, another object format: the same code, so the same count.
            assert (llrm_elf["ins"], llrm_elf["clocks"]) == (omf["ins"], omf["clocks"]), (level, llrm_elf["ins"], omf["ins"])
            names = sorted({n for v in (f"gcc{level}", f"clang{level}", f"llrmElf{level}") for n in rows[v]["functions"]},
                           key=lambda n: -rows[f"llrmElf{level}"]["functions"].get(n, (0, 0))[1])
            print(f"\n-{level} per function, executed instructions / clocks")
            print(f"{'function':22}{'gcc':>20}{'clang':>20}{'llrm':>20}   llrm/gcc clk")
            for name in names:
                cells = [rows[v]["functions"].get(name) for v in (f"gcc{level}", f"clang{level}", f"llrmElf{level}")]
                text = [f"{c[0]:>9}/{c[1]:<9}" if c else f"{'inlined':>19}" for c in cells]
                ratio = f"{cells[2][1] / cells[0][1]:.2f}" if cells[0] and cells[2] else ""
                print(f"{name:22}" + " ".join(f"{t:>19}" for t in text) + f"   {ratio}")
    if args.json:
        args.json.write_text("".join(json.dumps({k: v for k, v in r.items() if k != "reports"}) + "\n" for r in rows.values()))
    return 0 if all(r["ok"] for r in rows.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
