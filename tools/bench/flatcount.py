"""
The work of a flat (dos32) program's kernel in an emulator, priced by the 486's clocks: the program's image, relinked as an ELF by
`dosbatch.link_target(image=)`, runs from its entry in unicorn, with the DPMI and DOS calls its start-up makes answered, and from the
kernel's entry to its return the instructions, the memory operands and the clocks of vsgcc's table (the one `icount` prices the 16-bit
programs by) are counted. DOSBox counts the same kernel's instructions but has no price; the two counts agree, and the caller says so
where they do not.

A call it does not know stops the run and is reported, never guessed.
"""

from __future__ import annotations

import re
import struct
import sys
from pathlib import Path

from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_HOOK_INTR, UC_MODE_32, Uc, UcError
from unicorn.x86_const import (
    UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ES,
    UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_FS, UC_X86_REG_GDTR, UC_X86_REG_GS, UC_X86_REG_SS,
)
from iced_x86 import Decoder, Mnemonic

for _bench in sorted((Path(__file__).resolve().parents[2] / "crates/target").glob("*/bench")):  # the targets' emulators (icount)
    sys.path.insert(0, str(_bench))
sys.path.insert(0, str(next((Path(__file__).resolve().parents[2] / "crates" / "target").glob("*/vsgcc"))))

import icount  # noqa: E402

TOP = 0x0A000000  # the emulated memory: the image sits at 0x08048000, the heap above it, the stack below the image
HEAP = 0x08800000
STACK = 0x07F00000  # 1 MiB below the image base. Only the order matters: in DOSBox under DOS/32A the stack (phys 0x1817ac-0x181a7c) is below the image's temp-string table, so a pointer test against it takes the same path here
GDT = 0x9000
FLAT_CODE, FLAT_DATA = 0x00CF9A000000FFFF, 0x00CF92000000FFFF  # base 0, 4 GiB, 32-bit: the stack is ESP, not SP


WORD = (1 << 16) - 1  # a DPMI call's 16-bit register halves
LIMIT = 400_000_000  # instructions the emulator runs before it gives up: a kernel of 10^9 (grep over 10 MB) is not priced
AGREE = 0.005  # the emulator's count and DOSBox's differ by this much of the count or less, or 64 instructions


DOSBOX_REP_SLICE = 4  # DOSBox counts a repeated string instruction once, and once more for every 4 iterations (measured on 6 kernels, 3,950 to 149,800)


def agrees(counted: int, dosbox: int, rep_iterations: int = 0) -> bool:
    """DOSBox's count is the emulator's once the iterations of the kernel's repeated string instructions (`Run.rep_iterations`) are counted as DOSBox
    does. Without the allowance a kernel with a `rep stosd` over a page (matmul, ring, sieve, bintree, huge on head: the loop idioms made them)
    read as another run's."""
    expected = counted + rep_iterations // DOSBOX_REP_SLICE
    return abs(expected - dosbox) <= max(64, AGREE * dosbox)


def symbol(image: Path, name: str) -> int | None:
    """The address of `name`, from the link map beside `image` (the ELF has no symbol table)."""
    found = re.search(rf"^([0-9a-f]{{8}})\*?\s+{re.escape(name)}\s*$", image.with_suffix(".map").read_text(), re.M | re.I)
    return int(found.group(1), 16) if found else None


def _segments(image: Path) -> list[tuple[int, bytes, int]]:
    """(address, bytes, memory size) of each loadable segment of a 32-bit little-endian ELF."""
    data = image.read_bytes()
    (phoff,) = struct.unpack_from("<I", data, 28)
    phentsize, phnum = struct.unpack_from("<HH", data, 42)
    out = []
    for at in range(phnum):
        kind, offset, vaddr, _, filesz, memsz, _, _ = struct.unpack_from("<8I", data, phoff + at * phentsize)
        if kind == 1:
            out.append((vaddr, data[offset : offset + filesz], memsz))
    return out


def layout_errors(image: Path) -> list[str]:
    """What is wrong with the file layout of an ELF `jwlink` wrote: a loadable segment that starts before the one below it ends, or a table
    (relocations, symbols, strings) that starts before the last segment's bytes end, so that the tables are written over the program. A frames
    image linked with 5 LOADs (a 4 KiB gap before CONST2) had its relocation tables at 0x102f, inside code that runs to 0x19f5: the emulator ran
    the relocation records, `Invalid memory read`, 21 of 39 bq32 rows with no clocks."""
    data = image.read_bytes()
    (phoff,) = struct.unpack_from("<I", data, 28)
    phentsize, phnum = struct.unpack_from("<HH", data, 42)
    loads = [struct.unpack_from("<8I", data, phoff + at * phentsize) for at in range(phnum)]
    loads = [(offset, filesz) for kind, offset, _, _, filesz, *_ in loads if kind == 1]
    errors = []
    end = 0
    for offset, filesz in loads:
        if offset < end:
            errors.append(f"a loadable segment at file offset {offset:#x} starts inside the one before it, which ends at {end:#x}")
        end = max(end, offset + filesz)
    (shoff,) = struct.unpack_from("<I", data, 32)
    shentsize, shnum = struct.unpack_from("<HH", data, 46)
    for at in range(shnum if shoff else 0):
        _, kind, _, addr, offset, size = struct.unpack_from("<6I", data, shoff + at * shentsize)
        if addr == 0 and size and kind not in (0, 8) and offset < end:
            errors.append(f"section {at} (type {kind}) is written at file offset {offset:#x}, over the program that ends at {end:#x}")
    return errors


def run(image: Path, region: str, cwd: Path, limit: int = LIMIT) -> icount.Run:
    """`image` from its entry; the counts are of the function `region` (a symbol) and what it calls. Files the program opens live in `cwd`."""
    result = icount.Run()
    if layout := layout_errors(image):
        result.error = "jwlink wrote a corrupt ELF: " + "; ".join(layout)
        return result
    entry_of = symbol(image, region)
    if entry_of is None:
        result.error = f"{region} is not in the image's symbols"
        return result
    data = image.read_bytes()
    (entry,) = struct.unpack_from("<I", data, 24)
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(0, TOP)
    segments = _segments(image)
    if segments and STACK >= min(address for address, _, _ in segments):
        result.error = f"the image starts at {min(address for address, _, _ in segments):#x}, not above the stack at {STACK:#x}: the layout is not the extender's"
        return result
    for address, blob, _ in segments:
        uc.mem_write(address, blob)
    uc.mem_write(GDT, struct.pack("<QQQ", 0, FLAT_CODE, FLAT_DATA))
    uc.reg_write(UC_X86_REG_GDTR, (0, GDT, 23, 0))
    for register in (UC_X86_REG_DS, UC_X86_REG_ES, UC_X86_REG_SS, UC_X86_REG_FS, UC_X86_REG_GS):
        uc.reg_write(register, 0x10)
    uc.reg_write(UC_X86_REG_CS, 0x08)
    uc.reg_write(UC_X86_REG_ESP, STACK)

    state = {"heap": HEAP, "ret": None, "entry_sp": 0, "done": False, "before": None, "first": True, "files": {}, "next": 5}
    counts = icount.Counts()
    price, multiplier_of, _prefixes = icount._prices()
    instructions: dict[int, tuple] = {}
    read, write = uc.reg_read, uc.reg_write

    def cstring(at):
        out = b""
        while (byte := bytes(uc.mem_read(at + len(out), 1))) != b"\0":
            out += byte
        return out.decode("latin1")

    def carry(on):
        flags = read(UC_X86_REG_EFLAGS)
        write(UC_X86_REG_EFLAGS, (flags | 1) if on else (flags & ~1))

    def interrupt(uc, number, _):
        ax = read(UC_X86_REG_EAX) & WORD
        if number == 0x31 and ax in (0x0201, 0x0203, 0x0205, 0x0900, 0x0901, 0x0902):
            carry(False)  # DPMI vectors, the interrupt flag: accepted
        elif number == 0x31 and ax == 0x0204:
            write(UC_X86_REG_ECX, 0x08)
            write(UC_X86_REG_EDX, 0)
            carry(False)
        elif number == 0x31 and ax == 0x0501:
            size = ((read(UC_X86_REG_EBX) & WORD) << 16) | (read(UC_X86_REG_ECX) & WORD)
            at = state["heap"]
            state["heap"] += (size + 0xFFF) & ~0xFFF
            write(UC_X86_REG_EBX, at >> 16)
            write(UC_X86_REG_ECX, at & WORD)
            write(UC_X86_REG_ESI, 0)
            write(UC_X86_REG_EDI, 1)
            carry(False)
        elif number == 0x21 and ax >> 8 == 0x4C:
            result.exit_code = ax & 0xFF
            uc.emu_stop()
        elif number == 0x21 and ax >> 8 == 0x40:
            handle, count, at = read(UC_X86_REG_EBX), read(UC_X86_REG_ECX), read(UC_X86_REG_EDX)
            if handle in (1, 2):
                result.output += bytes(uc.mem_read(at, count))
            write(UC_X86_REG_EAX, count)
            carry(False)
        elif number == 0x21 and ax >> 8 in (0x3C, 0x3D):
            name = cstring(read(UC_X86_REG_EDX))
            try:
                handle = state["next"]
                state["files"][handle] = open(cwd / name, "w+b" if ax >> 8 == 0x3C else "r+b")
                state["next"] += 1
                write(UC_X86_REG_EAX, handle)
                carry(False)
            except OSError:
                write(UC_X86_REG_EAX, 2)
                carry(True)
        elif number == 0x21 and ax >> 8 == 0x3F:
            handle, count = read(UC_X86_REG_EBX), read(UC_X86_REG_ECX)
            got = state["files"][handle].read(count) if handle in state["files"] else b""
            uc.mem_write(read(UC_X86_REG_EDX), got)
            write(UC_X86_REG_EAX, len(got))
            carry(False)
        elif number == 0x21 and ax >> 8 == 0x3E:
            file = state["files"].pop(read(UC_X86_REG_EBX), None)
            file and file.close()
            carry(False)
        elif number == 0x21 and ax >> 8 == 0x42:
            file = state["files"].get(read(UC_X86_REG_EBX))
            offset = (read(UC_X86_REG_ECX) << 16 | read(UC_X86_REG_EDX) & WORD) & ((1 << 32) - 1)
            where = file.seek(offset, ax & 0xFF) if file else 0
            write(UC_X86_REG_EDX, where >> 16)
            write(UC_X86_REG_EAX, where & WORD)
            carry(False)
        elif number == 0x21 and ax >> 8 == 0x44 and ax & 0xFF == 0:
            write(UC_X86_REG_EDX, 0)  # every handle a file
            carry(False)
        elif number == 0x21 and ax >> 8 == 0x30:
            write(UC_X86_REG_EAX, 0x0005)
        else:
            result.error = f"int {number:#x} ax={ax:#x}"
            uc.emu_stop()

    def step(uc, address, size, _):
        if state["ret"] is None:
            if address != entry_of or state["done"]:
                return
            state["ret"] = struct.unpack("<I", bytes(uc.mem_read(read(UC_X86_REG_ESP), 4)))[0]
            state["entry_sp"] = read(UC_X86_REG_ESP)
        elif address == state["ret"] and read(UC_X86_REG_ESP) > state["entry_sp"]:
            before = state["before"]
            if before is not None:
                counts.clocks += price(before[1], True, state["first"], before[2], before[3], before[4])  # the `ret`
            state["before"], state["done"] = None, True
            uc.emu_stop()
            return
        info = instructions.get(address)
        if info is None:
            raw = bytes(uc.mem_read(address, min(size, 15)))
            one = Decoder(32, raw, ip=address).decode()
            memory = 0 if one.mnemonic == Mnemonic.LEA else sum(1 for at in range(one.op_count) if one.op_kind(at) in icount.MEMORY_KINDS)
            repeated = one.has_rep_prefix or one.has_repe_prefix or one.has_repne_prefix
            info = instructions[address] = (one, memory, repeated, _prefixes(raw))
        one, memory, repeated, prefixes = info
        before = state["before"]
        if repeated and read(UC_X86_REG_ECX) == 0 and before is not None and before[0] == address:
            return  # the last firing of a repeated string instruction that ran, with ECX at 0, does nothing (one with ECX 0 to start with is counted once)
        if before is not None:
            counts.clocks += price(before[1], address != before[0] + before[1].len, state["first"], before[2], before[3], before[4])
        state["first"] = not (repeated and before is not None and before[0] == address)
        factor = icount._multiplier(multiplier_of, uc, one) if one.mnemonic in (Mnemonic.MUL, Mnemonic.IMUL) else None
        state["before"] = (address, one, memory, factor, prefixes)
        # DOSBox counts a repeated string instruction once, the hook here fires once per iteration: the count is the first firing's, the
        # clocks (the table's first-iteration and per-iteration prices) are every firing's.
        if not (repeated and before is not None and before[0] == address):
            counts.instructions += 1
            counts.memory_operands += memory
        else:
            result.rep_iterations += 1

    uc.hook_add(UC_HOOK_INTR, interrupt)
    uc.hook_add(UC_HOOK_CODE, step)
    try:
        uc.emu_start(entry, 0, count=limit)
    except UcError as error:
        result.error = result.error or f"emulator: {error} at eip {uc.reg_read(UC_X86_REG_EIP):#x}"
    if state["done"]:
        result.region = counts
    elif not result.error:
        entered = state["ret"] is not None
        if result.exit_code is not None:
            result.error = "the program exited " + ("before the kernel returned" if entered else "without entering the kernel")
        elif entered:
            result.error = f"the kernel had not returned at the emulator's limit of {limit} instructions"
        else:
            result.error = "the region was never entered"
    return result
