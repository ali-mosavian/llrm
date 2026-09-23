"""Write src/backend/select_sweep.rs: Python's select.emit over a broad set of shapes.

The Rust port must encode every one of these to the same bytes and fields.
Run: .venv/bin/python tools/select_sweep.py
"""

import itertools
import sys
from pathlib import Path

from iced_x86 import Register

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))  # this checkout's qbopt, not the one the venv installed

from qbopt.model import ir
from qbopt.backend import select
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

NAMES = {value: name for name, value in vars(Register).items() if name.isupper() and isinstance(value, int)}
OUT = ROOT / "src/backend/select_sweep.rs"


def rust_name(name: str) -> str:
    return "None" if name == "NONE" else name


def reg(one: int) -> str:
    return f"R::{rust_name(NAMES[one])}"


def addr(one: Addr | None) -> str:
    if one is None:
        return "None"
    space = one.space.name.capitalize()
    return f"Some(a(Space::{space}, {one.disp}, {one.index}, {reg(one.base)}, {reg(one.segment)}))"


def held(one: ir.Held | None) -> str:
    return "None" if one is None else f"Some(h({one.value}, {one.width}))"


def loc(one: ir.Loc) -> str:
    match one:
        case ir.Reg(register=register, width=width):
            return f"rg({reg(register)}, {width})"
        case ir.Imm(value=value, width=width, address=address):
            return f"im({value}, {width}, {addr(address)})"
        case ir.St(index=index):
            return f"st({index})"
        case ir.Held(value=value, width=width):
            return f"hd({value}, {width})"
        case ir.Mem():
            return (
                f"m({addr(one.addr)}, {one.width}, {reg(one.through)}, {one.offset}, {one.disp_width}, "
                f"{held(one.base)}, {held(one.index)}, {one.scale}, {reg(one.index_through)})"
            )
        case ir.Address():
            return (
                f"ad({addr(one.addr)}, {reg(one.through)}, {reg(one.index)}, {one.scale}, {one.offset}, {one.disp_width})"
            )
    raise ValueError(one)


def opt(value) -> str:
    return "None" if value is None else f"Some({value})"


def emitted(made: select.Emitted | None) -> str:
    if made is None:
        return "None"
    fields = ", ".join(str(one) for one in made.fields)
    return (
        f'Some(("{made.code.hex()}", {opt(made.displacement_at)}, {opt(made.immediate_at)}, '
        f"vec![{fields}], {'true' if made.symbolic else 'false'}))"
    )


def remap(where) -> str:
    if where is None:
        return "None"
    one = ", ".join(f"({reg(k)}, {reg(v)})" for k, v in where.items())
    return f"Some(vec![{one}])"


def held_map(held) -> str:
    if held is None:
        return "None"
    one = ", ".join(f"({k}, {reg(v)})" for k, v in held.items())
    return f"Some(vec![{one}])"


def case(what: ir.Semantics, at=0, short=False, relocated=False, where=None, held=None) -> str:
    try:
        made = emitted(select.emit(what, at=at, where=where, short=short, relocated=relocated, held=held))
        check = "check"
    except (ValueError, OverflowError, IndexError) as error:
        made, check = f'"{error}"', "raises"
    name = "None" if what.name is None else f'Some("{what.name}")'
    dests = ", ".join(loc(one) for one in what.dests)
    sources = ", ".join(loc(one) for one in what.sources)
    return (
        f"    {check}(sem(Op::{what.op.name.title().replace('_', '')}, {name}, vec![{dests}], vec![{sources}], "
        f"{opt(what.target)}, {'true' if what.indirect else 'false'}), {at}, {'true' if short else 'false'}, "
        f"{'true' if relocated else 'false'}, {remap(where)}, {held_map(held)}, {made});"
    )

R = Register
S = ir.Semantics
O = ir.Operation
WIDE = (R.EAX, R.ECX, R.EDX, R.EBX, R.ESI, R.EDI, R.EBP, R.ESP)
NARROW = (R.AX, R.CX, R.DX, R.BX, R.SI, R.DI, R.BP, R.SP)
BYTE = (R.AL, R.CL, R.DL, R.BL, R.AH, R.CH, R.DH, R.BH)
SEGS = (R.CS, R.DS, R.ES, R.SS, R.FS, R.GS)
WIDTH = {**{r: 4 for r in WIDE}, **{r: 2 for r in NARROW}, **{r: 1 for r in BYTE}, **{r: 2 for r in SEGS}}
VALUES = (0, 1, -1, 2, 127, 128, -128, -129, 255, 0x1286, 0x7FFF, 0x8000, 0xFFFF, 0xFF80, 0x12345678, 0xBFFFFFF9, -0x80000000)

SOME = (0, 1, -1, 127, 128, -129, 0x1286, 0xFFFF, 0x12345678, 0xBFFFFFF9)


def r(one):
    return ir.Reg(one, WIDTH[one])


def cells(width: int) -> list[ir.Mem]:
    out = [
        ir.Mem(Addr(Space.FRAME, -4), width),
        ir.Mem(Addr(Space.FRAME, -0x200), width),
        ir.Mem(Addr(Space.FRAME, 0), width),
        ir.Mem(Addr(Space.FRAME, 6), width, index_through=R.SI),
        ir.Mem(Addr(Space.FRAME, 6), width, index_through=R.BX),
        ir.Mem(Addr(Space.FRAME, 6, base=R.SI), width),
        ir.Mem(Addr(Space.SEGMENT, 0x1234, 5), width),
        ir.Mem(Addr(Space.SEGMENT, 0x6, base=R.SI), width, through=R.SI),
        ir.Mem(Addr(Space.SEGMENT, 0x2, base=R.SI), width, R.BX, 2, 1, base=ir.Held(17, 2)),
        ir.Mem(Addr(Space.EXTERNAL, 0, 3), width),
        ir.Mem(Addr(Space.LITERAL, 0x0A, base=R.SI), width, through=R.SI, offset=0x0A),
        ir.Mem(Addr(Space.LITERAL, 0x10), width),
        ir.Mem(Addr(Space.LITERAL, 0x2, base=R.SI), width, R.BX, 2, 1, base=ir.Held(17, 2)),
        ir.Mem(Addr(Space.FAR, 0, base=R.BX, segment=R.ES), width),
        ir.Mem(Addr(Space.FAR, 0x200, base=R.BX, segment=R.ES), width),
        ir.Mem(Addr(Space.FAR, 0, base=R.BX, segment=R.ES), width, R.DI, 0, 0, base=ir.Held(21, 2)),
        ir.Mem(Addr(Space.FAR, 4, base=R.BX), width),
        ir.Mem(Addr(Space.GROUP, 4), width),
        ir.Mem(Addr(Space.STACK, 4), width),
        ir.Mem(None, width, through=R.BX),
        ir.Mem(None, width, through=R.SI, offset=4),
        ir.Mem(None, width, through=R.BP, offset=-0x22, disp_width=2),
        ir.Mem(None, width, through=R.BP),
        ir.Mem(None, width, through=R.DI, offset=0x300),
        ir.Mem(None, width, through=R.AX),
        ir.Mem(None, width),
        ir.Mem(Addr(Space.FAR, 2, segment=R.ES), width, R.BX, index=ir.Held(3, 2), index_through=R.SI),
        ir.Mem(Addr(Space.FAR, 2, segment=R.ES), width, R.BX, index=ir.Held(3, 4), scale=2, index_through=R.SI),
        ir.Mem(Addr(Space.LITERAL, 0x40), width, R.EBX, index=ir.Held(3, 4), scale=4, index_through=R.ECX),
        ir.Mem(Addr(Space.LITERAL, 0), width, R.EBP, index=ir.Held(3, 4), scale=1, index_through=R.EAX),
        ir.Mem(Addr(Space.LITERAL, 0x400), width, R.NONE, index=ir.Held(3, 4), scale=8, index_through=R.EDX),
        ir.Mem(Addr(Space.SEGMENT, 0), width, R.BX, index=ir.Held(3, 2), index_through=R.SI),
        ir.Mem(Addr(Space.LITERAL, 0), width, R.BX, base=ir.Held(4, 2)),
    ]
    return out


def sweep() -> list[str]:
    lines: list[str] = []
    add = lines.append
    regs = (R.EAX, R.ECX, R.ESP, R.AX, R.BX, R.SI, R.BP, R.AL, R.AH, R.BL, R.CS, R.DS, R.ES, R.FS)
    for into, outof in itertools.product(regs, repeat=2):
        add(case(S(O.MOVE, "mov", (r(into),), (r(outof),))))
    for into in regs:
        for value in VALUES:
            add(case(S(O.MOVE, "mov", (r(into),), (ir.Imm(value, WIDTH[into]),))))
    for width in (1, 2, 4):
        for cell in cells(width):
            for one in (R.AL, R.AX, R.EAX, R.ES):
                add(case(S(O.MOVE, "mov", (r(one),), (cell,))))
                add(case(S(O.MOVE, "mov", (cell,), (r(one),))))
            for value in (0, -1, 0xC1747C23):
                add(case(S(O.MOVE, "mov", (cell,), (ir.Imm(value, width),))))
    for segment in (R.ES, R.FS, R.GS, R.DS):
        for cell in cells(4)[:8]:
            add(case(S(O.MOVE, "les", (r(R.BX), r(segment)), (cell,))))
            add(case(S(O.MOVE, "lfs", (r(R.SI), r(segment)), (cell,))))
    for name in select.TWO_OPERAND + ("rol", "imul", "xchg", "", "foo"):
        for dest, source in itertools.product((R.AX, R.EAX, R.AL, R.ECX), repeat=2):
            add(case(S(O.BINARY, name, (r(dest),), (r(dest), r(source)))))
        if name not in ("add", "sbb", "cmp", "xor", "rol", ""):
            continue
        for dest in (R.AX, R.EAX, R.BX, R.AL):
            for value in SOME:
                for relocated in (False, True):
                    add(case(S(O.BINARY, name, (r(dest),), (r(dest), ir.Imm(value, WIDTH[dest]))), relocated=relocated))
        for width in (1, 2, 4):
            for cell in cells(width)[::8]:
                for one in (R.AL, R.AX, R.EAX):
                    add(case(S(O.BINARY, name, (r(one),), (r(one), cell))))
                    add(case(S(O.BINARY, name, (cell,), (cell, r(one)))))
                for value in (0, -1, 200, 0x1234):
                    for relocated in (False, True):
                        add(case(S(O.BINARY, name, (cell,), (cell, ir.Imm(value, width))), relocated=relocated))
    for name in select.SHIFTS + ("shld", "bogus"):
        for dest in (R.AX, R.EAX, R.BX, R.AL, R.CL):
            for count in (ir.Imm(1, 1), ir.Imm(3, 1), ir.Imm(31, 1), ir.Reg(R.CL, 1), ir.Reg(R.DL, 1)):
                add(case(S(O.BINARY, name, (r(dest),), (r(dest), count))))
        for width in (1, 2, 4):
            for cell in cells(width)[::8]:
                for count in (ir.Imm(1, 1), ir.Imm(3, 1), ir.Reg(R.CL, 1)):
                    add(case(S(O.BINARY, name, (cell,), (cell, count))))
    for name in select.ONE_OPERAND + ("bswap", "", "shr"):
        for dest in (R.AX, R.EAX, R.BX, R.AL, R.SI, R.EDI):
            add(case(S(O.UNARY, name, (r(dest),), (r(dest),))))
        for width in (1, 2, 4):
            for cell in cells(width)[::4]:
                add(case(S(O.UNARY, name, (cell,), (cell,))))
    for one in regs:
        add(case(S(O.PUSH, "push", (), (r(one),))))
        add(case(S(O.POP, "pop", (r(one),), ())))
        if one in SEGS:
            add(case(S(O.PUSH, "push", (), (ir.Reg(one, 4),))))
            add(case(S(O.POP, "pop", (ir.Reg(one, 4),), ())))
    for value in VALUES + (0x80000000, 0xEDCBA987, 0xFFFFFFFF):
        for width in (2, 4):
            for relocated in (False, True):
                add(case(S(O.PUSH, "push", (), (ir.Imm(value, width),)), relocated=relocated))
    for width in (1, 2, 4):
        for cell in cells(width)[::2]:
            add(case(S(O.PUSH, "push", (), (cell,))))
            add(case(S(O.POP, "pop", (cell,), ())))
    for name in ("jz", "jnz", "jl", "jge", "jb", "jae", "ja", "jbe", "jo", "jcxz", "loop", "bogus"):
        for target, at in ((0x10, 0), (0x100, 0x80), (0x4000, 0x10), (0x70, 0x0)):
            for short in (False, True):
                add(case(S(O.BRANCH, name, (), (), target=target), at=at, short=short))
    for target, at in ((0x10, 0), (0x100, 0x80), (0x4000, 0x10)):
        for short in (False, True):
            add(case(S(O.JUMP, "jmp", (), (), target=target), at=at, short=short))
        add(case(S(O.CALL, "call", (), (), target=target), at=at))
    add(case(S(O.CALL, "call", (), ()), at=0x4E))
    add(case(S(O.ESCAPE, "jmp", (), ())))
    for one in (ir.Reg(R.BX, 2), ir.Reg(R.EBX, 4), *cells(2)[::3], *cells(4)[::3]):
        add(case(S(O.CALL, "call", (), (one,), indirect=True)))
    add(case(S(O.RETURN, "ret", (), ())))
    add(case(S(O.RETURN, "retf", (), ())))
    for value in (0, 2, 6, 0xFFFF):
        add(case(S(O.RETURN, "retf", (), (ir.Imm(value, 2),))))
    for name in ("cmp", "test", None):
        for one in (R.AX, R.EAX, R.BX, R.AL, R.ECX):
            for value in (0, 1, -1, 0x80, 0x1234):
                for relocated in (False, True):
                    add(case(S(O.COMPARE, name, (), (r(one), ir.Imm(value, WIDTH[one]))), relocated=relocated))
            for other in (R.AX, R.CX, R.EAX, R.EDX, R.AL):
                add(case(S(O.COMPARE, name, (), (r(one), r(other)))))
        for width in (1, 2, 4):
            for cell in cells(width)[::4]:
                for value in (0, -1, 200, 0x1234):
                    add(case(S(O.COMPARE, name, (), (cell, ir.Imm(value, width)))))
                for one in (R.AL, R.AX, R.EAX):
                    add(case(S(O.COMPARE, name, (), (r(one), cell))))
                    add(case(S(O.COMPARE, name, (), (cell, r(one)))))
    for name in ("fcom", "fcomp", "fcompp", "ficom", "fbogus"):
        add(case(S(O.COMPARE, name, (), (ir.St(0), ir.St(1)))))
        for width in (2, 4, 8):
            add(case(S(O.COMPARE, name, (), (ir.St(0), ir.Mem(Addr(Space.FRAME, -8), width)))))
    for name in ("imul", "mul", "idiv"):
        for dest in (R.AX, R.EAX, R.CX):
            for source in (R.AX, R.CX, R.ECX):
                add(case(S(O.MULTIPLY, name, (r(dest),), (r(dest), r(source)))))
                for value in (3, 1000):
                    add(case(S(O.MULTIPLY, name, (r(dest),), (r(source), r(source), ir.Imm(value, 2)))))
                    add(case(S(O.MULTIPLY, name, (r(dest),), (r(source), ir.Imm(value, 2)))))
            for cell in cells(2)[::5] + cells(4)[::5]:
                add(case(S(O.MULTIPLY, name, (r(dest),), (r(dest), cell))))
                add(case(S(O.MULTIPLY, name, (r(dest),), (cell, ir.Imm(3, 2)))))
                add(case(S(O.MULTIPLY, name, (r(dest),), (r(dest), cell, ir.Imm(300, 2)))))
        for source in (R.CX, R.ECX, R.BL, R.SI):
            add(case(S(O.MULTIPLY, name, (r(R.AX), r(R.DX)), (r(R.AX), r(source)))))
        for cell in cells(2)[::3] + cells(4)[::3] + cells(1)[::5]:
            add(case(S(O.MULTIPLY, name, (r(R.AX), r(R.DX)), (r(R.AX), cell))))
    for name in ("idiv", "div", "imul"):
        for source in (R.CX, R.ECX, R.BL, R.SI):
            add(case(S(O.DIVIDE, name, (r(R.AX), r(R.DX)), (r(R.AX), r(R.DX), r(source)))))
        for cell in cells(2)[::3] + cells(4)[::3] + cells(1)[::5]:
            add(case(S(O.DIVIDE, name, (r(R.AX), r(R.DX)), (r(R.AX), r(R.DX), cell))))
    addresses = [
        ir.Address(Addr(Space.FRAME, -4)),
        ir.Address(Addr(Space.SEGMENT, 0x10, 2)),
        ir.Address(Addr(Space.LITERAL, 0x10)),
        ir.Address(Addr(Space.GROUP, 0x10)),
        ir.Address(Addr(Space.FRAME, -4), index=R.SI),
        ir.Address(None, R.EAX, R.EAX, 2, 0, 0),
        ir.Address(None, R.EAX, R.EAX, 4, 0, 0),
        ir.Address(None, R.BX, R.SI, 1, 6, 0),
        ir.Address(None, R.BP, R.NONE, 1, 0, 0),
        ir.Address(None, R.BX, R.NONE, 1, 0x300, 0),
        ir.Address(None, R.BX, R.NONE, 1, 4, 2),
        ir.Address(None, R.NONE, R.NONE, 1, 4, 0),
        ir.Address(None, R.NONE, R.ESI, 8, 4, 0),
    ]
    for into in (R.AX, R.EAX, R.BX, R.AL, R.ES):
        for one in addresses:
            add(case(S(O.ADDRESS, "lea", (r(into),), (one,))))
    for name in ("stosb", "stosw", "stosd", "movsb", ""):
        add(case(S(O.FILL, name, (), ())))
    for op in (O.NOTHING, O.LEAVE, O.EXTEND, O.FLOAT_UNARY):
        for name in (None, "", "nop", "leave", "cwd", "cdq", "wait", "sahf", "fsqrt", "fchs", "fabs", "bogus"):
            add(case(S(op, name, (), ())))
    for name in ("movsx", "movzx"):
        for dest, source in itertools.product((R.AX, R.EAX, R.BX, R.EBX, R.AL), (R.AL, R.BH, R.AX, R.SI, R.EAX)):
            add(case(S(O.EXTEND, name, (r(dest),), (r(source),))))
        for dest in (R.AX, R.EAX, R.BX, R.AL):
            for width in (1, 2, 4):
                for cell in cells(width)[::4]:
                    add(case(S(O.EXTEND, name, (r(dest),), (cell,))))
    for name in select.FLOAT_MEMORY + select.INT_MEMORY + ("fbogus",):
        for width in (2, 4, 8, 10):
            for cell in cells(width)[::10]:
                if name.startswith(("fst", "fist")):
                    add(case(S(O.FLOAT_STORE, name, (cell,), (ir.St(0),))))
                elif name.startswith(("fld", "fild")):
                    add(case(S(O.FLOAT_LOAD, name, (ir.St(0),), (cell,))))
                else:
                    add(case(S(O.FLOAT_ARITH, name, (ir.St(0),), (ir.St(0), cell))))
    for index in range(10):
        add(case(S(O.FLOAT_LOAD, "fld", (ir.St(0),), (ir.St(index),))))
        add(case(S(O.EXCHANGE, "fxch", (ir.St(0), ir.St(index)), (ir.St(0), ir.St(index)))))
        add(case(S(O.EXCHANGE, "fxch", (ir.St(0), ir.St(index)), (ir.St(index), ir.St(0)))))
        if index < 8:
            add(case(S(O.FLOAT_STORE, "fstp", (ir.St(index),), (ir.St(0),))))
            add(case(S(O.FLOAT_STORE, "fst", (ir.St(index),), (ir.St(0),))))
        for name in ("faddp", "fsubp", "fmulp", "fdivp", "fsubrp", "fdivrp", "fbogusp"):
            add(case(S(O.FLOAT_ARITH_POP, name, (ir.St(index),), (ir.St(index), ir.St(0)))))
        for name in ("fadd", "fsub", "fsubr", "fmul", "fdiv", "fdivr", "fbogus"):
            add(case(S(O.FLOAT_ARITH, name, (ir.St(0),), (ir.St(0), ir.St(index)))))
            add(case(S(O.FLOAT_ARITH, name, (ir.St(index),), (ir.St(index), ir.St(0)))))
            add(case(S(O.FLOAT_ARITH, name, (ir.St(index),), (ir.St(index), ir.St(1)))))
            add(case(S(O.FLOAT_ARITH, name, (ir.St(index),), (ir.St(2), ir.St(0)))))
    for name in ("fldz", "fld1", "fldpi"):
        add(case(S(O.FLOAT_LOAD, name, (ir.St(0),), ())))
    for name in ("fsqrt", "fchs", "fabs", "fbogus"):
        add(case(S(O.FLOAT_UNARY, name, (ir.St(0),), (ir.St(0),))))
        add(case(S(O.FLOAT_ARITH, name, (ir.St(0),), ())))
    add(case(S(O.BARRIER, "fnstsw", (ir.Reg(R.AX, 2),), ())))
    add(case(S(O.BARRIER, "fnstsw", (ir.Reg(R.AX, 4),), ())))
    for name in ("fldcw", "fnstcw", "fbogus"):
        for cell in cells(2)[::3] + cells(4)[:2]:
            add(case(S(O.BARRIER, name, (), (cell,))))
            add(case(S(O.BARRIER, name, (cell,), ())))
    for one, other in itertools.product((R.AX, R.CX, R.EAX, R.EDX, R.AL, R.BL), repeat=2):
        add(case(S(O.EXCHANGE, "xchg", (r(one), r(other)), (r(other), r(one)))))
    for one in (R.AL, R.AX, R.EAX, R.CX):
        for width in (1, 2, 4):
            for cell in cells(width)[::4]:
                add(case(S(O.EXCHANGE, "xchg", (r(one), cell), (cell, r(one)))))
                add(case(S(O.EXCHANGE, "xchg", (cell, r(one)), (r(one), cell))))
    for name in ("shld", "shrd", "shl"):
        for count in (ir.Imm(16, 1), ir.Imm(1, 1), ir.Reg(R.CL, 1), ir.Reg(R.DL, 1)):
            for low, high in ((R.EAX, R.EDX), (R.AX, R.DX), (R.EBX, R.ECX)):
                add(case(S(O.FUNNEL, name, (r(low),), (r(low), r(high), count))))
    for wide, low, high in ((R.EAX, R.AX, R.DX), (R.ECX, R.CX, R.BX), (R.ESI, R.SI, R.DI), (R.AX, R.AL, R.AH)):
        add(case(ir.restoring(r(wide), r(low), r(high))))
    for value in (0xA000, 0x40, 0x1FFFF, -1):
        for segment in SEGS:
            add(case(S(O.MOVE, "mov", (r(segment),), (ir.Imm(value, 2),))))
    # The remap and the allocation.
    where = {R.SI: R.DI, R.ESI: R.EDI, R.BX: R.SI}
    cell = ir.Mem(Addr(Space.LITERAL, 0x0A, base=R.SI), 2, through=R.SI, offset=0x0A, disp_width=1)
    add(case(S(O.BINARY, "add", (r(R.BX),), (r(R.BX), cell)), where=where))
    add(case(S(O.BINARY, "add", (r(R.BX),), (r(R.BX), cell)), where={}))
    add(case(S(O.MOVE, "mov", (r(R.BX),), (r(R.SI),)), where=where))
    add(case(S(O.MOVE, "mov", (r(R.BX),), (ir.Mem(None, 2, through=R.BX, offset=4),)), where=where))
    add(case(S(O.ADDRESS, "lea", (r(R.BX),), (ir.Address(None, R.BX, R.SI, 1, 6, 0),)), where=where))
    add(case(S(O.ADDRESS, "lea", (r(R.BX),), (ir.Address(Addr(Space.FRAME, -4, base=R.SI)),)), where=where))
    add(
        case(
            S(O.MOVE, "mov", (ir.Held(9, 2),), (ir.Held(7, 4),)),
            held={9: R.EBX, 7: R.ECX},
        )
    )
    add(case(S(O.MOVE, "mov", (ir.Held(9, 2),), (ir.Imm(1, 2),)), held={}))
    add(case(S(O.MOVE, "mov", (ir.Held(9, 2),), (ir.Imm(1, 2),)), held={9: R.EBX}))
    add(case(S(O.MOVE, "mov", (ir.Held(9, 1),), (ir.Imm(1, 1),)), held={9: R.EBX}))
    add(case(S(O.MOVE, "mov", (ir.Held(9, 2),), (ir.Imm(1, 2),)), held={8: R.EBX}))
    add(
        case(
            S(O.MOVE, "mov", (r(R.AX),), (ir.Mem(Addr(Space.SEGMENT, 0), 2, base=ir.Held(4, 2)),)),
        )
    )
    return lines


def rendered(lines: list[str]) -> str:
    chunks = [lines[at : at + 400] for at in range(0, len(lines), 400)]
    body = "".join(
        f"\n#[test]\nfn sweep_{index:02}() {{\n" + "\n".join(chunk) + "\n}\n" for index, chunk in enumerate(chunks)
    )
    return (
        "//! Generated by tools/select_sweep.py from Python's `select.emit`: do not edit.\n\n"
        "#![allow(clippy::unreadable_literal)]\n\n"
        "use super::sweep_support::*;\n" + body
    )


def main() -> None:
    lines = sweep()
    OUT.write_text(rendered(lines))
    print(f"{len(lines)} cases -> {OUT}")


if __name__ == "__main__":
    main()
