# Old-selector test reds

Eleven `isel_tests` went red at #4992 (`pressuresink` deleted; first bad commit d50878167, bisect first-parent, good 30e8dc659). They pin the instruction order and registers of `--isel=old`, which retires with the flip (#4512). Each carries `#[ignore = "OLD-SEL-n ..."]`; the flip PR deletes them with the old selector.

m16 sizes are x86 encodings summed by hand over the instructions that differ, `-march=i386` forms. The DAG selector (`--isel=dag`, `llrm-c -Os -m16 -march=i386`) compiles the C shapes of rows 5 and 8 without the growth (`fld; fadd dword ptr [bp+10]`, no second cell). Rows 6 and 7 were not run on it.

| Row | Test | Delta | Size |
|---|---|---|---|
| OLD-SEL-1 | `test_far_pointers_compare_by_words` | order only (xor/mov interleave) | 0 B |
| OLD-SEL-2 | `test_ptrtoint_of_a_far_global_to_a_word_is_its_offset` | register choice: `mov cx, offset messages+8; cmp [bp+6], cx` where the test names `ax` | 0 B |
| OLD-SEL-3 | `a_va_copy_moves_the_list_pointer_and_a_va_end_is_no_code` | order only (the `mov ax,[bp+6]` moves ahead of the `lea`) | 0 B |
| OLD-SEL-4 | `test_near_globals_are_symbols` | order only | 0 B |
| OLD-SEL-5 | `test_a_float_argument_read_after_a_call_is_loaded_after_it` | `fld; fld; faddp` where the old order was `fld; fadd dword ptr [bp+10]` (load fold lost) | +2 B, +1 insn |
| OLD-SEL-6 | `test_a_float_phi_takes_a_constant_input` | `mov ax,[bp+6]; test ax,ax` where it was `cmp word ptr [bp+6], 0` | +1 B, +1 insn |
| OLD-SEL-7 | `test_a_huge_pointer_difference_is_whole_strides_and_the_offset_remainder` | the offsets go through `dx`/`si` (`push si`/`pop si`) where `movzx` read memory | +6 B, +4 insn |
| OLD-SEL-8 | `test_a_float_in_a_cell_a_call_may_write_is_not_read_again_after_it` | a cell no call can reach is copied to a second cell before the call (`fld [bp-8]; fstp [bp-18]`) and read from it after | +6 B, +2 insn |
| OLD-SEL-9 | `test_huge_pointers_compare_by_selector_then_offset` | order only | 0 B |
| OLD-SEL-10 | `test_m16_emits_386_forms` | register choice (`movsx edx` for `ecx`) | 0 B |
| OLD-SEL-11 | `test_a_far_pointer_a_loop_steps_is_two_phis` | register choice (`les si`, `[bx]` for `[si]`) | 0 B |

Growth hidden by these ignores, old selector only: rows 5, 6, 7 and 8 (+15 B in total over the four). None is in the flip table; rows 5 and 8 are clean on the DAG; rows 6 and 7 are not checked there.
