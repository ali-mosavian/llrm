"""Target-dependent ranking of constant arithmetic, not MIR semantics."""
from qbopt.cycles import timings


def cost(cpu: str, operation: str) -> int:
    if cpu == "386":
        # The representative ranking used by tools/opportunity.py.
        return {"shift_ri": 3, "alu_rr": 2, "mov_rr": 2,
                "imul_r32": 22, "idiv_r32": 43, "cdq": 2}[operation]
    return timings.COST[operation][timings.ARCHS.index(cpu)]


def validate(cpu: str) -> None:
    if cpu not in ("386", *timings.ARCHS):
        raise ValueError(f"unknown CPU target: {cpu}")


def immediate_multiply(cpu: str, number: int) -> int:
    """Core clocks for audited positive imm8; other forms still use the old estimate.

    Intel 80386 Programmer's Reference Manual, IMUL: for positive m,
    max(ceil(log2(m)), 3) + 6. Restrict to positive imm8 so the value
    is identical in both word and dword forms. See docs/timing-audit.md.
    """
    if cpu == "386" and 0 <= number <= 127:
        return max((number - 1).bit_length() if number else 0, 3) + 6
    return cost(cpu, "imul_r32")


def scale(number: int, cpu: str) -> tuple[tuple[str, int], ...] | None:
    """Binary and signed-digit chains, including destructive-operand copies."""
    validate(cpu)
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
        if (cpu in ("386", "P6") and len(parts) == 3 and parts[0][0] == "shl"
            and 1 <= parts[0][1] <= 3 and parts[1] == ("add", 0) and parts[2][0] == "shl"):
            # peephole.addresses selects LEA + SHL for this exact shape.
            # 386: two core clocks. P6: GCC pentiumpro_cost ranks indexed
            # LEA at one unit, like a shift, versus four for multiply.
            return (2 if cpu == "386" else 1) + cost(cpu, "shift_ri")
        # One copy seeds the accumulator without destroying the source.
        return cost(cpu, "mov_rr") + sum(cost(cpu, "shift_ri" if name == "shl" else "alu_rr")
                                         for name, _ in parts)
    best = min((chain(False), chain(True)), key=clocks)
    return best if clocks(best) < immediate_multiply(cpu, number) else None
