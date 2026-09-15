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
    code: tuple = ()


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
    private: frozenset[str] = frozenset()  # data segments outside DGROUP


def text(module: Module) -> str:
    out = [".model medium", ".386", ""]
    out += [f"public {name}" for name in module.publics]
    for segment, lines in module.data:
        private = segment in module.private
        out.append(SEGMENTS.get(segment, f"{segment} segment word public '{'FAR_DATA' if private else 'DATA'}'"))
        out += [f"extern {name}:{kind}" for name, kind in module.externs if kind == "byte"]
        out += lines
        if segment not in SEGMENTS:
            out.append(f"{segment} ends")
            if not private:
                out.append(f"DGROUP group {segment}")
    out += [f"extern {name}:{kind}" for name, kind in module.externs if kind != "byte"]
    out.append(f".code {module.code}")
    for number, procedure in enumerate(module.procedures):
        out += _procedure(procedure, module.names, number)
    out.append("end")
    return "\n".join(out) + "\n"


def _procedure(procedure: Procedure, names: dict, number: int) -> list[str]:
    saved = [low for whole, low in SAVED.items() if whole in _roots(procedure.body)]
    reserve = procedure.reserve + (procedure.reserve & 1)
    # Inline code is bytes this printer cannot read, so it may address the frame.
    framed = bool(reserve) or Register.EBP in _roots(procedure.body) or any(one.code for one in procedure.callees.values())
    leave = [f"pop {target.name_of(one)}" for one in reversed(saved)]
    leave += ["leave"] if reserve else ["pop bp"] * framed
    out = [f"{procedure.name} proc {'far' if procedure.far else 'near'}"]
    if framed:
        out += ["    push bp", "    mov bp, sp"]
    if reserve:
        out.append(f"    sub sp, {reserve}")
    out += [f"    push {target.name_of(one)}" for one in saved]
    blocks = procedure.body.blocks
    for index, block in enumerate(blocks):
        out.append(f"L{number}_{block.at}:")
        for one in block.insns:
            if one.what is None:
                raise Unprintable(f"{procedure.name} at {one.at}: an instruction with no semantics")
            try:
                lines = _instruction(one, procedure, names, number, leave)
            except Unprintable as error:
                raise Unprintable(f"{procedure.name} at {one.at}: {error}") from error
            out += [f"    {line}" for line in lines]
        fall = _falls_to(block, procedure.name)
        if fall is not None and (index + 1 == len(blocks) or blocks[index + 1].at != fall):
            out.append(f"    jmp L{number}_{fall}")
    out.append(f"{procedure.name} endp")
    return out


def _falls_to(block: lir.LirBlock, name: str) -> int | None:
    """The successor control reaches by running off the block's end, if any."""
    last = next((one.what for one in reversed(block.insns) if one.what.op is not ir.Operation.NOTHING), None)
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


def _instruction(one: lir.Insn, procedure: Procedure, names: dict, number: int, leave: list) -> list[str]:
    what = one.what
    name = what.name or ""
    dests = [_operand(x, names) for x in what.dests]
    sources = [_operand(x, names) for x in what.sources]
    match what.op:
        case ir.Operation.NOTHING:
            return [name] if name not in ("", "nop") else []
        case ir.Operation.MOVE if _segment(what.dests[0]) and isinstance(what.sources[0], ir.Imm):
            # x86 has no immediate move into a segment register; the stack holds it for one instruction.
            return [f"pushw {sources[0]}", f"pop {dests[0]}"]
        case ir.Operation.MOVE | ir.Operation.ADDRESS:
            return [f"{name} {dests[0]}, {sources[0]}"]
        case ir.Operation.BINARY:
            return [f"{name} {dests[0]}, {sources[1]}"]
        case ir.Operation.UNARY:
            return [f"{name} {dests[0]}"]
        case ir.Operation.COMPARE if name.startswith("f"):
            memory = [text for text, source in zip(sources, what.sources) if isinstance(source, ir.Mem)]
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
            if isinstance(what.sources[0], ir.Imm) and what.sources[0].address is None:
                return [f"push{'d' if what.sources[0].width == 4 else 'w'} {sources[0]}"]
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
            return [f"{name} L{number}_{what.target}"]
        case ir.Operation.CALL:
            callee = procedure.callees.get(one.at)
            if callee is None:
                raise Unprintable("a call with no callee")
            if callee.code:
                return list(_code(callee.code))
            return [f"call {'far ptr ' if callee.far else ''}{callee.name}"]
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
            return [*leave, name or ("retf" if procedure.far else "ret")]
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
    return isinstance(where, ir.Reg) and where.register in (Register.ES, Register.DS, Register.SS, Register.FS, Register.GS)


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
            return f"{size}[{registers}{disp}]"
        case Space.FAR if registers:
            return f"{size}{target.name_of(address.segment)}:[{registers}{disp}]"
    raise Unprintable(f"cell {cell}")


def _signed(n: int) -> str:
    return f"+{n}" if n > 0 else (str(n) if n < 0 else "")
