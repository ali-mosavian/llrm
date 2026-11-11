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

## Against the best SDCC (the bar), cells with a bar

Bar: best of the four SDCC 4.5.0 variants per axis (owner table 2026-11-10 12:00Z); O1/Os/Oz above the bar = worse on either
axis, O2/O3 = clocks + 3 x bytes. Script `ab/bar.py` (base = tip 0db87355f6, B = this PR).

| level | cells with a bar | above the bar, base -> B | above on both axes, base -> B | newly above | no longer above |
|---|---|---|---|---|---|
| O2 | 75 | 4 -> 4 | 3 -> 3 | none | 0 |
| Os | 74 | 14 -> 13 | 3 -> 3 | none | 1 |
| O1 | 74 | 14 -> 12 | 3 -> 3 | rep_count (289 -> 340 B; bar 273) | 3 |
| Oz | 74 | 14 -> 13 | 4 -> 4 | none | 1 |
| O3 | 76 | 3 -> 3 | 2 -> 2 | none | 0 |

The O1 clocks trade is below the bar on both axes: `div_constants_long` 15493 -> 14173 B (bar 15095) and 478.7M -> 479.7M T (bar
991M); `div_constants_ulong` 10353 -> 9634 B (bar 10022), 290.8M -> 301.9M T (bar 656M). Both were above the bar on bytes.
`wide_shift_by_one` +100 B stays under its bar (1110 -> 1210 vs 1567 B).

## Causes (largest first)

1. wide_shift_by_one +100 B (O1/O2/O3): base calls `__mulsi3` (x 514), B emits the inline shift-and-add form. Seen in the
   asm; the price that flipped (`prefers`, DEVIATE z80-callinline-price) is not yet read. Card owed.
2. rep_count +51 B at O1 (289 -> 340; SDCC 273; the one cell newly above the bar): `_bench` 221 -> 277 B, the pointer sums
   change from `ld hl,#N; add hl,de` to `ld l,e; ld h,d; ld bc,#N; add hl,bc; ld bc,#0`. Probe: with `bench` returning a short
   (same source otherwise) base and B give the same 280 B, so the long result register is the cause. Consistent with the
   accumulator's low word now sitting in HL, which `ld a,(hl)` and `add hl,rr` need; the first differing allocation decision is
   not yet read. Pin analog to check: greedy copy-hint weighting by block frequency (a hint from a cold return block against
   hot loop uses).
3. loop_versioning +40, recdeep +16, word_compare_branches +16, then the +1 to +14 B ripples: causes not yet read; cards owed
   per row, grouped by cause.

Row Z80-LONG-ORDER-WORSE in `above-main-rows.md` owns the cards (exit 2026-11-24).

## Callers and the runtime

Every routine that takes or returns a long: `z80_rt.s` (`__mulsi3`, `__ashlsi3`, `__lshrsi3`, `__ashrsi3`, `__ashrhi3`,
`__s_udivremsi4`, `__s_divremsi4`, `__s_udivsi3`, `__s_umodsi3`, `__dvr_ep`), `runtime/compiler-rt/config/z80/` (`int32.s`,
`umulhisi3.s`, `sf32_*.s`). Compiled C in compiler-rt follows the convention. Word `__divremhi4`/`__udivhi3` keep DE quotient,
HL remainder; `lower_divrem_word` takes the remainder as the first part.
