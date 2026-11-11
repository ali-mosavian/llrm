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

## Same-tip recheck (2026-11-11)

The table above compares the tip 0db87355f6 with the PR binary; the two trees differ by more than #9013. Recheck with
one tree: 3b47b16114 (the commit before #9013) against 7009514d87 (after), the 20 worse programs above, `sweep.py
--ours-only`, 20 of 20 compiled per level and side. Pairs are (bytes, clocks), post minus pre.

- O2 / O3 (identical): div_constants_short (14, -359226), fixed_point (-1, 30), frameless (-14, 20163), int64_division (1, 1008),
  licm_graph_callee_footprint (1, 6000), loop_versioning (22, 8206), recdeep (36, -67189), rep_count (17, 40716),
  switch_peeled_table (8, 0), word_compare_branches (28, 1862), idioms (77, -116133). 11 worse.
- Os: frameless (-5, 24305), licm_graph_callee_footprint, loop_versioning (-23, 2605), rep_count (1, 9093),
  word_compare_branches (9, 13328), copy_runs (-9, 348), idioms (-80, 354936), memmove_overlap (1, 3308),
  switch_jump_table (2, -28). 9 worse. Oz: the same less the last two. 7 worse.
- O1: 16 worse, div_constants_long (-1320, 10701788) and div_constants_ulong (-719, 11067860) as before; also
  wide_shift_by_one (26, -40623), rep_count (-22, 41846), phi_slot_unknown_arm (5, 61548).

Corrections to the first list: `wide_shift_by_one` is 0 B and 0 T at O2/O3 (1110 B both sides), +26 B at O1, -57 B at Os;
the +100 B came from the older base tree, not from #9013. `rep_count` at O1 is 22 B smaller on the same tip, so it is not
newly above its bar from #9013. `recmany_o1` is not worse. `shrink_wrapped_frame`, `int64_division` at O1 improve in bytes.

## Causes

`wide_shift_by_one` O1 (+26 B, -5.4% T): `main` only, 971 -> 996 B; both sides keep the call form (price call 1028 -> 1054 B,
decomposed 892 -> 827 B; the O1 rule picks by relative regret). The delta is allocation churn around the long `fold32`
argument and result, now HL low / DE high. Output of `LLRM_Z80_PRICE_WHY` (local instrument, not committed).
`idioms` O2 (+77 B, -116k T): one `main`, 2091 -> 2058 instructions but `(ix+d)` accesses 854 -> 909 (3 B each), register moves 234 -> 182,
pushes 57 -> 31: more stack-slot traffic, fewer moves; the first differing allocation decision is not yet read.
Other cells (recdeep +36, word_compare_branches +28, loop_versioning +22, rep_count +17 at O2): first
differing decision not yet read.

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

## Callers and the runtime

Every routine that takes or returns a long: `z80_rt.s` (`__mulsi3`, `__ashlsi3`, `__lshrsi3`, `__ashrsi3`, `__ashrhi3`,
`__s_udivremsi4`, `__s_divremsi4`, `__s_udivsi3`, `__s_umodsi3`, `__dvr_ep`), `runtime/compiler-rt/config/z80/` (`int32.s`,
`umulhisi3.s`, `sf32_*.s`). Compiled C in compiler-rt follows the convention. Word `__divremhi4`/`__udivhi3` keep DE quotient,
HL remainder; `lower_divrem_word` takes the remainder as the first part.
