"""The data axis against the linked image, from two sources: vary one array, link, and compare the change in the MZ-header image
(what the loader maps) with the change in `sizes.data_bytes(obj, exe, map)[0]` (what the object and map say is stored).

  LLRM_BIN=<dist with jwasm/jwlink> python tools/axis_differential.py

C and Nib, an array of 16 / 1000 / 4000 bytes, zero or initialised. Not covered: a BSS that jwlink stores before the stack (map check only) and
BASIC (MS LINK)."""

import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).parent / "dosbatch"))

import llrmbin  # noqa: E402
import sizes  # noqa: E402

C = "extern void report(long);\n{decl}\nint main(void){{ a[3]=7; report(a[3]); return 0; }}\n"
NIB = (
    "var a: u8[{n}] = {init}\nvar k: i16 = 3\nfn main() -> i16:\n    let mut s: i16 = 0\n    let mut i: i16 = k\n    while i < {n}:\n"
    "        a[i] = u8(i16(a[i]) + k)\n        s = s + i16(a[i])\n        i += 7\n    return s\n"
)  # live and non-constant: a write-only array is shrunk to one element by the optimizer


def image(exe: Path) -> int:
    """The bytes the loader maps: the file minus the header paragraphs."""
    data = exe.read_bytes()
    last, pages, _, header = struct.unpack("<HHHH", data[2:10])
    return (pages - 1) * 512 + (last or 512) - header * 16


def differences(base: tuple[int, int], cells: dict[str, tuple[int, int]]) -> list[str]:
    """The cells whose change in image bytes is not the change in stored data bytes, each against `base` = (image, stored)."""
    return [f"{label}: image {got[0] - base[0]:+d}, axis {got[1] - base[1]:+d}" for label, got in cells.items() if got[0] - base[0] != got[1] - base[1]]


def measure(kind: str, n: int, initialised: bool) -> tuple[int, int]:
    """(image bytes, stored data bytes by the axis) of the program with `char a[n]` (C) or `u8[n]` (Nib)."""
    import dosbatch

    work = Path(tempfile.mkdtemp())
    obj, exe, listing = work / "t.obj", work / "t.exe", work / "t.map"
    if kind == "c":
        (work / "t.c").write_text(C.format(decl=f"char a[{n}] = {{1}};" if initialised else f"char a[{n}];"))
        command = [str(dosbatch.BIN / "llrm-cc"), "-O1", *llrmbin.c_args(16), "-c", str(work / "t.c"), "-o", str(obj)]
        subprocess.run(command, env={**os.environ, **llrmbin.cc_environment()}, check=True, capture_output=True)
        dosbatch.link_target(dosbatch.REAL_MODE, obj, exe, work, listing)
    else:
        (work / "t.nib").write_text(NIB.format(n=n, init=f"[{int(initialised)}] * {n}"))
        subprocess.run([str(dosbatch.BIN / "llrm-nib"), "-O1", "-m16", "-march=i386", str(work / "t.nib"), "-o", str(obj)], check=True, capture_output=True)
        real = dosbatch.link_target
        dosbatch.link_target = lambda *a, **k: real(*a, **{**k, "listing": listing})
        try:
            dosbatch.link_nib(dosbatch.REAL_MODE, work / "t.nib", obj, exe, work, "-O1", ())
        finally:
            dosbatch.link_target = real
    return image(exe), sizes.data_bytes(obj, exe, listing)[0]


VARIANTS = ((16, True), (1000, False), (1000, True), (4000, False), (4000, True))


def main() -> int:
    bad = 0
    for kind in ("c", "nib"):
        base = measure(kind, 16, False)
        cells = {f"{kind} a[{n}] init={init}": measure(kind, n, init) for n, init in VARIANTS}
        wrong = differences(base, cells)
        print(f"{kind}: {len(cells) - len(wrong)}/{len(cells)} equal", *wrong, sep="\n  " if wrong else "")
        bad += len(wrong)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
