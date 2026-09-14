"""Allocated LIR as jwasm source.

The C path's emitter while there is no OMF writer that builds a module from
nothing: jwasm owns segments, fixups and encodings, and the output reads as
what it is. Every operand is already placed; an unplaced one is an error.
"""

from dataclasses import dataclass

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.objectfile.module import Space

SIZES = {1: "byte", 2: "word", 4: "dword"}
# Callee-saved under the C convention, saved whole: a 16-bit caller keeps SI
# and DI, and a caller built here may keep all 32 bits.
SAVED = (Register.ESI, Register.EDI)
SEGMENTS = {"_DATA": ".data", "_BSS": ".data?", "CONST": ".const"}


class Unprintable(Exception):
    """An instruction or operand this printer has no spelling for."""


@dataclass(frozen=True, slots=True)
class Callee:
    name: str
    far: bool


@dataclass(frozen=True, slots=True)
class Procedure:
    name: str
    public: bool
    far: bool
    body: lir.LirBody
    reserve: int  # bytes below bp: locals and spill slots
    callees: dict[int, Callee]


@dataclass(frozen=True, slots=True)
class Module:
    code: str  # the code segment's name, MODULE_TEXT
    names: dict[tuple[Space, int], str]
    externs: tuple[tuple[str, str], ...]  # (name, "far" | "near" | "byte")
    publics: tuple[str, ...]
    data: tuple[tuple[str, tuple[str, ...]], ...]  # (segment, lines)
    procedures: tuple[Procedure, ...]


def text(module: Module) -> str:
    out = [".model medium", ".386", ""]
    out += [f"public {name}" for name in module.publics]
    for segment, lines in module.data:
        out.append(SEGMENTS.get(segment, f"{segment} segment word public 'DATA'"))
        out += [f"extern {name}:{kind}" for name, kind in module.externs if kind == "byte"]
        out += lines
        if segment not in SEGMENTS:
            out.append(f"{segment} ends")
    out += [f"extern {name}:{kind}" for name, kind in module.externs if kind != "byte"]
    out.append(f".code {module.code}")
    for number, procedure in enumerate(module.procedures):
        out += _procedure(procedure, module.names, number)
    out.append("end")
    return "\n".join(out) + "\n"


def _procedure(procedure: Procedure, names: dict, number: int) -> list[str]:
    saved = [one for one in SAVED if one in _roots(procedure.body)]
    reserve = procedure.reserve + (procedure.reserve & 1)
    out = [f"{procedure.name} proc {'far' if procedure.far else 'near'}", "    push bp", "    mov bp, sp"]
    if reserve:
        out.append(f"    sub sp, {reserve}")
    out += [f"    push {target.name_of(one)}" for one in saved]
    for block in procedure.body.blocks:
        out.append(f"L{number}_{block.at}:")
        for one in block.insns:
            if one.what is None:
                raise Unprintable(f"{procedure.name} at {one.at}: an instruction with no semantics")
            try:
                lines = _instruction(one, procedure, names, number, saved)
            except Unprintable as error:
                raise Unprintable(f"{procedure.name} at {one.at}: {error}") from error
            out += [f"    {line}" for line in lines]
    out.append(f"{procedure.name} endp")
    return out


def _roots(body: lir.LirBody) -> set:
    found = set()
    for one in body.insns:
        if one.what is None:
            continue
        for where in (*one.what.dests, *one.what.sources):
            match where:
                case ir.Reg(register=register):
                    found.add(ir.ROOT.get(register, register))
                case ir.Mem(through=through, index_through=index):
                    found.update(ir.ROOT.get(one, one) for one in (through, index))
    return found


def _instruction(one: lir.Insn, procedure: Procedure, names: dict, number: int, saved: list) -> list[str]:
    what = one.what
    name = what.name or ""
    dests = [_operand(x, names) for x in what.dests]
    sources = [_operand(x, names) for x in what.sources]
    match what.op:
        case ir.Operation.NOTHING:
            return [name] if name not in ("", "nop") else []
        case ir.Operation.MOVE | ir.Operation.ADDRESS:
            return [f"{name} {dests[0]}, {sources[0]}"]
        case ir.Operation.BINARY:
            return [f"{name} {dests[0]}, {sources[1]}"]
        case ir.Operation.UNARY:
            return [f"{name} {dests[0]}"]
        case ir.Operation.COMPARE:
            return [f"{name or 'cmp'} {sources[0]}, {sources[1]}"]
        case ir.Operation.MULTIPLY if len(dests) == 1:
            if len(sources) == 3:
                return [f"imul {dests[0]}, {sources[1]}, {sources[2]}"]
            if isinstance(what.sources[1], ir.Imm):
                return [f"imul {dests[0]}, {sources[0]}, {sources[1]}"]
            return [f"imul {dests[0]}, {sources[1]}"]
        case ir.Operation.MULTIPLY | ir.Operation.DIVIDE:
            return [f"{name} {sources[-1]}"]
        case ir.Operation.EXTEND:
            return [f"{name} {dests[0]}, {sources[0]}"] if name in ("movsx", "movzx") else [name]
        case ir.Operation.PUSH:
            if isinstance(what.sources[0], ir.Imm) and what.sources[0].address is None:
                return [f"push{'d' if what.sources[0].width == 4 else 'w'} {sources[0]}"]
            return [f"push {sources[0]}"]
        case ir.Operation.POP:
            return [f"pop {dests[0]}"]
        case ir.Operation.EXCHANGE:
            return [f"xchg {dests[0]}, {dests[1]}"]
        case ir.Operation.FUNNEL:
            return [f"{name} {dests[0]}, {sources[1]}, {sources[2]}"]
        case ir.Operation.BRANCH | ir.Operation.JUMP:
            return [f"{name} L{number}_{what.target}"]
        case ir.Operation.CALL:
            callee = procedure.callees.get(one.at)
            if callee is None:
                raise Unprintable("a call with no callee")
            return [f"call {'far ptr ' if callee.far else ''}{callee.name}"]
        case ir.Operation.RETURN:
            restore = [f"pop {target.name_of(register)}" for register in reversed(saved)]
            return [*restore, "mov sp, bp", "pop bp", name or ("retf" if procedure.far else "ret")]
    raise Unprintable(f"{what}")


def _operand(where, names: dict) -> str:
    match where:
        case ir.Reg(register=register):
            return target.name_of(register)
        case ir.Imm(value=value, address=None):
            return str(value)
        case ir.Imm(value=value, address=address):
            if address.space is Space.GROUP:
                return "DGROUP"
            return f"offset {names[(address.space, address.index)]}{_signed(address.disp + value)}"
        case ir.Mem():
            return _memory(where, names)
        case ir.Address(addr=address, index=Register.NONE) if address is not None:
            return _memory(ir.Mem(address, 2), names).removeprefix("word ptr ")
        case ir.Address(through=through, index=index, scale=scale, offset=offset):
            return f"[{_registers(through, index, scale)}{_signed(offset)}]"
    raise Unprintable(f"operand {where}")


def _registers(base, index, scale: int = 1) -> str:
    parts = [target.name_of(base)] if base != Register.NONE else []
    if index != Register.NONE:
        parts.append(target.name_of(index) + (f"*{scale}" if scale != 1 else ""))
    return "+".join(parts)


def _memory(cell: ir.Mem, names: dict) -> str:
    size = f"{SIZES[cell.width]} ptr "
    address = cell.addr
    # As select.operand_of: a named address carries the displacement, and
    # `offset` is only the displacement of a cell with none.
    if address is None:
        if cell.through == Register.NONE:
            raise Unprintable(f"cell {cell}")
        return f"{size}[{target.name_of(cell.through)}{_signed(cell.offset)}]"
    registers = _registers(cell.through, cell.index_through, cell.scale)
    disp = _signed(address.disp)
    match address.space:
        case Space.FRAME:
            return f"{size}[bp{disp}]"
        case Space.SEGMENT | Space.EXTERNAL:
            symbol = names[(address.space, address.index)]
            return f"{size}{symbol}{disp}" + (f"[{registers}]" if registers else "")
        case Space.LITERAL if registers:
            return f"{size}[{registers}{disp}]"
        case Space.FAR if registers:
            return f"{size}{target.name_of(address.segment)}:[{registers}{disp}]"
    raise Unprintable(f"cell {cell}")


def _signed(n: int) -> str:
    return f"+{n}" if n > 0 else (str(n) if n < 0 else "")
