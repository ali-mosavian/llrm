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
from qbopt.lift import tail
from qbopt.declen import run
from qbopt.flags import Flag
from qbopt.lift import FIXUP
from qbopt.lift import Value
from qbopt.lift import Bridge
from qbopt.lift import encode
from qbopt.lift import needed
from qbopt.lift import refuse
from qbopt.lift import sizeof
from qbopt.module import Addr
from qbopt.lift import Emitted
from qbopt.lift import operand
from qbopt.lift import regions
from qbopt.module import Space
from qbopt.declen import decode
from helpers import classify_code
from qbopt.lift import emit_region
from qbopt.module import literal_only


def test_operand_refuses_a_segment_override() -> None:
    # `es: mov ax,[0x1234]` -- operand() has no notion of a segment at all,
    # and would otherwise conflate `es:[x]` with `ds:[x]`. Zero instances in
    # the reachable code of any of the 110 real fixtures (measured), so this
    # is a defensive refusal, not a fix to an observed failure.
    insn = decode(hx("26 8B 06 34 12"), 0)
    assert insn is not None
    assert insn.has_segment_override
    assert operand(insn, literal_only) is None


def test_operand_resolves_the_same_field_without_the_override() -> None:
    insn = decode(hx("8B 06 34 12"), 0)
    assert insn is not None
    assert not insn.has_segment_override
    assert operand(insn, literal_only) == Addr(Space.LITERAL, 0x1234)


def test_operand_refuses_a_group_relative_address() -> None:
    # A fixup naming a GRPDEF rather than a SEGDEF: a different index
    # namespace from Space.SEGMENT's, which operand() has no business
    # resolving as if it were an ordinary address. Zero instances in the
    # reachable code of any of the 110 real fixtures (measured), so this is
    # a defensive refusal too.
    def group_only(_field_offset: int, literal: int) -> Addr:
        return Addr(Space.GROUP, literal, 1)

    insn = decode(hx("8B 06 34 12"), 0)
    assert insn is not None
    assert operand(insn, group_only) is None


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
    ("name", "enc", "want"),
    [
        ("ax-implicit", "05 34 12", 0x1234),
        ("rm16,imm16", "81 C0 34 12", 0x1234),
        ("rm16,imm8 positive", "83 C0 04", 4),
        ("rm16,imm8 negative", "83 C0 FF", 0xFFFF),
    ],
)
def test_classifier_recognises_every_immediate_alu_encoding(name: str, enc: str, want: int) -> None:
    # docs/residue.md's D: three different encodings BC picks between for the
    # same operation depending on what's shortest, and classify() has to read
    # the right 16-bit pattern out of each one.
    decoded = classify_code(hx(enc))
    assert decoded is not None, name
    assert decoded.kind is Kind.ALU_IMM
    assert decoded.imm == want, name


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


@pytest.mark.parametrize(
    ("enc", "want_imm", "what"),
    [
        # docs/residue.md's own two worked D examples, byte-identical to
        # bench/nbody.bas at 0x017e and 0x02ea.
        ("05 00 00 83 D2 04", 0x40000, "add ax,0 / adc dx,4 -- together += 0x40000"),
        ("83 C0 01 83 D2 00", 1, "add ax,1 / adc dx,0 -- together += 1"),
    ],
)
def test_immediate_pair_combines_low_and_high_into_one_alui(enc: str, want_imm: int, what: str) -> None:
    v = one("A1 5E 00 8B 16 60 00   " + enc)
    assert [x.op for x in v] == [Op.LOAD, Op.ALUI], what
    assert v[1].imm == want_imm, what
    assert v[1].alu == "add"


def test_immediate_pair_reads_a_negative_combination_correctly() -> None:
    # sub ax,-1 / sbb dx,-1 -- both halves imm8-sign-extended -- combine to
    # the 32-bit pattern 0xFFFFFFFF, which has to come back as the signed -1
    # iced's own builders accept, not the raw unsigned pattern they reject.
    v = one("A1 5E 00 8B 16 60 00   83 E8 FF 83 DA FF")
    assert [x.op for x in v] == [Op.LOAD, Op.ALUI]
    assert v[1].imm == -1


@pytest.mark.parametrize(
    ("enc", "why"),
    [
        ("05 01 00 83 F2 02", "add ax,1 / xor dx,2 -- different families, not one operation"),
        ("83 C0 01 83 DA 00", "add ax,1 / sbb dx,0 -- add pairs only with adc, never sbb"),
    ],
)
def test_immediate_pair_refuses_mismatched_families(enc: str, why: str) -> None:
    v = one("A1 5E 00 8B 16 60 00   " + enc)
    assert [x.op for x in v] == [Op.LOAD], why


def test_immediate_pair_needs_a_loaded_value() -> None:
    # An immediate ALU pair with nothing already live in the pair is not
    # something BC emits and not something this invents handling for --
    # mirrors Kind.ALU's own "nothing loaded" refusal.
    v = one("05 00 00 83 D2 04")
    assert v == []


@pytest.mark.parametrize(
    ("enc", "want", "why"),
    [
        ("83 C0 05 83 D2 00", "6683c005", "fits a signed byte -- shortest, rm32,imm8"),
        ("83 E8 FF 83 DA FF", "6683e8ff", "-1 also fits a signed byte"),
        ("05 00 00 83 D2 04", "660500000400", "0x40000 needs the full 32 bits -- pair 0 takes the eax shortcut"),
    ],
)
def test_immediate_encoding_on_pair_zero(enc: str, want: str, why: str) -> None:
    v = one("A1 5E 00 8B 16 60 00   " + enc)
    assert encode(v[1]).hex() == want, why


def test_immediate_encoding_on_pair_one_has_no_eax_shortcut() -> None:
    # cx:bx widens into ecx, which has no 1-byte-opcode immediate form --
    # a value too big for imm8 has to take the general rm32,imm32 encoding.
    v = one("8B 0E 5E 00 8B 1E 60 00   81 E1 34 12 81 E3 78 56")
    assert v[1].op is Op.ALUI
    assert encode(v[1]).hex() == "6681e134127856"


def test_an_opaque_instruction_invalidates_the_pairs() -> None:
    # `inc ax` touches a tracked register (ax), so it is not one of E's
    # bridgeable gaps -- it still clears both pairs, same as before E existed.
    v = one("A1 5E 00 8B 16 60 00  40  23 06 5A 00 23 16 5C 00")
    assert [x.op for x in v] == [Op.LOAD]


def test_an_instruction_touching_no_tracked_register_bridges_the_gap() -> None:
    # docs/residue.md's E: `inc si` touches neither ax/dx nor cx/bx, so
    # lift() steps over it without invalidating live[0], and regions() (given
    # the bridge lift() reports) treats the load and the alu pair after the
    # gap as one contiguous region.
    b = hx("A1 5E 00 8B 16 60 00  46  23 06 5A 00 23 16 5C 00")
    v, _stores, bridges = lift(b, 0, len(b))
    assert [x.op for x in v] == [Op.LOAD, Op.ALUM]
    assert bridges == [Bridge(7, 8)]
    assert len(regions(v, bridges)) == 1
    assert len(regions(v)) == 2, "without the bridge, the gap still splits the region"


def test_opaque_bytes_split_a_region_and_leave_the_value_live() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 0090 90A1 62 00 8B 16 64 00 A3 66 00 89 16 68 00")
    v, _, _ = lift(b, 0, len(b))
    assert len(regions(v)) == 2
    need = needed(v)
    assert need[1] is True, "a value in a register when a region ends is live"
    assert all(need[n] for n, x in enumerate(v) if x.op == Op.STORE), "a store is always needed"


def test_everything_feeding_a_store_is_needed_transitively() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00")
    v, _, _ = lift(b, 0, len(b))
    assert all(needed(v))


def test_tail_widens_the_negate_idiom_right_after_a_seeded_call() -> None:
    # docs/residue.md's own G/H worked example: idiv/restore/sub/sbb/neg/adc/
    # neg/store -- the NEGATE idiom right after a call's own result, provably
    # already correct in pair 0 (ir.RESTORE_EFFECTS[0]), which lift()'s own
    # walk could never reach on its own because the call sat in front of it
    # as a wall it does not understand.
    b = hx("F7 D8 83 D2 00 F7 DA  A3 5E 00 89 16 60 00")
    instructions = run(b, 0, len(b))[0]
    seed = Value(Op.CALL, at=-5, end=0, pair=0, absorbed=Emitted(b"\x01"))
    values = tail(instructions, b, len(b), literal_only, seed)
    assert [v.op for v in values] == [Op.CALL, Op.NEG, Op.STORE]


def test_tail_stops_outright_on_the_first_unrecognised_instruction() -> None:
    # Unlike lift()'s own walk, which invalidates and keeps scanning, a call
    # site's tail has nothing to resume into -- the first instruction it does
    # not recognise ends the chain rather than being skipped over.
    b = hx("90  A3 5E 00 89 16 60 00")
    instructions = run(b, 0, len(b))[0]
    seed = Value(Op.CALL, at=-5, end=0, pair=0, absorbed=Emitted(b"\x01"))
    values = tail(instructions, b, len(b), literal_only, seed)
    assert values == [seed]


def test_refuse_still_gates_a_region_a_call_seeds() -> None:
    # Op.CALL is deliberately excluded from computes()'s own divergence
    # check (its flag safety is calls.absorb()'s own SYNTHESISED/ALL gate,
    # checked before this ever runs) -- but the NEG right after it still has
    # to trip refuse() exactly as it would if a plain LOAD had started the
    # region instead. A region a call seeds is not a hole in the gate.
    values = [
        Value(Op.CALL, at=0, end=5, pair=0, absorbed=Emitted(b"\x01")),
        Value(Op.NEG, at=5, end=7, pair=0, s1=0),
    ]
    need = needed(values)
    need[0] = True
    region = [0, 1]
    assert refuse(values, need, region, Flag.ZF) is not None
    assert refuse(values, need, region, Flag.NONE) is None


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
    v, _, _ = lift(b, 0, len(b))
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
    values, _, _ = lift(code, 0, len(code))
    need = needed(values)
    for region in regions(values):
        emitted = emit_region(values, need, region, Flag.NONE, frozenset())
        assert emitted is not None
        wanted = sum(sizeof(values[i]) for i in region if need[i])
        wanted += sum(len(FIXUP[p]) for p in {values[i].pair for i in region if need[i]})
        assert len(emitted.code) == wanted
        assert 0x90 not in emitted.code[wanted:], "there is no padding to hold a nop"


def test_emit_region_drops_a_restore_the_caller_proves_dead() -> None:
    # docs/residue.md's I: the caller (rewrite.py, via qbopt.registers) is the
    # only thing that knows whether dx/bx is ever read again -- emit_region()
    # itself just has to honour what it's told, and honour it per pair.
    code = hx("A1 5E 00 8B 16 60 00   A3 62 00 89 16 64 00")
    values, _, _ = lift(code, 0, len(code))
    need = needed(values)
    region = regions(values)[0]
    live = emit_region(values, need, region, Flag.NONE, frozenset())
    dead = emit_region(values, need, region, Flag.NONE, frozenset({0}))
    assert live is not None and dead is not None
    assert live.code.endswith(FIXUP[0]), "nothing proven dead -- the restore stays"
    assert not dead.code.endswith(FIXUP[0]), "pair 0 proven dead -- its restore is dropped"
    assert dead.code == live.code[: -len(FIXUP[0])]


def test_emit_region_only_drops_the_pair_actually_proven_dead() -> None:
    # cx:bx and ax:dx are independent -- proving one dead must never touch
    # the other's own restore.
    code = hx("8B 0E 5E 00 8B 1E 60 00  A1 62 00 8B 16 64 00  0B 0E 66 00 0B 1E 68 00  23 06 6A 00 23 16 6C 00")
    values, _, _ = lift(code, 0, len(code))
    need = needed(values)
    region = regions(values)[0]
    emitted = emit_region(values, need, region, Flag.NONE, frozenset({0}))
    assert emitted is not None
    assert FIXUP[1] in emitted.code, "pair 1 was never proven dead"
    assert FIXUP[0] not in emitted.code, "pair 0 was"


def test_a_widened_operand_says_where_its_fixup_must_go() -> None:
    # The displacement field holds zero, exactly as BC's does; the address comes
    # from a fixup, so the emitter has to say which bytes need one.
    code = hx("A1 5E 00 8B 16 60 00  A3 62 00 89 16 64 00")
    values, _, _ = lift(code, 0, len(code))
    need = needed(values)
    emitted = emit_region(values, need, regions(values)[0], Flag.NONE, frozenset())
    assert emitted is not None
    # nothing is relocated here: a unit test has no fixups behind it
    assert emitted.relocations == ()


def test_a_pair_inside_an_immediate_is_not_a_pair() -> None:
    # mov word [0x005E], 0x5EA1 -- whose displacement and immediate together
    # spell a load pair starting four bytes in. A matcher that retries at every
    # byte finds it, and would rewrite the middle of an instruction.
    code = hx("C7 06 5E 00 A1 5E 00 8B 16 60 00")
    assert classify_code(code[4:]) is not None, "those bytes really do look like a load"
    values, _, _ = lift(code, 0, len(code))
    assert values == []


def test_every_value_begins_where_an_instruction_begins() -> None:
    # the trap above, then filler to a boundary, then a real pair at 0x0a
    code = hx("C7 06 5E 00 A1 5E   00 8B 16 60   A1 5E 00 8B 16 60 00")
    boundaries = {insn.at for insn in run(code, 0, len(code))[0]}
    values, _, _ = lift(code, 0, len(code))
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


# ---------------------------------------------------------------------------
# docs/residue.md's E -- an interleaved instruction that touches no tracked
# register bridges the gap, real shape from bench/nbody.bas (di computes a
# second array's own index between a load pair and the alu pair that reads
# through it):
#
#     mov ax,[si]      mov dx,[si+2]     load pair, pair 0
#     mov di,[0x96]    shl di,2          the OTHER array's index -- edi only
#     sub ax,[di+6]    sbb dx,[di+8]     alu pair, pair 0, resumed
#     mov [0x76],ax    mov [0x78],dx     store pair, pair 0


E_WORKED_EXAMPLE = hx(
    "8B 44 00"  # mov ax,[si]
    "8B 54 02"  # mov dx,[si+2]
    "8B 3E 96 00"  # mov di,[0x96]
    "C1 E7 02"  # shl di,2
    "2B 45 06"  # sub ax,[di+6]
    "1B 55 08"  # sbb dx,[di+8]
    "89 06 76 00"  # mov [0x76],ax
    "89 16 78 00"  # mov [0x78],dx
)


def test_e_worked_example_bridges_the_di_gap_into_one_region() -> None:
    values, _stores, bridges = lift(E_WORKED_EXAMPLE, 0, len(E_WORKED_EXAMPLE))
    assert [v.op for v in values] == [Op.LOAD, Op.ALUM, Op.STORE]
    assert bridges == [Bridge(6, 13)], "mov di,[..] and shl di,2 coalesce into one bridged span"
    assert len(regions(values, bridges)) == 1
    assert len(regions(values)) == 2, "without the bridge the load is stranded on its own"


def test_e_worked_example_emits_smaller_and_carries_the_gap_through_unchanged() -> None:
    code = E_WORKED_EXAMPLE
    values, _stores, bridges = lift(code, 0, len(code))
    need = needed(values, bridges=bridges)
    region = regions(values, bridges)[0]
    emitted = emit_region(values, need, region, Flag.NONE, frozenset(), code, bridges)
    assert emitted is not None
    assert len(emitted.code) < len(code), f"{len(emitted.code)} bytes must beat BC's own {len(code)}"
    # the gap's own bytes -- mov di,[0x96]/shl di,2 -- are neither widened nor
    # moved, so they must still appear verbatim inside the emitted region
    assert hx("8B 3E 96 00 C1 E7 02") in emitted.code


def test_e_carries_a_fixup_inside_a_bridged_instruction_forward() -> None:
    # The bug this guards against, found on bench/nbody.bas: `mov di,[0x96]`
    # inside the bridged gap holds a real relocated address (an array's own
    # base). Splicing its raw bytes into the region without also carrying its
    # own fixup forward left di loaded with a bare zero after rewriting --
    # the array's base was gone, and every address computed off di after that
    # was wrong.
    code = E_WORKED_EXAMPLE
    di_load = decode(code, 6)
    assert di_load is not None and di_load.disp_at is not None
    di_field = di_load.disp_at

    def resolve(field_offset: int, literal: int) -> Addr:
        if field_offset == di_field:
            return Addr(Space.SEGMENT, 0x96, 9)
        return literal_only(field_offset, literal)

    values, _stores, bridges = lift(code, 0, len(code), resolve)
    assert bridges == [Bridge(6, 13, (di_field,))]

    need = needed(values, bridges=bridges)
    region = regions(values, bridges)[0]
    emitted = emit_region(values, need, region, Flag.NONE, frozenset(), code, bridges)
    assert emitted is not None
    matches = [at for at, field in emitted.relocations if field == di_field]
    assert len(matches) == 1, "the gap's own fixup must survive into the emitted region exactly once"
    new_at = matches[0]
    # the field's own bytes moved, unchanged, to wherever the load ahead of
    # it widened to -- still the same two placeholder bytes BC itself wrote
    assert emitted.code[new_at : new_at + 2] == code[di_field : di_field + 2]


def test_e_does_not_bridge_after_a_store() -> None:
    # suite/nots.bas's own real bug: BC sometimes pre-stages a call's own
    # argument at a frame address this pass cannot tell apart from an
    # ordinary local (`mov [bp-14h],dx` right before `call far B$PSSD`) --
    # bridging past that store let a later, unrelated value's own restore
    # land between the store and the call, corrupting what the call read.
    # Bridging only ever continues a value still in flight in a register,
    # never one already committed to memory -- `inc si` alone would bridge
    # fine (docs/residue.md's own E shape), but not right after a store.
    code = hx("A1 5E 00 8B 16 60 00") + hx("A3 62 00 89 16 64 00") + hx("46") + hx("A1 66 00 8B 16 68 00")
    values, _stores, bridges = lift(code, 0, len(code))
    assert [v.op for v in values] == [Op.LOAD, Op.STORE, Op.LOAD]
    assert bridges == []


def test_e_does_not_bridge_after_a_store_of_a_different_pair() -> None:
    # A single trailing `values[-1]` check misses a store still reachable
    # across an intervening, unrelated value -- pair 1's own fresh load sits
    # between pair 0's store and the gap, so `values[-1]` alone is a LOAD,
    # not a STORE, but pair 0's own commit is still exactly one gap away.
    code = (
        hx("A1 5E 00 8B 16 60 00")  # LOAD pair 0
        + hx("A3 62 00 89 16 64 00")  # STORE pair 0
        + hx("8B 0E 66 00 8B 1E 68 00")  # LOAD pair 1 -- values[-1], not a store
        + hx("46")  # inc si -- bridgeable in isolation
        + hx("23 0E 6A 00 23 1E 6C 00")  # ALU pair 1, would resume live[1] if bridged
    )
    values, _stores, bridges = lift(code, 0, len(code))
    assert [v.op for v in values] == [Op.LOAD, Op.STORE, Op.LOAD]
    assert bridges == []


def test_e_does_not_bridge_across_a_jump() -> None:
    # a jmp's own FlowControl is not NEXT, so nothing after it is a reliable
    # fall-through -- fixtures/omf's jumps-p-g2-zd.obj has exactly this shape
    # mid-statement, and treating it as bridgeable merged three independently
    # widenable statements into one, which anchored_inside() then refused
    # outright because the jump's own target landed inside the merged span.
    code = hx("A1 5E 00") + hx("8B 16 60 00") + hx("EB 00") + hx("23 06 5A 00") + hx("23 16 5C 00")
    values, _stores, bridges = lift(code, 0, len(code))
    assert [v.op for v in values] == [Op.LOAD]
    assert bridges == []


def test_e_does_not_bridge_a_call() -> None:
    # a far call's real effect is the callee's, unknowable here -- CLOBBERS'
    # own reasoning in flags.py applies just as much to E's own gap test.
    code = hx("A1 5E 00") + hx("8B 16 60 00") + hx("9A 00 00 00 00") + hx("23 06 5A 00") + hx("23 16 5C 00")
    values, _stores, bridges = lift(code, 0, len(code))
    assert [v.op for v in values] == [Op.LOAD]
    assert bridges == []


# ---------------------------------------------------------------------------
# docs/residue.md's F -- an INTEGER's own sign extension to LONG, invisible
# to lift() before Op.MOVSX existed because cwd is not a value it tracked.


def test_movsx_recognises_a_register_sourced_sign_extension() -> None:
    # bench/nbody.bas's own worked shape: `mov ax,bx` / `cwd`.
    code = hx("8B C3") + hx("99")
    values, _stores, _bridges = lift(code, 0, len(code))
    assert len(values) == 1
    assert values[0].op is Op.MOVSX
    assert values[0].src_reg == Register.BX
    assert values[0].mem is None
    assert encode(values[0]) == hx("66 0F BF C3"), "movsx eax,bx"


def test_movsx_recognises_a_memory_sourced_sign_extension() -> None:
    # bench/nbody.bas's other worked shape: `mov ax,[bp-18h]` / `cwd` -- the
    # mov half is already one of classify()'s own LOADS codes.
    code = hx("8B 46 E8") + hx("99")
    values, _stores, _bridges = lift(code, 0, len(code))
    assert len(values) == 1
    assert values[0].op is Op.MOVSX
    assert values[0].mem == Addr(Space.FRAME, -0x18)
    assert encode(values[0]) == hx("66 0F BF 46 E8"), "movsx eax,[bp-18h]"


def test_movsx_does_not_claim_the_immediate_constant_shape() -> None:
    # `mov ax,imm16 / cwd` is calls.widened_constant_at()'s own, narrower
    # idiom (mov ax,imm16/cwd/push dx/push ax, all four contiguous) --
    # _sign_extend_step must leave it alone rather than double-claim it.
    code = hx("B8 01 00") + hx("99")
    values, _stores, _bridges = lift(code, 0, len(code))
    assert values == []


def test_movsx_does_not_pull_a_following_argument_push_into_the_value_graph() -> None:
    # `push dx` / `push ax` reads dx and ax -- both tracked registers -- so
    # it is neither bridged (E) nor recognised as a value of its own; it
    # simply invalidates pair 0 like any other unrecognised instruction.
    # Folding BC's own argument push into the widened value graph was tried
    # and measured net negative (it changed region boundaries enough to stop
    # pattern B's own restore/re-push fold from firing where it used to,
    # docs/residue.md's own F section has the numbers) and was dropped.
    code = hx("8B C3") + hx("99") + hx("52 50")  # mov ax,bx / cwd / push dx / push ax
    values, _stores, bridges = lift(code, 0, len(code))
    assert [v.op for v in values] == [Op.MOVSX]
    assert bridges == []


def test_movsx_needs_the_cwd_byte_adjacent_not_just_next_in_the_stream() -> None:
    # A reachability gap can put a non-adjacent instruction next in the
    # decoded stream even though it is not next in the bytes -- a jump-over
    # of dead code between two reached regions is exactly this shape.
    # _sign_extend_step must not claim a cwd that does not immediately
    # follow the mov, or whatever real bytes sit in between are silently
    # dropped from the program.
    code = hx("8B C3") + hx("00") * 5 + hx("99")  # mov ax,bx ... cwd, 5 bytes apart
    mov_insn = decode(code, 0)
    cwd_insn = decode(code, 7)
    assert mov_insn is not None and cwd_insn is not None
    values, _stores, _bridges = lift(code, 0, len(code), stream=[mov_insn, cwd_insn])
    assert values == []


class _FarValue:
    """Only the fields lift.memory() reads, so no real Value is needed."""

    def __init__(self, mem: object) -> None:
        self.mem = mem
        self.dlen = 0
        self.op = "mov"


def test_a_widened_far_pointer_keeps_its_segment_override() -> None:
    """`mov ax,es:[bx]` widened must still say `es:`.

    lift.memory() names Space.SEGMENT, Space.FRAME and Space.GROUP and lets
    everything else fall into a bare `MemoryOperand(base, disp)`. Space.FAR
    lands there, and its `segment` -- the whole thing that makes it far --
    was dropped, so a pair reading through es came back reading through ds.

    In qb-qrender's sys.obj that turned

        mov ax,es:[bx] ; mov dx,es:[bx+2]      into    mov eax,[bx]
        mov es:[bx],ax ; mov es:[bx+2],dx      into    mov [bx],eax

    reading and writing the wrong segment. The program linked and then
    corrupted itself: "String space corrupt", or a hang, on the only
    real program this pass has.

    tools/mutate.py has carried a `segment-override-not-refused` mutation
    since before an override was resolved rather than refused, and it has
    been reporting "pattern appears 0 times" -- the guard stopped running
    when the line it patched was rewritten, and nothing noticed.
    """
    from iced_x86 import Code
    from iced_x86 import Encoder
    from iced_x86 import Instruction

    from qbopt import lift as lifting
    from qbopt.module import Space
    from qbopt.module import far_pointer

    where = far_pointer(0, Register.BX, Register.ES)
    assert where.space is Space.FAR and where.segment is Register.ES

    operand = lifting.memory(_FarValue(where))
    encoder = Encoder(16)
    encoder.encode(Instruction.create_reg_mem(Code.MOV_R32_RM32, Register.EAX, operand), 0)
    got = encoder.take_buffer()
    assert got[0] == 0x26, f"the es: prefix is gone: {got.hex(' ')}"
