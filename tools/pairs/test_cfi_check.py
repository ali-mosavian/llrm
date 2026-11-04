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


def test_a_register_offset_access_is_not_an_immediate_to_follow():
    """`str x1, [x0, x10]` stopped the checker on the first register-offset access of the corpus."""
    code = ["0 str\tx1, [x0, x10]", "4 ldr\tx2, [x0, x1, lsl #2]", "8 ret"]
    assert c.check(code, [(0, 31, 0, {})]) == []


def test_the_disassembly_keeps_the_immediates():
    """`#-16` was cut off as a comment, so no stack adjustment was seen and 31 of 128 functions looked wrong."""
    text = "0000000000000000 <f>:\n   0:\tstp\tx29, x30, [sp, #-16]!\n   4:\tbl\t0 <g>\t// R_AARCH64_CALL26 g\n"
    assert c.code_of(text) == {"f": ["0 stp\tx29, x30, [sp, #-16]!", "4 bl\t0 <g>"]}


def test_a_function_that_does_not_start_the_section_has_its_rows_from_its_own_start():
    """The rows are offsets from the function; the disassembly's are from the section. Every function after the first read as wrong."""
    shifted = [f"{int(line.split(' ', 1)[0], 16) + 0x18:x} {line.split(' ', 1)[1]}" for line in CODE]
    assert c.check(shifted, ROWS) == []


def test_a_stack_adjustment_shifted_by_twelve_is_followed():
    code = ["0 sub\tsp, sp, #0x1, lsl #12", "4 stp\tx29, x30, [sp]", "8 ret"]
    assert c.check(code, [(0, 31, 0, {}), (4, 31, 4096, {}), (8, 31, 4096, {29: -4096, 30: -4088})]) == []


def test_saves_through_a_scratch_address_register_are_followed():
    """A big frame saves through `add x16, sp, #off; stp .., [x16]`: the checker saw no store and called the rows wrong."""
    code = ["0 sub\tsp, sp, #0xe40", "4 add\tx16, sp, #0xe30", "8 stp\tx29, x30, [x16]", "c add\tx29, sp, #0xe30", "10 mov\tx0, x19", "14 ret"]
    rows = [(0, 31, 0, {}), (4, 31, 3648, {}), (16, 29, 16, {29: -16, 30: -8})]
    assert c.check(code, rows) == []


def test_a_spilled_temporary_is_not_a_missing_save():
    """`str x12, [sp, #8]` is a spill: only the callee-saved registers must be in the rows."""
    code = ["0 sub\tsp, sp, #0x10", "4 str\tx12, [sp, #8]", "8 bl\t0 <g>", "c add\tsp, sp, #0x10", "10 ret"]
    assert c.check(code, [(0, 31, 0, {}), (4, 31, 16, {}), (16, 31, 0, {})]) == []


def test_a_stack_adjustment_through_a_constant_register_is_followed():
    """A frame of 4128 bytes is `mov x16, #4128; sub sp, sp, x16`: the checker read the stack pointer as unmoved."""
    code = ["0 mov\tx16, #0x1020", "4 sub\tsp, sp, x16", "8 stp\tx29, x30, [sp]", "c bl\t0 <g>", "10 ret"]
    rows = [(0, 31, 0, {}), (8, 31, 4128, {}), (12, 31, 4128, {29: -4128, 30: -4120})]
    assert c.check(code, rows) == []


def test_code_after_the_first_unconditional_exit_is_not_followed():
    """A block laid out after the epilogue restates the frame; the straight line from the entry no longer describes it."""
    code = CODE + ["18 mov\tx0, x1"]
    rows = ROWS + [(24, 29, 16, {30: -8, 29: -16})]
    assert c.check(code, rows) == []


def test_a_frame_register_set_by_a_shifted_immediate_is_followed():
    """`add x29, sp, #1, lsl #12` read as sp+1: a 4128-byte frame's rows were 'wrong' (quicksort, once the record moved to 4096)."""
    code = [
        "0 sub\tsp, sp, #0x1, lsl #12",
        "4 sub\tsp, sp, #0x20",
        "8 stp\tx29, x30, [sp, #4096]",
        "c add\tx29, sp, #0x1, lsl #12",
        "10 ret",
    ]
    rows = [(0, 31, 0, {}), (4, 31, 4096, {}), (8, 31, 4128, {}), (16, 29, 32, {30: -24, 29: -32})]
    assert c.check(code, rows) == []
