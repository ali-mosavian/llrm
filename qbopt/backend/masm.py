"""Allocated LIR as jwasm source.

`listing` is the procedure as emitted, frame and all; this prints it and
omfwrite.py encodes it, so the two cannot drift. Every operand is already
placed; an unplaced one is an error.
"""

from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

SIZES = {1: "byte", 2: "word", 4: "dword", 8: "qword", 10: "tbyte"}
# Callee-saved under the C convention: a Borland caller keeps SI and DI, not
# their upper halves, and a caller built here keeps nothing across a call.
SAVED = {Register.ESI: Register.SI, Register.EDI: Register.DI}
SEGMENTS = {"_DATA": ".data", "_BSS": ".data?", "CONST": ".const"}


class Unprintable(Exception):
    """An instruction or operand this printer has no spelling for."""


@dataclass(frozen=True, slots=True)
class Callee:
    name: str
    far: bool
    # Inline assembly laid down in place of a call: bytes, and (kind, symbol, offset) for a fixup.
    code: tuple["InlinePart", ...] = ()


@dataclass(frozen=True, slots=True)
class Procedure:
    name: str
    public: bool
    far: bool
    body: lir.LirBody
    reserve: int  # bytes below bp: locals and spill slots
    callees: dict[int, Callee]


@dataclass(frozen=True, slots=True)
class Label:
    name: str


@dataclass(frozen=True, slots=True)
class Fill:
    size: int
    byte: int | None  # None: uninitialised


@dataclass(frozen=True, slots=True)
class Pointer:
    name: str
    offset: int
    far: bool


@dataclass(frozen=True, slots=True)
class Align:
    to: int


type Datum = Label | Fill | Pointer | Align | bytes
type InlinePart = bytes | tuple[str, str, int]


@dataclass(frozen=True, slots=True)
class Module:
    code: str  # the code segment's name, MODULE_TEXT
    names: dict[tuple[Space, int], str]
    externs: tuple[tuple[str, str], ...]  # (name, "far" | "near" | "byte")
    publics: tuple[str, ...]
    data: tuple[tuple[str, tuple[Datum, ...]], ...]  # (segment, items)
    procedures: tuple[Procedure, ...]
    private: frozenset[str] = frozenset()  # data segments outside DGROUP


def text(module: Module) -> str:
    out = [".model medium", ".386", ""]
    out += [f"public {name}" for name in module.publics]
    for segment, items in module.data:
        private = segment in module.private
        out.append(SEGMENTS.get(segment, f"{segment} segment word public '{'FAR_DATA' if private else 'DATA'}'"))
        out += [f"extern {name}:byte" for name, kind in module.externs if kind == "byte"]
        out += [line for item in items for line in datum(item)]
        if segment not in SEGMENTS:
            out.append(f"{segment} ends")
            if not private:
                out.append(f"DGROUP group {segment}")
    out += [
        f"extern {name}:{'byte' if kind == 'far-byte' else kind}" for name, kind in module.externs if kind != "byte"
    ]
    out.append(f".code {module.code}")
    for number, procedure in enumerate(module.procedures):
        out += _procedure(procedure, module.names, number)
    out.append("end")
    return "\n".join(out) + "\n"


def datum(item: Datum) -> list[str]:
    match item:
        case Label(name=name):
            return [f"{name} label byte"]
        case Fill(size=size, byte=byte):
            return [f"    db {size} dup ({'?' if byte is None else byte})"]
        case Pointer(name=name, offset=offset, far=far):
            return [f"    {'dd' if far else 'dw'} {name}{_signed(offset)}"]
        case Align(to=to):
            return [f"    align {to}"]
    return list(_code((item,)))


type Item = Label | Callee | ir.Semantics


def listing(procedure: Procedure, number: int) -> list[Item]:
    """The procedure as emitted, frame included: what this prints and omfwrite encodes.

    A branch's target is still a block; `label(number, at)` names it.
    """
    saved = [low for whole, low in SAVED.items() if whole in _roots(procedure.body)]
    reserve = procedure.reserve + (procedure.reserve & 1)
    # Inline code is bytes this printer cannot read, so it may address the frame.
    framed = (
        bool(reserve) or Register.EBP in _roots(procedure.body) or any(one.code for one in procedure.callees.values())
    )
    bp, sp = ir.Reg(Register.BP, 2), ir.Reg(Register.SP, 2)
    leave = [ir.Semantics(ir.Operation.POP, "pop", (ir.Reg(one, 2),)) for one in reversed(saved)]
    if reserve:
        leave.append(ir.Semantics(ir.Operation.NOTHING, "leave"))
    elif framed:
        leave.append(ir.Semantics(ir.Operation.POP, "pop", (bp,)))
    out: list[Item] = []
    if framed:
        out += [
            ir.Semantics(ir.Operation.PUSH, "push", (), (bp,)),
            ir.Semantics(ir.Operation.MOVE, "mov", (bp,), (sp,)),
        ]
    if reserve:
        out.append(ir.Semantics(ir.Operation.BINARY, "sub", (sp,), (sp, ir.Imm(reserve, 2))))
    out += [ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(one, 2),)) for one in saved]
    blocks = procedure.body.blocks
    for index, block in enumerate(blocks):
        out.append(Label(label(number, block.at)))
        for one in block.insns:
            what = one.what
            if what is None:
                raise Unprintable(f"{procedure.name} at {one.at}: an instruction with no semantics")
            match what.op:
                case ir.Operation.MOVE if _segment(what.dests[0]) and isinstance(what.sources[0], ir.Imm):
                    # x86 has no immediate move into a segment register; the stack holds it for one instruction.
                    out += [
                        ir.Semantics(ir.Operation.PUSH, "push", (), (replace(what.sources[0], width=2),)),
                        ir.Semantics(ir.Operation.POP, "pop", what.dests),
                    ]
                case ir.Operation.CALL:
                    if what.indirect:
                        out.append(what)
                    else:
                        callee = procedure.callees.get(one.at)
                        if callee is None:
                            raise Unprintable(f"{procedure.name} at {one.at}: a call with no callee")
                        out.append(callee)
                case ir.Operation.RETURN:
                    out += [*leave, replace(what, name=what.name or ("retf" if procedure.far else "ret"))]
                case _:
                    out.append(what)
        fall = _falls_to(block, procedure.name)
        if fall is not None and (index + 1 == len(blocks) or blocks[index + 1].at != fall):
            out.append(ir.Semantics(ir.Operation.JUMP, "jmp", target=fall))
    return out


def label(number: int, at: int) -> str:
    return f"L{number}_{at}"


def _procedure(procedure: Procedure, names: dict, number: int) -> list[str]:
    out = [f"{procedure.name} proc {'far' if procedure.far else 'near'}"]
    for item in listing(procedure, number):
        match item:
            case Label(name=name):
                out.append(f"{name}:")
            case Callee(code=code) if code:
                out += [f"    {line}" for line in _code(code)]
            case Callee(name=name, far=far):
                out.append(f"    call {'far ptr ' if far else ''}{name}")
            case _:
                try:
                    out += [f"    {line}" for line in _instruction(item, names, number)]
                except Unprintable as error:
                    raise Unprintable(f"{procedure.name}: {error}") from error
    out.append(f"{procedure.name} endp")
    return out


def _falls_to(block: lir.LirBlock, name: str) -> int | None:
    """The successor control reaches by running off the block's end, if any."""
    last = next(
        (one.what for one in reversed(block.insns) if one.what is not None and one.what.op is not ir.Operation.NOTHING),
        None,
    )
    if last is not None and last.op in (ir.Operation.JUMP, ir.Operation.RETURN):
        return None
    taken = last.target if last is not None and last.op is ir.Operation.BRANCH else None
    rest = [one for one in block.succ if one != taken] or [one for one in block.succ]
    if len(rest) > 1:
        raise Unprintable(f"{name}: block {block.at} leaves for {block.succ} with no instruction choosing")
    return rest[0] if rest else None


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


def _instruction(what: ir.Semantics, names: dict, number: int) -> list[str]:
    name = what.name or ""
    if what.op is ir.Operation.FILL:
        # Its operands are the registers the instruction names in its opcode.
        return [f"rep {name}"]
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
        case ir.Operation.COMPARE if name.startswith("f"):
            memory = [text for text, source in zip(sources, what.sources, strict=True) if isinstance(source, ir.Mem)]
            return [f"{name} {memory[0]}" if memory else name]
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
            match what.sources[0]:
                case ir.Imm(address=None, width=width) | ir.Imm(address=Addr(space=Space.GROUP), width=width):
                    return [f"push{'d' if width == 4 else 'w'} {sources[0]}"]
            return [f"push {sources[0]}"]
        case ir.Operation.POP:
            return [f"pop {dests[0]}"]
        case ir.Operation.EXCHANGE if name == "fxch":
            return [f"fxch {dests[1]}"]
        case ir.Operation.EXCHANGE:
            return [f"xchg {dests[0]}, {dests[1]}"]
        case ir.Operation.FUNNEL:
            return [f"{name} {dests[0]}, {sources[1]}, {sources[2]}"]
        case ir.Operation.BRANCH | ir.Operation.JUMP:
            if what.target is None:
                raise Unprintable(f"{name or 'jump'} with no target")
            return [f"{name} {label(number, what.target)}"]
        case ir.Operation.CALL if what.indirect and len(sources) == 1:
            return [f"call {sources[0]}"]
        case ir.Operation.BARRIER:
            return [f"{name} {(dests or sources)[0]}"]
        case ir.Operation.FLOAT_LOAD:
            return [name] if name in ("fldz", "fld1") or not sources else [f"{name} {sources[0]}"]
        case ir.Operation.FLOAT_STORE:
            return [f"{name} {dests[0]}"]
        case ir.Operation.FLOAT_ARITH if isinstance(what.sources[-1], ir.Mem):
            return [f"{name} {sources[-1]}"]
        case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_ARITH_POP:
            return [f"{name} {dests[0]}, {sources[-1]}"]
        case ir.Operation.FLOAT_UNARY:
            return [name]
        case ir.Operation.RETURN:
            return [name]
    raise Unprintable(f"{what}")


def _code(parts: tuple):
    for part in parts:
        match part:
            case bytes():
                for start in range(0, len(part), 16):
                    yield "db " + ",".join(f"0{byte:02x}h" for byte in part[start : start + 16])
            case ("offset", name, offset):
                yield f"dw offset {name}{_signed(offset)}"
            case ("segment", name, _):
                yield f"dw seg {name}"


def _segment(where) -> bool:
    return isinstance(where, ir.Reg) and where.register in (
        Register.ES,
        Register.DS,
        Register.SS,
        Register.FS,
        Register.GS,
    )


def _operand(where, names: dict) -> str:
    match where:
        case ir.Reg(register=register):
            return target.name_of(register)
        case ir.St(index=index):
            return f"st({index})"
        case ir.Imm(value=value, address=None):
            return str(value)
        case ir.Imm(value=value, address=address):
            if address.space is Space.GROUP:
                return names[(address.space, address.index)]
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
            segment = "" if address.segment == Register.NONE else f"{target.name_of(address.segment)}:"
            return f"{size}{segment}[{registers}{disp}]"
        case Space.FAR if registers:
            return f"{size}{target.name_of(address.segment)}:[{registers}{disp}]"
    raise Unprintable(f"cell {cell}")


def _signed(n: int) -> str:
    return f"+{n}" if n > 0 else (str(n) if n < 0 else "")
