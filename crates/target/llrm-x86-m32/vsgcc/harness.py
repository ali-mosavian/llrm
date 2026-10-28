"""One emulator for every compiler's m32 object: link, run main, count the kernel's work.

    harness.py PROG VARIANT      VARIANT: llrm (-O2) llrmO1 llrmO3 llrmOs llrmElfO2 llrmElfOs, and gcc and clang each at O1 O2 O3 Os

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

from iced_x86 import Code, CpuidFeature, Decoder, MemorySize, MemorySizeExt, Mnemonic, OpKind, Formatter, FormatterSyntax, FlowControl, Register, RegisterExt
from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, Uc
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EIP

REPO = Path(__file__).resolve().parents[4]
OUT = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))   # build products and results, never in the tree
BENCH = REPO / "bench"
KERNELS = Path(__file__).resolve().parent / "kernels"
sys.path.insert(0, str(Path(__file__).resolve().parent))
import clocks  # noqa: E402


def source_dir(prog):
    """`bench/PROG`, or `kernels/PROG` for the x_ kernels written for this comparison."""
    return KERNELS / prog if (KERNELS / prog).is_dir() else BENCH / prog
sys.path.insert(0, str(REPO / "tools"))
import llrmbin  # noqa: E402

LLRM = llrmbin.bin_dir() / "llrm-cc"   # where cargo put it; build.sh and ctime.py use the same


def cc_env(bits=32):
    """The environment to run `LLRM` in: the caller's, with what llrm-cc needs. LLRM_CLANG_M16 is for 16-bit compiles
    only: a 32-bit one given it refuses its clang as not the pinned one."""
    env = {**os.environ, **llrmbin.cc_environment()}
    if bits != 16:
        env.pop("LLRM_CLANG_M16", None)
    return env


STUB, SENT, STACK_TOP, MEM = 0x8000, 0x7000, 0x7F0000, 0x800000
# main is entered as after a call from code that keeps the System V stack boundary: esp + 4, the caller's, a multiple of 16.
ENTRY_ESP = STACK_TOP - 4
BASE = 0x10000
OMF_VARIANTS = ("llrmO1", "llrm", "llrmO3", "llrmOs")   # "llrm" is -O2
X87 = {CpuidFeature.FPU, CpuidFeature.FPU287, CpuidFeature.FPU287XL_ONLY, CpuidFeature.FPU387, CpuidFeature.FPU387SL_ONLY}
# The rest of the 486's x87 table (Intel 240440-002, least of each range): no compiler is favoured by a price it lacks.
X87_CLOCKS = {
    Mnemonic.FRNDINT: 21, Mnemonic.FSCALE: 30, Mnemonic.FXTRACT: 16, Mnemonic.FPREM: 70, Mnemonic.FPREM1: 72,
    Mnemonic.FSIN: 257, Mnemonic.FCOS: 257, Mnemonic.FSINCOS: 292, Mnemonic.FPTAN: 200, Mnemonic.FPATAN: 218,
    Mnemonic.F2XM1: 140, Mnemonic.FYL2X: 196, Mnemonic.FYL2XP1: 171, Mnemonic.FXAM: 8, Mnemonic.FTST: 4,
    Mnemonic.FLDPI: 4, Mnemonic.FLDL2E: 4, Mnemonic.FLDL2T: 4, Mnemonic.FLDLG2: 4, Mnemonic.FLDLN2: 4,
    Mnemonic.FDECSTP: 3, Mnemonic.FINCSTP: 3, Mnemonic.FFREE: 3, Mnemonic.FNCLEX: 7,
    Mnemonic.FBLD: 70, Mnemonic.FBSTP: 172, Mnemonic.FLDENV: 44, Mnemonic.FNSTENV: 67, Mnemonic.FNSAVE: 143,
    Mnemonic.FRSTOR: 131, Mnemonic.FNOP: 3, Mnemonic.FFREEP: 3, Mnemonic.FSTPNCE: 3, Mnemonic.FNENI: 3,
    Mnemonic.FNDISI: 3, Mnemonic.FNSETPM: 3,
    # Not on a 486 (P6 and later, SSE3): priced as the instruction they replace, which favours neither side.
    Mnemonic.FCMOVB: 4, Mnemonic.FCMOVBE: 4, Mnemonic.FCMOVE: 4, Mnemonic.FCMOVNB: 4, Mnemonic.FCMOVNBE: 4,
    Mnemonic.FCMOVNE: 4, Mnemonic.FCMOVNU: 4, Mnemonic.FCMOVU: 4, Mnemonic.FCOMI: 4, Mnemonic.FCOMIP: 4,
    Mnemonic.FUCOMI: 4, Mnemonic.FUCOMIP: 4, Mnemonic.FISTTP: 29,
    Mnemonic.FNINIT: 17, Mnemonic.FICOM: 15, Mnemonic.FICOMP: 15, Mnemonic.FSTCW: 3, Mnemonic.FNSTCW: 3,
}
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


def memory_address(uc, ins, operand):
    """The address memory operand `operand` of `ins` names, before the instruction runs (flat model: segments are ignored)."""
    kind = ins.op_kind(operand)
    if kind in (OpKind.MEMORY_ESDI, OpKind.MEMORY_ESEDI):
        return uc.reg_read(UC_X86_REG_EDI)
    if kind in (OpKind.MEMORY_SEG_SI, OpKind.MEMORY_SEG_ESI):
        return uc.reg_read(UC_X86_REG_ESI)
    address = ins.memory_displacement
    if ins.memory_base != Register.NONE:
        address += uc.reg_read(UNICORN[ins.memory_base])
    if ins.memory_index != Register.NONE:
        address += uc.reg_read(UNICORN[ins.memory_index]) * ins.memory_index_scale
    return address & 0xFFFFFFFF


MISALIGNED = 3


def misaligned_operands(uc, ins):
    """How many of the instruction's memory operands the 486 reads or writes misaligned, each costing MISALIGNED clocks: Intel486
    data sheet 240440-002, section 10.1, Instruction Clock Count Assumptions, 2: "Accesses are aligned. Add three clocks to each
    misaligned access." Misaligned is Table 2.1, Data Type Alignment Requirements: a word at 2, a dword or single real at 4,
    a double or extended real at 8."""
    count = 0
    for operand in range(ins.op_count):
        if ins.op_kind(operand) in MEMORY:
            size = MemorySizeExt.size(ins.memory_size)
            if size > 1 and memory_address(uc, ins, operand) % (2 if size == 2 else 4 if size <= 4 else 8):
                count += 1
    return count


SHIFTS = (Mnemonic.SHL, Mnemonic.SHR, Mnemonic.SAR, Mnemonic.ROL, Mnemonic.ROR)
ALU = (Mnemonic.ADD, Mnemonic.SUB, Mnemonic.AND, Mnemonic.OR, Mnemonic.XOR, Mnemonic.ADC, Mnemonic.SBB)
SETS = (Mnemonic.SETA, Mnemonic.SETAE, Mnemonic.SETB, Mnemonic.SETBE, Mnemonic.SETE, Mnemonic.SETNE, Mnemonic.SETG, Mnemonic.SETGE,
        Mnemonic.SETL, Mnemonic.SETLE, Mnemonic.SETS, Mnemonic.SETNS)
IMMEDIATES = (OpKind.IMMEDIATE8, OpKind.IMMEDIATE8TO32, OpKind.IMMEDIATE32, OpKind.IMMEDIATE16, OpKind.IMMEDIATE8TO16,
              OpKind.IMMEDIATE8TO64, OpKind.IMMEDIATE32TO64)
ONE_SHIFTS = {getattr(Code, f"{m}_RM{w}_1") for m in ("SHL", "SHR", "SAR", "ROL", "ROR") for w in (8, 16, 32)}


def _operand_bytes(ins):
    """Width of the instruction's r/m operand."""
    if ins.op_count and ins.op_kind(0) == OpKind.REGISTER:
        return {1: 1, 2: 2, 4: 4}.get(RegisterExt.size(ins.op_register(0)), 4)
    return max(1, MemorySizeExt.size(ins.memory_size))


def cost(ins, taken, first_rep, mem, multiplier=None, prefixes=0):
    """The 486's clocks of `ins`, each from the compiler's own `timings.times` (clocks.py), and `prefixes` operand- and address-size
    prefixes (clocks.prefixes of its bytes) at the table's clock each. A branch is `taken` or not; a string instruction is the
    `first_rep` of its repeat or one more; `multiplier` is the operand a MUL reads."""
    return _form_clocks(ins, taken, first_rep, multiplier) + prefixes * clocks.PREFIX


def _form_clocks(ins, taken, first_rep, multiplier):
    m = ins.mnemonic
    n = ins.op_count
    k0 = ins.op_kind(0) if n else None
    k1 = ins.op_kind(1) if n > 1 else None
    dst_mem = k0 in MEMORY
    src_mem = k1 in MEMORY
    imm = k1 in IMMEDIATES
    if m == Mnemonic.MOV:
        if k0 == OpKind.REGISTER and RegisterExt.is_segment_register(ins.op_register(0)):
            return clocks.clocks("mov_seg_r")
        return clocks.clocks("mov_mr" if dst_mem else "mov_rm" if src_mem else "mov_ri" if imm else "mov_rr")
    if m == Mnemonic.LEA:
        return clocks.clocks("lea")
    if m in (Mnemonic.MOVZX, Mnemonic.MOVSX):
        return clocks.clocks("movzx")
    if m in ALU:
        return clocks.clocks("alu_mr" if dst_mem else "alu_rm" if src_mem else "alu_ri" if imm else "alu_rr")
    if m in (Mnemonic.CMP, Mnemonic.TEST):
        return clocks.clocks("alu_rm" if dst_mem or src_mem else "alu_ri" if imm else "alu_rr")
    if m in (Mnemonic.INC, Mnemonic.DEC, Mnemonic.NEG, Mnemonic.NOT):
        return clocks.clocks("alu_mr" if dst_mem else "alu_rr")
    if m == Mnemonic.PUSH:
        return clocks.clocks("push_m" if dst_mem else "push_i" if k0 in IMMEDIATES else "push_r")
    if m == Mnemonic.POP:
        if k0 == OpKind.REGISTER and RegisterExt.is_segment_register(ins.op_register(0)):
            return clocks.clocks("pop_seg")
        return clocks.clocks("pop_m" if dst_mem else "pop_r")
    if m in SHIFTS:
        if k1 is not None and k1 == OpKind.REGISTER:
            return clocks.UNTABLED["shift_cl_mem" if dst_mem else "shift_cl"]
        if dst_mem:
            return clocks.UNTABLED["shift_imm_mem"]
        return clocks.clocks("shift_r1" if ins.code in ONE_SHIFTS else "shift_ri")
    if m in (Mnemonic.IMUL, Mnemonic.MUL):
        # Data dependent: 13 to 42 by the multiplier; 13 was the least, which made every product cheap.
        return multiply_clocks(multiplier if multiplier is not None else 0, m == Mnemonic.IMUL)
    if m in (Mnemonic.DIV, Mnemonic.IDIV):
        width = _operand_bytes(ins)
        signed = m == Mnemonic.IDIV
        if width == 1:
            return clocks.UNTABLED["idiv_b" if signed else "div_b"]
        return clocks.audited(f"{'sdiv' if signed else 'udiv'}_w{2 if width == 2 else 4}")
    if m in (Mnemonic.CDQ, Mnemonic.CWD, Mnemonic.CWDE, Mnemonic.CBW):
        return clocks.clocks("cdq")
    if m == Mnemonic.XCHG:
        return clocks.clocks("xchg_rr")
    if m in SETS:
        return clocks.clocks("setcc")
    if m == Mnemonic.CALL:
        return clocks.clocks("call_far", far=True) if k0 in (OpKind.FAR_BRANCH16, OpKind.FAR_BRANCH32) else clocks.clocks("call_near")
    if m == Mnemonic.RET:
        return clocks.clocks("ret_pop") if n else clocks.clocks("ret_near")
    if m == Mnemonic.RETF:
        return clocks.clocks("ret_pop", far=True) if n else clocks.clocks("ret_far", far=True)
    if m == Mnemonic.ENTER:
        return clocks.UNTABLED["enter"]
    if m == Mnemonic.LEAVE:
        return clocks.clocks("leave")
    if m == Mnemonic.JMP:
        return clocks.clocks("jmp_short" if k0 in (OpKind.NEAR_BRANCH16, OpKind.NEAR_BRANCH32, OpKind.NEAR_BRANCH64) else "jmp_indirect")
    if ins.flow_control == FlowControl.CONDITIONAL_BRANCH:
        return clocks.clocks("jcc") if taken else clocks.clocks("jcc_not_taken")
    if m in (Mnemonic.STOSB, Mnemonic.STOSW, Mnemonic.STOSD):
        return clocks.clocks("rep_stos_cell") + (clocks.clocks("rep_stos") if first_rep else 0)
    if m in (Mnemonic.MOVSB, Mnemonic.MOVSW, Mnemonic.MOVSD):
        return clocks.clocks("rep_movs_cell") + (clocks.clocks("rep_movs") if first_rep else 0)
    if m in (Mnemonic.CMPSB, Mnemonic.CMPSW, Mnemonic.CMPSD):
        return clocks.UNTABLED["cmps"] + (clocks.UNTABLED["cmps_first"] if first_rep else 0)
    if m in (Mnemonic.SCASB, Mnemonic.SCASW, Mnemonic.SCASD):
        return clocks.UNTABLED["scas"] + (clocks.UNTABLED["scas_first"] if first_rep else 0)
    if m in (Mnemonic.LODSB, Mnemonic.LODSW, Mnemonic.LODSD):
        return clocks.UNTABLED["loads"]
    if m in (Mnemonic.FLD, Mnemonic.FLD1, Mnemonic.FLDZ):
        return clocks.clocks("x87_load") if m == Mnemonic.FLD and dst_mem else clocks.UNTABLED["fld_reg"]
    if m in (Mnemonic.FST, Mnemonic.FSTP):
        return clocks.clocks("x87_store") if dst_mem else clocks.UNTABLED["fst_reg"]
    if m in (Mnemonic.FADD, Mnemonic.FSUB, Mnemonic.FSUBR, Mnemonic.FADDP, Mnemonic.FSUBP, Mnemonic.FSUBRP):
        return clocks.clocks("x87_add_m" if dst_mem else "x87_add")
    if m in (Mnemonic.FMUL, Mnemonic.FMULP):
        return clocks.clocks("x87_mul_m" if dst_mem else "x87_mul")
    if m in (Mnemonic.FDIV, Mnemonic.FDIVP, Mnemonic.FDIVR, Mnemonic.FDIVRP):
        return clocks.clocks("x87_div_m" if dst_mem else "x87_div")
    if m == Mnemonic.FSQRT:
        return clocks.UNTABLED["fsqrt"]
    if m == Mnemonic.FXCH:
        return clocks.clocks("x87_exchange")
    if m == Mnemonic.FILD:
        return clocks.UNTABLED["fild"]
    if m in (Mnemonic.FIST, Mnemonic.FISTP):
        return clocks.UNTABLED["fist"]
    if m in (Mnemonic.FCOM, Mnemonic.FCOMP, Mnemonic.FUCOM, Mnemonic.FUCOMP, Mnemonic.FCOMPP, Mnemonic.FUCOMPP):
        return clocks.UNTABLED["fcom"]
    if m in (Mnemonic.FCHS, Mnemonic.FABS):
        return clocks.UNTABLED["fchs" if m == Mnemonic.FCHS else "fabs"]
    if m == Mnemonic.FNSTSW:
        return clocks.UNTABLED["fnstsw"]
    if m in (Mnemonic.FNSTCW, Mnemonic.FLDCW):
        return clocks.clocks("x87_control_store") if m == Mnemonic.FNSTCW else clocks.clocks("x87_control_load")
    # Integer memory operands (Intel 240440-002, the 486 table; the least of each range, which does not
    # favour the compiler that uses them): m32int / m16int. They were unpriced and fell through to 1 clock,
    # which made gcc's `fimul` (x_matmul_f) free.
    if m in (Mnemonic.FIADD, Mnemonic.FISUB, Mnemonic.FISUBR):
        return clocks.UNTABLED["fiadd32" if ins.memory_size == MemorySize.INT32 else "fiadd16"]
    if m == Mnemonic.FIMUL:
        return clocks.UNTABLED["fimul32" if ins.memory_size == MemorySize.INT32 else "fimul16"]
    if m in (Mnemonic.FIDIV, Mnemonic.FIDIVR):
        return clocks.UNTABLED["fidiv32" if ins.memory_size == MemorySize.INT32 else "fidiv16"]
    if m in X87_CLOCKS:
        return X87_CLOCKS[m]
    if m == Mnemonic.SAHF:
        return clocks.clocks("sahf")
    if X87.intersection(ins.cpuid_features()):
        raise ValueError(f"the model has no price for the x87 instruction {Formatter(FormatterSyntax.INTEL).format(ins)}")
    return 1


def built_abi():
    """The convention build.sh compiled llrm's side in, as it recorded it: a work directory of an earlier build says so too."""
    record = OUT / "abi"
    return record.read_text().strip() if record.exists() else "watcom"


def stub_digest():
    """What stub.s says now: a stub is current when the one it was built from said the same."""
    import hashlib
    return hashlib.sha256(Path(__file__).with_name("stub.s").read_bytes()).hexdigest()


def record_stub(work):
    """Say in `work` which stub.s its stub.elf was built from; run.sh and the tests that build one do."""
    (Path(work) / "stub.elf.src").write_text(stub_digest())


def fresh_stub():
    """The stub the harness links is built from stub.s: one built from an older stub.s lacks its symbols (`report_` after #743).
    A file's age says nothing of it (a checkout makes stub.s newer than any build): the digest it was built from does."""
    built, record = OUT / "stub.elf", OUT / "stub.elf.src"
    assert record.exists() and record.read_text() == stub_digest(), f"{built} was not built from this stub.s: run.sh builds it"
    return built


def run(prog, variant, hot=False, limit=300_000_000):
    stub = nm(fresh_stub())
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
    esp = ENTRY_ESP
    uc.mem_write(esp, struct.pack("<III", SENT, 0, 0))
    uc.reg_write(UC_X86_REG_ESP, esp)
    code_end = max(va + len(b) for va, b, _ in segs)
    # OMF's default-convention name is `report_`, ELF's is `report`: both are the stub's, and the caller says where the value is.
    report_at = {stub["report"], stub.get("report_", stub["report"])}
    # llrm's watcom convention takes the value in EAX; gcc and clang, and llrm under -mabi=sysv, push it.
    in_eax = variant.startswith("llrm") and built_abi() == "watcom"
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
            raw = bytes(uc.mem_read(address, min(size, 15)))
            ins = Decoder(32, raw, ip=address).decode()
            mem = 0 if ins.mnemonic == Mnemonic.LEA else sum(1 for i in range(ins.op_count) if ins.op_kind(i) in MEMORY)
            rep = ins.has_rep_prefix or ins.has_repe_prefix or ins.has_repne_prefix
            info = cache[address] = (ins, mem, rep, ins.mnemonic == Mnemonic.RET, clocks.prefixes(raw))
        ins, mem, rep, ret, prefixed = info
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
            total["clocks"] += cost(pins[0], taken, st.get("firstrep", True), pins[1], st.get("multiplier"), pins[4])
        st["firstrep"] = not (rep and prev == address)
        if ins.mnemonic == Mnemonic.NOP:   # alignment padding (clang): counted apart, not work
            total["nops"] += 1
            st["prev"] = address
            hits[address] += 1
            return
        total["ins"] += 1
        total["mem"] += mem
        if mem and ins.mnemonic != Mnemonic.LEA:
            late = misaligned_operands(uc, ins)
            total["misaligned"] += late
            total["clocks"] += MISALIGNED * late
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
    return dict(prog=prog, variant=variant, ins=total["ins"], mem=total["mem"], misaligned=total["misaligned"], nops=total["nops"], clocks=total["clocks"], reports=reports,
                hits=hits, edges=edges, kern=kern, main=main, code_end=code_end, uc=uc, cache=cache, syms=syms)


def code_bytes(prog, variant):
    """Size of the kernel's code: the CODE-class segments (OMF) or .text (ELF) of its object, like tools/sizes.py, and for
    gcc and clang the runtime members they pulled in, so that a divide they send to libgcc is counted and not left out."""
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
    return sum(int(l.split()[1]) for l in out.splitlines() if l.startswith(".text")) + archive_code(prog, variant)


def archive_code(prog, variant):
    """The code of the archive members the link pulled in (libgcc's i64 divide), from the linker's map: runtime the program runs,
    and no padding between sections."""
    import re
    path = OUT / "b" / f"{prog}.{variant}.map"
    if not path.is_file():
        return 0
    total = 0
    for match in re.finditer(r"^ (\.text\S*)\s*\n?\s+0x[0-9a-f]+\s+(0x[0-9a-f]+)\s+(\S+\.a\([^)]*\))", path.read_text(), re.M):
        total += int(match.group(2), 16)
    return total


def main():
    prog, variant = sys.argv[1], sys.argv[2]
    r = run(prog, variant)
    expect = [int(x) for x in (source_dir(prog) / f"{prog}.out").read_text().split()]
    print(json.dumps(dict(prog=prog, variant=variant, ins=r["ins"], mem=r["mem"], misaligned=r["misaligned"], nops=r["nops"], clocks=r["clocks"], code=code_bytes(prog, variant), ok=r["reports"] == expect, got=r["reports"][:4])))


if __name__ == "__main__":
    main()
