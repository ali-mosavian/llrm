import eh_frame_check as c

# push rbp; mov rbp, rsp; push rbx; sub 0x18, rsp; ...; add 0x18, rsp; pop rbx; pop rbp; ret
CODE = [(0, "pushq %rbp"), (1, "movq %rsp, %rbp"), (4, "pushq %rbx"), (5, "subq $0x18, %rsp"), (9, "nop"), (0xa, "addq $0x18, %rsp"), (0xe, "popq %rbx"), (0xf, "popq %rbp"), (0x10, "retq")]
ROWS = [(0, 7, 8, {}), (1, 7, 16, {6: -16}), (4, 6, 16, {6: -16}), (5, 6, 16, {6: -16, 3: -24})]


def test_rows_that_say_what_the_code_does_are_clean():
    assert c.check(CODE, ROWS) == []


def test_a_cfa_offset_that_misses_a_push_is_a_problem():
    bad = [ROWS[0], (1, 7, 8, {6: -16})] + ROWS[2:]
    assert any("CFA = r7+8" in p and "r7+16" in p for p in c.check(CODE, bad))


def test_a_save_in_the_wrong_cell_is_a_problem():
    bad = ROWS[:3] + [(5, 6, 16, {6: -16, 3: -32})]
    assert any("register 3" in p and "CFA-32" in p and "CFA-24" in p for p in c.check(CODE, bad))


def test_a_pushed_register_in_no_row_is_a_problem():
    bad = ROWS[:3] + [(5, 6, 16, {6: -16})]
    assert any("register 3" in p and "no row" in p for p in c.check(CODE, bad))


def test_a_frame_register_not_taken_up_is_a_problem():
    """After `mov rsp, rbp` the CFA is rbp+16 and stays there as the stack grows; rows still on rsp say 24 after the next push."""
    bad = [ROWS[0], ROWS[1], (4, 7, 16, {6: -16}), (5, 7, 24, {6: -16, 3: -24})]
    assert any("r6+16" in p for p in c.check(CODE, bad))


def test_a_function_that_does_not_start_the_section_is_read_from_its_own_start():
    """The disassembly numbers a function from its section, the rows from the function: read together, a function at section offset 0x10 had rows 16 bytes early (3 of 10 test functions flagged)."""
    shifted = [(off + 2, text) for off, text in CODE]
    assert c.check(shifted, ROWS) == []


def test_a_later_spill_of_a_saved_register_is_not_its_save_cell():
    """`mov %r12, 8(%rsp)` after the body begins is a spill, not the save the rows describe: taking it for the save flagged 6 of 564 functions on the corpus."""
    code = CODE[:5] + [(9, "movq %r12, 0x8(%rsp)")] + CODE[5:]
    assert c.check(code, ROWS) == []


def test_a_body_that_pushes_stack_arguments_is_not_modelled():
    """`sub $0x10, %rsp; call f` with `ret $0x10` in f: the callee pops, so the rows stay put while the code seems to keep 16 bytes; os_locals_between_pushes `probe` was flagged for it."""
    code = [(0, "subq $0x58, %rsp"), (4, "leaq 0x1(%rdi), %rax"), (8, "subq $0x10, %rsp"), (0xc, "callq 0x0"), (0x11, "addq $0x58, %rsp"), (0x15, "retq")]
    rows = [(0, 7, 8, {}), (4, 7, 96, {}), (0x11, 7, 96, {}), (0x15, 7, 8, {})]
    assert c.check(code, rows)[0].startswith(c.SKIPPED)
