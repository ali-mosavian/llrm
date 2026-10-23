# The llrm-c lib reds (`cargo test --release -p llrm-c --lib --features toolchain`)

Parity 2c850dc22: 186 tests, 15 red after #3812 (the 16th, the interrupt handler, is fixed). Red since 10-18 to 10-20 unseen: the feature was never run by a gate layer (#3817 adds it). First-red merges are by build (`lib-bisect.log`, `bisect-regparm.log`, `bisect-multi.log`).

| test (compile::tests unless named) | first red merge | PR, owner | listing, actual vs expected | class |
|---|---|---|---|---|
| a_counter_read_only_as_offsets_counts_to_zero | 21aadae64 LFTR on | #3241 backend-slopes | loop `inc bx; cmp word ptr [bp+10], bx`, expected `inc bx; cmp bx, cx` | regression, not wrong code: the bound stays in a memory operand in the loop (one memory operand per trip) |
| m32_lists_a_loop_over_int_pointers | 21aadae64 | #3241 | listing differs (loop form) | as above, to be read with the test |
| addresses_differing_by_base_share_one_stepped_offset | e4ec349af m16 forms 16-bit addresses only | #3093 cycles-queue-quiet-host | `add bx, 2; dec cx` vs `inc dx; cmp dx, cx` | stale: same instruction count, counts to zero; confirm against the pin's llc before editing |
| subscripts_of_one_stride_share_one_offset | e4ec349af | #3093 | same as above | stale (same) |
| m16_regparm3_a_copy_and_a_unit_add_stay_when_the_lea_is_longer_than_the_inc | 917da7e1b visitAdd folds | #3098 uselist-port | `add si, si; or si, 1` for the unit add and copy | to classify with the pin (add/or form) |
| m16_regparm3_calleepop_removes_the_stack_arguments_in_the_callee | fa539b485 simplify each body after inlining | #2741 cycles-queue-quiet-host | `retf` where `ret 4` is expected: the callee no longer pops | regression candidate: calleepop did not fire on the fixture (bigger frame: the callee kept 8 bytes of locals); not wrong code, the callers pop |
| m16_regparm3_the_first_three_arguments_arrive_by_their_size_and_the_fourth_on_the_stack | 2abaf530f InstCombine in the pipeline | #2890 simplify-combine-wiring-2128 | `movsx ax, al; add ax, dx; movsx edx, ax; add edx, ecx; mov eax, [bp+6]; add eax, edx` | stale or regression: the add chain order changed; compare instruction count with the old listing |
| m16_regparm3_a_small_struct_argument_is_on_the_stack | 3b5b0c930 SROA and mem2reg default | #2484 | two `movzx` plus `shl 16`/`or` for a small struct | stale (SROA), more instructions than before: regression candidate |
| m16_watcom_a_struct_result_is_written_through_si_and_returned_in_ax | 3b5b0c930 | #2484 | `lea cx, [edx+eax]` and three stores through si | stale (SROA) |
| m16_watcom_a_long_and_a_far_pointer_take_a_pair_and_a_passed_over_register_stays_free | bdec33313 greedy by default | #2312 | `shld edx, eax, 16` for the far result | stale (allocation) |
| m16_watcom_words_arrive_in_ax_dx_bx_cx_and_the_callee_pops_the_rest | (bisect running) | | `mov ax,[bp+6]; add ax,dx` for expected `add dx, [bp+6]`: 11 instructions, was 9 | regression: loads no longer folded into the adds |
| m32_an_unmarked_function_takes_registers_and_pops_its_stack_arguments | (bisect running) | | `mov ecx,[esp+4]; add ecx, eax; mov eax,[esp+8]; add eax, ecx`, expected `add eax,[esp+4]; add eax,[esp+8]`: 8 instructions, was 6 | regression: loads no longer folded into the adds |
| m32_a_small_struct_argument_is_a_register_and_a_large_one_is_memory | (bisect running) | | `mov ecx,[esp+16]; add ecx,[esp+8]; mov eax,[esp+20]; add eax,ecx` | regression candidate (same fold) |
| m32_lists_a_string_copy_through_esi_edi_ecx | (bisect running) | | no `rep movsd` in the listing | to read with the test |
| a_volatile_far_store_to_video_memory_leaves_the_loop_its_cells | d9150e8bc (first-red doubtful: a constfold signature change) | #2581 | loop cells differ | to classify |
| m16_regparm3_a_slot_a_register_still_holds_is_read_from_the_register | bb2051d0d LiveRangeShrink | #3735 w-4 | dx is no longer stored to a slot at entry | stale premise; `_chop` at -Os is still right: `bench recchop` -Os and -O2 run on 2c850dc22 with the program's output matching `recchop.out` |

Actions: one PR per cause. Stale: the test is rewritten to assert its property, not the listing. Regression: a fix at the cause (the load-fold into add is the largest: three tests), owner named above or the current owner of the pass.
