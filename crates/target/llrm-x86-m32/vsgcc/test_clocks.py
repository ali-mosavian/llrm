"""The emulator's clocks are the compiler's own `timings.times` (clocks.py), and a prefix costs what the table says."""
import clocks
import harness
from iced_x86 import Decoder


def one(bits, *code):
    return Decoder(bits, bytes(code)).decode()


def test_a_divide_costs_the_audited_bound_of_its_width():
    """Every divide cost 40 (DIV) or 43 (IDIV) whatever its width, where the compiler's table has 24 and 27 for a word: recmany's and
    recsum's `% 7` on an int read 16 clocks dearer at m16 than the 486 charges, and the whole of their gap to m32 was that."""
    assert harness.cost(one(16, 0xF7, 0xFB), False, True, 0) == 27       # idiv bx
    assert harness.cost(one(16, 0xF7, 0xF3), False, True, 0) == 24       # div bx
    assert harness.cost(one(32, 0xF7, 0xFB), False, True, 0) == 43       # idiv ebx
    assert harness.cost(one(32, 0xF7, 0xF3), False, True, 0) == 40       # div ebx


def test_an_operand_size_prefix_costs_a_clock_on_a_486():
    """`mov eax, ebx` in real mode is 66h 89 D8 and the 486 spends a clock decoding the 66h; the emulator charged nothing for it,
    where 12% of frames' m16 clocks were prefixed instructions (the compiler's division.rs already charged them)."""
    raw = bytes([0x66, 0x89, 0xD8])
    assert clocks.prefixes(raw) == 1 and clocks.prefixes(bytes([0x89, 0xC3])) == 0
    ins = Decoder(16, raw).decode()
    assert harness.cost(ins, False, True, 0, None, clocks.prefixes(raw)) == clocks.clocks("mov_rr") + clocks.PREFIX == 2
    assert harness.cost(ins, False, True, 0) == 1, "a cost asked with no prefixes counts none"


def test_the_forms_the_table_prices_are_read_from_it():
    """`pop reg` was 1 clock and `ret` and `leave` literals of the emulator's own; the table has 4, 5 and 5."""
    assert harness.cost(one(32, 0x58), False, True, 0) == clocks.clocks("pop_r") == 4
    assert harness.cost(one(32, 0xC3), False, True, 0) == clocks.clocks("ret_near")
    assert harness.cost(one(32, 0xC9), False, True, 0) == clocks.clocks("leave") == 5
    assert harness.cost(one(16, 0xCB), False, True, 0) == clocks.clocks("ret_far", far=True)


def test_a_price_kept_beside_the_table_is_one_the_table_lacks():
    """The emulator keeps a figure of its own only where `timings.times` has no row for the form."""
    tabled = set(clocks._FLAT[1]) | set(clocks._REAL[1])
    assert not tabled & set(clocks.UNTABLED), sorted(tabled & set(clocks.UNTABLED))


def test_a_double_shift_costs_the_sheets_clocks_not_one():
    """SHLD/SHRD fell through to 1 clock, so main's `shrd` (parity/scalar, a 64-bit shift) read cheaper than the 486's 2 to 4."""
    assert harness.cost(one(32, 0x0F, 0xAC, 0xD0, 0x05), False, True, 0) == 2   # shrd eax, edx, 5
    assert harness.cost(one(32, 0x0F, 0xAD, 0xD0), False, True, 0) == 3         # shrd eax, edx, cl
    assert harness.cost(one(32, 0x0F, 0xA4, 0xD0, 0x05), False, True, 0) == 2   # shld eax, edx, 5
    assert harness.cost(one(32, 0x0F, 0xAC, 0x10, 0x05), False, True, 0) == 3   # shrd [eax], edx, 5
    assert harness.cost(one(32, 0x0F, 0xAD, 0x10), False, True, 0) == 4         # shrd [eax], edx, cl
