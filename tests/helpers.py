from qbopt.lift import Decoded
from qbopt.declen import decode
from qbopt.lift import classify


def hx(s: str) -> bytes:
    return bytes.fromhex(s.replace(" ", ""))


def classify_code(code: bytes) -> Decoded | None:
    """What lift makes of the first instruction in these bytes."""
    insn = decode(code, 0)
    return classify(insn) if insn is not None else None
