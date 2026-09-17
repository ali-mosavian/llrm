"""Target-dependent ranking of constant arithmetic, not MIR semantics."""

from qbopt.backend import cpu as targets


def cost(cpu: str | targets.Profile, operation: str) -> int:
    return targets.profile(cpu).cost(operation)


def validate(cpu: str | targets.Profile) -> None:
    targets.profile(cpu)


def immediate_multiply(cpu: str | targets.Profile, number: int) -> int:
    """Core clocks for audited positive imm8; other forms still use the old estimate.

    Intel 80386 Programmer's Reference Manual, IMUL: for positive m,
    max(ceil(log2(m)), 3) + 6. Restrict to positive imm8 so the value
    is identical in both word and dword forms. See docs/timing-audit.md.
    """
    if targets.profile(cpu).name == "386" and 0 <= number <= 127:
        return max((number - 1).bit_length() if number else 0, 3) + 6
    return cost(cpu, "imul_r32")


def scale(number: int, cpu: str | targets.Profile) -> tuple[tuple[str, int], ...] | None:
    """Binary and signed-digit chains, including destructive-operand copies."""
    target = targets.profile(cpu)
    if number <= 1:
        return None

    def chain(signed):
        digits = []
        remaining = number
        while remaining > 1:
            digit = (2 - remaining % 4 if signed else 1) if remaining & 1 else 0
            digits.append(digit)
            remaining = (remaining - digit) // 2
        parts = []
        shift = 0
        for digit in reversed(digits):
            shift += 1
            if digit:
                parts.extend((("shl", shift), ("add" if digit > 0 else "sub", 0)))
                shift = 0
        if shift:
            parts.append(("shl", shift))
        return tuple(parts)

    def clocks(parts):
        if (
            target.name in ("386", "P6")
            and len(parts) == 3
            and parts[0][0] == "shl"
            and 1 <= parts[0][1] <= 3
            and parts[1] == ("add", 0)
            and parts[2][0] == "shl"
        ):
            # peephole.addresses selects LEA + SHL for this exact shape.
            # 386: two core clocks. P6: GCC pentiumpro_cost ranks indexed
            # LEA at one unit, like a shift, versus four for multiply.
            return (2 if target.name == "386" else 1) + cost(target, "shift_ri")
        # One copy seeds the accumulator without destroying the source.
        return cost(target, "mov_rr") + sum(
            cost(target, "shift_ri" if name == "shl" else "alu_rr") for name, _ in parts
        )

    best = min((chain(False), chain(True)), key=clocks)
    return best if clocks(best) < immediate_multiply(target, number) else None
