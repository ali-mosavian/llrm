"""Select signed constant division without exposing machine choices to MIR."""
from qbopt.backend import arithmetic, timing
from qbopt.model import ir


def magic(divisor: int, bits: int) -> tuple[int, int]:
    """Positive-divisor form of LLVM's SignedDivisionByConstantInfo algorithm."""
    if not 1 < divisor < 1 << (bits - 1):
        raise ValueError("positive signed divisor greater than one required")
    half = 1 << (bits - 1)
    boundary = half - 1 - half % divisor
    first, first_rem = divmod(half, boundary)
    second, second_rem = divmod(half, divisor)
    exponent = bits - 1
    while True:
        exponent += 1
        first *= 2
        first_rem *= 2
        if first_rem >= boundary:
            first += 1
            first_rem -= boundary
        second *= 2
        second_rem *= 2
        if second_rem >= divisor:
            second += 1
            second_rem -= divisor
        delta = divisor - second_rem
        if first > delta or (first == delta and first_rem):
            break
    multiplier = ((second + 1 + half) % (half * 2)) - half
    return multiplier, exponent - bits


def reciprocal(dividend, divisor, results, fresh, cpu):
    width = dividend.width
    if width != 4 or not 1 < divisor < 1 << 31:
        return None
    multiply_cost = timing.signed_multiply(cpu, width, full=True)
    divide_cost = timing.signed_divide(cpu, width)
    if multiply_cost is None or divide_cost is None:
        return None
    multiplier, shift = magic(divisor, 32)
    chain = arithmetic.scale(divisor, cpu)
    cost = lambda name: arithmetic.cost(cpu, name)
    reconstruction = (sum(cost("shift_ri" if name == "shl" else "alu_rr") for name, _ in chain)
                      if chain else timing.signed_multiply(cpu, width).maximum)
    # Materialize magic, seed multiply, preserve dividend and correction,
    # and seed reconstruction. Allocation may eliminate some of these moves.
    estimate = (5 * cost("mov_rr") + multiply_cost.maximum + reconstruction
                + (1 + bool(shift)) * cost("shift_ri")
                + (2 + (multiplier < 0)) * cost("alu_rr"))
    if cpu == "P5":
        # Intel 241430-004 section 24.3: one clock per prefix. Every
        # dword operation needs 66h in this 16-bit code segment. Charge
        # the reserved copies too; do not assume prefix decoding overlaps.
        estimate += 5 + 5 + (multiplier < 0) + bool(shift) + (len(chain) if chain else 1)
    direct = divide_cost.minimum
    if estimate >= direct:
        return None
    parts = []
    def emit(operation, name, sources, into=None):
        into = into or ir.Held(fresh(), width)
        parts.append(ir.Semantics(operation, name, (into,), tuple(sources)))
        return into
    constant = emit(ir.Operation.MOVE, "mov", (ir.Imm(multiplier, width),))
    low, high = ir.Held(fresh(), width), ir.Held(fresh(), width)
    parts.append(ir.Semantics(ir.Operation.MULTIPLY, "imul", (low, high), (dividend, constant)))
    if multiplier < 0:
        high = emit(ir.Operation.BINARY, "add", (high, dividend))
    sign = emit(ir.Operation.BINARY, "shr", (high, ir.Imm(31, 1)))
    if shift:
        high = emit(ir.Operation.BINARY, "sar", (high, ir.Imm(shift, 1)))
    quotient = emit(ir.Operation.BINARY, "add", (high, sign), results[0])
    product = quotient
    if chain:
        for name, amount in chain:
            product = emit(ir.Operation.BINARY, name,
                           (product, ir.Imm(amount, 1) if name == "shl" else quotient))
    else:
        product = emit(ir.Operation.MULTIPLY, "imul", (quotient, ir.Imm(divisor, width)))
    emit(ir.Operation.BINARY, "sub", (dividend, product), results[1])
    return tuple(parts)
