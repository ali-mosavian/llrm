"""QB HIR through the existing machine pipeline to a fresh OMF module.

This first emission boundary is intentionally a procedure module: it emits
far Pascal SUB/FUNCTION bodies and their data, but refuses executable module
statements until the BASIC module header/startup records are represented.
Refusing is important here; a C-shaped object containing a procedure named
``__main`` is not a valid BASIC main module merely because LINK accepts it.
"""

import struct
from pathlib import Path
from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register

from qbopt import hir
from qbopt import flow
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.backend import frame
from qbopt.backend import lower
from qbopt.objectfile import omf
from qbopt.backend import phielim
from qbopt.backend import omfwrite
from qbopt.backend import prologue
from qbopt.optimize import transform
from qbopt.backend import cpu as targets
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space
from qbopt.frontend.qb.abi import physicalize
from qbopt.frontend.qb.inline_x87 import finalized


class EmissionError(ValueError):
    """HIR is valid but does not yet have a truthful BASIC object spelling."""


_READ_DATA_OBJECT = "$qb$readData"
_STATEMENT_TABLE_OBJECT = "$qb$statementTable"


@dataclass(frozen=True, slots=True)
class _SegmentWord:
    """Frontend-owned MASM ``dw seg symbol`` data initializer."""

    name: str


def _object_name(name: str) -> str:
    """The linker spelling BC uses: uppercase and without a type suffix."""
    return name.rstrip("%&!#$").upper()


def _empty_main(function: hir.Function) -> bool:
    return all(
        not block.instructions and block.terminator.kind is hir.TerminatorKind.RETURN and not block.terminator.operands
        for block in function.blocks
    )


def _data(module: hir.Module) -> tuple[dict[tuple[Space, int], str], dict[str, tuple[masm.Datum | _SegmentWord, ...]]]:
    names: dict[tuple[Space, int], str] = {(Space.GROUP, 0): "DGROUP"}
    internal = {
        one.id: one
        for one in module.data
        if one.linkage is hir.DataLinkage.INTERNAL and one.name not in {_READ_DATA_OBJECT, _STATEMENT_TABLE_OBJECT}
    }
    for object_ in module.data:
        if object_.name in {_READ_DATA_OBJECT, _STATEMENT_TABLE_OBJECT}:
            continue
        if object_.linkage is hir.DataLinkage.EXTERNAL:
            names[(Space.EXTERNAL, object_.id)] = object_.name
        else:
            names[(Space.SEGMENT, object_.id)] = f"{_object_name(module.name)}$D{object_.id}"

    grouped: dict[str, list[masm.Datum]] = {"BC_DATA": [], "BC_CN": [], "FSL_CONST": []}
    for object_ in internal.values():
        segment = (
            "FSL_CONST"
            if object_.address in (hir.AddressKind.FAR, hir.AddressKind.HUGE)
            else ("BC_CN" if object_.readonly else "BC_DATA")
        )
        items = grouped[segment]
        label = names[(Space.SEGMENT, object_.id)]
        items.append(masm.Label(label))
        cursor = 0
        for relocation in sorted(object_.relocations, key=lambda one: one.at):
            far = relocation.address in (hir.AddressKind.FAR, hir.AddressKind.HUGE)
            width = 4 if far else 2
            if relocation.at < cursor or relocation.at + width > len(object_.bytes):
                raise EmissionError(f"{object_.name}: overlapping or out-of-range data relocation")
            if relocation.at != cursor:
                items.append(bytes(object_.bytes[cursor : relocation.at]))
            target = names.get((Space.SEGMENT, relocation.target))
            if target is None:
                raise EmissionError(f"{object_.name}: relocation names unknown data object {relocation.target}")
            if relocation.address is hir.AddressKind.SEGMENT:
                if relocation.addend:
                    raise EmissionError(f"{object_.name}: a segment selector cannot carry an offset")
                items.append(_SegmentWord(target))
            else:
                items.append(masm.Pointer(target, relocation.addend, far))
            cursor = relocation.at + width
        if cursor != len(object_.bytes):
            items.append(bytes(object_.bytes[cursor:]))
    return names, {name: tuple(items) for name, items in grouped.items()}


def _read_data_lines(module: hir.Module) -> tuple[bytes, ...]:
    """Decode the frontend-owned DATA statement stream.

    QB45 ``rt/read.asm`` starts at MODULE_CODE.OF_DS (``BC_DS+2``).  Each
    NUL-terminated source-text line is followed by a two-byte code address;
    ``B$ReadVal`` skips that word at end of line.  The semantic frontend keeps
    only the text lines in this reserved object.  The OMF adapter supplies a
    relocated, valid code address before every line and the measured
    ``FFFF 01`` out-of-DATA sentinel after the last one.
    """
    objects = [one for one in module.data if one.name == _READ_DATA_OBJECT]
    if not objects:
        return ()
    if len(objects) != 1:
        raise EmissionError("a module may contain only one READ/DATA stream")
    object_ = objects[0]
    if (
        object_.linkage is not hir.DataLinkage.INTERNAL
        or not object_.readonly
        or object_.relocations
        or object_.address is not hir.AddressKind.NEAR
    ):
        raise EmissionError("the READ/DATA stream has an invalid storage contract")
    payload = bytes(object_.bytes)
    if not payload or payload[-1] != 0:
        raise EmissionError("the READ/DATA stream must contain NUL-terminated lines")
    lines = tuple(payload[:-1].split(b"\0"))
    if any(not line or not line.isascii() for line in lines):
        raise EmissionError("READ/DATA lines must be nonempty ASCII source text")
    return lines


def _object_data(
    segment: omfwrite.Segment,
    index: int,
    items: tuple[masm.Datum | _SegmentWord, ...],
    symbols: dict[str, tuple[int, int]],
) -> None:
    """Encode QB data, including the real-mode selector-only relocation."""
    for item in items:
        match item:
            case masm.Label(name=name):
                symbols[name] = (index, len(segment.image))
            case masm.Fill(size=size, byte=None):
                segment.skip(size)
            case masm.Fill(size=size, byte=byte) if byte is not None:
                segment.put(bytes([byte]) * size)
            case masm.Pointer(name=name, offset=offset, far=far):
                loc = omfwrite.POINTER if far else omfwrite.OFFSET
                segment.put(bytes(omfwrite.WIDE[loc]), (omfwrite.Fixup(0, loc, name),))
                struct.pack_into("<H", segment.image, len(segment.image) - omfwrite.WIDE[loc], offset & 0xFFFF)
            case _SegmentWord(name=name):
                segment.put(bytes(2), (omfwrite.Fixup(0, omfwrite.BASE, name),))
            case masm.Align(to=to):
                segment.put(bytes(-len(segment.image) % to))
            case bytes():
                segment.put(item)


def _empty_procedure(name: str) -> masm.Procedure:
    return masm.Procedure(name, False, False, lir.LirBody(name, 1, (), {}, {}), 0, {})


def _statement_metadata(module: hir.Module) -> tuple[tuple[int, int, int, int], ...]:
    """Decode frontend-private (function, block, first instruction, line) rows."""
    objects = [one for one in module.data if one.name == _STATEMENT_TABLE_OBJECT]
    if len(objects) != 1:
        raise EmissionError("a QB module must carry exactly one statement-table metadata object")
    object_ = objects[0]
    if (
        object_.linkage is not hir.DataLinkage.INTERNAL
        or not object_.readonly
        or object_.relocations
        or object_.address is not hir.AddressKind.NEAR
        or len(object_.bytes) % 14
    ):
        raise EmissionError("the statement-table metadata object has an invalid storage contract")
    payload = bytes(object_.bytes)
    return tuple(
        (
            int.from_bytes(payload[at : at + 4], "little"),
            int.from_bytes(payload[at + 4 : at + 8], "little"),
            int.from_bytes(payload[at + 8 : at + 12], "little"),
            int.from_bytes(payload[at + 12 : at + 14], "little"),
        )
        for at in range(0, len(payload), 14)
    )


def _statement_table_blocks(function: hir.Function) -> frozenset[int]:
    """Statement rows excluding code reachable only inside the handler."""
    blocks = {one.id: one for one in function.blocks}
    handler_only: set[int] = set()
    pending = [function.error_handler] if function.error_handler is not None else []
    while pending:
        block = pending.pop()
        if block in handler_only or block not in blocks:
            continue
        handler_only.add(block)
        # RESUME NEXT's semantic successors are the possible runtime
        # destinations, not handler-owned statements. Stop the handler walk
        # at that transfer so post-error continuation rows remain eligible.
        if not any(one.callee == "B$RESN" for one in blocks[block].instructions):
            pending.extend(blocks[block].terminator.targets)
    return frozenset(blocks) - handler_only


def _drop_resume_successors(body: lir.LirBody, calls: dict[int, str]) -> lir.LirBody:
    """Remove semantic RESUME dispatch edges after they served allocation."""
    return replace(
        body,
        blocks=tuple(
            replace(block, succ=())
            if any(calls.get(instruction.at) == "B$RESN" for instruction in block.insns)
            else block
            for block in body.blocks
        ),
    )


def _statement_procedure(entries: tuple[tuple[int, int, str, int], ...]) -> masm.Procedure:
    """Emit MODULE_CODE.OF_STA's relocated (offset,line) rows and zero end."""
    code: list[masm.InlinePart] = []
    for _procedure, _order, label, line in entries:
        code.extend((("offset", label, 0), struct.pack("<H", line)))
    code.append(b"\0\0")
    instruction = lir.Insn(
        1,
        None,
        ir.Semantics(ir.Operation.CALL, "statement-table"),
        (),
        (),
    )
    body = lir.LirBody("$QB$STAT", 1, (lir.LirBlock(1, (instruction,)),), {}, {})
    return masm.Procedure(
        "$QB$STAT",
        False,
        False,
        body,
        0,
        {1: masm.Callee("$statement-table", False, tuple(code))},
    )


def _split_statement_blocks(body: lir.LirBody, markers: frozenset[int]) -> tuple[lir.LirBody, dict[int, int]]:
    """Restore source-statement labels after MIR legitimately merged blocks.

    The optimizer continues to see ordinary MIR and may merge straight-line
    source blocks. RESUME's runtime table nevertheless needs an address at the
    first retained machine operation of each statement. Split allocated LIR
    only at those operation identities; no operation is copied, deleted, or
    re-ordered, and ordinary incoming edges still target the first piece.
    """
    next_block = max((block.at for block in body.blocks), default=0) + 1
    made: list[lir.LirBlock] = []
    labels: dict[int, int] = {}
    for block in body.blocks:
        positions: dict[int, list[int]] = {}
        for index, instruction in enumerate(block.insns):
            # MIR addresses are unique within a body. MIR operation ids are
            # not a source-statement key: source instruction 22 and an
            # independently generated jump may both carry id 22.
            source = getattr(instruction.op, "at", None)
            if source in markers:
                positions.setdefault(index, []).append(source)
        if not positions:
            made.append(block)
            continue
        boundaries = sorted({0, *positions})
        ids = [block.at, *range(next_block, next_block + len(boundaries) - 1)]
        next_block += len(boundaries) - 1
        for segment, start in enumerate(boundaries):
            end = boundaries[segment + 1] if segment + 1 < len(boundaries) else len(block.insns)
            at = ids[segment]
            successors = (ids[segment + 1],) if segment + 1 < len(ids) else block.succ
            made.append(replace(block, at=at, insns=block.insns[start:end], succ=successors))
            for marker in positions.get(start, ()):
                labels[marker] = at
    return replace(body, blocks=tuple(made)), labels


def _resume_label_transfers(
    body: lir.LirBody,
    resume_blocks: dict[int, int],
    statement_instructions: dict[int, int],
    statement_labels: dict[int, int],
    procedure: int,
    code_names: dict[int, str],
) -> lir.LirBody:
    """Insert B$RESA's measured AX target after final statement layout."""
    if not resume_blocks:
        return body
    serial = max((one.at for block in body.blocks for one in block.insns), default=0) + 1
    blocks = []
    found: set[int] = set()
    for block in body.blocks:
        instructions = []
        for instruction in block.insns:
            target_block = resume_blocks.get(instruction.at)
            if target_block is not None:
                source_instruction = statement_instructions.get(target_block)
                target = statement_labels.get(source_instruction) if source_instruction is not None else None
                if target is None:
                    raise EmissionError(f"RESUME target block {target_block} has no retained source statement")
                key = -(len(code_names) + 1)
                code_names[key] = masm.label(procedure, target)
                ax = ir.Reg(Register.AX, 2)
                instructions.append(
                    lir.Insn(
                        serial,
                        (serial, serial),
                        ir.Semantics(
                            ir.Operation.MOVE,
                            "mov",
                            (ax,),
                            (ir.Imm(0, 2, Addr(Space.SEGMENT, 0, key)),),
                        ),
                        (),
                        (),
                    )
                )
                serial += 1
                found.add(instruction.at)
            instructions.append(instruction)
        blocks.append(replace(block, insns=tuple(instructions)))
    missing = set(resume_blocks) - found
    if missing:
        raise EmissionError(f"RESUME call sites vanished before final layout: {sorted(missing)}")
    return replace(body, blocks=tuple(blocks))


def _compiler_switches(program: hir.Program) -> int:
    """Return the measured BC ``U_FLAG`` word for this compilation profile.

    ``U_FLAG`` is consumed by the BASIC runtime, not descriptive metadata.
    These are the exact optimized /FPi profiles emitted by each compatible
    compiler; VBDOS additionally records its /R array layout choice.
    """
    if program.float_mode is hir.FloatMode.ALTERNATE:
        if program.runtime is not hir.RuntimeProfile.PDS71:
            raise EmissionError("alternate floating-point runtime is only measured for PDS 7.1")
        return 0x1088  # BC /O /FPa /G2
    flags = {
        hir.RuntimeProfile.QB45: 0x1080,  # BC /O /FPi
        hir.RuntimeProfile.PDS71: 0x1084,  # BC /O /FPi /G2
        hir.RuntimeProfile.VBDOS: 0x12C4,  # BC /O /FPi /G3 /E
    }[program.runtime]
    if program.runtime is hir.RuntimeProfile.VBDOS and program.array_order is hir.ArrayOrder.ROW_MAJOR:
        flags |= 0x0100  # /R
    return flags


def _header(program: hir.Program) -> bytes:
    module = program.modules[0]
    name = _object_name(module.name).encode("ascii", "strict")[:8].ljust(8, b" ")
    # MODULE_CODE in runtime/inc/addr.inc. Every symbolic word is an offset,
    # framed through DGROUP by the shared writer.
    out = bytearray(b"bl" + name + bytes(34) + b"\xff\xff" + struct.pack("<H", _compiler_switches(program)))
    if len(out) != 48:
        raise AssertionError("MODULE_CODE is exactly O_ENT bytes")
    # The addends live in the image. The remaining words are zero.
    struct.pack_into("<H", out, 12, 2)  # OF_DS is BC_DS + 2.
    return bytes(out)


def _ends_program(body: lir.LirBody) -> tuple[lir.LirBody, dict[int, masm.Callee]]:
    """Spell a BASIC module return as the runtime's non-returning B$CEND."""
    sites: dict[int, masm.Callee] = {}
    blocks = []
    for block in body.blocks:
        instructions = []
        exits = False
        for instruction in block.insns:
            what = instruction.what
            if what is not None and what.op is ir.Operation.RETURN:
                exits = True
                sites[instruction.at] = masm.Callee("B$CEND", True)
                instruction = replace(
                    instruction,
                    what=ir.Semantics(ir.Operation.CALL, "call"),
                )
            instructions.append(instruction)
        # Only the rewritten return becomes non-returning. Branch and jump
        # blocks retain their CFG edges; MASM listing uses the untaken edge to
        # insert an explicit jump when it is not the next laid-out block.
        blocks.append(replace(block, insns=tuple(instructions), succ=() if exits else block.succ))
    return replace(body, blocks=tuple(blocks), noreturn=True), sites


def _register_error_handler(
    body: lir.LirBody,
    address: Addr,
    *,
    local: bool,
    after_at: int | None = None,
) -> tuple[lir.LirBody, dict[int, masm.Callee]]:
    """Insert the measured module- or procedure-error registration.

    Keep the four instructions structured.  Opaque inline bytes make the
    generic MASM shell conservatively invent a BP frame, but QB main is
    frameless and the error runtime interprets BP as its own frame chain.
    VBDOS LOCERR.OBJ and PDS71 PDLOCAL.OBJ establish that ON LOCAL ERROR uses
    B$OEGP with only the offset word, after B$ENRA has installed the procedure
    frame. Ordinary ON ERROR uses B$OEGA's CS:offset pair.
    """
    at = max((one.at for block in body.blocks for one in block.insns), default=0) + 1
    ax = ir.Reg(Register.AX, 2)
    moved = lir.Insn(
        at,
        (at, at),
        ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(0, 2, address),)),
        (),
        (),
    )
    registration = (
        (
            moved,
            lir.Insn(
                at + 1,
                (at + 1, at + 1),
                ir.Semantics(ir.Operation.PUSH, "push", (), (ax,)),
                (),
                (),
            ),
            lir.Insn(
                at + 2,
                (at + 2, at + 2),
                ir.Semantics(ir.Operation.CALL, "on-local-error-register"),
                (),
                (),
            ),
        )
        if local
        else (
            lir.Insn(
                at,
                (at, at),
                ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(0, 2, address),)),
                (),
                (),
            ),
            lir.Insn(
                at + 1,
                (at + 1, at + 1),
                ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.CS, 2),)),
                (),
                (),
            ),
            lir.Insn(
                at + 2,
                (at + 2, at + 2),
                ir.Semantics(ir.Operation.PUSH, "push", (), (ax,)),
                (),
                (),
            ),
            lir.Insn(
                at + 3,
                (at + 3, at + 3),
                ir.Semantics(ir.Operation.CALL, "on-error-register"),
                (),
                (),
            ),
        )
    )

    def insert(block: lir.LirBlock) -> lir.LirBlock:
        if block.at != body.entry:
            return block
        if after_at is None:
            return replace(block, insns=(*registration, *block.insns))
        made = []
        inserted = False
        for instruction in block.insns:
            made.append(instruction)
            if instruction.at == after_at:
                made.extend(registration)
                inserted = True
        if not inserted:
            raise EmissionError("ON LOCAL ERROR registration has no B$ENRA entry site")
        return replace(block, insns=tuple(made))

    blocks = tuple(insert(block) for block in body.blocks)
    call_at = at + (2 if local else 3)
    return replace(body, blocks=blocks), {call_at: masm.Callee("B$OEGP" if local else "B$OEGA", True)}


def _initialize_frame(body: lir.LirBody, size: int) -> tuple[lir.LirBody, dict[int, masm.Callee]]:
    """Zero a native frame as BASIC's runtime entry routines do.

    QB variables begin at zero, and runtime-managed string/array descriptors
    require that invariant before their first assignment.  The shared backend
    deliberately owns only reservation; this source ABI initialization stays
    in the frontend and runs before any source instruction.
    """
    size += size & 1
    if not size:
        return body, {}
    if size > 0x7FFE:
        raise EmissionError(f"{body.name}: {size} byte native frame exceeds a 16-bit BP displacement")
    at = max((one.at for block in body.blocks for one in block.insns), default=0) + 1
    initialize = lir.Insn(
        at,
        (at, at),
        ir.Semantics(ir.Operation.CALL, "frame-zero"),
        (),
        (),
    )
    blocks = tuple(
        replace(block, insns=(initialize, *block.insns)) if block.at == body.entry else block for block in body.blocks
    )
    code = (
        bytes.fromhex("06 57 16 07 31 c0 8d be")
        + struct.pack("<h", -size)
        + bytes([0xB9])
        + struct.pack("<H", size // 2)
        + bytes.fromhex("fc f3 ab 5f 07"),
    )
    return replace(body, blocks=blocks), {at: masm.Callee("$frame_zero", False, code)}


_RUNTIME_FRAME_HEADER = {
    hir.RuntimeProfile.QB45: 10,
    hir.RuntimeProfile.PDS71: 18,
    hir.RuntimeProfile.VBDOS: 20,
}


def _runtime_frame(
    body: lir.LirBody,
    size: int,
    runtime: hir.RuntimeProfile,
    temporary_strings: int,
) -> tuple[lir.LirBody, dict[int, masm.Callee]]:
    """Enter and leave a BASIC runtime frame inside the native shell.

    B$FCMD and the managed-string runtime consult BASIC's current frame, so a
    merely zeroed C-style frame is not sufficient. MASM's small native shell
    saves BP (and any used SI/DI) before this body; account for those words in
    incoming parameter addresses. B$ENRA then owns a runtime-specific header
    immediately below BP, so every source local and spill must be rebased below
    that header. B$EXSA returns to the shell, whose epilogue restores its saves.

    BX is the maximum number of runtime-produced STRING temporaries an HIR
    instruction consumes and produces together. The runtime allocates that
    procedure's temporary-string block for B$EXSA. Array descriptors are
    instead owned by their explicit B$DDIM/B$ERAS pair. This count must come
    from resolved typed expressions, never from allocator spill slots.
    """
    size += size & 1
    if size > 0x7FFE:
        raise EmissionError(f"{body.name}: {size} byte BASIC frame exceeds a 16-bit BP displacement")
    if not 0 <= temporary_strings <= 0xFFFF:
        raise EmissionError(f"{body.name}: too many temporary STRING slots")
    serial = max((one.at for block in body.blocks for one in block.insns), default=0) + 1
    cx = ir.Reg(Register.CX, 2)
    bx = ir.Reg(Register.BX, 2)
    enter = (
        lir.Insn(serial, (serial, serial), ir.Semantics(ir.Operation.MOVE, "mov", (cx,), (ir.Imm(size, 2),)), (), ()),
        lir.Insn(
            serial + 1,
            (serial + 1, serial + 1),
            ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (ir.Imm(temporary_strings, 2),)),
            (),
            (),
        ),
        lir.Insn(
            serial + 2,
            (serial + 2, serial + 2),
            ir.Semantics(ir.Operation.CALL, "call"),
            (),
            (),
        ),
    )
    leave_at = serial + 3
    leave = lir.Insn(
        leave_at,
        (leave_at, leave_at),
        ir.Semantics(ir.Operation.CALL, "call"),
        (),
        (),
    )
    framed = replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=(enter + block.insns if block.at == body.entry else block.insns),
            )
            for block in body.blocks
        ),
    )
    framed = replace(
        framed,
        blocks=tuple(
            replace(
                block,
                insns=tuple(
                    instruction
                    for one in block.insns
                    for instruction in (
                        (leave, one) if one.what is not None and one.what.op is ir.Operation.RETURN else (one,)
                    )
                ),
            )
            for block in framed.blocks
        ),
    )

    # masm's native shell precedes B$ENRA, so its saved words remain between
    # the far return address and the source-level parameters.
    saved = [low for whole, low in masm.SAVED.items() if whole in masm._roots(framed)]
    adjustment = 2 + 2 * len(saved)  # pushed BP plus SI/DI actually used

    header = _RUNTIME_FRAME_HEADER[runtime]

    def operand(where: ir.Loc) -> ir.Loc:
        if isinstance(where, (ir.Mem, ir.Address)) and where.addr is not None and where.addr.space is Space.FRAME:
            # BASIC's BP frame has one stable geometric boundary even after
            # allocation has discarded Mem.stack_argument: parameters are at
            # positive displacements and locals are negative. The native shell
            # moves only the former upward; B$ENRA's header moves only the
            # latter downward.
            moved = where.addr.disp + adjustment if where.addr.disp > 0 else where.addr.disp - header
            return replace(where, addr=replace(where.addr, disp=moved))
        return where

    framed = replace(
        framed,
        blocks=tuple(
            replace(
                block,
                insns=tuple(
                    replace(
                        one,
                        what=replace(
                            one.what,
                            dests=tuple(map(operand, one.what.dests)),
                            sources=tuple(map(operand, one.what.sources)),
                        ),
                    )
                    if one.what is not None
                    else one
                    for one in block.insns
                ),
            )
            for block in framed.blocks
        ),
    )
    return framed, {
        serial + 2: masm.Callee("B$ENRA", True),
        leave_at: masm.Callee("B$EXSA", True),
    }


def _temporary_string_slots(module: hir.Module, function: hir.Function) -> int:
    """Count frame-owned dynamic STRING descriptors for B$ENRA.

    Runtime-produced descriptors live on the runtime temporary chain and do
    not request entries in the procedure-local handle block. Raw VBDOS output
    uses BX=0 for COM_TOKENIZE and an LDFS/SCAT array append, but BX=1 for a
    nested COMMAND$/LTRIM$/RTRIM$ expression assigned to one local STRING and
    for COM_ARG's STRING function-result descriptor. The count is therefore a
    property of typed frame places, not expression-result liveness.
    """
    types = {one.id: one for one in module.types}
    return sum(place.storage is hir.Storage.LOCAL and types[place.type].name == "string" for place in function.places)


def _address_values(body: lir.LirBody) -> lir.LirBody:
    """Turn allocated ADDRESS cells into LEA's non-memory operand spelling."""

    def instruction(one: lir.Insn) -> lir.Insn:
        what = one.what
        if what is None or what.op is not ir.Operation.ADDRESS:
            return one
        sources = []
        for source in what.sources:
            if not isinstance(source, ir.Mem):
                sources.append(source)
                continue
            addr = source.addr
            if addr is not None and addr.space is Space.FRAME and source.base is not None:
                sources.append(
                    ir.Address(
                        None,
                        through=Register.BP,
                        index=source.through,
                        scale=source.scale,
                        offset=addr.disp,
                        disp_width=source.disp_width,
                    )
                )
                continue
            if addr is not None and source.base is not None and source.through != Register.NONE:
                addr = replace(addr, base=source.through)
            sources.append(
                ir.Address(
                    addr,
                    through=source.through,
                    index=source.index_through,
                    scale=source.scale,
                    offset=source.offset,
                    disp_width=source.disp_width,
                )
            )
        return replace(one, what=replace(what, sources=tuple(sources)))

    return replace(
        body,
        blocks=tuple(replace(block, insns=tuple(instruction(one) for one in block.insns)) for block in body.blocks),
    )


def _source_instructions(body: lir.LirBody) -> lir.LirBody:
    """Drop source-generated ESCAPE markers, which own no legacy bytes."""
    return replace(
        body,
        blocks=tuple(
            replace(block, insns=tuple(one for one in block.insns if one.what is not None)) for block in body.blocks
        ),
    )


def _handler_at(function: hir.Function) -> int | None:
    return function.error_handler


def _optimizer_resume_edges(body: mir.MirBody) -> tuple[mir.MirBody, dict[int, mir.Op]]:
    """Expose explicit RESUME transfers while MIR memory optimization runs.

    B$RESA never returns to the following instruction, but it does transfer to
    a known source block after unwinding the handler.  Without that semantic
    edge, DSE cannot see a local store in the handler reaching a load after
    RESUME and deletes the store.  The physical ABI remains the measured
    ``mov ax, offset target / call B$RESA``: the temporary jumps are removed
    after optimization.
    """
    labels = {block.at for block in body.blocks}
    serial = max((op.at for block in body.blocks for op in block.ops), default=0)
    restored: dict[int, mir.Op] = {}
    blocks = []
    for block in body.blocks:
        resumes = [op for op in block.ops if op.kind is mir.Kind.CALL and op.name.startswith("$QB$RESA:")]
        if not resumes:
            blocks.append(block)
            continue
        if len(resumes) != 1 or not block.ops or block.ops[-1].kind is not mir.Kind.ESCAPE:
            raise EmissionError(f"{body.entry}: malformed explicit RESUME transfer in block {block.at}")
        try:
            target = int(resumes[0].name.removeprefix("$QB$RESA:"))
        except ValueError as error:
            raise EmissionError(f"invalid RESUME target marker {resumes[0].name!r}") from error
        if target not in labels:
            raise EmissionError(f"RESUME target block {target} does not exist")
        serial += 1
        jump = mir.Op(
            serial,
            ir.Operation.JUMP,
            "",
            (),
            (),
            kind=mir.Kind.JUMP,
            target=target,
            reads_complete=True,
        )
        restored[serial] = block.ops[-1]
        blocks.append(replace(block, ops=(*block.ops[:-1], jump), succ=(target,)))
    return replace(body, blocks=tuple(blocks)), restored


def _drop_optimizer_resume_edges(body: mir.MirBody, restored: dict[int, mir.Op]) -> mir.MirBody:
    if not restored:
        return body
    missing = set(restored)
    blocks = []
    for block in body.blocks:
        markers = [op for op in block.ops if op.at in restored]
        if not markers:
            blocks.append(block)
            continue
        if len(markers) != 1 or block.ops[-1] is not markers[0]:
            raise EmissionError(f"optimizer moved the temporary RESUME edge in block {block.at}")
        marker = markers[0]
        missing.remove(marker.at)
        blocks.append(replace(block, ops=(*block.ops[:-1], restored[marker.at]), succ=()))
    if missing:
        raise EmissionError(f"optimizer deleted temporary RESUME edges {sorted(missing)}")
    return replace(body, blocks=tuple(blocks))


def optimized(program: hir.Program, function: hir.Function, body: hir.Lowered) -> hir.Lowered:
    """Run the production MIR optimizer while preserving QB ABI side entries."""
    module = next(
        (one for one in program.modules if function in one.functions),
        None,
    )
    if module is None:
        raise EmissionError(f"{function.name}: function is not part of this program")
    target = targets.profile("386")
    dgroup = frozenset(
        one.id
        for one in module.data
        if one.linkage is hir.DataLinkage.INTERNAL
        and one.name != _READ_DATA_OBJECT
        and one.address not in (hir.AddressKind.FAR, hir.AddressKind.HUGE)
    )
    optimizer_body, resume_edges = _optimizer_resume_edges(body.body)
    semantic_calls = {
        operation.at: operation.name
        for block in optimizer_body.blocks
        for operation in block.ops
        if operation.kind is mir.Kind.CALL
    }
    entries = tuple(
        dict.fromkeys(
            (
                *function.external_entries,
                *(() if function.error_handler is None else (function.error_handler,)),
            )
        )
    )
    # RESUME and ON ERROR enter these blocks from the runtime, independently
    # of the ordinary source predecessor.  Make every such edge visible while
    # optimization is running.  Restoring the original handler only after an
    # ordinary one-entry optimization preserved its code but not its value
    # semantics: propagation could make a resumable statement depend on a
    # definition which the external entry bypasses.
    rooted, temporary_root = _machine_side_entry(optimizer_body, entries)
    transformed = transform.applied(
        rooted,
        dgroup,
        semantic_calls,
        # Runtime RESUME entries can jump directly into a loop, making the
        # analysis root irreducible. Scalar promotion requires a dominator
        # tree and, more importantly, must not replace frame state that such
        # an entry deliberately reloads with a value from the ordinary path.
        promote_=temporary_root is None,
        registers=target.register_capacity,
        call_registers=target.call_register_capacity,
        index_scales=target.address_scales,
        # The common 16-bit address-folder does not yet express the
        # BP+{SI,DI} pair restriction to allocation. Let explicit QB
        # base-plus-offset MIR survive instead of recreating an illegal
        # BP+BX LEA below HIR. Other MIR optimizations remain enabled.
        address_forms=(),
        costs=target.operations,
    )
    transformed = _drop_optimizer_resume_edges(transformed, resume_edges)
    if temporary_root is not None:
        transformed = replace(
            transformed,
            entry=body.body.entry,
            blocks=tuple(block for block in transformed.blocks if block.at != temporary_root),
        )
    checked, _root = _machine_side_entry(transformed, entries)
    problems = mir.verify(checked)
    if problems:
        raise EmissionError(f"optimized external-entry body is invalid: {problems[:3]}")
    return replace(body, body=transformed)


def lowering_target() -> targets.Profile:
    """The 386 profile with only address forms valid for QB far-array HIR.

    A dynamic BASIC array explicitly loads both words of its far data pointer.
    The generic secondary SIB folder widens those word definitions before the
    far-load selector combines them into LES, leaving the folder's promoted
    values without definitions. Keep native 16-bit forms; only the conflicting
    secondary form is outside this frontend's lowering contract.
    """
    target = targets.profile("386")
    return replace(target, address_forms=tuple(form for form in target.address_forms if not form.secondary))


def _machine_side_entry(body: mir.MirBody, entries: tuple[int, ...]) -> tuple[mir.MirBody, int | None]:
    """Give the one-entry machine pipeline a temporary external-entry switch.

    An empty block with several successors is sufficient for graph analyses,
    but it is not a machine CFG: no instruction chooses an edge.  Use an
    unconstrained entry value and a real semantic switch so every machine
    phase sees valid control flow.  The switch and every comparison block its
    lowering creates are discarded before source ABI emission.
    """
    # A statement's original block may have been merged into an earlier block
    # by MIR optimization.  Its instruction identity still lets
    # `_split_statement_blocks` recover the exact runtime label after
    # allocation; naming the vanished source block here instead leaves a
    # dangling successor.  This root exists only to retain independently
    # reachable blocks which are still physical CFG roots.
    labels = {block.at for block in body.blocks}
    entries = tuple(one for one in dict.fromkeys((body.entry, *entries)) if one in labels)
    if len(entries) == 1:
        return body, None
    root = max((block.at for block in body.blocks), default=0) + 1
    operation_at = max((op.at for block in body.blocks for op in block.ops), default=0) + 1
    value_id = (
        max(
            (
                value.id
                for block in body.blocks
                for value in (
                    *(phi.result for phi in block.phis),
                    *(value for op in block.ops for value in (*op.defines, *op.uses)),
                )
                if value.id is not None
            ),
            default=0,
        )
        + 1
    )
    selector = mir.Value(value_id, operation_at, variable=value_id, version=1)
    switch = mir.Op(
        operation_at,
        ir.Operation.JUMP,
        "",
        (),
        (selector,),
        kind=mir.Kind.SWITCH,
        args=(mir.Held(selector, 2),),
        # The default is the real language entry.  FinalControlFlow lays a
        # switch's default path after its comparison chain; once that
        # analysis-only chain is removed, the BASIC runtime must still find
        # the ordinary entry at O_ENT rather than an error-handler side root.
        target=entries[0],
        cases=tuple((number, target) for number, target in enumerate(entries[1:], 1)),
        reads_complete=True,
    )
    made = replace(body, entry=root, blocks=(mir.MirBlock(root, (), (switch,), entries), *body.blocks))
    problems = mir.verify(made)
    if problems:
        raise EmissionError(f"temporary ON ERROR root is invalid: {problems[:3]}")
    return made, root


def _drop_machine_side_entry(
    body: lir.LirBody,
    roots: frozenset[int],
    entry: int,
    entry_fallback: int | None,
) -> lir.LirBody:
    if not roots:
        return body
    blocks = tuple(block for block in body.blocks if block.at not in roots)
    labels = {block.at for block in blocks}
    if entry not in labels:
        # FinalControlFlow may thread an empty source entry into its first
        # statement. The source ABI still needs a distinct pre-statement
        # location for B$ENRA and ON LOCAL ERROR registration: putting those
        # on the resumable statement itself would re-enter the frame when the
        # runtime resumes there. Recreate only that ABI wrapper and make its
        # transfer explicit, since machine layout has already run.
        if entry_fallback is None or entry_fallback not in labels:
            raise EmissionError(f"{body.name}: machine pipeline removed the BASIC entry target")
        at = max((one.at for block in blocks for one in block.insns), default=0) + 1
        jump = lir.Insn(
            at,
            (at, at),
            ir.Semantics(ir.Operation.JUMP, "jmp", target=entry_fallback),
            (),
            (),
        )
        blocks = (lir.LirBlock(entry, (jump,), (entry_fallback,), ()), *blocks)
    return replace(body, entry=entry, blocks=blocks)


_SEGMENT_SHAPE = {
    "COMMON": (0x78, "BLANK"),
    "BC_DATA": (0x48, "BC_DATA"),
    "BC_FT": (0x48, "BC_SEGS"),
    "BC_CN": (0x68, "BC_SEGS"),
    "BC_DS": (0x68, "BC_SEGS"),
    "BC_SAB": (0x48, "BC_SEGS"),
    "BC_SA": (0x48, "BC_SEGS"),
    "FSL_CONST": (0x60, "FAR_DATA"),
}


def _basic_segment_classes(data: bytes, code: str) -> bytes:
    """Apply the BASIC segment classes/combine modes to a fresh OMF envelope."""
    records = omf.parse(data)
    old_names = omf.names(records)
    names = list(old_names)
    for name in ("BC_CODE", "BLANK", "BC_DATA", "BC_SEGS"):
        if name not in names:
            names.append(name)
    name_index = {name: index for index, name in enumerate(names) if index}
    rewritten: list[omf.Record] = []
    lnames_done = False
    for record in records:
        if record.type & 0xFE == omf.LNAMES:
            if lnames_done:
                raise EmissionError("fresh OMF unexpectedly contains multiple LNAMES records")
            body = b"".join(bytes([len(name.encode("latin1"))]) + name.encode("latin1") for name in names[1:])
            rewritten.append(omf.Record(record.type, body))
            lnames_done = True
            continue
        if record.type & 0xFE != omf.SEGDEF:
            rewritten.append(record)
            continue
        body = record.body
        at = 1 + (3 if body[0] >> 5 == 0 else 0) + 2
        segment_name_index, after_name = omf._index(body, at)
        _class_index, after_class = omf._index(body, after_name)
        overlay_index, after_overlay = omf._index(body, after_class)
        segment_name = old_names[segment_name_index]
        acbp, class_name = (
            (0x68, "BC_CODE")
            if segment_name == code
            else _SEGMENT_SHAPE.get(segment_name, (body[0], old_names[_class_index]))
        )
        made = (
            bytes([acbp])
            + body[1:at]
            + omf.as_index(segment_name_index)
            + omf.as_index(name_index[class_name])
            + omf.as_index(overlay_index)
            + body[after_overlay:]
        )
        rewritten.append(omf.Record(record.type, made))
    return b"".join(record.emit() for record in rewritten)


def assembled(program: hir.Program) -> masm.Module:
    """Compile one QB HIR module to the shared assembly model."""
    hir.verify(program)
    if len(program.modules) != 1:
        raise EmissionError("one OMF object represents exactly one QB module")
    module = program.modules[0]
    semantic = hir.lower(program)
    functions = tuple(module.functions)
    if len(semantic) != len(functions):
        raise EmissionError("HIR lowering did not preserve the function table")

    callable_names = {one.name: _object_name(one.name) for one in module.callables}
    defined = {_object_name(one.name) for one in module.callables if one.defined}
    procedures: list[masm.Procedure] = []
    code_names: dict[int, str] = {}
    statement_metadata = _statement_metadata(module)
    statement_targets: list[tuple[int, int, str, int]] = []
    referenced_calls: set[str] = set()
    for function, body in zip(functions, semantic, strict=True):
        handler_at = _handler_at(function)
        body = optimized(program, function, body)
        physical = physicalize(program, function, body)
        ordinary_entry = physical.lowered.body.entry
        ordinary_block = physical.lowered.body.block(ordinary_entry)
        ordinary_fallback = (
            ordinary_block.succ[0] if ordinary_block is not None and len(ordinary_block.succ) == 1 else None
        )
        external_entries = tuple(
            dict.fromkeys(
                (
                    *function.external_entries,
                    *(() if handler_at is None else (handler_at,)),
                )
            )
        )
        machine_body, temporary_root = _machine_side_entry(
            physical.lowered.body,
            external_entries,
        )
        low = flow.verified(
            lower.lowered(
                body.name,
                machine_body,
                physical.calls,
                set(),
                physical.contracts,
                cpu=lowering_target(),
                occurrences={},
                hints=physical.hints,
                pointer_model=physical.pointer_model,
            ),
            "lower",
            in_ssa=True,
        )
        temporary_blocks = (
            frozenset(block.at for block in low.blocks) - frozenset(block.at for block in physical.lowered.body.blocks)
            if temporary_root is not None
            else frozenset()
        )
        owned_frame = frame.of(low, physical.calls, family=program.runtime.value)
        in_ssa = True
        for phase in flow.machine({}, owned_frame, physical.calls, basic_semantics=True):
            # masm.Procedure owns a native BP frame and reserves the complete
            # local/spill extent. The shared Prologue pass is for an already
            # existing BC/runtime frame and would reserve the spill tail twice.
            if isinstance(phase, prologue.Prologue):
                continue
            if isinstance(phase, phielim.PhiElimination):
                in_ssa = False
            low = flow.checked(low, phase, in_ssa=in_ssa)
        low = _drop_machine_side_entry(
            low,
            temporary_blocks,
            ordinary_entry,
            ordinary_fallback,
        )
        final = finalized(low, parameter_bytes=function.abi.parameter_bytes if function.abi else 0)
        callees = dict(final.callees)
        resume_blocks: dict[int, int] = {}
        for at, name in physical.calls.items():
            if name.startswith("$QB$RESA:"):
                try:
                    target_block = int(name.removeprefix("$QB$RESA:"))
                except ValueError as error:
                    raise EmissionError(f"invalid RESUME target marker {name!r}") from error
                resume_blocks[at] = target_block
                object_name = "B$RESA"
            else:
                object_name = callable_names.get(name, name)
            referenced_calls.add(object_name)
            callees[at] = masm.Callee(object_name, at in physical.far_calls)
        reserve = -min(min(owned_frame.slots.values(), default=0), owned_frame.floor)
        final_body = _source_instructions(final.body)
        public = function.name != "__main"
        if public:
            final_body, runtime_frame = _runtime_frame(
                final_body,
                reserve,
                program.runtime,
                _temporary_string_slots(module, function),
            )
            callees.update(runtime_frame)
            referenced_calls.update(("B$ENRA", "B$EXSA"))
        else:
            # A module body normally has no frame in BC output.  When our
            # allocator needs spill space, however, a merely native BP frame
            # is invisible to B$GETMODCODE: the first DATA/READ call then
            # finds no module header and reports a spurious syntax error.
            # B$ENRA/B$EXSA are the measured QB45/PDS/VBDOS frame protocol
            # used by every source procedure, and make the spill frame part
            # of the runtime's own chain.  With no spill there remains no
            # entry/exit overhead, matching the ordinary module shape.
            if reserve:
                final_body, initialize = _runtime_frame(
                    final_body,
                    reserve,
                    program.runtime,
                    0,
                )
                referenced_calls.update(("B$ENRA", "B$EXSA"))
            else:
                final_body, initialize = _initialize_frame(final_body, reserve)
            callees.update(initialize)
        final_body = _address_values(final_body)
        if function.error_handler is not None:
            assert handler_at is not None
            handler_key = -(len(code_names) + 1)
            code_names[handler_key] = masm.label(len(procedures), handler_at)
            enter_at = next(
                (at for at, callee in callees.items() if callee.name == "B$ENRA"),
                None,
            )
            final_body, registration = _register_error_handler(
                final_body,
                Addr(Space.SEGMENT, 0, handler_key),
                local=function.error_handler_local,
                after_at=enter_at if function.error_handler_local else None,
            )
            callees.update(registration)
            referenced_calls.add("B$OEGP" if function.error_handler_local else "B$OEGA")
        if not public:
            final_body, exits = _ends_program(final_body)
            callees.update(exits)
            referenced_calls.add("B$CEND")
        final_body = _drop_resume_successors(final_body, physical.calls)
        procedure_number = len(procedures)
        statement_blocks = _statement_table_blocks(function)
        source_instructions = body.source_instructions or {}
        all_rows = tuple(
            (source_block, source_instructions[instruction], line)
            for function_id, source_block, instruction, line in statement_metadata
            if function_id == function.id and instruction in source_instructions
        )
        statement_instructions = {source_block: instruction for source_block, instruction, _line in all_rows}
        rows = tuple(
            (source_block, instruction, line)
            for source_block, instruction, line in all_rows
            if source_block in statement_blocks
        )
        resume_markers = {
            statement_instructions[target] for target in resume_blocks.values() if target in statement_instructions
        }
        final_body, statement_labels = _split_statement_blocks(
            final_body,
            frozenset(instruction for _block, instruction, _line in rows) | resume_markers,
        )
        final_body = _resume_label_transfers(
            final_body,
            resume_blocks,
            statement_instructions,
            statement_labels,
            procedure_number,
            code_names,
        )
        layout_order = {block.at: index for index, block in enumerate(final_body.blocks)}
        for _source_block, instruction, line in rows:
            at = statement_labels.get(instruction)
            if at is not None:
                statement_targets.append(
                    (
                        procedure_number,
                        layout_order[at],
                        masm.label(procedure_number, at),
                        line,
                    )
                )
        procedures.append(
            masm.Procedure(
                "$QB$MAIN" if not public else _object_name(function.name),
                public,
                True,
                final_body,
                # B$ENRA, when present, owns both the ten-byte runtime header
                # and the CX bytes of locals below BP.  Asking masm's native
                # shell to reserve those bytes first shifts FR_BFRAME,
                # FR_CLOCALS, and FR_GOSUB away from their documented offsets;
                # ON ERROR then reads a spill as the local count and reports
                # Out of stack space.  Every nonzero reserve selected the
                # runtime-frame path above, so the native shell owns none.
                0,
                callees,
            )
        )

    procedures.append(_statement_procedure(tuple(sorted(statement_targets))))
    names, data_by_segment = _data(module)
    names.update(((Space.SEGMENT, key), name) for key, name in code_names.items())
    read_data = tuple(
        item for line in _read_data_lines(module) for item in (masm.Pointer("$QB$MAIN", 0, False), line + b"\0")
    )
    external_data = {object_.name for object_ in module.data if object_.linkage is hir.DataLinkage.EXTERNAL}
    externs = {(name, "far") for name in referenced_calls - defined}
    externs.update((name, "byte") for name in external_data)
    code = f"{_object_name(module.name)}_CODE"
    basic_data = (
        ("COMMON", (masm.Label("$QB$COMMON"),)),
        ("BC_DATA", (masm.Label("$QB$DATA"), bytes(6), *data_by_segment["BC_DATA"])),
        ("BC_FT", (masm.Label("$QB$FT"),)),
        ("BC_CN", (masm.Label("$QB$CN"), *data_by_segment["BC_CN"])),
        ("BC_DS", (masm.Label("$QB$DS"), *read_data, b"\xff\xff\x01")),
        ("BC_SAB", (masm.Label("$QB$SAB"),)),
        ("BC_SA", (masm.Label("$QB$SA"), masm.Pointer("$QB$HEADER", 0, True))),
        ("FSL_CONST", data_by_segment["FSL_CONST"]),
    )
    return masm.Module(
        code=code,
        names=names,
        externs=tuple(sorted(externs)),
        publics=tuple(procedure.name for procedure in procedures if procedure.public),
        data=basic_data,
        procedures=tuple(procedures),
        private=frozenset({"FSL_CONST"}),
    )


def object_bytes(program: hir.Program, source: str | Path) -> bytes:
    """Emit a complete fresh BASIC-envelope OMF object."""
    module = assembled(program)
    # Build the same semantic segments as backend.omfwrite.written, then add
    # the BASIC-owned MODULE_CODE envelope before asking its canonical record
    # serializer to write OMF. This stays frontend-owned and leaves the shared
    # assembly model and writer contract unchanged.
    segments = [omfwrite.Segment(module.code, "CODE", False)]
    named = {"_DATA": omfwrite.Segment("_DATA", "DATA", True)}
    for name, _items in module.data:
        if name not in named:
            private = name in module.private
            named[name] = omfwrite.Segment(name, "FAR_DATA" if private else "DATA", not private)
    segments += named.values()
    symbols: dict[str, tuple[int, int]] = {}
    for name, items in module.data:
        index = [one.name for one in segments].index(name)
        _object_data(segments[index], index, items, symbols)
    omfwrite._code(segments[0], module, symbols)

    header = _header(program)
    code = segments[0]
    code.image = bytearray(header) + code.image
    code.spans = [[0, 48], *[[start + 48, end + 48] for start, end in code.spans]]
    if not module.procedures or module.procedures[-1].name != "$QB$STAT":
        raise EmissionError("the BASIC statement table must be the final code procedure")
    statement_data = masm.label(len(module.procedures) - 1, 1)
    code.fixups = [
        # The table is data carried by an opaque inline item.  The generic
        # assembly model conservatively emits a private BP prologue before
        # such an item, so OF_STA must name its first block label after that
        # prologue, not the procedure symbol.  Pointing at PUSH BP made ERL
        # read 3C00h and RESUME NEXT scan instruction bytes as table rows.
        omfwrite.Fixup(10, omfwrite.OFFSET, statement_data),
        omfwrite.Fixup(12, omfwrite.OFFSET, "$QB$DS"),
        omfwrite.Fixup(14, omfwrite.OFFSET, "$QB$DATA"),
        omfwrite.Fixup(16, omfwrite.OFFSET, "$QB$FT"),
        omfwrite.Fixup(24, omfwrite.OFFSET, "$QB$COMMON"),
        omfwrite.Fixup(32, omfwrite.OFFSET, "$QB$CN"),
        *(replace(one, at=one.at + 48) for one in code.fixups),
    ]
    symbols = {name: (segment, offset + 48 if segment == 0 else offset) for name, (segment, offset) in symbols.items()}
    symbols["$QB$HEADER"] = (0, 0)
    externs = dict(module.externs)
    records = omfwrite._records(module, Path(source).name, segments, symbols, externs)
    emitted = b"".join(record.emit() for record in records)
    return _basic_segment_classes(emitted, module.code)
