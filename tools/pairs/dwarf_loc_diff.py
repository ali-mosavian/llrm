#!/usr/bin/env python3
"""Variable locations, ours against the pin's, on one IR.

Each C file goes through the pin clang and `opt -O2` once; the pin `llc` and `llrm-llc` then make an x86-64 object of that same `.ll`. A function whose
code is the same instruction for instruction is compared: its location lists (begin, end, expression) must match. A function whose code differs is
refused, never counted as equal: different code has different places.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def run(command: list[str]) -> str:
    done = subprocess.run(command, capture_output=True, text=True, timeout=600)
    if done.returncode != 0:
        raise RuntimeError((done.stderr.strip().splitlines() or [f"exit {done.returncode}"])[-1][:200])
    return done.stdout


def code_of(disassembly: str) -> dict[str, list[str]]:
    """Each function's instructions as `offset text`, less the comments a relocation adds."""
    out: dict[str, list[str]] = {}
    current: list[str] | None = None
    for line in disassembly.splitlines():
        if (head := re.match(r"^[0-9a-f]+ <(.+)>:$", line)) is not None:
            current = out.setdefault(head.group(1), [])
        elif current is not None and (row := re.match(r"^\s*([0-9a-f]+):\t(.*)$", line)):
            current.append(f"{row.group(1)} {row.group(2).split('#')[0].strip()}")
    return out


def variables_of(path: str) -> dict[tuple[str, str], tuple]:
    """(function, variable) -> ('expr', ops) or ('list', ((begin, end, ops), ...), (function low, high)), read with pyelftools."""
    from elftools.elf.elffile import ELFFile  # noqa: PLC0415

    with open(path, "rb") as handle:
        dwarf = ELFFile(handle).get_dwarf_info()
        lists = dwarf.location_lists()
        out: dict[tuple[str, str], tuple] = {}
        for cu in dwarf.iter_CUs():
            for die in cu.iter_DIEs():
                name, loc = die.attributes.get("DW_AT_name"), die.attributes.get("DW_AT_location")
                if die.tag not in ("DW_TAG_variable", "DW_TAG_formal_parameter") or name is None:
                    continue
                parent, scope = die.get_parent(), ""
                while parent is not None and parent.tag != "DW_TAG_subprogram":
                    parent = parent.get_parent()
                span = (0, 0)
                if parent is not None:
                    scope = parent.attributes["DW_AT_name"].value.decode()
                    low, high = parent.attributes.get("DW_AT_low_pc"), parent.attributes.get("DW_AT_high_pc")
                    if low is not None and high is not None:
                        # high_pc is an address in one writer and a length in the other.
                        span = (low.value, high.value if high.form == "DW_FORM_addr" else low.value + high.value)
                key = (scope, name.value.decode())
                if loc is None:
                    out[key] = ("none",)
                elif loc.form in ("DW_FORM_exprloc", "DW_FORM_block1", "DW_FORM_block"):
                    out[key] = ("expr", tuple(loc.value))
                else:
                    entries = lists.get_location_list_at_offset(loc.value, die) if loc.form == "DW_FORM_sec_offset" else lists.get_location_lists_at_index(loc.value) if hasattr(lists, "get_location_lists_at_index") else lists.get_location_list_at_offset(loc.value, die)
                    out[key] = ("list", tuple((e.begin_offset, e.end_offset, tuple(e.loc_expr)) for e in entries if hasattr(e, "loc_expr")), span)
    return out


GLOBAL_ADDRESS = (3, 161)  # DW_OP_addr and DW_OP_addrx: where the address comes from is the writer's form, not the place


def meaning(found: tuple) -> tuple:
    """A location as what it says: a list of one range over the whole function is its expression, and the operand of a global's address is a form."""
    if found[0] == "list" and len(found[1]) == 1 and found[1][0][:2] == found[2]:
        found = ("expr", found[1][0][2])
    if found[0] == "expr" and found[1] and found[1][0] in GLOBAL_ADDRESS:
        return ("expr", "global")
    return found[:2]


def compare(pin: dict, ours: dict) -> dict[str, list]:
    """Each variable of the pin's object under `same`, `form` (the same place written another way), `different` or `missing` (ours has none)."""
    out: dict[str, list] = {"same": [], "form": [], "different": [], "missing": []}
    for key, found in pin.items():
        mine = ours.get(key, ("none",))
        if mine == found:
            out["same"].append(key)
        elif found[0] == "none":
            continue
        elif mine[0] == "none":
            out["missing"].append(key)
        elif meaning(mine) == meaning(found):
            out["form"].append(key)
        else:
            out["different"].append(key)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--pin", default=os.path.expanduser("~/scratch/llrm-pin-stage/bin"))
    parser.add_argument("--llc", required=True, help="llrm-llc")
    args = parser.parse_args()
    work = Path(tempfile.mkdtemp(prefix="dwarfloc-"))
    total = {"same": 0, "form": 0, "different": 0, "missing": 0, "refused": 0, "files refused": 0}
    for source in map(Path, args.sources):
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", "--target=x86_64-unknown-linux-gnu", "-fno-pic", "-O2", "-g", "-gdwarf-5", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", str(ir), "-S", "-o", str(opt)])
            pin_o, our_o = work / f"{source.stem}.pin.o", work / f"{source.stem}.our.o"
            run([f"{args.pin}/llc", "-O2", "-relocation-model=static", "-dwarf-version=5", "-filetype=obj", str(opt), "-o", str(pin_o)])
            run([args.llc, "-O2", "-g", "-o", str(our_o), str(opt)])
            pin_code, our_code = (code_of(run(["objdump", "-d", "--no-show-raw-insn", str(o)])) for o in (pin_o, our_o))
            pin_vars, our_vars = variables_of(str(pin_o)), variables_of(str(our_o))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            total["files refused"] += 1
            continue
        same_code = {name for name in pin_code if our_code.get(name) == pin_code[name]}
        keep = lambda key: key[0] == "" or key[0] in same_code  # noqa: E731
        found = compare({k: v for k, v in pin_vars.items() if keep(k)}, {k: v for k, v in our_vars.items() if keep(k)})
        refused = [k for k in pin_vars if not keep(k)]
        for kind, keys in found.items():
            total[kind] += len(keys)
            for scope, name in keys:
                if kind in ("different", "missing"):
                    print(f"  {kind} {source.name} {scope}.{name}: pin {pin_vars[(scope, name)][:2]} ours {our_vars.get((scope, name), ('none',))[:2]}")
        total["refused"] += len(refused)
    compared = sum(total[k] for k in ("same", "form", "different", "missing"))
    print(f"dwarf_loc_diff: {compared} variables compared, {total['refused']} refused (function code differs), {total['files refused']} files refused: {total['same']} same, {total['form']} same place in another form, {total['different']} different, {total['missing']} missing in ours")
    return 2 if compared == 0 else (1 if total["different"] + total["missing"] else 0)


if __name__ == "__main__":
    sys.exit(main())
