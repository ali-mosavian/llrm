import shutil
import struct
import argparse
import subprocess
from pathlib import Path

from iced_x86 import OpKind
from iced_x86 import Mnemonic
from iced_x86 import Register

from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.objectfile import module

CASES = (
    (0x40000000, 0x40800000, 0x41000000),
    (0x3DCCCCCD, 0x3E4CCCCD, 0x40400000),
    (0x4B800000, 0x3F800000, 0x40400000),
    (0x4B800000, 0xCB800000, 0x40400000),
    (0x00800000, 0x00000001, 0x3F000000),
    (0x7F7FFFFF, 0x7F7FFFFF, 0x40000000),
    (0x3F800000, 0xBF800000, 0x00000000),
    (0x3F800000, 0x00000000, 0x00000000),
    (0x80000000, 0x80000000, 0x3F800000),
    (0x7FC12345, 0x3F800000, 0x40000000),
    (0x7F812345, 0x3F800000, 0x40000000),
    (0x7F800000, 0xFF800000, 0x40000000),
)
CONTROLS = tuple(
    0x7F | precision | rounding for precision in (0, 0x200, 0x300) for rounding in (0, 0x400, 0x800, 0xC00)
)
TRAP_CONTROLS = tuple(0x37F & ~(1 << bit) for bit in range(6))


def kernel(data: bytes) -> tuple[str, int]:
    found = module.of(omf.parse(data))
    if found is None:
        raise ValueError("missing code segment")
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        raise ValueError(mapped)
    partition = blocks.partition(found, mapped)
    instructions = sorted((one for block in partition for one in block.insns), key=lambda one: one.at)
    starts = [
        one.at
        for one in instructions
        if one.insn.mnemonic == Mnemonic.MOV
        and one.insn.op0_kind == OpKind.REGISTER
        and one.insn.op0_register == Register.AX
        and one.insn.op1_kind == OpKind.IMMEDIATE16
        and one.insn.immediate(1) == 1
    ]
    print_at = min(at for at, name in found.calls.items() if name == "B$PSSD")
    preceding = next(one for one in instructions if one.end == print_at)
    if len(starts) != 1 or preceding.insn.mnemonic != Mnemonic.PUSH or preceding.insn.op0_kind != OpKind.IMMEDIATE16:
        raise ValueError("arithmetic kernel boundaries are not established")
    start, end = starts[0], preceding.at
    frame_size = 0
    for one in instructions:
        if not start <= one.at < end or one.insn.memory_base != Register.BP:
            continue
        displacement = one.insn.memory_displacement & 0xFFFF
        if displacement < 0x8000:
            raise ValueError("unexpected nonlocal frame access in arithmetic kernel")
        frame_size = max(frame_size, 0x10000 - displacement)
    raw = bytearray(found.code[start:end])
    for block in partition:
        for decoded in block.insns:
            if not start <= decoded.at < end:
                continue
            offset = decoded.at - start
            if raw[offset] == 0xCD:
                number = raw[offset + 1]
                if 0x34 <= number <= 0x3B:
                    raw[offset : offset + 2] = bytes((0x9B, 0xD8 + number - 0x34))
                elif number == 0x3D:
                    raw[offset : offset + 2] = b"\x90\x9b"
                else:
                    raise ValueError("unexpected interrupt in arithmetic kernel")
    lines = []
    position = 0
    for fixup in sorted(omf.fixups(found.records), key=lambda item: item.offset):
        if fixup.seg != found.seg or not start <= fixup.offset < end:
            continue
        if fixup.loc != omf.LOC_OFF16 or fixup.target != "segment" or fixup.index != found.program_data:
            raise ValueError("unexpected arithmetic-kernel relocation")
        offset = fixup.offset - start
        if any(raw[offset : offset + 2]):
            raise ValueError("nonzero relocation addend")
        lines.extend(("db " + ",".join(str(byte) for byte in raw[position:offset]), f"dw offset state + {fixup.disp}"))
        position = offset + 2
    if position < len(raw):
        lines.append("db " + ",".join(str(byte) for byte in raw[position:]))
    lines.append("ret")
    return "\n".join(lines), frame_size


def assembly(original: bytes, candidate: bytes, bare: bool = False, traps: bool = False) -> str:
    if traps and not bare:
        raise ValueError("unmasked traps require the bare-metal transport")
    before, before_frame = kernel(original)
    after, after_frame = kernel(candidate)
    chosen = CONTROLS + TRAP_CONTROLS if traps else CONTROLS
    controls = ",".join(map(str, chosen))
    cases = ",".join(str(word) for case in CASES for word in case)
    size = len(chosen) * len(CASES) * 32
    trap_setup = (
        """xor ax, ax
    mov es, ax
    mov word ptr es:[40h], offset fpuTrap
    mov word ptr es:[42h], cs
    mov eax, cr0
    and eax, 0fffffff3h
    or eax, 20h
    mov cr0, eax"""
        if traps
        else ""
    )
    output = (
        f"""mov si, offset results
    mov cx, {size}
    mov dx, 0e9h
    rep outsb
    mov dx, 0f4h
    mov ax, 10h
    out dx, ax
halted:
    cli
    hlt
    jmp halted"""
        if bare
        else f"""mov ah, 3ch
    xor cx, cx
    mov dx, offset filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, {size}
    mov dx, offset results
    int 21h
    jc failed
    cmp ax, {size}
    jne failed
    mov ah, 3eh
    int 21h
    mov dx, offset complete
    mov ah, 9
    int 21h
    mov ax, 4c00h
    int 21h
failed:
    mov ax, 4c01h
    int 21h"""
    )
    return f""".model tiny
.386
.code
org 100h
start:
    push cs
    pop ds
    {trap_setup}
    push cs
    pop es
    cld
    mov cursor, offset results
    mov bp, offset controls
    mov cx, {len(chosen)}
controlLoop:
    push cx
    mov si, offset samples
    mov cx, {len(CASES)}
sampleLoop:
    mov bx, offset originalKernel
    call runKernel
    mov bx, offset candidateKernel
    call runKernel
    add si, 12
    loop sampleLoop
    add bp, 2
    pop cx
    loop controlLoop
    fninit
    {output}
runKernel:
    pushad
    fninit
    mov trapped, 0
    fldcw word ptr ds:[bp]
    mov eax, [si]
    mov dword ptr [state+6], eax
    mov eax, [si+4]
    mov dword ptr [state+10], eax
    mov eax, [si+8]
    mov dword ptr [state+14], eax
    mov dword ptr [state+18], 7fc0deedh
    mov dword ptr [state+22], 7fc0deedh
    mov dword ptr [state+26], 0
    mov word ptr [state+30], 0
    mov bp, sp
    sub sp, {max(before_frame, after_frame)}
    call bx
    mov sp, bp
    cmp trapped, 0
    jne captured
    fnstsw word ptr [state+32]
captured:
    mov si, offset state+18
    mov di, cursor
    mov cx, 8
    rep movsw
    mov cursor, di
    popad
    ret
fpuTrap:
    fnstsw word ptr [state+32]
    mov trapped, 1
    fnclex
    add sp, 6
    ret
originalKernel:
{before}
candidateKernel:
{after}
controls dw {controls}
samples dd {cases}
cursor dw 0
trapped db 0
state db 34 dup(0)
filename db 'CHECK.BIN',0
complete db 'CHECK COMPLETE',13,10,'$'
results db {size} dup(0)
end start
"""


def qualified(data: bytes) -> None:
    def record(control: int, case: int) -> tuple[int, int, int, int, int]:
        offset = (CONTROLS.index(control) * len(CASES) + case) * 32
        return struct.unpack_from("<IIIHH", data, offset)

    if record(0x37F, 0) != (0x42400000, 0x3F400000, 0x43F3C000, 11, 0):
        raise ValueError("floating environment: exact-input sanity check failed")
    # SINGLE(0.1), SINGLE(0.2), then extended addition and multiplication by 3.
    # At 53-bit precision this lies strictly between these adjacent SINGLEs.
    if (record(0x67F, 1)[0], record(0xA7F, 1)[0]) != (0x3F666666, 0x3F666667):
        raise ValueError("floating environment: directed SINGLE rounding was not observed")
    if not record(0x37F, 7)[4] & 0x4:
        raise ValueError("floating environment: divide-by-zero status flag was not observed")


def checked(data: bytes, traps: bool = False) -> int:
    controls = CONTROLS + TRAP_CONTROLS if traps else CONTROLS
    expected = len(controls) * len(CASES) * 32
    if len(data) != expected:
        raise ValueError(f"expected {expected} result bytes, got {len(data)}")
    qualified(data)
    if traps:
        for bit, control in enumerate(TRAP_CONTROLS):
            start = controls.index(control) * len(CASES) * 32
            records = [struct.unpack_from("<IIIHH", data, start + case * 32) for case in range(len(CASES))]
            if not any(
                counter < 11 and status & (0x80 | (1 << bit)) == (0x80 | (1 << bit))
                for _, _, _, counter, status in records
            ):
                raise ValueError(f"floating environment: no observed unmasked trap for {control:04x}")
    for index, (before, after) in enumerate(struct.iter_unpack("16s16s", data)):
        if before != after:
            control, case = divmod(index, len(CASES))
            raise ValueError(f"control={controls[control]:04x} case={case}: {before.hex()} != {after.hex()}")
    return len(controls) * len(CASES)


def bootloader(sectors: int) -> str:
    if not 1 <= sectors <= 17:
        raise ValueError("comparison payload must fit the first floppy track")
    return f""".model tiny
.386
.code
org 7c00h
start:
    cli
    xor ax, ax
    mov ss, ax
    mov sp, 7c00h
    sti
    mov ax, 1000h
    mov es, ax
    mov bx, 100h
    mov ax, {0x200 + sectors}
    mov cx, 2
    xor dh, dh
    int 13h
    jc failed
    cli
    mov ax, 1000h
    mov ss, ax
    mov sp, 0fffeh
    sti
    db 0eah
    dw 100h, 1000h
failed:
    cli
    hlt
    jmp failed
    org 7dfeh
    dw 0aa55h
end start
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--qemu", action="store_true")
    parser.add_argument("--traps", action="store_true")
    args = parser.parse_args()
    if args.check:
        result = args.directory / ("TRAPS.BIN" if args.traps else "QEMU.BIN" if args.qemu else "CHECK.BIN")
        print(f"{checked(result.read_bytes(), traps=args.traps)} exact state comparisons passed")
        return
    original = Path("fixtures/omf/fpcsex-p-g2.obj").read_bytes()
    candidate = (args.directory / "REF.OBJ").read_bytes()
    source = args.directory / ("qemu.asm" if args.qemu else "check.asm")
    source.write_text(assembly(original, candidate, bare=args.qemu, traps=args.traps))
    assembler = shutil.which("jwasm") or "/Users/alim/work/other/d32x/toolchains/native/bin/jwasm"
    payload = args.directory / ("QEMU.COM" if args.qemu else "CHECK.COM")
    subprocess.run([assembler, "-bin", "-q", f"-Fo{payload}", str(source)], check=True)
    if args.qemu:
        code = payload.read_bytes()
        boot = args.directory / "boot.asm"
        boot.write_text(bootloader((len(code) + 511) // 512))
        binary = args.directory / "boot.bin"
        subprocess.run([assembler, "-bin", "-q", f"-Fo{binary}", str(boot)], check=True)
        sector = binary.read_bytes()
        if len(sector) != 512 or sector[-2:] != b"\x55\xaa":
            raise ValueError("invalid boot sector")
        (args.directory / "qemu.img").write_bytes((sector + code).ljust(1440 * 1024, b"\0"))


if __name__ == "__main__":
    main()
