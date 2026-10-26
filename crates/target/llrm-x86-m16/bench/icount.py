"""
Deterministic work of a DOS program: run its EXE in a real-mode emulator (unicorn) and count, inside
one function and everything it calls, the instructions executed and the memory operands they name.

The numbers do not depend on the host, DOSBox's cycle setting or the time of day, so they gate: a
change that makes a benchmark execute more is a regression, whatever the clock says.

DOS is emulated just far enough for what the three runtimes call: file I/O and exit through INT 21h,
the BIOS tick, video and keyboard as a machine with nothing attached. An interrupt it does not know
stops the run and is reported, never guessed.
"""

from __future__ import annotations

import io
import struct
import sys
from pathlib import Path
from dataclasses import field
from dataclasses import dataclass

from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_HOOK_INTR, UC_MODE_16, Uc, UcError
from unicorn.x86_const import (
    UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_CS, UC_X86_REG_CX, UC_X86_REG_DS, UC_X86_REG_DX, UC_X86_REG_EFLAGS,
    UC_X86_REG_ES, UC_X86_REG_IP, UC_X86_REG_SP, UC_X86_REG_SS,
)
from iced_x86 import Decoder, Mnemonic, OpKind

PSP = 0x1000
LOAD = PSP + 0x10  # the image's first paragraph
MEMORY_KINDS = {OpKind.MEMORY, OpKind.MEMORY_SEG_SI, OpKind.MEMORY_SEG_ESI, OpKind.MEMORY_SEG_RSI, OpKind.MEMORY_ESDI, OpKind.MEMORY_ESEDI, OpKind.MEMORY_ESRDI}
RETURNS = {Mnemonic.RET, Mnemonic.RETF}


@dataclass
class Counts:
    instructions: int = 0
    memory_operands: int = 0
    # The 486's clocks over the same instructions: vsgcc's table (crates/target/llrm-x86-m32/vsgcc/harness.py), the one the
    # m32 comparison with gcc and clang prices by. A branch taken costs more than one not taken, so it is the stream's, not the count's.
    clocks: int = 0


def _prices():
    """vsgcc's `cost`, `multiplier_of` and the prefixes of an instruction's bytes: one table for both targets, not a copy of it."""
    path = str(Path(__file__).resolve().parents[2] / "llrm-x86-m32" / "vsgcc")
    if path not in sys.path:
        sys.path.insert(0, path)
    import harness  # noqa: E402

    return harness.cost, harness.multiplier_of, harness.clocks.prefixes


NEVER_ENTERED = "the region was never entered"


@dataclass
class Run:
    output: bytes = b""
    exit_code: int | None = None
    region: Counts | None = None  # None: the region was never entered
    error: str = ""
    files: dict[str, bytes] = field(default_factory=dict)
    rep_iterations: int = 0  # flat target: the region's repeated string instructions' iterations after the first of each (tools/bench/flatcount.py)


def _multiplier(multiplier_of, uc, one):
    """The multiplier operand's value, or None (the table's least) where vsgcc's reader does not know the operand's form:
    it reads 32-bit forms, and a 16-bit program also multiplies by a memory operand through a segment."""
    try:
        return multiplier_of(uc, one)
    except (KeyError, ValueError):
        return None


def _decode(raw: bytes) -> tuple[int, bool, bool]:
    """(memory operands, is a return, is repeated) of the instruction at the start of `raw`."""
    one = Decoder(16, raw).decode()
    memory = 0 if one.mnemonic == Mnemonic.LEA else sum(1 for at in range(one.op_count) if one.op_kind(at) in MEMORY_KINDS)
    return memory, one.mnemonic in RETURNS, one.has_rep_prefix or one.has_repe_prefix or one.has_repne_prefix


def run(exe: Path, entry: int | None, cwd: Path, limit: int = 400_000_000, watch: set | None = None, only: tuple[int, int] | None = None, whole: bool = False, stdin: bytes | None = None) -> Run:
    """`exe` to its exit; `entry` is the linear address, from the image's first byte, of the function whose
    work is counted (None: count nothing, only run). `only` is a range of such addresses: of the region, what runs outside it
    (the scene around the code under test) is not counted, and the region still ends where `entry` returns. Files the program
    opens live in `cwd`. `whole` counts the program from its first instruction to its exit (the start-up, the runtime and what it calls
    included), `entry` unused; `stdin` is what handle 0 reads."""
    data = exe.read_bytes()
    (_, last, pages, nrel, header, _, _, ss, sp, _, ip, cs, relocations, _) = struct.unpack("<2sHHHHHHHHHHHHH", data[:28])
    size = (pages - 1) * 512 + (last or 512)
    uc = Uc(UC_ARCH_X86, UC_MODE_16)
    uc.mem_map(0, 0x100000)
    uc.mem_write(LOAD * 16, data[header * 16 : size])
    for at in range(nrel):
        offset, segment = struct.unpack("<HH", data[relocations + 4 * at : relocations + 4 * at + 4])
        where = (LOAD + segment) * 16 + offset
        word = struct.unpack("<H", uc.mem_read(where, 2))[0]
        uc.mem_write(where, struct.pack("<H", (word + LOAD) & 0xFFFF))
    uc.mem_write((PSP - 1) * 16, b"Z" + struct.pack("<HH", PSP, 0x9FFF - PSP))  # the one memory block, the last, ours
    uc.mem_write(PSP * 16 + 2, struct.pack("<H", 0x9FFF))
    uc.mem_write(PSP * 16 + 0x2C, struct.pack("<H", 0x0FF0))
    uc.mem_write(PSP * 16 + 0x80, b"\x00\r")
    for register, value in ((UC_X86_REG_DS, PSP), (UC_X86_REG_ES, PSP), (UC_X86_REG_SS, LOAD + ss), (UC_X86_REG_SP, sp), (UC_X86_REG_CS, LOAD + cs), (UC_X86_REG_IP, ip)):
        uc.reg_write(register, value)

    result = Run()
    state = {"active": whole, "done": False, "entry_sp": -1 if whole else 0, "files": {0: io.BytesIO(stdin)} if stdin is not None else {}, "next": 5, "vectors": {}}
    counts = Counts()
    price, multiplier_of, _prefixes = _prices()
    decoded: dict[int, tuple[int, bool, bool]] = {}
    instructions = {}  # address -> iced instruction
    prefixes = {}  # address -> operand- and address-size prefixes it carries
    target = None if entry is None else LOAD * 16 + entry

    def step(uc, address, size, _):
        if watch is not None:
            watch.add(address - LOAD * 16)
        if not state["active"]:
            if address != target or state["done"]:
                return
            state["active"] = True
            state["entry_sp"] = uc.reg_read(UC_X86_REG_SP)
        info = decoded.get(address)
        if info is None:
            info = decoded[address] = _decode(bytes(uc.mem_read(address, min(size, 15))))
        memory, returns, repeated = info
        # A repeated string instruction fires the hook once per iteration, and once more with CX at 0, when it does nothing.
        if repeated and uc.reg_read(UC_X86_REG_CX) == 0:
            return
        if only is not None and not only[0] <= address - LOAD * 16 < only[1]:
            # Outside the code counted: what left it (a call, a return) is charged as taken, and nothing here is.
            before = state.get("before")
            if before is not None:
                counts.clocks += price(before[1], True, state["first"], before[2], before[3], before[4])
                state["before"] = None
            if returns and uc.reg_read(UC_X86_REG_SP) == state["entry_sp"]:
                state["active"] = False
                state["done"] = True
            return
        one = instructions.get(address)
        if one is None:
            raw = bytes(uc.mem_read(address, min(size, 15)))
            one = instructions[address] = Decoder(16, raw, ip=address).decode()
            prefixes[address] = _prefixes(raw)
        # An instruction costs what it did once the next one's address says whether its branch was taken.
        before = state.get("before")
        if before is not None:
            counts.clocks += price(before[1], address != before[0] + before[1].len, state["first"], before[2], before[3], before[4])
        state["first"] = not (repeated and before is not None and before[0] == address)
        # The 486's multiply is data dependent: its multiplier, read before the instruction runs.
        factor = _multiplier(multiplier_of, uc, one) if one.mnemonic in (Mnemonic.MUL, Mnemonic.IMUL) else None
        state["before"] = (address, one, memory, factor, prefixes[address])
        counts.instructions += 1
        counts.memory_operands += memory
        if returns and uc.reg_read(UC_X86_REG_SP) == state["entry_sp"]:
            counts.clocks += price(one, True, True, memory, factor, prefixes[address])
            state["active"] = False
            state["done"] = True

    def cstring(segment, offset):
        out = b""
        while (byte := bytes(uc.mem_read(segment * 16 + offset, 1))) != b"\0":
            out += byte
            offset += 1
        return out.decode("latin1")

    def carry(on):
        flags = uc.reg_read(UC_X86_REG_EFLAGS)
        uc.reg_write(UC_X86_REG_EFLAGS, (flags | 1) if on else (flags & ~1))

    def interrupt(uc, number, _):
        read, write = uc.reg_read, uc.reg_write
        ax = read(UC_X86_REG_AX)
        ah = ax >> 8
        if watch is not None:
            watch.add(("int", number, ax, read(UC_X86_REG_BX), read(UC_X86_REG_CX), read(UC_X86_REG_DX), read(UC_X86_REG_ES)))
        if number in state["vectors"] and number != 0x21:
            # an interrupt the program installed a handler for (BC's floating-point emulator, on INT 34h-3Dh): enter it
            segment, offset = state["vectors"][number]
            sp, ss = read(UC_X86_REG_SP), read(UC_X86_REG_SS)
            frame = struct.pack("<HHH", read(UC_X86_REG_IP) & 0xFFFF, read(UC_X86_REG_CS), read(UC_X86_REG_EFLAGS) & 0xFFFF)
            uc.mem_write(ss * 16 + ((sp - 6) & 0xFFFF), frame)
            write(UC_X86_REG_SP, (sp - 6) & 0xFFFF)
            write(UC_X86_REG_CS, segment)
            write(UC_X86_REG_IP, offset)
        elif number == 0x21:
            if ah == 0x4C:
                result.exit_code = ax & 0xFF
                uc.emu_stop()
            elif ah in (0x3C, 0x3D):
                name = cstring(read(UC_X86_REG_DS), read(UC_X86_REG_DX))
                try:
                    handle = state["next"]
                    state["files"][handle] = open(cwd / name, "w+b" if ah == 0x3C else "r+b")
                    state["next"] += 1
                    write(UC_X86_REG_AX, handle)
                    carry(False)
                except OSError:
                    write(UC_X86_REG_AX, 2)
                    carry(True)
            elif ah == 0x3F:
                handle, count = read(UC_X86_REG_BX), read(UC_X86_REG_CX)
                got = state["files"][handle].read(count) if handle in state["files"] else b""
                uc.mem_write(read(UC_X86_REG_DS) * 16 + read(UC_X86_REG_DX), got)
                write(UC_X86_REG_AX, len(got))
                carry(False)
            elif ah == 0x40:
                handle, count = read(UC_X86_REG_BX), read(UC_X86_REG_CX)
                chunk = bytes(uc.mem_read(read(UC_X86_REG_DS) * 16 + read(UC_X86_REG_DX), count))
                if handle in (1, 2):
                    result.output += chunk
                else:
                    state["files"][handle].write(chunk)
                write(UC_X86_REG_AX, count)
                carry(False)
            elif ah == 0x3E:
                file = state["files"].pop(read(UC_X86_REG_BX), None)
                file and file.close()
                carry(False)
            elif ah == 0x44 and (ax & 0xFF) == 0:
                write(UC_X86_REG_DX, 0)  # every handle a file: the runtimes then write through 40h
                carry(False)
            elif ah == 0x09:
                at = read(UC_X86_REG_DS) * 16 + read(UC_X86_REG_DX)
                text = b""
                while (byte := bytes(uc.mem_read(at + len(text), 1))) != b"$":
                    text += byte
                result.output += text
            elif ah == 0x02:
                result.output += bytes([read(UC_X86_REG_DX) & 0xFF])
            elif ah == 0x30:
                write(UC_X86_REG_AX, 0x0005)
            elif ah == 0x2C:
                write(UC_X86_REG_CX, 0)
                write(UC_X86_REG_DX, 0)
            elif ah == 0x35:
                segment, offset = state["vectors"].get(ax & 0xFF, (0, 0))
                write(UC_X86_REG_BX, offset)
                write(UC_X86_REG_ES, segment)
                carry(False)
            elif ah == 0x33:
                write(UC_X86_REG_DX, read(UC_X86_REG_DX) & 0xFF00)  # break checking off
                carry(False)
            elif ah == 0x06:
                if (read(UC_X86_REG_DX) & 0xFF) == 0xFF:
                    write(UC_X86_REG_EFLAGS, read(UC_X86_REG_EFLAGS) | 0x40)  # no key waiting
                else:
                    result.output += bytes([read(UC_X86_REG_DX) & 0xFF])
            elif ah == 0x0B:
                write(UC_X86_REG_AX, ax & 0xFF00)
            elif ah == 0x4A:
                room = 0xA000 - read(UC_X86_REG_ES)  # paragraphs from the block to the top of conventional memory
                if read(UC_X86_REG_BX) > room:
                    write(UC_X86_REG_BX, room)
                    write(UC_X86_REG_AX, 8)  # not enough memory: the program learns how much there is
                    carry(True)
                else:
                    carry(False)
            elif ah == 0x25:
                state["vectors"][ax & 0xFF] = (read(UC_X86_REG_DS), read(UC_X86_REG_DX))
                carry(False)
            elif ah in (0x1A, 0x0E, 0x42):
                carry(False)
            else:
                result.error = f"int 21h ah={ah:#x}"
                uc.emu_stop()
        elif number == 0x1A and ah == 0:
            write(UC_X86_REG_CX, 0)
            write(UC_X86_REG_DX, 0)
            write(UC_X86_REG_AX, 0)
        elif number == 0x10:
            if ah == 0x0F:
                write(UC_X86_REG_AX, 0x5003)
                write(UC_X86_REG_BX, 0)
        elif number == 0x11:
            write(UC_X86_REG_AX, 0x0023)  # a floppy, an 8087, 80x25 colour
        elif number == 0x12:
            write(UC_X86_REG_AX, 640)
        elif number == 0x16:
            if ah in (1, 0x11):
                flags = read(UC_X86_REG_EFLAGS)
                write(UC_X86_REG_EFLAGS, flags | 0x40)  # no key waiting
        elif number == 0x33:
            write(UC_X86_REG_AX, 0)  # no mouse
        elif number in (0x28, 0x2A):
            pass  # DOS idle and critical section hooks, with no network and nothing to do
        else:
            result.error = f"int {number:#x}"
            uc.emu_stop()

    uc.hook_add(UC_HOOK_CODE, step)
    uc.hook_add(UC_HOOK_INTR, interrupt)
    try:
        uc.emu_start((LOAD + cs) * 16 + ip, 0xFFFFF, count=limit)
    except UcError as error:
        result.error = result.error or f"emulator: {error} at {uc.reg_read(UC_X86_REG_CS):x}:{uc.reg_read(UC_X86_REG_IP):x}"
    for handle, file in state["files"].items():
        file.close()
    if whole and result.exit_code is not None:
        result.region = counts
    elif entry is not None and state["done"]:
        result.region = counts
    elif entry is not None and not result.error:
        result.error = "the region was entered but never returned" if state["active"] else NEVER_ENTERED
    return result
