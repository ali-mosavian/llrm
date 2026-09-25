from tools import innerloops


def _record(kind: int, body: bytes) -> bytes:
    return bytes([kind]) + (len(body) + 1).to_bytes(2, "little") + body + b"\0"


def _object(code: bytes, name: str) -> bytes:
    lnames = b"".join(bytes([len(one)]) + one for one in (b"S_CODE", b"BC_CODE"))
    return (
        _record(0x96, lnames)
        + _record(0x98, bytes([0x60]) + len(code).to_bytes(2, "little") + bytes([1, 2, 1]))
        + _record(0x90, bytes([0, 1, len(name)]) + name.encode() + b"\0\0\0")
        + _record(0xA0, bytes([1, 0, 0]) + code)
    )


def test_counts_only_the_innermost_call_free_loop_as_decoded():
    """The scoreboard read a dump taken before encoding, not the object's code."""
    code = bytes.fromhex(
        "31c9"  # xor cx,cx
        "b80a00"  # outer: mov ax,10
        "26030d"  # inner: add cx,es:[di]
        "83c702"  # add di,2
        "48"  # dec ax
        "75f7"  # jne inner
        "4b"  # dec bx
        "75f0"  # jne outer
        "9a00000000"  # again: call far 0:0
        "4b"  # dec bx
        "75f8"  # jne again
        "cb"  # retf
    )
    found = innerloops.loops(_object(code, "SUM"))
    assert [(one.name, one.size, one.memory) for one in found] == [("SUM#0", 4, 1)]
