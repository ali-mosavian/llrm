"""Frame-derived near pointers retain the stack segment through lowering."""

from pathlib import Path

import pytest
from iced_x86 import Decoder
from iced_x86 import Register

from qbopt import flow
from qbopt.model import ir
from qbopt.objectfile import module
from qbopt.objectfile import omf
from qbopt.rewrite import rewrite
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.backend import frame
from qbopt.backend import lower
from qbopt.backend import select
from qbopt.backend import phielim
from qbopt.backend import prologue
from qbopt.objectfile.module import Addr
from qbopt.frontend.declen import BITNESS
from qbopt.objectfile.module import Space
from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


class _Values:
    next = 100

    def fresh(self) -> int:
        self.next += 1
        return self.next


def _lowered_fill(
    value: mir.Arg,
    count: mir.Arg,
    *,
    space: Space = Space.FRAME,
    offset: int = -1024,
    preserve_flags: bool = False,
) -> tuple[ir.Semantics, ...]:
    op = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.FILL,
        args=(value, count, mir.Const(0, 2)),
        stores=(mir.MemRef(Addr(space, offset), value.width, space=space),),
    )
    return lower._fill(op, _Values(), preserve_flags=preserve_flags)


def test_a_frame_derived_indirect_cell_keeps_its_stack_segment() -> None:
    """Matmul overwrote loop spills when ``[bx]`` defaulted to DS instead of SS."""
    cell = ir.Mem(Addr(Space.LITERAL, 0, segment=Register.SS), 4, Register.BX, base=ir.Held(1, 2))
    made = select.move_into(cell, Register.EAX)

    assert made is not None
    got = next(iter(Decoder(BITNESS, made.code, ip=0)))
    assert got.memory_base == Register.BX
    assert got.memory_segment == Register.SS


def test_frame_fill_lowering_uses_the_stack_segment() -> None:
    """A frame memset must copy SS, rather than DS, into string-destination ES."""
    lowered = _lowered_fill(mir.Const(0, 1), mir.Const(32, 2), offset=-32)

    pushed = [one.sources for one in lowered if one.op is ir.Operation.PUSH]
    assert (ir.Reg(Register.SS, 2),) in pushed
    assert (ir.Reg(Register.DS, 2),) not in pushed


def test_constant_byte_fill_uses_only_dwords_when_there_is_no_remainder() -> None:
    """A 1024-byte clear took 1024 STOSB iterations instead of 256 STOSD iterations.

    An exactly divisible constant count has no residual operation to emit.
    """

    lowered = _lowered_fill(mir.Const(0, 1), mir.Const(1024, 2))

    assert [one.name for one in lowered if one.op is ir.Operation.FILL] == ["stosd"]


@pytest.mark.parametrize(
    ("count", "expected"),
    [
        (0, []),
        (1, ["stosb"]),
        (2, ["stosw"]),
        (3, ["stosw", "stosb"]),
        (4, ["stosd"]),
        (1024, ["rep stosd"]),
        (1025, ["rep stosd", "stosb"]),
        (1026, ["rep stosd", "stosw"]),
        (1027, ["rep stosd", "stosw", "stosb"]),
    ],
)
def test_constant_byte_fill_uses_the_exact_widest_store_sequence(count: int, expected: list[str]) -> None:
    """A constant tail used counted byte stores even when one word and byte suffice."""
    lowered = _lowered_fill(mir.Const(0xA5, 1), mir.Const(count, 2))

    assert [
        line for one in lowered if one.op is ir.Operation.FILL for line in masm._instruction(one, {}, 0)
    ] == expected


def test_a_single_string_store_has_no_rep_prefix() -> None:
    """One residual byte paid for ``mov cx,1; rep stosb`` instead of ``stosb``."""
    one = ir.Semantics(
        ir.Operation.FILL,
        "stosb",
        (ir.Mem(None, 0), ir.Reg(Register.DI, 2)),
        (ir.Reg(Register.AL, 1), ir.Reg(Register.DI, 2), ir.Reg(Register.ES, 2)),
    )

    emitted = select.emit(one)
    assert emitted is not None
    assert emitted.code == bytes.fromhex("aa")
    assert masm._instruction(one, {}, 0) == ["stosb"]


def test_narrow_constant_tails_reuse_the_wide_accumulator_value() -> None:
    """The word and byte tail reloaded AX and AL after EAX already held both."""
    lowered = _lowered_fill(mir.Const(0xA5, 1), mir.Const(1027, 2))
    patterns = {0xA5, 0xA5A5, 0xA5A5A5A5}
    loaded = [
        source
        for one in lowered
        if one.op is ir.Operation.MOVE
        for source in one.sources
        if isinstance(source, ir.Imm) and source.value in patterns
    ]

    assert loaded == [ir.Imm(0xA5A5A5A5, 4)]


def test_allocated_constant_fill_loads_its_accumulator_once() -> None:
    """A 1027-byte fill reloaded EAX before each narrower residual store."""
    op = mir.Op(
        1,
        ir.Operation.FILL,
        "fill",
        (),
        (),
        kind=mir.Kind.FILL,
        args=(mir.Const(0xA5, 1), mir.Const(1027, 2), mir.Const(0, 2)),
        stores=(mir.MemRef(Addr(Space.FRAME, -1027), 1, space=Space.FRAME),),
    )
    source = mir.MirBody(1, (mir.MirBlock(1, (), (op,), ()),))
    body = flow.verified(lower.lowered("fill", source, {}, set(), {}), "lower", in_ssa=True)
    owned = frame.of(body, {})
    in_ssa = True
    for phase in flow.machine(flow._pinned(body), owned, {}, cpu="386"):
        if isinstance(phase, prologue.Prologue):
            continue
        if isinstance(phase, phielim.PhiElimination):
            in_ssa = False
        body = flow.checked(body, phase, in_ssa=in_ssa)

    listing = [line for one in body.insns if one.what for line in masm._instruction(one.what, {}, 0)]
    relevant = [line for line in listing if "stos" in line or line.startswith(("mov eax", "mov ax", "mov al"))]

    assert relevant == ["mov eax, 2779096485", "rep stosd", "stosw", "stosb"]


def test_constant_fill_replicates_the_element_across_the_dword() -> None:
    """Widening is a constant-element rule, not a zero-fill special case."""

    lowered = _lowered_fill(mir.Const(0xA5, 1), mir.Const(100, 2), space=Space.SEGMENT, offset=0)
    immediates = [source for one in lowered for source in one.sources if isinstance(source, ir.Imm)]

    assert ir.Imm(0xA5A5A5A5, 4) in immediates
    assert [one.name for one in lowered if one.op is ir.Operation.FILL] == ["stosd"]


def test_runtime_byte_fill_has_a_dword_bulk_and_byte_remainder() -> None:
    """An unknown byte count still uses the widest bulk operation and a bounded tail."""

    lowered = _lowered_fill(mir.Const(0, 1), mir.Held(mir.Value(7, 0, mir.Kind.COPY), 2))

    assert [one.name for one in lowered if one.op is ir.Operation.FILL] == ["stosd", "stosb"]


def test_runtime_fill_keeps_its_original_width_when_flags_are_live() -> None:
    """Deriving a runtime quotient must not clobber flags that survive the semantic fill."""

    lowered = _lowered_fill(mir.Const(0, 1), mir.Held(mir.Value(7, 0, mir.Kind.COPY), 2), preserve_flags=True)

    assert [one.name for one in lowered if one.op is ir.Operation.FILL] == ["stosb"]
    assert not any(one.op is ir.Operation.BINARY for one in lowered)


@pytest.mark.full
def test_local_array_fill_uses_the_stack_segment_end_to_end() -> None:
    """Sieve returned zero: its frame memset copied DS to ES and cleared unrelated data."""
    text = cfront.compiled((FIXTURES / "fill.cgs").read_text(), "fill", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    start, end = lines.index("_fill_local proc far"), lines.index("_fill_local endp")
    body = lines[start:end]

    assert "push ss" in body
    assert "push ds" not in body


def test_a_frame_descriptor_through_a_near_pointer_has_no_stack_override() -> None:
    """nestud grew 12 bytes of `ss:` once its descriptor loads carried frame provenance."""
    out, _ = rewrite(
        (Path(__file__).resolve().parents[1] / "fixtures" / "omf" / "nestud-q-O.obj").read_bytes(), dry_run=False
    )
    code = module.of(omf.parse(out)).code

    overridden = [
        one
        for one in Decoder(BITNESS, code, ip=0)
        if one.segment_prefix == Register.SS and one.memory_base in (Register.SI, Register.DI, Register.BX)
    ]

    assert not overridden
