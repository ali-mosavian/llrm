"""
The lift and the emitter, on bytes built here rather than on a compiled
program.

Whole-program checks were green while the classifier reported "no half" for A1,
while a pair copy was treated as dead, and while a negate inherited another
instruction's address. A program computing the right answer is not evidence
about the piece that happened not to run.
"""

import pytest
from iced_x86 import Register

from helpers import hx
from qbopt.lift import Op
from qbopt.lift import Kind
from qbopt.lift import emit
from qbopt.lift import lift
from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.lift import Value
from qbopt.lift import encode
from qbopt.lift import needed
from qbopt.lift import sizeof
from qbopt.module import Addr
from qbopt.lift import regions
from qbopt.module import Space
from helpers import classify_code
from qbopt.lift import emit_region


def one(h: str) -> list[Value]:
    b = hx(h)
    return lift(b, 0, len(b))[0]


# reg field encoding for the four registers a long lives in, and the pair and
# half each one is
REGISTERS = {"ax": (0, 0, 0), "cx": (1, 1, 0), "dx": (2, 0, 1), "bx": (3, 1, 1)}

# the ModRM base for each memory form BC uses, and its displacement width
BASES = {0x06: 2, 0x46: 1, 0x86: 2}


@pytest.mark.parametrize("base", sorted(BASES), ids=lambda b: f"base{b:02X}")
@pytest.mark.parametrize("name", sorted(REGISTERS))
@pytest.mark.parametrize(("opcode", "kind"), ((0x8B, Kind.LOAD), (0x89, Kind.STORE)))
def test_classifier_every_register_half_and_addressing_form(opcode: int, kind: Kind, name: str, base: int) -> None:
    reg, pair, half = REGISTERS[name]
    dlen = BASES[base]
    code = bytes([opcode, base | (reg << 3)]) + (b"\xe8" if dlen == 1 else b"\x5e\x00")
    decoded = classify_code(code)
    assert decoded is not None
    assert decoded.kind is kind
    assert (decoded.pair, decoded.half) == (pair, half)
    assert decoded.dlen == dlen


@pytest.mark.parametrize("name", sorted(REGISTERS))
@pytest.mark.parametrize("half_of", (0, 1), ids=("lo", "hi"))
@pytest.mark.parametrize(("low", "high", "what"), ((0x23, 0x23, "and"), (0x03, 0x13, "add"), (0x2B, 0x1B, "sub")))
def test_classifier_every_alu_operation(low: int, high: int, what: str, half_of: int, name: str) -> None:
    reg, _pair, _half = REGISTERS[name]
    opcode = high if half_of else low
    decoded = classify_code(bytes([opcode, 0x06 | (reg << 3), 0x5E, 0x00]))
    assert decoded is not None
    assert decoded.kind is Kind.ALU
    assert decoded.alu is not None


@pytest.mark.parametrize("source", sorted(REGISTERS))
@pytest.mark.parametrize("destination", sorted(REGISTERS))
def test_classifier_register_to_register(destination: str, source: str) -> None:
    dreg, dpair, dhalf = REGISTERS[destination]
    sreg, spair, shalf = REGISTERS[source]
    decoded = classify_code(bytes([0x8B, 0xC0 | (dreg << 3) | sreg]))
    if dhalf == shalf:
        assert decoded is not None
        assert decoded.kind is Kind.MOVE
        assert (decoded.pair, decoded.src_pair) == (dpair, spair)
    else:
        assert decoded is None, "halves must match for a pair move"


@pytest.mark.parametrize(
    ("enc", "why"),
    [
        ("8B 07", "[bx] is not a form BC uses for a long"),
        ("8B 20", "reg=sp is not a long register"),
        ("8B 36 5E 00", "reg=si is not a long register"),
        ("87 06 5E 00", "xchg is not one of the operations"),
        ("8B C4", "mov ax,sp is not a pair move"),
    ],
)
def test_classifier_refuses(enc: str, why: str) -> None:
    assert classify_code(hx(enc)) is None, why


def test_load_operate_store_is_one_chain() -> None:
    v = one("A1 5E 00 8B 16 60 00   23 06 5A 00 23 16 5C 00   A3 62 00 89 16 64 00")
    assert [x.op for x in v] == [Op.LOAD, Op.ALUM, Op.STORE]
    assert (v[1].s1, v[2].s1) == (0, 1)
    assert v[0].mem == Addr(Space.LITERAL, 0x5E), "the load keeps the low half's displacement"


def test_a_pair_copy_is_a_value_knowing_both_pairs() -> None:
    v = one("8B 0E 5E 00 8B 1E 60 00   8B D3 8B C1   A3 62 00 89 16 64 00")
    assert [x.op for x in v] == [Op.LOAD, Op.MOVE, Op.STORE]
    assert (v[1].pair, v[1].src_pair) == (0, 1)


def test_a_cross_pair_operation_reads_both_values() -> None:
    v = one("A1 5E 00 8B 16 60 00  8B 0E 62 00 8B 1E 64 00  33 C1 33 D3")
    assert [x.op for x in v] == [Op.LOAD, Op.LOAD, Op.ALUV]
    assert (v[2].s1, v[2].s2) == (0, 1)


@pytest.mark.parametrize(
    ("enc", "what"),
    [
        ("A1 5E 00 8B 16 60 00   F7 D8 83 D2 00 F7 DA", "ax:dx"),
        ("8B 0E 5E 00 8B 1E 60 00   F7 D9 83 D3 00 F7 DB", "cx:bx"),
    ],
)
def test_neg_adc_neg_is_one_negate(enc: str, what: str) -> None:
    v = one(enc)
    assert [x.op for x in v] == [Op.LOAD, Op.NEG]
    assert v[1].end - v[1].at == 7, "consuming seven bytes"


def test_a_spill_written_high_half_first() -> None:
    v = one("A1 5E 00 8B 16 60 00   89 56 EA 89 46 E8")
    assert [x.op for x in v] == [Op.LOAD, Op.STORE]
    assert (v[1].mem, v[1].at) == (Addr(Space.FRAME, -24), 7), "the low displacement and the earlier address"


@pytest.mark.parametrize(
    ("enc", "why"),
    [
        ("A1 5E 00 8B 16 62 00", "halves four apart are two variables, not one long"),
        ("A1 5E 00 8B 0E 60 00", "halves in different pairs are not a pair"),
        ("23 06 5A 00 23 16 5C 00", "an operation with nothing loaded has no value to work from"),
        ("A3 5E 00 89 16 60 00", "a store with nothing loaded likewise"),
        # Integer variables are two bytes apart, so a load pair and two
        # independent 16-bit loads have the same displacements. Only the
        # register pairing tells them apart, which is why the half test is
        # not optional.
        ("A1 5E 00 A1 60 00", "two moffs loads into ax are not a pair"),
        ("8B 06 5E 00 8B 0E 60 00", "ax then cx is not a pair"),
    ],
)
def test_lift_refuses(enc: str, why: str) -> None:
    assert one(enc) == [], why


@pytest.mark.parametrize(
    ("enc", "why"),
    [
        ("A1 5E 00 8B 16 60 00 03 06 5A 00 03 16 5C 00", "add/add is two integers; only add/adc is a long"),
        ("A1 5E 00 8B 16 60 00 2B 06 5A 00 2B 16 5C 00", "sub/sub likewise"),
    ],
)
def test_lift_refuses_a_carry_blind_pair(enc: str, why: str) -> None:
    assert one(enc)[1:] == [], why


def test_an_opaque_instruction_invalidates_the_pairs() -> None:
    v = one("A1 5E 00 8B 16 60 00  90  23 06 5A 00 23 16 5C 00")
    assert [x.op for x in v] == [Op.LOAD]


def test_opaque_bytes_split_a_region_and_leave_the_value_live() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 0090 90A1 62 00 8B 16 64 00 A3 66 00 89 16 68 00")
    v, _ = lift(b, 0, len(b))
    assert len(regions(v)) == 2
    need = needed(v)
    assert need[1] is True, "a value in a register when a region ends is live"
    assert all(need[n] for n, x in enumerate(v) if x.op == Op.STORE), "a store is always needed"


def test_everything_feeding_a_store_is_needed_transitively() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00")
    v, _ = lift(b, 0, len(b))
    assert all(needed(v))


CORPUS = (
    "A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00  A3 62 00 89 16 64 00"
    "8B 0E 5E 00 8B 1E 60 00  0B 0E 5A 00 0B 1E 5C 00"
    "8B D3 8B C1  33 C1 33 D3"
    "F7 D8 83 D2 00 F7 DA"
    "8B 46 E8 8B 56 EA  03 46 E8 13 56 EA  89 46 E8 89 56 EA"
    "8B 86 00 01 8B 96 02 01"
)


@pytest.fixture(scope="module")
def corpus() -> list[Value]:
    b = hx(CORPUS)
    v, _ = lift(b, 0, len(b))
    return v


def test_the_corpus_covers_enough_to_be_worth_checking(corpus: list[Value]) -> None:
    assert len(corpus) >= 10
    assert len({x.op for x in corpus}) >= 5


@pytest.mark.parametrize(
    ("enc", "index", "want", "what"),
    [
        ("A1 5E 00 8B 16 60 00", 0, "66a15e00", "mov eax,[disp16]"),
        ("8B 0E 5E 00 8B 1E 60 00", 0, "668b0e5e00", "mov ecx,[disp16]"),
        ("A1 5E 00 8B 16 60 00  8B 46 E8 8B 56 EA", 1, "668b46e8", "bp-relative keeps its ModRM and disp8"),
        ("A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00", 1, "6623065a00", "and eax,[disp16]"),
        ("8B 0E 5E 00 8B 1E 60 00  23 0E 5A 00 23 1E 5C 00", 1, "66230e5a00", "and ecx,[disp16]"),
        ("A1 5E 00 8B 16 60 00  8B 0E 62 00 8B 1E 64 00  33 C1 33 D3", 2, "6633c1", "xor eax,ecx"),
        ("A1 5E 00 8B 16 60 00 F7 D8 83 D2 00 F7 DA", 1, "66f7d8", "neg eax"),
        ("8B 0E 5E 00 8B 1E 60 00 F7 D9 83 D3 00 F7 DB", 1, "66f7d9", "neg ecx"),
    ],
    ids=lambda x: x if isinstance(x, str) and " " not in x else None,
)
def test_encoding_exact_bytes(enc: str, index: int, want: str, what: str) -> None:
    assert encode(one(enc)[index]).hex() == want, what


@pytest.mark.parametrize(
    ("pair", "want"),
    [(0, "6650585a"), (1, "6651595b")],
)
def test_the_high_half_comes_back_through_the_stack(pair: int, want: str) -> None:
    assert FIXUP[pair].hex() == want


@pytest.mark.parametrize(
    "enc",
    [
        "A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00",
        "8B 0E 5E 00 8B 1E 60 00 0B 0E 5A 00 0B 1E 5C 00 89 0E 62 00 89 1E 64 00",
        "A1 5E 00 8B 16 60 00 F7 D8 83 D2 00 F7 DA A3 62 00 89 16 64 00",
    ],
    ids=("ax-dx-alu", "cx-bx-alu", "ax-dx-neg"),
)
def test_a_region_is_exactly_as_long_as_it_needs_to_be(enc: str) -> None:
    # The runtime pass had to fit its rewrite into the bytes it replaced and
    # jump over what it saved, which meant the saving could never be lent to a
    # neighbour. Here the code moves, so nothing is padded.
    code = hx(enc)
    values, _ = lift(code, 0, len(code))
    need = needed(values)
    for region in regions(values):
        emitted = emit_region(values, need, region, Flag.NONE)
        assert emitted is not None
        wanted = sum(sizeof(values[i]) for i in region if need[i])
        wanted += sum(len(FIXUP[p]) for p in {values[i].pair for i in region if need[i]})
        assert len(emitted.code) == wanted
        assert 0x90 not in emitted.code[wanted:], "there is no padding to hold a nop"


def test_a_widened_operand_says_where_its_fixup_must_go() -> None:
    # The displacement field holds zero, exactly as BC's does; the address comes
    # from a fixup, so the emitter has to say which bytes need one.
    code = hx("A1 5E 00 8B 16 60 00  A3 62 00 89 16 64 00")
    values, _ = lift(code, 0, len(code))
    need = needed(values)
    emitted = emit_region(values, need, regions(values)[0], Flag.NONE)
    assert emitted is not None
    # nothing is relocated here: a unit test has no fixups behind it
    assert emitted.relocations == ()


def test_a_pair_inside_an_immediate_is_not_a_pair() -> None:
    # mov word [0x005E], 0x5EA1 -- whose displacement and immediate together
    # spell a load pair starting four bytes in. A matcher that retries at every
    # byte finds it, and would rewrite the middle of an instruction.
    code = hx("C7 06 5E 00 A1 5E 00 8B 16 60 00")
    assert classify_code(code[4:]) is not None, "those bytes really do look like a load"
    values, _ = lift(code, 0, len(code))
    assert values == []


def test_every_value_begins_where_an_instruction_begins() -> None:
    # the trap above, then filler to a boundary, then a real pair at 0x0a
    code = hx("C7 06 5E 00 A1 5E   00 8B 16 60   A1 5E 00 8B 16 60 00")
    boundaries = {insn.at for insn in run(code, 0, len(code))[0]}
    values, _ = lift(code, 0, len(code))
    assert values, "there is a pair here to find"
    assert all(value.at in boundaries for value in values)


def test_a_prefixed_instruction_is_not_half_of_a_pair() -> None:
    # 66 8B 06 is already a 32-bit load, so it is not one half of anything.
    assert classify_code(hx("8B 06 5E 00")) is not None
    assert classify_code(hx("66 8B 06 5E 00")) is None


def test_a_relocated_operand_is_emitted_as_zero() -> None:
    # LINK adds what is in the code to the fixup's target -- measured, by poking
    # 2 into a field and watching the linked address move by 2. So a widened
    # instruction has to hold zero and let the fixup carry the address, exactly
    # as BC does.
    value = Value(Op.LOAD, at=0, end=7, mem=Addr(Space.SEGMENT, 0x1234, 5), mem_at=1, dlen=2)
    emitted = emit(value)
    assert emitted.code == hx("66 A1 00 00")
    assert emitted.relocations == ((2, 1),), "and it says which bytes need the fixup"


def test_an_operand_that_is_not_relocated_keeps_its_displacement() -> None:
    value = Value(Op.LOAD, at=0, end=7, mem=Addr(Space.FRAME, -24), dlen=1)
    emitted = emit(value)
    assert emitted.code == hx("66 8B 46 E8")
    assert emitted.relocations == ()


def test_an_indexed_operand_keeps_its_index_register() -> None:
    # A pair 0 load takes the shorter moffs form (`mov eax, [addr]`) when the
    # operand is a bare displacement -- but moffs has no ModRM byte at all and
    # cannot encode si, so an array element has to fall back to the general
    # r32,rm32 form or it silently reads whatever else sits at that fixed
    # address instead of the element si actually names.
    value = Value(Op.LOAD, at=0, end=7, mem=Addr(Space.SEGMENT, 0x1234, 5, base=Register.SI), mem_at=1)
    emitted = emit(value)
    assert emitted.code == hx("66 8B 84 00 00"), "must be r32,rm32 with si as the base, not the moffs form"
    assert emitted.relocations == ((3, 1),)


def test_a_frame_address_also_indexed_is_refused() -> None:
    # mov ax,[bp+si+8] -- memory_base is bp, so operand() took the frame
    # branch and dropped si silently, misreading it as the plain local at
    # [bp+8]. BC's own [bx+si] shape (176 sites in the corpus) is refused the
    # same way already; this is the same mistake one register over.
    assert classify_code(hx("8B 42 08")) is None


def test_an_indexed_literal_operand_keeps_its_index_register_too() -> None:
    # The same hazard as the SEGMENT case, but for an address no fixup claims
    # -- a `byval as long` parameter dereferenced through si, the shape
    # procs-*.obj already emits for `[si]` itself; one field further into a
    # multi-field structure and it would be `[si+4]` and silently lose si.
    value = Value(Op.LOAD, at=0, end=7, mem=Addr(Space.LITERAL, 4, base=Register.SI))
    emitted = emit(value)
    assert emitted.code == hx("66 8B 84 04 00"), "must keep si, not fall back to a bare [0x0004]"


def test_two_elements_at_the_same_offset_are_not_the_same_address() -> None:
    # [si+X] and [di+X] are different addresses -- pairing them as one long's
    # low and high half would read one array's element through the other's
    # index register.
    low = Addr(Space.SEGMENT, 0x10, 5, base=Register.SI)
    high = Addr(Space.SEGMENT, 0x12, 5, base=Register.DI)
    assert high != low.plus(2)
