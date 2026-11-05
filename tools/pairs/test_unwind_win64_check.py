import unwind_win64_check as c

CODE = {0: ["0 pushq %rsi", "1 pushq %rdi", "2 subq $0xb8, %rsp", "9 movaps %xmm6, 0x30(%rsp)", "11 movaps %xmm7, 0x40(%rsp)"], 0x19: ["19 movl %ecx, %edi"]}
TABLE = """
Function Table:
  Start Address: f
  Size of prolog: 25
    Unwind Codes:
      0x19: UOP_SaveXMM128 XMM7 [0x0040]
      0x11: UOP_SaveXMM128 XMM6 [0x0030]
      0x09: UOP_AllocLarge 23
      0x02: UOP_PushNonVol RDI
      0x01: UOP_PushNonVol RSI
"""


def test_codes_that_describe_the_prologue_are_clean():
    assert c.check(CODE, {"f": 0}, TABLE) == []


def test_a_register_save_with_no_code_is_a_problem():
    """A prologue store the unwind data leaves out is a callee-saved register an unwinder restores from the wrong place."""
    missing = TABLE.replace("      0x19: UOP_SaveXMM128 XMM7 [0x0040]\n", "")
    assert any("SaveXMM128" in problem for problem in c.check(CODE, {"f": 0}, missing))


def test_a_code_at_the_wrong_offset_is_a_problem():
    moved = TABLE.replace("0x02: UOP_PushNonVol RDI", "0x03: UOP_PushNonVol RDI")
    assert c.check(CODE, {"f": 0}, moved) != []


def test_a_push_of_rax_and_a_probed_allocation_are_allocations():
    """`push rax` (one slot) and `sub rsp, rax` after __chkstk carry an Alloc code; read as a saved register or as no instruction they gave 4 false problems on the corpus."""
    code = {0: ["0 pushq %rax", "1 movl $0x2028, %eax", "6 callq 0x0", "b subq %rax, %rsp"], 0x10: ["10 ret"]}
    table = "\nFunction Table:\n  Start Address: f\n  Size of prolog: 16\n    Unwind Codes:\n      0x10: UOP_AllocLarge 1\n      0x01: UOP_AllocSmall 8\n"
    assert c.check(code, {"f": 0}, table) == []


def test_a_function_that_pushes_a_register_and_has_no_table_is_a_problem():
    """A frame with no .pdata entry cannot be unwound: the Windows unwinder gives up on the first such function."""
    assert any("no unwind data" in problem for problem in c.check(CODE, {"f": 0}, ""))
