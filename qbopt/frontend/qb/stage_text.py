"""Diagnostic MASM/Intel projection for virtual and allocated LIR.

This is deliberately outside the backend: it prints pass state and does not
participate in selection, allocation, encoding, or object emission.
"""

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.model import ir
from qbopt.backend import target
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

SIZES = {1: "byte", 2: "word", 4: "dword", 8: "qword", 10: "tbyte"}


def instruction_text(what: ir.Semantics) -> tuple[str, ...]:
    """Render one LIR operation in Intel operand order and MASM spelling."""
    name = what.name or ""
    dests = tuple(_operand(one) for one in what.dests)
    sources = tuple(_operand(one) for one in what.sources)
    match what.op:
        case ir.Operation.NOTHING:
            return (name,) if name not in ("", "nop") else ()
        case ir.Operation.MOVE | ir.Operation.ADDRESS:
            return (f"{name} {dests[0]}, {sources[0]}",)
        case ir.Operation.BINARY:
            return (f"{name} {dests[0]}, {sources[1]}",)
        case ir.Operation.UNARY:
            return (f"{name} {dests[0]}",)
        case ir.Operation.COMPARE if name.startswith("f"):
            memory = [text for text, source in zip(sources, what.sources, strict=True) if isinstance(source, ir.Mem)]
            return (f"{name} {memory[0]}" if memory else name,)
        case ir.Operation.COMPARE:
            return (f"{name or 'cmp'} {sources[0]}, {sources[1]}",)
        case ir.Operation.MULTIPLY if len(dests) == 1:
            if len(sources) == 3:
                return (f"imul {dests[0]}, {sources[1]}, {sources[2]}",)
            if isinstance(what.sources[1], ir.Imm):
                return (f"imul {dests[0]}, {sources[0]}, {sources[1]}",)
            return (f"imul {dests[0]}, {sources[1]}",)
        case ir.Operation.MULTIPLY | ir.Operation.DIVIDE:
            return (f"{name} {sources[-1]}",)
        case ir.Operation.EXTEND:
            return (f"{name} {dests[0]}, {sources[0]}" if name in ("movsx", "movzx") else name,)
        case ir.Operation.PUSH:
            suffix = ""
            if isinstance(what.sources[0], ir.Imm):
                suffix = "d" if what.sources[0].width == 4 else "w"
            return (f"push{suffix} {sources[0]}",)
        case ir.Operation.POP:
            return (f"pop {dests[0]}",)
        case ir.Operation.EXCHANGE if name == "fxch":
            return (f"fxch {dests[1]}",)
        case ir.Operation.EXCHANGE:
            return (f"xchg {dests[0]}, {dests[1]}",)
        case ir.Operation.FUNNEL:
            return (f"{name} {dests[0]}, {sources[1]}, {sources[2]}",)
        case ir.Operation.BRANCH | ir.Operation.JUMP:
            return (f"{name} L0_{what.target}",)
        case ir.Operation.CALL if what.indirect and sources:
            return (f"call {sources[0]}",)
        case ir.Operation.CALL:
            return (f"call {name}",)
        case ir.Operation.FILL:
            return (f"rep {name}",)
        case ir.Operation.BARRIER:
            operands = dests or sources
            return (f"{name} {operands[0]}" if operands else name,)
        case ir.Operation.FLOAT_LOAD:
            return (name if name in ("fldz", "fld1") or not sources else f"{name} {sources[0]}",)
        case ir.Operation.FLOAT_STORE:
            operands = dests or sources
            return (f"{name} {operands[0]}" if operands else name,)
        case ir.Operation.FLOAT_ARITH if isinstance(what.sources[-1], ir.Mem):
            return (f"{name} {sources[-1]}",)
        case ir.Operation.FLOAT_ARITH | ir.Operation.FLOAT_ARITH_POP:
            return (f"{name} {dests[0]}, {sources[-1]}",)
        case ir.Operation.FLOAT_UNARY | ir.Operation.LEAVE | ir.Operation.RETURN:
            return (name,) if name else ()
        case ir.Operation.ESCAPE:
            return (f"jmp far ptr {sources[0] if sources else name}",)
        case ir.Operation.RESTORE:
            return (f"restore {', '.join((*dests, *sources))}",)
        case ir.Operation.DATA:
            return (name or "db ?",)
    return (f"; unprintable {what.op.value} {name}",)


def inline_text(parts: tuple[object, ...]) -> tuple[str, ...]:
    """Render a finalized in-place intrinsic body as MASM data directives."""
    lines: list[str] = []
    for part in parts:
        match part:
            case bytes():
                lines.extend(
                    "db " + ",".join(f"0{byte:02x}h" for byte in part[start : start + 16])
                    for start in range(0, len(part), 16)
                )
            case ("offset", str(name), int(offset)):
                lines.append(f"dw offset {name}{_signed(offset)}")
            case ("segment", str(name), _):
                lines.append(f"dw seg {name}")
    return tuple(lines)


def _operand(where: ir.Loc) -> str:
    match where:
        case ir.Reg(register=register):
            return target.name_of(register)
        case ir.Held(value=value):
            return f"v{value}"
        case ir.St(index=index):
            return f"st({index})"
        case ir.Imm(value=value, address=None):
            return str(value)
        case ir.Imm(value=value, address=address):
            if address.space is Space.GROUP:
                return _symbol(address)
            return f"offset {_symbol(address)}{_signed(address.disp + value)}"
        case ir.Mem():
            return _memory(where)
        case ir.Address(addr=address, index=Register.NONE) if address is not None:
            return _memory(ir.Mem(address, 2)).removeprefix("word ptr ")
        case ir.Address(through=through, index=index, scale=scale, offset=offset):
            return f"[{_registers(through, index, scale)}{_signed(offset)}]"
    return f"<{type(where).__name__.lower()}>"


def _memory(cell: ir.Mem) -> str:
    size = f"{SIZES.get(cell.width, f'{cell.width}-byte')} ptr "
    address = cell.addr
    if cell.base is not None or cell.index is not None or cell.selector is not None:
        registers = []
        if cell.base is not None:
            registers.append(f"v{cell.base.value}")
        if cell.index is not None:
            registers.append(f"v{cell.index.value}" + (f"*{cell.scale}" if cell.scale != 1 else ""))
        inside = "+".join(registers)
        displacement = cell.offset if address is None else address.disp
        if address is not None and address.space in (Space.SEGMENT, Space.EXTERNAL, Space.GROUP):
            inside = _symbol(address) + (("+" + inside) if inside else "")
        selector = f"v{cell.selector.value}:" if cell.selector is not None else ""
        return f"{size}{selector}[{inside}{_signed(displacement)}]"
    if address is None:
        base = target.name_of(cell.through) if cell.through != Register.NONE else "?"
        return f"{size}[{base}{_signed(cell.offset)}]"
    registers = _registers(cell.through, cell.index_through, cell.scale)
    disp = _signed(address.disp)
    match address.space:
        case Space.FRAME:
            return f"{size}[bp{disp}]"
        case Space.SEGMENT | Space.EXTERNAL | Space.GROUP:
            return f"{size}{_symbol(address)}{disp}" + (f"[{registers}]" if registers else "")
        case Space.LITERAL:
            segment = "" if address.segment == Register.NONE else f"{target.name_of(address.segment)}:"
            return f"{size}{segment}[{registers}{disp}]"
        case Space.FAR:
            return f"{size}{target.name_of(address.segment)}:[{registers}{disp}]"
        case Space.STACK:
            return f"{size}[sp{disp}]"
    return f"{size}[?]"


def _registers(base: Register_, index: Register_, scale: int = 1) -> str:
    parts = [target.name_of(base)] if base != Register.NONE else []
    if index != Register.NONE:
        parts.append(target.name_of(index) + (f"*{scale}" if scale != 1 else ""))
    return "+".join(parts)


def _symbol(address: Addr) -> str:
    return f"{address.space.value}_{address.index}"


def _signed(number: int) -> str:
    return f"+{number}" if number > 0 else (str(number) if number < 0 else "")
