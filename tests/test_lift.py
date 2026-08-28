"""
The lift and the emitter, on bytes built here rather than on a compiled
program.

Whole-program checks were green while the classifier reported "no half" for A1,
while a pair copy was treated as dead, and while a negate inherited another
instruction's address. A program computing the right answer is not evidence
about the piece that happened not to run.
"""

import pytest

from helpers import hx
from qbopt.lift import NEG
from qbopt.lift import REG
from qbopt.lift import ALUM
from qbopt.lift import ALUV
from qbopt.lift import LOAD
from qbopt.lift import MOVE
from qbopt.lift import FIXUP
from qbopt.lift import PAIRS
from qbopt.lift import STORE
from qbopt.lift import Value
from qbopt.lift import lift
from qbopt.lift import encode
from qbopt.lift import needed
from qbopt.lift import sizeof
from qbopt.lift import decode1
from qbopt.lift import regions
from qbopt.lift import emit_region

ALU = sorted(PAIRS)  # 23 and, 0B or, 33 xor, 03 add, 2B sub
HI = {lo: PAIRS[lo][0] for lo in ALU}
BASES = {0x06: 2, 0x46: 1, 0x86: 2}  # ModRM mod/rm -> displacement bytes


def one(h: str) -> list[Value]:
    b = hx(h)
    return lift(b, 0, len(b))[0]


@pytest.mark.parametrize("base", sorted(BASES), ids=lambda b: f"base{b:02X}")
@pytest.mark.parametrize("reg", range(4))
@pytest.mark.parametrize(("opcode", "kind"), ((0x8B, "ld"), (0x89, "st")))
def test_classifier_every_register_half_and_addressing_form(opcode: int, kind: str, reg: int, base: int) -> None:
    dl = BASES[base]
    b = bytes([opcode, base | (reg << 3)]) + (b"\xe8" if dl == 1 else b"\x5e\x00")
    a = decode1(b, 0)
    assert a is not None
    assert a[0] == kind
    assert (a[1], a[3]) == REG[reg]
    assert (a[7], a[8]) == (base, dl)


@pytest.mark.parametrize("reg", range(4))
@pytest.mark.parametrize("half", (0, 1), ids=("lo", "hi"))
@pytest.mark.parametrize("lo", ALU, ids=lambda lo: f"{lo:02X}")
def test_classifier_every_alu_operation(lo: int, half: int, reg: int) -> None:
    op = HI[lo] if half else lo
    a = decode1(bytes([op, 0x06 | (reg << 3), 0x5E, 0x00]), 0)
    assert a is not None
    assert a[0] == "op"
    assert a[4] == op


@pytest.mark.parametrize("sreg", range(4))
@pytest.mark.parametrize("dreg", range(4))
def test_classifier_register_to_register(dreg: int, sreg: int) -> None:
    a = decode1(bytes([0x8B, 0xC0 | (dreg << 3) | sreg]), 0)
    if REG[dreg][1] == REG[sreg][1]:
        assert a is not None
        assert a[0] == "mv"
        assert (a[1], a[2]) == (REG[dreg][0], REG[sreg][0])
    else:
        assert a is None, "halves must match for a pair move"


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
    assert decode1(hx(enc), 0) is None, why


def test_load_operate_store_is_one_chain() -> None:
    v = one("A1 5E 00 8B 16 60 00   23 06 5A 00 23 16 5C 00   A3 62 00 89 16 64 00")
    assert [x.op for x in v] == [LOAD, ALUM, STORE]
    assert (v[1].s1, v[2].s1) == (0, 1)
    assert v[0].mem == 0x5E, "the load keeps the low half's displacement"


def test_a_pair_copy_is_a_value_knowing_both_pairs() -> None:
    v = one("8B 0E 5E 00 8B 1E 60 00   8B D3 8B C1   A3 62 00 89 16 64 00")
    assert [x.op for x in v] == [LOAD, MOVE, STORE]
    assert (v[1].pair, v[1].src_pair) == (0, 1)


def test_a_cross_pair_operation_reads_both_values() -> None:
    v = one("A1 5E 00 8B 16 60 00  8B 0E 62 00 8B 1E 64 00  33 C1 33 D3")
    assert [x.op for x in v] == [LOAD, LOAD, ALUV]
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
    assert [x.op for x in v] == [LOAD, NEG]
    assert v[1].end - v[1].at == 7, "consuming seven bytes"


def test_a_spill_written_high_half_first() -> None:
    v = one("A1 5E 00 8B 16 60 00   89 56 EA 89 46 E8")
    assert [x.op for x in v] == [LOAD, STORE]
    assert (v[1].mem, v[1].at) == (-24, 7), "the low displacement and the earlier address"


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
    assert [x.op for x in v] == [LOAD]


def test_opaque_bytes_split_a_region_and_leave_the_value_live() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 0090 90A1 62 00 8B 16 64 00 A3 66 00 89 16 68 00")
    v, _ = lift(b, 0, len(b))
    assert len(regions(v)) == 2
    need = needed(v)
    assert need[1] is True, "a value in a register when a region ends is live"
    assert all(need[n] for n, x in enumerate(v) if x.op == STORE), "a store is always needed"


def test_everything_feeding_a_store_is_needed_transitively() -> None:
    b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00")
    v, _ = lift(b, 0, len(b))
    assert all(needed(v))


# If sizeof and encode disagree the region overruns the code after it or wastes
# what it claimed. Two parallel switch statements do not stay in step on their own.
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


def test_sizing_and_encoding_agree_for_every_form(corpus: list[Value]) -> None:
    for n, x in enumerate(corpus):
        assert sizeof(x, None) == len(encode(x)), f"v{n} {x.op} base {x.base:02X}"


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
    [(0, "668bd066c1ea10"), (1, "668bd966c1eb10")],
)
def test_the_high_half_comes_back_from_the_widened_register(pair: int, want: str) -> None:
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
def test_a_region_never_grows_and_the_slack_is_jumped_over(enc: str) -> None:
    b = hx(enc)
    v, _ = lift(b, 0, len(b))
    need = needed(v)
    for reg in regions(v):
        span = v[reg[-1]].end - v[reg[0]].at
        out = emit_region(v, need, reg)
        if out is None:
            continue
        assert len(out) == span, "padded to exactly the bytes it replaced"
        core = sum(sizeof(v[n], None) for n in reg if need[n])
        core += sum(len(FIXUP[p]) for p in {v[n].pair for n in reg if need[n]})
        if span - core >= 2:
            assert out[core] == 0xEB, "the slack begins with a jump"
            assert out[core + 1] == span - core - 2, "clearing exactly the slack"
            assert all(x == 0x90 for x in out[core + 2 :]), "and the rest is nops"


def test_too_small_a_region_is_refused_not_overrun() -> None:
    b = hx("8B 0E 5E 00 8B 1E 60 00 89 0E 62 00 89 1E 64 00")  # 16 bytes, pair 1
    v, _ = lift(b, 0, len(b))
    need = needed(v)
    for reg in regions(v):
        out = emit_region(v, need, reg)
        span = v[reg[-1]].end - v[reg[0]].at
        assert out is None or len(out) <= span
