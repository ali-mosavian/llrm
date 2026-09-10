"""Target pointer representation, kept below the MIR boundary.

The BASIC huge-pointer ABI advances a selector by 1 << huge_shift whenever
the byte offset crosses 64K. Runtime gwini.asm documents the OS-dependent
contract; nhinit.asm sets 12 for DOS. Callers must supply an established
model, not infer it from an arithmetic CPU tuning choice.
"""

from dataclasses import dataclass

from qbopt.model import ir


@dataclass(frozen=True, slots=True)
class Model:
    huge_shift: int | ir.Mem

    def __post_init__(self):
        if isinstance(self.huge_shift, ir.Mem):
            if self.huge_shift.width != 1:
                raise ValueError("huge-pointer selector shift must be a byte")
        elif not 0 <= self.huge_shift < 16:
            raise ValueError("unsupported huge-pointer selector shift")

    def offset(self, pointer, displacement, result, fresh):
        """Lower one whole pointer addition to arithmetic over abstract variables."""
        if any(not isinstance(arg, (ir.Held, ir.Imm)) or arg.width != 4
               for arg in (pointer, displacement)) or not isinstance(result, ir.Held) or result.width != 4:
            raise ValueError("pointer offset requires a pointer, displacement and result at width 4")
        parts = []

        def binary(name, left, right, destination=None):
            destination = destination or ir.Held(fresh(), 4)
            parts.append(ir.Semantics(ir.Operation.BINARY, name, (destination,), (left, right)))
            return destination

        low = binary("and", pointer, ir.Imm(0xffff, 4))
        total = binary("add", low, displacement)
        pages = binary("shr", total, ir.Imm(16, 1))
        if isinstance(self.huge_shift, ir.Mem):
            shift = ir.Held(fresh(), 1)
            parts.append(ir.Semantics(ir.Operation.MOVE, "mov", (shift,), (self.huge_shift,)))
        else:
            shift = ir.Imm(self.huge_shift, 1)
        delta = binary("shl", pages, shift)
        selector = binary("shr", pointer, ir.Imm(16, 1))
        advanced = binary("add", selector, delta)
        high = binary("shl", advanced, ir.Imm(16, 1))
        low = binary("and", total, ir.Imm(0xffff, 4))
        binary("or", high, low, result)
        return tuple(parts)
