"""qcport's modules through the C path, from streams wccq recorded.

pal and qglsurf built this way render dm3ish to the Borland build's md5.
"""

from pathlib import Path

from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def _asm(module: str) -> list[str]:
    text = cfront.compiled((FIXTURES / f"{module}.cgs").read_text(), module)
    return [line.strip() for line in text.splitlines()]


def test_implicit_conversion_is_the_raise_s():
    """wcc leaves `(long) byte - short` as mixed-width operands; the raise
    refused pal_bestfit with `used at width 4` until it converted them."""
    lines = _asm("pal")
    at = lines.index("movzx ax, byte ptr [bx]")
    assert lines[at + 1 : at + 5] == ["movzx eax, ax", "mov bx, word ptr [bp+6]", "movsx ebx, bx", "sub eax, ebx"]


def test_far_pointer_return_in_dx_ax():
    """pal_current's `(PalRgb far *) pal_now` crashed the return on an address
    that was not yet a value."""
    lines = _asm("pal")
    body = lines[lines.index("_pal_current proc far") : lines.index("_pal_current endp")]
    assert "mov ax, DGROUP" in body and "mov bx, offset _pal_now" in body
    assert body[-6:-3] == ["mov eax, dword ptr [bp-4]", "mov edx, eax", "shr edx, 16"]


def test_calls_push_in_convention_order():
    """cdecl pushes last first and pops after; pascal pushes first first."""
    lines = _asm("qglsurf")
    at = lines.index("call far ptr _asset_seek")
    assert lines[at - 3 : at + 2] == ["lea bx, [bp-10]", "push bx", "push ax", "call far ptr _asset_seek", "add sp, 4"]
    at = lines.index("call far ptr QGLSFNEW")
    assert lines[at - 6 : at] == [
        "mov ax, word ptr [bp+10]",
        "mov ebx, dword ptr [bp-14]",
        "mov cx, word ptr [bp+8]",
        "push cx",
        "push bx",
        "push ax",
    ]
