#!/usr/bin/env python3
"""Our x86-64 SysV `.eh_frame` against our own prologue, no pin: from the entry to the first `ret` the stack depth, the frame register and the
stores of saved registers are tracked, and each row of the function's unwind table must say what is true at its address (the CFA register and
offset, and the cell of each register it says is saved); after the prologue the rows must name every callee-saved register the code pushed.

    eh_frame_check.py OBJ... [--tools DIR]     llvm-objdump in DIR (default ~/scratch/llvm-build-aarch64/bin)
"""
import argparse
import re
import subprocess
import sys
from pathlib import Path

RSP, RBP = 7, 6
NUMBER = {"rax": 0, "rdx": 1, "rcx": 2, "rbx": 3, "rsi": 4, "rdi": 5, "rbp": 6, "rsp": 7, **{f"r{n}": n for n in range(8, 16)}}
CALLEE_SAVED = {3, 6, 12, 13, 14, 15}
SKIPPED = "not modelled: "


def code_of(disassembly: str) -> dict[str, list[tuple[int, str]]]:
    """Each function's instructions as (offset, AT&T text) from `llvm-objdump -d --no-show-raw-insn`."""
    out: dict[str, list[tuple[int, str]]] = {}
    current = None
    for line in disassembly.splitlines():
        if head := re.match(r"^[0-9a-f]+ <(.+)>:$", line):
            current = out.setdefault(head.group(1), [])
        elif current is not None and (row := re.match(r"^\s*([0-9a-f]+):\s*\t(.*)$", line)):
            current.append((int(row.group(1), 16), re.sub(r"\s+", " ", row.group(2).split("#")[0].strip())))
    return out


def _imm(text: str) -> int:
    return int(text.lstrip("$"), 0)


def check(code: list[tuple[int, str]], rows: list[tuple]) -> list[str]:
    """The problems of one function: `code` its (offset, text), `rows` the (pc, CFA register, CFA offset, {register: CFA-relative cell}) of its FDE."""
    problems = []
    # the rows count from the function's start; the disassembly from the section's
    code = [(off - code[0][0], text) for off, text in code]
    ret_at = None  # the first return: the code after it is another path the model does not follow
    depth = 8  # the CFA minus the stack pointer: the return address is the first cell
    cfa_reg, frame_off = RSP, 0
    saved_cells: dict[int, int] = {}  # callee-saved register -> CFA-relative cell the code stored it in
    # the state in force from each instruction's end on: (address, CFA register, CFA offset, cells)
    states: list[tuple[int, int, int, dict[int, int]]] = [(code[0][0], RSP, 8, {})]
    prologue = True  # until the first instruction that is not a push, a frame set-up or a store to the stack
    for i, (off, text) in enumerate(code):
        if prologue and not re.match(r"(push[qlw]? %\w+|mov[qlw]? %rsp, %rbp|sub[qlw]? \$\S+, %rsp|mov[qlw]? %\w+, (-?0x[0-9a-f]+|-?\d+)?\(%r(sp|bp)\))$", text):
            prologue = False
        # A stack adjustment in the body is an argument area for a call; whether the callee pops it (`ret $N`) or the caller does is not in this
        # function's code, so the depth after the call is unknown: such a function is not modelled, not judged.
        if not prologue and re.match(r"(push[qlw]? %\w+|sub[qlw]? \$\S+, %rsp)$", text):
            return [f"{SKIPPED}stack arguments pushed in the body at {off:#x}"]
        if text.startswith("ret"):
            ret_at = off
            break
        if m := re.match(r"push[qlw]? %(\w+)$", text):
            depth += 8
            if (number := NUMBER.get(m.group(1))) in CALLEE_SAVED and prologue:
                saved_cells.setdefault(number, -depth)
        elif re.match(r"mov[qlw]? %rsp, %rbp$", text):
            cfa_reg, frame_off = RBP, depth
        elif m := re.match(r"sub[qlw]? (\$\S+), %rsp$", text):
            depth += _imm(m.group(1))
        elif m := re.match(r"(?:add|lea)[qlw]? (\$?\S+), %rsp$", text):
            depth -= _imm(m.group(1).replace("(%rsp)", ""))
        elif re.match(r"pop[qlw]? %\w+$", text):
            depth -= 8
        elif text == "leave":
            cfa_reg, depth = RSP, 8
        elif m := re.match(r"mov[qlw]? %(\w+), (-?0x[0-9a-f]+|-?\d+)?\(%rsp\)$", text):
            if (number := NUMBER.get(m.group(1))) in CALLEE_SAVED and prologue:
                saved_cells.setdefault(number, int(m.group(2) or "0", 0) - depth)
        if i + 1 < len(code):
            states.append((code[i + 1][0], cfa_reg, depth if cfa_reg == RSP else frame_off, dict(saved_cells)))
    for pc, reg, offset, saved in rows:
        if ret_at is not None and pc > ret_at:
            continue
        before = [s for s in states if s[0] <= pc]
        if not before:
            continue
        _, want_reg, want_off, want_cells = before[-1]
        if (reg, offset) != (want_reg, want_off):
            problems.append(f"at {pc:#x} the rows say CFA = r{reg}+{offset} but the code makes it r{want_reg}+{want_off}")
        for number, cell in saved.items():
            if number in want_cells and want_cells[number] != cell:
                problems.append(f"at {pc:#x} register {number} is said to be at CFA{cell:+d} but the code stored it at CFA{want_cells[number]:+d}")
    if rows:
        for number, cell in saved_cells.items():
            if number not in rows[-1][3] and not any(number in r[3] for r in rows):
                problems.append(f"register {number}, stored at CFA{cell:+d}, is in no row")
    return problems


def rows_of(path: str) -> dict[str, list[tuple]]:
    """function -> rows (pc, CFA register, CFA offset, {register: cell}) of its FDE, offsets from the function's start."""
    from elftools.dwarf.callframe import FDE  # noqa: PLC0415
    from elftools.elf.elffile import ELFFile  # noqa: PLC0415

    with open(path, "rb") as handle:
        elf = ELFFile(handle)
        if elf.get_section_by_name(".eh_frame") is None:
            return {}
        starts = {s["st_value"]: s.name for s in elf.get_section_by_name(".symtab").iter_symbols() if s["st_info"]["type"] == "STT_FUNC"}
        out: dict[str, list[tuple]] = {}
        for entry in elf.get_dwarf_info().EH_CFI_entries() or []:
            if isinstance(entry, FDE) and (name := starts.get(entry["initial_location"])) is not None:
                begin = entry["initial_location"]
                out[name] = [
                    (row["pc"] - begin, row["cfa"].reg, row["cfa"].offset, {r: rule.arg for r, rule in row.items() if isinstance(r, int) and rule.type == "OFFSET" and r != 16})
                    for row in entry.get_decoded().table
                ]
    return out


def patched(data: bytes, section: tuple[int, int]) -> bytes | None:
    """`data` with the operand of the first `DW_CFA_def_cfa_offset` of the first FDE that has one made 8 bytes larger, or None where no FDE has one:
    the compiler's own rows with one wrong, the control that the checker fails on real output. `section` is the .eh_frame (file offset, size)."""
    start, size = section
    at = start
    while at + 8 <= start + size:
        length = int.from_bytes(data[at : at + 4], "little")
        if length == 0:
            break
        entry_end = at + 4 + length
        if int.from_bytes(data[at + 4 : at + 8], "little") != 0:  # an FDE: pc begin, range (4 each), then the augmentation length
            code = at + 8 + 8 + 1 + data[at + 16]
            while code < entry_end:
                op = data[code]
                if op == 0x0E and data[code + 1] < 0x70:
                    out = bytearray(data)
                    out[code + 1] += 8
                    return bytes(out)
                # the opcodes the compiler's rows use, and their operand bytes (small values: one byte each)
                code += 1 + {0x0E: 1, 0x0D: 1, 0x0C: 2, 0x02: 1, 0x03: 2, 0x04: 4}.get(op, 1 if op & 0xC0 == 0x80 else 0)
        at = entry_end
    return None


def control(obj: str, tools: str) -> bool:
    """Whether the checker flags `obj` once its first CFA offset is made wrong; False where the object has none to make wrong."""
    from elftools.elf.elffile import ELFFile  # noqa: PLC0415
    import tempfile  # noqa: PLC0415

    with open(obj, "rb") as handle:
        data = handle.read()
        handle.seek(0)
        section = ELFFile(handle).get_section_by_name(".eh_frame")
        where = (section["sh_offset"], section["sh_size"])
    wrong = patched(data, where)
    if wrong is None:
        return False
    with tempfile.NamedTemporaryFile(suffix=".o") as copy:
        copy.write(wrong)
        copy.flush()
        code = code_of(subprocess.run([f"{tools}/llvm-objdump", "-d", "--no-show-raw-insn", copy.name], capture_output=True, text=True, check=True).stdout)
        return any(check(code[name], rows) for name, rows in rows_of(copy.name).items() if code.get(name))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("objects", nargs="+")
    parser.add_argument("--control", action="store_true", help="the real-output control: make one CFA offset wrong in each object and require the checker to flag it")
    parser.add_argument("--tools", default=str(Path.home() / "scratch/llvm-build-aarch64/bin"))
    args = parser.parse_args()
    if args.control:
        made = [obj for obj in args.objects if (flagged := control(obj, args.tools)) is not None and flagged]
        print(f"eh_frame_check --control: {len(made)} of {len(args.objects)} objects flagged after a CFA offset was made wrong")
        return 0 if made else 3
    functions = bad = skipped = 0
    for obj in args.objects:
        code = code_of(subprocess.run([f"{args.tools}/llvm-objdump", "-d", "--no-show-raw-insn", obj], capture_output=True, text=True, check=True).stdout)
        tables = rows_of(obj)
        for name, rows in sorted(tables.items()):
            if name not in code or not code[name]:
                continue
            functions += 1
            problems = check(code[name], rows)
            if problems and problems[0].startswith(SKIPPED):
                functions -= 1
                skipped += 1
            elif problems:
                bad += 1
                print(f"{obj}: {name}: " + "; ".join(problems[:3]))
    print(f"eh_frame_check: {functions} functions, {bad} with rows that contradict the code, {skipped} not modelled")
    return 2 if functions == 0 else (1 if bad else 0)


if __name__ == "__main__":
    sys.exit(main())
