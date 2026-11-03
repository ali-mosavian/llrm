import cfi_check as c

CODE = [
    "0 stp\tx29, x30, [sp, #-16]!",
    "4 mov\tx29, sp",
    "8 bl\t0 <g>",
    "c add\tw0, w0, #0x1",
    "10 ldp\tx29, x30, [sp], #16",
    "14 ret",
]
ROWS = [(0, 31, 0, {}), (4, 31, 16, {}), (8, 29, 16, {30: -8, 29: -16}), (16, 31, 16, {30: -8, 29: -16}), (20, 31, 0, {})]


def test_rows_that_say_what_the_code_does_are_clean():
    assert c.check(CODE, ROWS) == []


def test_a_frame_address_that_is_not_the_entry_stack_pointer_is_a_problem():
    bad = ROWS[:2] + [(8, 29, 32, ROWS[2][3])] + ROWS[3:]
    assert any("x29+32" in problem for problem in c.check(CODE, bad))


def test_a_save_in_the_wrong_cell_is_a_problem():
    bad = ROWS[:2] + [(8, 29, 16, {30: -8, 29: -8})] + ROWS[3:]
    assert any("register 29" in problem for problem in c.check(CODE, bad))


def test_a_register_the_code_stored_and_the_rows_leave_out_is_a_problem():
    """Without x30 an unwinder cannot find the caller: the prologue's rows must name every store."""
    bad = ROWS[:2] + [(8, 29, 16, {29: -16})] + ROWS[3:]
    assert any("after the prologue" in problem for problem in c.check(CODE, bad))


def test_the_epilogue_pops_the_stack_in_the_rows_too():
    bad = ROWS[:4] + [(20, 31, 16, {})]
    assert any("sp+16" in problem for problem in c.check(CODE, bad))
