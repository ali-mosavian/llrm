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


def test_a_prefix_on_a_repeated_string_instruction_is_charged_once_not_each_iteration():
    """`rep stosd` in real mode is F3 66 AB; the emulator charged the 66h on every iteration: seven dwords cost 7 clocks over the 486's
    one, and lru c -O2 read +9 clocks for a fill that is +3 (the pass priced it right)."""
    raw = bytes([0x66, 0xAB])
    ins, prefixes = Decoder(16, raw).decode(), clocks.prefixes(raw)
    assert prefixes == 1
    cell, setup = clocks.clocks("rep_stos_cell"), clocks.clocks("rep_stos")
    assert harness.cost(ins, False, True, 0, None, prefixes) == cell + setup + clocks.PREFIX
    assert harness.cost(ins, False, False, 0, None, prefixes) == cell


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


def test_a_double_shift_costs_what_the_486_sheet_says_not_a_clock():
    """SHLD/SHRD fell through to the default price (1 clock) while the sheet has 2 (r,r,imm) and 3 (r,r,CL): the parity bq16 rows that
    use them were flattered."""
    assert harness.cost(one(32, 0x0F, 0xA4, 0xD3, 0x05), False, True, 0) == clocks.UNTABLED["shld_imm"] == 2      # shld ebx, edx, 5
    assert harness.cost(one(32, 0x0F, 0xAC, 0xD3, 0x05), False, True, 0) == clocks.UNTABLED["shld_imm"]          # shrd ebx, edx, 5
    assert harness.cost(one(32, 0x0F, 0xA5, 0xD3), False, True, 0) == clocks.UNTABLED["shld_cl"] == 3           # shld ebx, edx, cl
    assert harness.cost(one(32, 0x0F, 0xAD, 0xD3), False, True, 0) == clocks.UNTABLED["shld_cl"]                 # shrd ebx, edx, cl


def test_an_extended_load_and_store_cost_what_the_486_sheet_says_not_the_dword_figure():
    """Every `fld m` cost 3 and every `fst(p) m` 8 (the table's m32/m64 and m64 rows): `fld tbyte` is 6 on the sheet (Table 10.3), `fstp tbyte`
    6 and `fstp dword` 7. The long double rows read 3 clocks a `fld tbyte` cheap, which understated every compiler's m80 traffic and the gain of
    keeping a constant on the stack (x_ld_to_int: 256 loads = 768 clocks)."""
    assert harness.cost(one(32, 0xDB, 0x28), False, True, 0) == clocks.UNTABLED["fld_m80"] == 6      # fld tbyte [eax]
    assert harness.cost(one(32, 0xD9, 0x00), False, True, 0) == clocks.clocks("x87_load") == 3       # fld dword [eax]
    assert harness.cost(one(32, 0xDD, 0x00), False, True, 0) == 3                                     # fld qword [eax]
    assert harness.cost(one(32, 0xDB, 0x38), False, True, 0) == clocks.UNTABLED["fstp_m80"] == 6     # fstp tbyte [eax]
    assert harness.cost(one(32, 0xD9, 0x18), False, True, 0) == clocks.UNTABLED["fst_m32"] == 7       # fstp dword [eax]
    assert harness.cost(one(32, 0xDD, 0x18), False, True, 0) == clocks.clocks("x87_store") == 8       # fstp qword [eax]

def test_a_register_copy_costs_what_the_486_sheet_says_and_the_compilers_table_agrees():
    """`fld st(i)` is 4 clocks on the 486 sheet (240440-002, Table 10.3), `fld m64` 3. The compiler's table priced the copy as the load, so a
    hoisted operand copied each trip looked free (nbody -O2 m16). The harness figure is the sheet's, not read from the table: the two must agree."""
    assert harness.cost(one(32, 0xD9, 0xC1), False, True, 0) == clocks.UNTABLED["fld_reg"] == clocks.clocks("x87_copy") == 4   # fld st(1)
    assert clocks.clocks("x87_copy") == clocks.clocks("x87_load") + 1


def test_the_multiplier_operand_is_the_target_descriptions_not_an_environment_switch(monkeypatch):
    """quicksort -O2 read +29.4k, +8.0k or +18.7k clocks against the old compiler by which `VSGCC_IMUL_MODEL` the run happened to set: the
    harness priced a multiplier the compiler's table never stated. The description (`smul_multiplier`) is the one place, and the 486 says
    its source operand."""
    monkeypatch.setenv("VSGCC_IMUL_MODEL", "dest")
    assert harness.imul_model() == "source"
    assert clocks.audited("smul_multiplier") == 1


def test_a_multiply_costs_the_data_sheet_note_3_figure_from_the_descriptions_bounds(monkeypatch):
    """Intel 240440-002 note 3, by hand: 10 + max(log2|m|, n), n = 3 for +m and 5 for -m (13 for m = 0), Dword column 13/42. The
    emulator kept 10 and 42 as literals; it reads the description's `smul_min_w4`, `smul_max_w4`, `smul_negative_bits` now."""
    by_hand = [(0, True, 13), (1, True, 13), (7, True, 13), (8, True, 14), (255, True, 18), (0x7FFF, True, 25),
               (0x7FFFFFFF, True, 41), (-1 & 0xFFFFFFFF, True, 15), (-100000 & 0xFFFFFFFF, True, 27), (0xFFFFFFFF, False, 42)]
    for m, signed, want in by_hand:
        assert harness.multiply_clocks(m, signed) == want, (m, signed, want)
    # a CPU whose description states other bounds is priced by them, not by the 486's literals
    monkeypatch.setitem(clocks._FLAT[1], "smul_max_w4", 74)
    assert harness.multiply_clocks(0x7FFFFFFF, True) == 13 + (74 - 13) * (31 - 3) // 29
