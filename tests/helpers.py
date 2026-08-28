def hx(s: str) -> bytes:
    return bytes.fromhex(s.replace(" ", ""))
