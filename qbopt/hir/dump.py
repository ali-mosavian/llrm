"""Stable semantic projection used to diff source and object frontends."""

from qbopt.model import mir
from qbopt.model import memory
from qbopt.hir.lower import Lowered


class _Names:
    def __init__(self, body: mir.MirBody) -> None:
        self.blocks = {block.at: number for number, block in enumerate(body.blocks, 1)}
        self.values: dict[mir.Value, int] = {}
        self.objects: dict[memory.Object, int] = {}

    def value(self, value: mir.Value) -> str:
        number = self.values.setdefault(value, len(self.values) + 1)
        return f"f{number}" if value.flags else f"v{number}"

    def block(self, at: int) -> str:
        return f"b{self.blocks[at]}"

    def object(self, object_: memory.Object) -> str:
        number = self.objects.setdefault(object_, len(self.objects) + 1)
        extent = "?" if object_.extent is None else str(object_.extent)
        return f"{object_.kind.value}{number}:{extent}"


def _arg(one: mir.Arg, names: _Names) -> str:
    match one:
        case mir.Held(value, width):
            return f"{names.value(value)}:{width}"
        case mir.Const(number, width):
            return f"{number}:{width}"
        case mir.Cell(ref):
            if ref.provenance is None:
                where = str(ref.where)
            else:
                where = "|".join(
                    f"{names.object(part.object)}[{part.low}:{part.high}:{part.stride}/{part.width}]"
                    for part in sorted(
                        ref.provenance.slices,
                        key=lambda item: (item.object.kind, repr(item.object.identity), item.low, item.high),
                    )
                )
            base = f"+{names.value(ref.base)}" if ref.base is not None else ""
            segment = f"@{names.value(ref.segment)}" if ref.segment is not None else ""
            return f"cell({where}{base}{segment}):{ref.width}"
        case mir.Symbol(space, index, offset, width, addend):
            displacement = offset + addend
            suffix = f"+{displacement}" if displacement > 0 else (str(displacement) if displacement else "")
            return f"{space.value}{index}{suffix}:{width}"
        case mir.FrameAddress(offset, width):
            sign = f"+{offset}" if offset > 0 else str(offset)
            return f"frame[{sign}]:{width}"
        case mir.Opaque(name=name):
            return name or "opaque"
        case _:
            return type(one).__name__.lower()


_INFIX = frozenset(
    {
        mir.Kind.ADD,
        mir.Kind.SUB,
        mir.Kind.ADD_CARRY,
        mir.Kind.SUB_BORROW,
        mir.Kind.MUL,
        mir.Kind.SMULHI,
        mir.Kind.DIV,
        mir.Kind.REM,
        mir.Kind.DIVMOD,
        mir.Kind.UDIVMOD,
        mir.Kind.AND,
        mir.Kind.OR,
        mir.Kind.XOR,
        mir.Kind.SHL,
        mir.Kind.SHR,
        mir.Kind.SAR,
        mir.Kind.LT,
        mir.Kind.LE,
        mir.Kind.GT,
        mir.Kind.GE,
        mir.Kind.EQ,
        mir.Kind.NE,
        mir.Kind.BELOW,
        mir.Kind.BELOW_EQ,
        mir.Kind.ABOVE,
        mir.Kind.ABOVE_EQ,
        mir.Kind.FADD,
        mir.Kind.FSUB,
        mir.Kind.FMUL,
        mir.Kind.FDIV,
        mir.Kind.FCOMPARE,
    }
)


def _operation(op: mir.Op, names: _Names) -> str:
    args = tuple(_arg(one, names) for one in op.args)
    results = tuple(names.value(one.value) if isinstance(one, mir.Held) else _arg(one, names) for one in op.results)
    defined = tuple(names.value(one) for one in op.defines)
    left = results or defined
    spelling = op.name if op.op.value == "funary" and op.name else op.kind.value

    if op.kind is mir.Kind.STORE and len(results) == 1 and len(args) == 1:
        return f"{results[0]} <- {args[0]}"
    if op.kind in _INFIX and len(args) == 2:
        expression = f"{args[0]} {spelling} {args[1]}"
    elif op.kind is mir.Kind.CALL:
        expression = f"call {op.name}({', '.join(args)})"
    elif op.kind is mir.Kind.BRANCH:
        expression = "branch" + (f" {args[0]}" if args else "")
    elif op.kind is mir.Kind.RETURN:
        expression = "return" + (f" {', '.join(args)}" if args else "")
    elif len(args) == 1:
        expression = f"{spelling} {args[0]}"
    elif not args:
        expression = spelling
    else:
        expression = f"{spelling}({', '.join(args)})"
    return f"{', '.join(left)} <- {expression}" if left else expression


def mir_text(lowered: Lowered) -> str:
    """Print meaning with incidental block, value, and object identities removed."""
    names = _Names(lowered.body)
    lines = [f"function {lowered.name} entry {names.block(lowered.body.entry)}"]
    for block in lowered.body.blocks:
        successors = ", ".join(names.block(one) for one in block.succ) or "-"
        lines.append(f"{names.block(block.at)} -> {successors}")
        for op in block.ops:
            target = f" -> {names.block(op.target)}" if op.target is not None else ""
            cases = "" if not op.cases else " " + repr(tuple((value, names.block(at)) for value, at in op.cases))
            floating = ""
            if op.floating is not None:
                inputs = ",".join(one.value for one in op.floating.inputs)
                floating = (
                    f" [{inputs}->{op.floating.result.value};"
                    f"{op.floating.precision.value}/{op.floating.rounding.value}]"
                )
            lines.append(f"  {_operation(op, names)}{target}{cases}{floating}")
    return "\n".join(lines) + "\n"
