"""Relative byte offsets of whole MIR pointers; no pointer encoding is assumed.

Only accesses already proven inside the same allocation may use relative
offsets to establish disjointness. Unrelated pointer values never suffice.
Facts are rebuilt from the current SSA, not attached to operands that a later
transformation could retarget.
"""

from dataclasses import dataclass

from qbopt.model import mir


@dataclass(frozen=True, slots=True)
class Offsets:
    definitions: dict[mir.Value, mir.Op]

    def relative(self, ref: mir.MemRef) -> tuple[mir.Value, int] | None:
        if (not ref.pointer or ref.base is None or ref.base_width != 4
            or ref.addr is not None or ref.segment is not None):
            return None
        value, offset = ref.base, 0
        seen = set()
        while value not in seen:
            seen.add(value)
            op = self.definitions.get(value)
            if (op is None or op.results != (mir.Held(value, 4),) or op.merges
                or op.loads or op.stores or op.barrier):
                return value, offset
            match op.kind, op.args:
                case mir.Kind.COPY, (mir.Held(value=source, width=4),):
                    value = source
                case mir.Kind.PTR_OFFSET, (mir.Held(value=source, width=4), mir.Const(n=amount, width=4)):
                    offset += ((amount & 0xffffffff) ^ 0x80000000) - 0x80000000
                    if not -0x80000000 <= offset <= 0x7fffffff:
                        return None
                    value = source
                case _:
                    return value, offset
        return None

    def comparable(self, one: mir.MemRef, other: mir.MemRef) -> tuple[int, int] | None:
        if one.allocation is None or one.allocation != other.allocation:
            return None
        left, right = self.relative(one), self.relative(other)
        if left is None or right is None or left[0] != right[0]:
            return None
        return left[1], right[1]

    def disjoint(self, one: mir.MemRef, other: mir.MemRef) -> bool:
        offsets = self.comparable(one, other)
        if offsets is None or one.width <= 0 or other.width <= 0:
            return False
        left, right = offsets
        return left + one.width <= right or right + other.width <= left

    def same_bytes(self, one: mir.MemRef, other: mir.MemRef) -> bool:
        if mir.same_bytes(one, other):
            return True
        offsets = self.comparable(one, other)
        return offsets is not None and offsets[0] == offsets[1] and one.width == other.width


def offsets(body: mir.MirBody) -> Offsets:
    return Offsets({value: op for block in body.blocks for op in block.ops for value in op.defines})
