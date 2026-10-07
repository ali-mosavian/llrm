"""One emulator for every compiler's m32 object: link, run main, count the kernel's work.

    harness.py PROG VARIANT      VARIANT: llrm llrmOs llrmElfO2 llrmElfOs gccO2 gccOs clangO2 clangOs

llrm's OMF is linked here (segments by class, fixups resolved); ELF, llrm's (llrmElf*) and gcc/clang's, is linked by ld.
Both resolve report/memset/memcpy to stub.elf, loaded at STUB. The region counted is bench_PROG
from its first instruction to the return that pops its own entry frame, callees included
(crates/target/llrm-x86-m16/bench/icount.py's convention: a rep string instruction counts once per iteration).
"""
import json
import os
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

from iced_x86 import Decoder, Mnemonic, OpKind, Formatter, FormatterSyntax, FlowControl, Register, RegisterExt
from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, Uc
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EIP

REPO = Path(__file__).resolve().parents[4]
OUT = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))   # build products and results, never in the tree
BENCH = REPO / "bench"
sys.path.insert(0, str(REPO / "tools"))
import llrmbin  # noqa: E402

LLRM = llrmbin.bin_dir() / "llrm-c"   # where cargo put it; build.sh and ctime.py use the same
STUB, SENT, STACK_TOP, MEM = 0x8000, 0x7000, 0x7F0000, 0x800000
BASE = 0x10000
OMF_VARIANTS = ("llrm", "llrmOs")
MEMORY = {OpKind.MEMORY, OpKind.MEMORY_SEG_SI, OpKind.MEMORY_SEG_ESI, OpKind.MEMORY_ESDI, OpKind.MEMORY_ESEDI}


def nm(path):
    out = subprocess.run(["nm", "-g", "--defined-only", str(path)], capture_output=True, text=True, check=True).stdout
    return {line.split()[2]: int(line.split()[0], 16) for line in out.splitlines() if len(line.split()) == 3}


def elf_segments(path):
    data = Path(path).read_bytes()
    (_, _, _, _, entry, phoff, _, _, _, phentsize, phnum, *_rest) = struct.unpack("<16sHHIIIIIHHHHHH", data[:52])
    segs = []
    for i in range(phnum):
        t, off, va, _, fsz, msz, *_ = struct.unpack("<IIIIIIII", data[phoff + i * phentsize: phoff + i * phentsize + 32])
        if t == 1:
            segs.append((va, data[off: off + fsz], msz))
    return segs


def omf_link(path, stub):
    """Flat image of one llrm OMF object. Returns (segments, symbols)."""
    data = Path(path).read_bytes()
    lnames, segdefs, extdefs, pubs, fixups, ledata = [""], [], [], [], [], []
    at, cur = 0, None
    while at < len(data):
        kind, size = data[at], int.from_bytes(data[at + 1: at + 3], "little")
        body = data[at + 3: at + 2 + size]
        at += 3 + size
        if kind == 0x96:
            i = 0
            while i < len(body):
                lnames.append(body[i + 1: i + 1 + body[i]].decode("latin-1")); i += 1 + body[i]
        elif kind in (0x98, 0x99):
            attrs = body[0]; i = 1
            assert attrs >> 5 != 0 or False, "absolute segment"
            length = int.from_bytes(body[i: i + 4], "little"); i += 4
            name, i = body[i], i + 1
            klass = body[i]
            segdefs.append({"name": lnames[name], "class": lnames[klass], "len": length, "align": (attrs >> 5) & 7, "data": bytearray(length)})
        elif kind == 0x8C:
            i = 0
            while i < len(body):
                n = body[i]; extdefs.append(body[i + 1: i + 1 + n].decode()); i += 2 + n  # skip type index
        elif kind == 0x91:
            i = 0
            grp, i = body[i], i + 1
            seg, i = body[i], i + 1
            assert seg
            while i < len(body):
                n = body[i]; name = body[i + 1: i + 1 + n].decode()
                off = int.from_bytes(body[i + 1 + n: i + 5 + n], "little")
                pubs.append((seg - 1, off, name)); i += 6 + n
        elif kind == 0xA1:
            seg, off = body[0], int.from_bytes(body[1:5], "little")
            segdefs[seg - 1]["data"][off: off + len(body) - 5] = body[5:]
            cur = (seg - 1, off)
        elif kind == 0x9D:
            i = 0
            while i < len(body):
                b = body[i]
                if not b & 0x80:                       # THREAD
                    i += 2 if (b & 0x40) == 0 or True else 0
                    if (b & 0x1C) >> 2 < 3 and not (b & 0x40) or ((b >> 2) & 7) < 3 and (b & 0x40):
                        i += 1
                    # the llrm writer uses no threads (checked by the assert below)
                    raise AssertionError("thread fixup")
                locat = (b << 8) | body[i + 1]; i += 2
                fd = body[i]; i += 1
                mode, ltype, doff = (locat >> 14) & 1, (locat >> 10) & 15, locat & 0x3FF
                f, frame, t, p, targt = fd >> 7, (fd >> 4) & 7, (fd >> 3) & 1, (fd >> 2) & 1, fd & 3
                if not f and frame < 3:
                    i += 1
                tidx = body[i]; i += 1
                disp = 0
                if not p:
                    disp = int.from_bytes(body[i: i + 4], "little", signed=True); i += 4
                fixups.append((cur[0], cur[1] + doff, mode, ltype, targt, tidx, disp))
    # lay out: code classes, then the rest, 16-aligned per segment
    order = sorted(range(len(segdefs)), key=lambda k: (not segdefs[k]["class"].endswith("CODE"), k))
    base, addr = BASE, {}
    for k in order:
        al = max(16, 1 << (segdefs[k]["align"] and 2))
        base = (base + 15) & ~15
        addr[k] = base; base += segdefs[k]["len"]
    syms = {n: addr[s] + o for s, o, n in pubs}
    for s, off, mode, ltype, targt, tidx, disp in fixups:
        assert ltype in (9, 13), f"fixup location {ltype}"
        if targt == 0:
            tgt = addr[tidx - 1]
        elif targt == 2:
            n = extdefs[tidx - 1]
            tgt = stub.get(n) or stub.get(n.lstrip("_")) or syms.get(n)
            assert tgt is not None, f"unresolved {n}"
        else:
            raise AssertionError(f"target method {targt}")
        tgt += disp
        place = addr[s] + off
        add = int.from_bytes(segdefs[s]["data"][off: off + 4], "little", signed=True)
        val = tgt + add if mode else tgt + add - (place + 4)
        segdefs[s]["data"][off: off + 4] = (val & 0xFFFFFFFF).to_bytes(4, "little")
    segs = [(addr[k], bytes(segdefs[k]["data"]), segdefs[k]["len"]) for k in range(len(segdefs))]
    return segs, syms


UNICORN = {Register.EAX: UC_X86_REG_EAX, Register.EBX: UC_X86_REG_EBX, Register.ECX: UC_X86_REG_ECX, Register.EDX: UC_X86_REG_EDX,
           Register.EBP: UC_X86_REG_EBP, Register.ESI: UC_X86_REG_ESI, Register.EDI: UC_X86_REG_EDI, Register.ESP: UC_X86_REG_ESP}


def multiply_clocks(m, signed):
    """The i486's MUL and IMUL clocks for multiplier `m` (Intel 240440-002, Table 10.1 note 3): 10 + max(log2|m|, n),
    n = 3 for +m and 5 for -m, 13 for m = 0. The bits of |m| stand for log2|m|, rounded up."""
    if signed and m >= 1 << 31:
        m -= 1 << 32
    return 10 + max(abs(m).bit_length(), 5 if m < 0 else 3)


def multiplier_of(uc, ins):
    """The operand the 486's early-out reads: the immediate of the three-operand form, else the source (`MUL r/m`,
    `IMUL r, r/m`). The data sheet names it 'Multiplier' beside the register or memory operand, not the accumulator."""
    last = ins.op_count - 1
    kind = ins.op_kind(last)
    if kind in (OpKind.IMMEDIATE8, OpKind.IMMEDIATE8TO32, OpKind.IMMEDIATE32, OpKind.IMMEDIATE16, OpKind.IMMEDIATE8TO16):
        return ins.immediate(last) & 0xFFFFFFFF
    if kind == OpKind.REGISTER:
        reg = ins.op_register(last)
        value = uc.reg_read(UNICORN[RegisterExt.full_register32(reg)])
        return value & (0xFFFFFFFF if RegisterExt.is_gpr32(reg) else 0xFFFF if RegisterExt.is_gpr16(reg) else 0xFF)
    if kind in MEMORY:
        base = ins.memory_base
        address = ins.memory_displacement
        if base != Register.NONE:
            address += uc.reg_read(UNICORN[base])
        if ins.memory_index != Register.NONE:
            address += uc.reg_read(UNICORN[ins.memory_index]) * ins.memory_index_scale
        return int.from_bytes(bytes(uc.mem_read(address & 0xFFFFFFFF, 4)), "little")
    return 0


def cost(ins, taken, first_rep, mem, multiplier=None):
    m = ins.mnemonic
    n = ins.op_count
    k0 = ins.op_kind(0) if n else None
    k1 = ins.op_kind(1) if n > 1 else None
    dst_mem = k0 in MEMORY
    src_mem = k1 in MEMORY
    if m in (Mnemonic.MOV,):
        return 1
    if m == Mnemonic.LEA:
        return 1
    if m in (Mnemonic.MOVZX, Mnemonic.MOVSX):
        return 3
    if m in (Mnemonic.ADD, Mnemonic.SUB, Mnemonic.AND, Mnemonic.OR, Mnemonic.XOR, Mnemonic.ADC, Mnemonic.SBB):
        return 3 if dst_mem else (2 if src_mem else 1)
    if m in (Mnemonic.CMP, Mnemonic.TEST):
        return 2 if (dst_mem or src_mem) else 1
    if m in (Mnemonic.INC, Mnemonic.DEC, Mnemonic.NEG, Mnemonic.NOT):
        return 3 if dst_mem else 1
    if m == Mnemonic.PUSH:
        return 4 if dst_mem else 1
    if m == Mnemonic.POP:
        return 6 if dst_mem else 1
    if m in (Mnemonic.SHL, Mnemonic.SHR, Mnemonic.SAR, Mnemonic.ROL, Mnemonic.ROR):
        return (4 if dst_mem else 2) if k1 != OpKind.REGISTER else (5 if dst_mem else 3)
    if m in (Mnemonic.IMUL, Mnemonic.MUL):
        # Data dependent: 13 to 42 by the multiplier; 13 was the least, which made every product cheap.
        return multiply_clocks(multiplier if multiplier is not None else 0, m == Mnemonic.IMUL)
    if m == Mnemonic.DIV:
        return 40
    if m == Mnemonic.IDIV:
        return 43
    if m in (Mnemonic.CDQ, Mnemonic.CWD, Mnemonic.CWDE, Mnemonic.CBW, Mnemonic.XCHG, Mnemonic.SETA, Mnemonic.SETAE, Mnemonic.SETB, Mnemonic.SETBE, Mnemonic.SETE, Mnemonic.SETNE, Mnemonic.SETG, Mnemonic.SETGE, Mnemonic.SETL, Mnemonic.SETLE, Mnemonic.SETS, Mnemonic.SETNS):
        return 3 if m in (Mnemonic.CDQ, Mnemonic.CWD, Mnemonic.CWDE, Mnemonic.CBW) else (3 if m == Mnemonic.XCHG else 4 if dst_mem else 3)
    if m == Mnemonic.CALL:
        return 3
    if m == Mnemonic.RET:
        return 5
    if m == Mnemonic.LEAVE:
        return 5
    if m == Mnemonic.JMP:
        return 3
    if ins.flow_control == FlowControl.CONDITIONAL_BRANCH:
        return 3 if taken else 1
    if m in (Mnemonic.STOSB, Mnemonic.STOSW, Mnemonic.STOSD):
        return 1 + (7 if first_rep else 0)
    if m in (Mnemonic.MOVSB, Mnemonic.MOVSW, Mnemonic.MOVSD):
        return 3 + (12 if first_rep else 0)
    if m in (Mnemonic.CMPSB, Mnemonic.CMPSW, Mnemonic.CMPSD):
        return 7 + (5 if first_rep else 0)
    if m in (Mnemonic.SCASB, Mnemonic.SCASW, Mnemonic.SCASD):
        return 6 + (5 if first_rep else 0)
    if m in (Mnemonic.LODSB, Mnemonic.LODSW, Mnemonic.LODSD):
        return 2
    if m in (Mnemonic.FLD, Mnemonic.FLD1, Mnemonic.FLDZ):
        return 4 if m != Mnemonic.FLD else (3 if dst_mem else 4)
    if m in (Mnemonic.FST, Mnemonic.FSTP):
        return 7 if dst_mem else 3
    if m in (Mnemonic.FADD, Mnemonic.FSUB, Mnemonic.FSUBR, Mnemonic.FADDP, Mnemonic.FSUBP, Mnemonic.FSUBRP):
        return 10
    if m in (Mnemonic.FMUL, Mnemonic.FMULP):
        return 16
    if m in (Mnemonic.FDIV, Mnemonic.FDIVP, Mnemonic.FDIVR, Mnemonic.FDIVRP):
        return 73
    if m == Mnemonic.FSQRT:
        return 83
    if m == Mnemonic.FXCH:
        return 4
    if m in (Mnemonic.FILD,):
        return 13
    if m in (Mnemonic.FIST, Mnemonic.FISTP):
        return 29
    if m in (Mnemonic.FCOM, Mnemonic.FCOMP, Mnemonic.FUCOM, Mnemonic.FUCOMP, Mnemonic.FCOMPP, Mnemonic.FUCOMPP):
        return 4
    if m in (Mnemonic.FCHS, Mnemonic.FABS):
        return 6 if m == Mnemonic.FCHS else 3
    if m in (Mnemonic.FNSTSW, Mnemonic.FNSTCW, Mnemonic.FLDCW):
        return 3 if m == Mnemonic.FNSTSW else 4
    if m == Mnemonic.SAHF:
        return 2
    return 1


def run(prog, variant, hot=False, limit=300_000_000):
    stub = nm(OUT / "stub.elf")
    if variant in OMF_VARIANTS:
        segs, syms = omf_link(OUT / "o" / f"{prog}.{variant}.obj", stub)
        main = syms["_main"]
        # The default convention's name is `bench_x_`, cdecl's `_bench_x`.
        kern = syms.get(f"bench_{prog}_") or syms[f"_bench_{prog}"]
    else:
        elf = OUT / "b" / f"{prog}.{variant}.elf"
        segs, syms = elf_segments(elf), nm(elf)
        # llrm's ELF symbols are decorated with a leading underscore until the ABI says otherwise
        main, kern = syms.get("main", syms.get("_main")), syms.get(f"bench_{prog}", syms.get(f"_bench_{prog}"))
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(0, MEM)
    for va, blob, msz in segs + elf_segments(OUT / "stub.elf"):
        uc.mem_write(va, blob)
    uc.mem_write(SENT, b"\xf4")
    esp = STACK_TOP - 8
    uc.mem_write(esp, struct.pack("<III", SENT, 0, 0))
    uc.reg_write(UC_X86_REG_ESP, esp)
    code_end = max(va + len(b) for va, b, _ in segs)
    # OMF's default-convention name is `report_`, ELF's is `report`: both are the stub's, and the caller says where the value is.
    report_at = {stub["report"], stub.get("report_", stub["report"])}
    # llrm's default convention takes the value in EAX; gcc and clang push it.
    in_eax = variant.startswith("llrm")
    st = dict(active=False, sp=0, prev=None, prev_ins=None, cnt=0)
    total = Counter()   # instructions, mem, clocks
    hits = Counter()
    edges = Counter()   # (from, to) taken edges in region
    cache = {}
    reports = []
    calls = Counter()
    shadow = []

    def hook(uc, address, size, _):
        if address == SENT:
            uc.emu_stop(); return
        if address in report_at:
            if in_eax:
                reports.append(struct.unpack("<i", struct.pack("<I", uc.reg_read(UC_X86_REG_EAX)))[0])
            else:
                reports.append(struct.unpack("<i", uc.mem_read(uc.reg_read(UC_X86_REG_ESP) + 4, 4))[0])
        if not st["active"]:
            if address != kern:
                return
            st["active"] = True
            st["sp"] = uc.reg_read(UC_X86_REG_ESP)
            st["prev"] = None
        info = cache.get(address)
        if info is None:
            ins = Decoder(32, bytes(uc.mem_read(address, min(size, 15))), ip=address).decode()
            mem = 0 if ins.mnemonic == Mnemonic.LEA else sum(1 for i in range(ins.op_count) if ins.op_kind(i) in MEMORY)
            rep = ins.has_rep_prefix or ins.has_repe_prefix or ins.has_repne_prefix
            info = cache[address] = (ins, mem, rep, ins.mnemonic == Mnemonic.RET)
        ins, mem, rep, ret = info
        if rep and uc.reg_read(UC_X86_REG_ECX) == 0:
            return
        prev = st["prev"]
        if prev is not None:
            pins = cache[prev]
            # previous instruction's transfer: taken if we are not at its fall-through
            taken = address != prev + pins[0].len
            pm = pins[0].mnemonic
            if pm == Mnemonic.CALL:
                shadow.append(prev)
                edges[(prev, prev + pins[0].len)] += 1
            elif pm == Mnemonic.RET:
                if shadow: shadow.pop()
            else:
                edges[(prev, address)] += 1
        else:
            taken = False
        # clocks of the previous instruction need to know taken; charge it now
        if prev is not None and pins[0].mnemonic != Mnemonic.NOP:
            total["clocks"] += cost(pins[0], taken, st.get("firstrep", True), pins[1], st.get("multiplier"))
        st["firstrep"] = not (rep and prev == address)
        if ins.mnemonic == Mnemonic.NOP:   # alignment padding (clang): counted apart, not work
            total["nops"] += 1
            st["prev"] = address
            hits[address] += 1
            return
        total["ins"] += 1
        total["mem"] += mem
        hits[address] += 1
        st["prev"] = address
        # Read before it runs: the 486's multiply costs what its multiplier is.
        st["multiplier"] = multiplier_of(uc, ins) if ins.mnemonic in (Mnemonic.MUL, Mnemonic.IMUL) else None
        if ret and uc.reg_read(UC_X86_REG_ESP) == st["sp"]:
            total["clocks"] += cost(ins, True, True, mem)
            st["active"] = False

    uc.hook_add(UC_HOOK_CODE, hook)
    uc.emu_start(main, SENT, count=limit)
    assert total["ins"], f"{prog} {variant}: the kernel was never entered"
    eax = uc.reg_read(UC_X86_REG_EAX)
    code = b"".join(bytes(uc.mem_read(a, 1)) for a in ())
    return dict(prog=prog, variant=variant, ins=total["ins"], mem=total["mem"], nops=total["nops"], clocks=total["clocks"], reports=reports,
                hits=hits, edges=edges, kern=kern, main=main, code_end=code_end, uc=uc, cache=cache, syms=syms)


def code_bytes(prog, variant):
    """Size of the kernel's object code: the CODE-class segments (OMF) or .text (ELF), like tools/sizes.py."""
    if variant in OMF_VARIANTS:
        data = (OUT / "o" / f"{prog}.{variant}.obj").read_bytes()
        lnames, classes, sizes = [""], [], []
        at = 0
        while at < len(data):
            kind, size = data[at], int.from_bytes(data[at + 1: at + 3], "little")
            body = data[at + 3: at + 2 + size]; at += 3 + size
            if kind == 0x96:
                i = 0
                while i < len(body):
                    lnames.append(body[i + 1: i + 1 + body[i]].decode("latin-1")); i += 1 + body[i]
            elif kind == 0x99:
                sizes.append(int.from_bytes(body[1:5], "little")); classes.append(lnames[body[6]])
        return sum(s for s, c in zip(sizes, classes) if c.endswith("CODE"))
    out = subprocess.run(["size", "-A", str(OUT / "b" / f"{prog}.{variant}.o")], capture_output=True, text=True).stdout
    return sum(int(l.split()[1]) for l in out.splitlines() if l.startswith(".text"))


def main():
    prog, variant = sys.argv[1], sys.argv[2]
    r = run(prog, variant)
    expect = [int(x) for x in (BENCH / prog / f"{prog}.out").read_text().split()]
    print(json.dumps(dict(prog=prog, variant=variant, ins=r["ins"], mem=r["mem"], nops=r["nops"], clocks=r["clocks"], code=code_bytes(prog, variant), ok=r["reports"] == expect, got=r["reports"][:4])))


if __name__ == "__main__":
    main()
