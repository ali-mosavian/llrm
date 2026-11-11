# z80 long argument/result order: A/B (2026-11-10)

Base: release/next 0db87355f6 (llc f90bb78cf92f). Candidates: A = long argument DE low / HL high, result unchanged (llc 84a2a8575fe9);
B = result changed to HL low / DE high, the argument order (llc 18f1eb5f2ad7, this PR). Five levels, `sweep.py --ours-only` (ucsim
clocks), same pin clang, each side built from its own tree. The compiles that succeeded per level are the cells column; the
programs that failed to compile (16 to 18 per level, e.g. `bench_io`: `llrm_os.h`) are the same set on base and B.

| level | cells | same | better | worse (either axis) | bytes | clocks |
|---|---|---|---|---|---|---|
| O2 | 99 | 66 | 21 | 12 | -571 | -1507271 |
| Os | 98 | 52 | 35 | 11 | -1984 | -4645040 |
| O1 | 97 | 58 | 24 | 15 | -3133 | +21645118 |
| Oz | 98 | 52 | 36 | 10 | -899 | -4080631 |
| O3 | 100 | 67 | 21 | 12 | -721 | -1507010 |

B is chosen: its result list repeats the argument order, so it needs no custom rule (the pin's `AnalyzeCallResult` assigns parts in
order); A needs an ARM `f64AssignAAPCS` analog and has 30 to 38 worse cells per level.

O1 clocks: `div_constants_long` (-1320 B, +10.7M T, +2.3%) and `div_constants_ulong` (-719 B, +11.1M T, +3.8%) carry the
whole +21.6M; the O1 rule takes the smaller relative regret.

## Cells worse than the base, (bytes, clocks)

- O2 / O3: div_constants_short (14, -359226), fixed_point (-1, 30), frameless (-18, 19735), int64_division (1, 1008),
  licm_graph_callee_footprint (1, 6000), loop_versioning (40, 19499), recdeep (16, -92728), recmany_o1 (9, -35848),
  rep_count (51, 60286), switch_peeled_table (8, 0), wide_shift_by_one (100, 46382), word_compare_branches (-1, 1764).
- Os: copy_runs (-9, 348), fixed_point, frameless (-10, 19875), idioms (-80, 354936), licm_graph_callee_footprint,
  memmove_overlap (-4, 2837), recdeep (13, 205445), recmany_o1 (10, -17848), rep_count (1, 9093), switch_jump_table (2, -28),
  word_compare_branches (9, 13328).
- Oz: Os less memmove_overlap and switch_jump_table, plus shrink_wrapped_frame (2, 96).
- O1: div_constants_long, div_constants_short, div_constants_ulong, fixed_point, frameless, idioms (-78, 353610),
  int64_division (-328, 147150), licm_graph_callee_footprint, memmove_overlap, phi_slot_unknown_arm (-7, 2942), rep_count,
  shrink_wrapped_frame (-4, 732), switch_peeled_table, wide_shift_by_one, word_compare_branches (16, 0).

## Causes (largest first)

1. wide_shift_by_one +100 B (O1/O2/O3): base calls `__mulsi3` (x 514), B emits the inline shift-and-add form. Seen in the
   asm; the price that flipped (`prefers`, DEVIATE z80-callinline-price) is not yet read. Card owed.
2. rep_count +51, loop_versioning +40, recdeep +16, word_compare_branches +16, then the +1 to +14 B ripples: causes not yet
   read; cards owed per row, grouped by cause.

Row Z80-LONG-ORDER-WORSE in `above-main-rows.md` owns the cards (exit 2026-11-24).

## Callers and the runtime

Every routine that takes or returns a long: `z80_rt.s` (`__mulsi3`, `__ashlsi3`, `__lshrsi3`, `__ashrsi3`, `__ashrhi3`,
`__s_udivremsi4`, `__s_divremsi4`, `__s_udivsi3`, `__s_umodsi3`, `__dvr_ep`), `runtime/compiler-rt/config/z80/` (`int32.s`,
`umulhisi3.s`, `sf32_*.s`). Compiled C in compiler-rt follows the convention. Word `__divremhi4`/`__udivhi3` keep DE quotient,
HL remainder; `lower_divrem_word` takes the remainder as the first part.
