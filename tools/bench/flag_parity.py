"""Flag parity of ours vs each m16 reference compiler (the bench's own compile lines), one verdict per reference and level.

Each compiler's command line is reduced to the codegen profile it asks for: memory model, instruction set, optimisation class, inline policy of the
kernel, x87 use, stack checks. `parity(ours, reference)` names the fields that differ and whether each is a documented difference (the best
instruction set a reference compiler has, the reference's level map: DOCUMENTED). A field that differs and is not documented REFUSES the cell,
as tools/wasm_flag_check.py does for wasm (the arm64 matrix lost 24 of 72 cells to a flag on one side only).

usage: flag_parity.py   (prints the table for the bench's current command lines)"""
import re
import sys

# What a difference may be without refusing the cell: the reference's ISA is the best it has (ow -4 and bcc -3 are not i386, tc -1 and tcpp -2 are
# 16-bit ones), and the references have no separate light -O1 (BORLAND's level map, build_watcom's flags).
DOCUMENTED = {
    "isa": "the reference's best instruction set (llrm -march=i386)",
    "opt": "the references' level map: O1 and Os are their size level",
    "packing": "the reference's default struct packing (Borland byte, Watcom and llrm word): the bench sets none, it changes data bytes",
}


def ours_profile(flags: list[str], physical: tuple[str, ...] = ()) -> dict:
    level = next((one[1:] for one in flags if one in ("-O0", "-O1", "-O2", "-Os")), "O2")
    march = next((one.split("=", 1)[1] for one in flags if one.startswith("-march=")), "i386")
    return {"model": "medium", "isa": march, "opt": {"O2": "speed", "Os": "size"}.get(level, level), "kernel_call": "-fno-inline-functions" in flags or "kept", "fp": "x87", "stack_checks": False, "packing": 2, "defines": frozenset(physical)}


def inlines(argv: list[str]) -> bool:
    """Watcom's `-ox` is `-obmiler -s` (the `e` is inline expansion): on unless `-oe=0`; `-os -ol` has no `e`."""
    return any(one.startswith("-oe") and one != "-oe=0" for one in argv) or ("-ox" in argv and "-oe=0" not in argv)


def ow_profile(argv: list[str]) -> dict:
    isa = next((one for one in argv if re.fullmatch(r"-[0-6]", one)), "-0")
    size = "-os" in argv
    return {"model": {"-mm": "medium", "-ml": "large", "-ms": "small", "-mc": "compact", "-mh": "huge"}.get(next((one for one in argv if re.fullmatch(r"-m[smclh]", one)), ""), "?"),
            "isa": {"-4": "486", "-3": "386", "-2": "286", "-1": "186", "-0": "86"}[isa], "opt": "size" if size else "speed" if "-ox" in argv else "none",
            "kernel_call": not inlines(argv), "fp": "x87" if "-fpi87" in argv else "?", "stack_checks": "-s" not in argv,
            "packing": next((int(one[3:]) for one in argv if re.fullmatch(r"-zp\d+", one)), 2), "defines": frozenset(one for one in argv if one.startswith("-DPHYSICAL_"))}


def borland_profile(compiler: str, switches: list[str]) -> dict:
    """`bcc -c -mm -3 -f87 [-Ox] SWITCHES` (bcc.sh) or `tcc -c -mm -f87 SWITCHES`."""
    full = ["-c", "-mm", "-f87", *(["-3"] if compiler == "bcc" else []), *([] if compiler != "bcc" or any(one.startswith("-O") for one in switches) else ["-Ox"]), *switches]
    isa = {"-3": "386", "-2": "286", "-1": "186"}.get(next((one for one in full if one in ("-1", "-2", "-3")), ""), "86")
    speed = any(one in ("-Ox", "-G") for one in full)
    return {"model": {"-mm": "medium", "-ml": "large", "-ms": "small"}.get(next((one for one in full if re.fullmatch(r"-m[sml]", one)), ""), "?"), "isa": isa,
            "opt": "speed" if speed else "size", "kernel_call": None, "fp": "x87" if "-f87" in full else "?", "stack_checks": "-N" in full,
            "packing": 2 if "-a" in full else 1, "defines": frozenset(one for one in full if one.startswith("-DPHYSICAL_"))}


def basic_ours_profile(flags: list[str]) -> dict:  # BASIC programs read no physical address
    return {**ours_profile(flags), "model": None, "kernel_call": None, "stack_checks": None, "packing": None, "defines": None}


def basic_profile(switches: list[str]) -> dict:
    """BC, PDS 7.1 and VBDOS: `/O` stand-alone, `/FPi` inline 8087, no `/G2` given (8086 code), one optimisation level."""
    return {"model": None, "isa": "86", "opt": "speed", "kernel_call": None, "fp": "x87" if "/FPi" in switches else "?", "stack_checks": None, "packing": None, "defines": None}


def parity(ours: dict, reference: dict) -> tuple[list[str], list[str]]:
    """(refusing differences, documented ones): fields on which the two profiles differ. `kernel_call` None is a compiler with no such switch (the harness keeps
    the call by the volatile pointer, which every reference compiles the same)."""
    refused, documented = [], []
    for field in ours:
        if reference.get(field) is None or ours[field] == reference[field]:
            continue
        (documented if field in DOCUMENTED else refused).append(f"{field}: ours {ours[field]}, reference {reference[field]}")
    return refused, documented


def refuse(ours: dict, reference: dict, cell: str) -> None:
    refused, _ = parity(ours, reference)
    if refused:
        raise SystemExit(f"flag_parity: {cell}: codegen flags differ ({'; '.join(refused)})")


def table() -> str:
    sys.path.insert(0, str(__import__("pathlib").Path(__file__).parent))
    import bench

    rows, physical = [], tuple(bench.physical_defines())
    for level in ("O1", "O2", "Os"):
        mine = ours_profile([f"-{level}", "-fno-inline-functions", "-march=i386"], physical)
        ow_flags = ["-ox", "-oe=0"] if level == "O2" else ["-os", "-ol"]
        for name, profile in (("ow", ow_profile(["-zq", "-mm", "-ecc", "-s", "-DOWREF", "-4", "-fpi87", *physical, *ow_flags])), *((n, borland_profile(c, [*bench.BORLAND[n][2][level].split(), *physical])) for n, c in (("bcc", "bcc"), ("tc", "tcc"), ("tcpp", "tcc")))):
            refused, documented = parity(mine, profile)
            rows.append(f"{level}\t{name}\t{'REFUSED ' + '; '.join(refused) if refused else 'same' if not documented else 'documented: ' + '; '.join(documented)}")
    for level in ("O1", "O2", "Os"):
        mine = basic_ours_profile([f"-{level}", "-march=i386"])
        refused, documented = parity(mine, basic_profile("/O /FPi".split()))
        rows.append(f"{level}\tbc/pds71/vbdos\t{'REFUSED ' + '; '.join(refused) if refused else 'same' if not documented else 'documented: ' + '; '.join(documented)}")
    return "\n".join(rows)


def linkers() -> dict[str, str]:
    """The linker that produced each column's executables and its md5, read from the files the bench runs (not inferred from a map's format): C and
    Nib columns, ours and ow/bcc/tc/tcpp, are linked by jwlink (dosbatch.link_*); a BASIC column by the LINK.EXE of its own toolchain in DOSBox."""
    import hashlib

    sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parents[1] / "dosbatch"))
    import dosbatch

    def md5(path) -> str:
        return hashlib.md5(path.read_bytes()).hexdigest()[:12] if path.exists() else "absent"

    return {
        "c,nib,ow,bcc,tc,tcpp": f"jwlink {md5(dosbatch.BIN / 'jwlink')}",
        "bas,bq16,bc": f"qb45 LINK.EXE {md5(dosbatch.QB45 / 'LINK.EXE')}",
        "pds71": f"pds71 LINK.EXE {md5(dosbatch.PDS71 / 'BINB' / 'LINK.EXE')}",
        "vbdos": f"vbdos LINK.EXE {md5(dosbatch.VBDOS / 'BIN' / 'LINK.EXE')}",
    }


def check() -> None:
    """Print the table to stderr and refuse the run when any cell is REFUSED."""
    text = table()
    print("flag parity (ours vs reference, per level):\n" + text + "\nlinkers: " + "; ".join(f"{columns}: {one}" for columns, one in linkers().items()), file=sys.stderr)
    if "REFUSED" in text:
        raise SystemExit("flag_parity: a reference's codegen flags differ from ours; no references measured")


if __name__ == "__main__":
    print(table())
