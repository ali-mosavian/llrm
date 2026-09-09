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
        # One copy seeds the accumulator without destroying the source.
        return cost(cpu, "mov_rr") + sum(cost(cpu, "shift_ri" if name == "shl" else "alu_rr")
                                         for name, _ in parts)
    best = min((chain(False), chain(True)), key=clocks)
    return best if clocks(best) < cost(cpu, "imul_r32") else None
