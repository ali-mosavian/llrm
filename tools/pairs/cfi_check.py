#!/usr/bin/env python3
"""Our arm64 frame rules against our own code, not the pin's.

`cfi_diff.py` compares with the pin only where the code is the same; most functions differ in register and frame choices. Here the code is
followed instead: from the entry to the first `ret`, the stack pointer, the frame register and the stores of the saved registers are tracked,
and each row of the function's unwind table must say what is true at its address. A row's frame address must be the entry stack pointer; every
register it says is saved must be in the cell it says; and after the prologue the rows must name every register the code stored.
"""
import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

from dwarf_loc_diff import run

SP, FP = 31, 29


def code_of(disassembly: str) -> dict[str, list[str]]:
    """Each function's instructions as `offset text` (hex offset), immediates kept: `dwarf_loc_diff.code_of` cuts a line at `#`, which on arm64
    is the start of every immediate, so the stack adjustments were invisible and every frame address read as wrong."""
    out: dict[str, list[str]] = {}
    current: list[str] | None = None
    for line in disassembly.splitlines():
        if (head := re.match(r"^[0-9a-f]+ <(.+)>:$", line)) is not None:
            current = out.setdefault(head.group(1), [])
        elif current is not None and (row := re.match(r"^\s*([0-9a-f]+):\t(.*)$", line)):
            current.append(f"{row.group(1)} {row.group(2).split('//')[0].strip()}")
    return out


def reg_number(name: str) -> int | None:
    if name == "sp":
        return SP
    if name[0] == "x" and name[1:].isdigit():
        return int(name[1:])
    if name[0] == "d" and name[1:].isdigit():
        return 64 + int(name[1:])
    return None


def callee_saved(register: int) -> bool:
    """x19..x28, the frame record and d8..d15: what an unwinder must find; a spilled temporary is not its business."""
    return 19 <= register <= 30 or 72 <= register <= 79


def _imm(text: str) -> int:
    return int(text.strip("[]! ").lstrip("#"), 0)


def _operands(rest: str) -> list[str]:
    return [part.strip() for part in re.split(r",\s*(?![^\[]*\])", rest)] if rest else []


def check(code: list[str], rows: list[tuple]) -> list[str]:
    """Problems of `rows` ((pc, cfa register, cfa offset, {register: offset}) from the entry) against `code` (`pc text` lines)."""
    problems: list[str] = []
    sp = 0
    fp: int | None = None
    stored: dict[int, int] = {}
    # `add x16, sp, #off`: the scratch address a big frame's saves go through.
    alias: dict[str, int] = {}
    # `mov x16, #4128` and the like: a stack adjustment too big for an immediate goes through a register.
    constant: dict[str, int] = {}
    by_pc = {row[0]: row for row in rows}
    current = None
    verdict_at = None
    # The rows are offsets from the function's first instruction, the disassembly's are from the section's.
    base = int(code[0].split(" ", 1)[0], 16) if code else 0
    for line in code:
        pc_text, text = line.split(" ", 1)
        pc = int(pc_text, 16) - base
        mnemonic, _, rest = text.partition("\t")
        operands = _operands(rest)
        if verdict_at is None and mnemonic in {"bl", "blr", "ret"}:
            verdict_at = pc
        if pc in by_pc:
            current = by_pc[pc]
        if current is not None:
            _, cfa_reg, cfa_off, saved = current
            if cfa_reg == SP and cfa_off != -sp:
                problems.append(f"pc {pc}: frame address sp+{cfa_off}, but sp is {sp} from the entry")
            if cfa_reg == FP and (fp is None or cfa_off != -fp):
                problems.append(f"pc {pc}: frame address x29+{cfa_off}, but x29 is {fp} from the entry")
            for register, offset in saved.items():
                if stored.get(register) != offset:
                    problems.append(f"pc {pc}: register {register} said saved at cfa{offset:+}, stored at {stored.get(register)}")
        kept = {register for register in stored if callee_saved(register)}
        if pc == verdict_at and current is not None and set(current[3]) != kept:
            problems.append(f"pc {pc}: after the prologue the rows save {sorted(current[3])}, the code stored {sorted(kept)}")
        # Past the first unconditional exit the straight line is not the prologue's any more (a block laid out after the epilogue).
        if mnemonic in {"ret", "b", "br"}:
            break
        memory = next((op for op in operands if op.startswith("[")), None)
        pre = bool(memory) and memory.endswith("]!")
        post = bool(memory) and memory.endswith("]") and len(operands) >= 2 and operands[-1].startswith("#") and operands[-2] == memory
        base_register = memory.strip("[]!").split(",")[0].strip() if memory else ""
        on_sp = base_register == "sp" or base_register in alias
        parts = memory.strip("[]!").split(",") if memory else []
        # `[sp, x10]` or `[x0, x1, lsl #2]`: no immediate to follow.
        offset = _imm(parts[1]) if len(parts) > 1 and parts[1].strip().startswith("#") else 0
        count = 2 if mnemonic in {"stp", "ldp"} else 1
        registers = [reg_number(op) for op in operands[:count]]
        if mnemonic in {"stp", "str"} and on_sp:
            if pre:
                sp += offset
                offset = 0
            at = (sp if base_register == "sp" else alias[base_register]) + offset
            for index, register in enumerate(registers):
                if register is not None:
                    stored[register] = at + 8 * index
            if post:
                sp += _imm(operands[-1])
        elif mnemonic in {"ldp", "ldr"} and on_sp:
            for register in registers:
                stored.pop(register, None)
            if post:
                sp += _imm(operands[-1])
            elif pre:
                sp += offset
        elif mnemonic == "mov" and len(operands) == 2 and operands[1].startswith("#"):
            constant[operands[0]] = _imm(operands[1])
        elif mnemonic in {"sub", "add"} and operands[:2] == ["sp", "sp"] and operands[2] in constant:
            sp += (-1 if mnemonic == "sub" else 1) * constant[operands[2]]
        elif mnemonic == "add" and operands[1:2] == ["sp"] and operands[0] not in {"sp", "x29"} and operands[2] in constant:
            alias[operands[0]] = sp + constant[operands[2]]
        elif mnemonic == "add" and operands[:2] == ["x29", "sp"] and operands[2] in constant:
            fp = sp + constant[operands[2]]
        elif mnemonic == "add" and operands[1:2] == ["sp"] and operands[0] not in {"sp", "x29"} and operands[2].startswith("#"):
            shift = _imm(operands[3].split("#")[1]) if len(operands) > 3 and operands[3].startswith("lsl") else 0
            alias[operands[0]] = sp + (_imm(operands[2]) << shift)
        elif mnemonic in {"sub", "add"} and operands[:2] == ["sp", "sp"] and operands[2].startswith("#"):
            # `#1, lsl #12` is 4096.
            shift = _imm(operands[3].split("#")[1]) if len(operands) > 3 and operands[3].startswith("lsl") else 0
            sp += (-1 if mnemonic == "sub" else 1) * (_imm(operands[2]) << shift)
        elif mnemonic == "add" and operands[:2] == ["x29", "sp"] and operands[2].startswith("#"):
            fp = sp + _imm(operands[2])
        elif mnemonic == "mov" and operands[:2] == ["x29", "sp"]:
            fp = sp
        elif mnemonic == "mov" and operands[:2] == ["sp", "x29"] and fp is not None:
            sp = fp
        elif mnemonic == "sub" and operands[:2] == ["sp", "x29"] and fp is not None:
            sp = fp - _imm(operands[2])
    return problems


def rows_of(path: str) -> dict[str, list[tuple]]:
    """function -> rows (pc, cfa register, cfa offset, {register: offset}) of its FDE."""
    from elftools.dwarf.callframe import FDE  # noqa: PLC0415
    from elftools.elf.elffile import ELFFile  # noqa: PLC0415

    with open(path, "rb") as handle:
        elf = ELFFile(handle)
        if elf.get_section_by_name(".eh_frame") is None:
            return {}
        starts = {s["st_value"]: s.name for s in elf.get_section_by_name(".symtab").iter_symbols() if s["st_info"]["type"] == "STT_FUNC"}
        out: dict[str, list[tuple]] = {}
        for entry in elf.get_dwarf_info().EH_CFI_entries() or []:
            if not isinstance(entry, FDE):
                continue
            begin = entry["initial_location"]
            name = starts.get(begin)
            if name is None:
                continue
            table = []
            for row in entry.get_decoded().table:
                saved = {reg: rule.arg for reg, rule in row.items() if isinstance(reg, int) and rule.type == "OFFSET"}
                table.append((row["pc"] - begin, row["cfa"].reg, row["cfa"].offset, saved))
            out[name] = table
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"), help="the aarch64 pin LLVM bin directory (default: $LLRM_PIN_BIN)")
    parser.add_argument("--llc", required=True, help="llrm-arm64-llc")
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    work = Path(tempfile.mkdtemp(prefix="cficheck-"))
    functions = bad = refused = 0
    for source in map(Path, args.sources):
        try:
            ir, opt, obj = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll", work / f"{source.stem}.o"
            run([f"{args.pin}/clang", "--target=aarch64-unknown-linux-gnu", "-fno-pic", "-O2", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", "-vectorize-loops=false", "-vectorize-slp=false", str(ir), "-S", "-o", str(opt)])
            run([args.llc, "-O2", "--isel=dag", "-o", str(obj), str(opt)])
            code = code_of(run(["aarch64-linux-gnu-objdump", "-d", "--no-show-raw-insn", str(obj)]))
            tables = rows_of(str(obj))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            refused += 1
            continue
        for name, lines in sorted(code.items()):
            if name not in tables:
                continue
            functions += 1
            problems = check(lines, tables[name])
            if problems:
                bad += 1
                print(f"  {source.name} {name}: " + "; ".join(problems[:3]))
    print(f"cfi_check: {functions} functions, {bad} with rows that contradict the code, {refused} files refused")
    return 2 if functions == 0 else (1 if bad else 0)


if __name__ == "__main__":
    sys.exit(main())
