# #8532 priced multiply decomposition: the cells worse than base

Ours-only A/B, base = release/next 44a0657729, head = base + #8532 (2d399f8ff4), 68000/68020 x O1/O2/O3/Os, 75 programs change.
Bytes: code of the whole linked program (tools/m68k_corpus_matrix.py reader), all 1624 cells: 280 better, 86 worse, 1258 same.
Clocks: `cycles --exe` executed clocks of one run (functions outside h_*, plus `main`, `CYCLES_CALLS=1`), 560 of the 600 cells of the changed programs; five programs (qbsp, wc_mixed, algebra, parity, memory) did not link or run, so their 40 cells have bytes only; jpeg_fixed and jpeg_single exit after 194 instructions (no input), so their clocks say nothing.
Price: O1 strict (bytes and clocks), Os strict bytes, O2/O3 B-eq = d bytes + 2 x d clocks.
Rows (owner m68k-b): M68K-MUL-CALLTARGET 6, M68K-MUL-HALFWORD 4, M68K-MUL-RATE 72, M68K-MUL-SCHED 4 = 86 cells worse in bytes.

| program | cpu | level | base B | head B | d bytes | d clocks | by its price | row |
|---|---|---|---|---|---|---|---|---|
| bench/jpeg_fixed/jpeg_fixed | 68000 | O3 | 6556 | 6614 | +58 | 0 | unmeasured | M68K-MUL-CALLTARGET |
| bench/nbody_fixed/nbody_fixed | 68000 | O1 | 532 | 534 | +2 | 88 | WORSE (strict) | M68K-MUL-CALLTARGET |
| run/int64_division | 68000 | O1 | 654 | 694 | +40 | -120808 | WORSE (strict) | M68K-MUL-CALLTARGET |
| run/int64_division | 68000 | O2 | 652 | 674 | +22 | -121984 | rate better | M68K-MUL-CALLTARGET |
| run/int64_division | 68000 | O3 | 652 | 674 | +22 | -121984 | rate better | M68K-MUL-CALLTARGET |
| run/int64_division | 68000 | Os | 654 | 672 | +18 | -120288 | WORSE (strict bytes) | M68K-MUL-CALLTARGET |
| bench/matmul/matmul | 68000 | O1 | 306 | 308 | +2 | -4192 | WORSE (strict) | M68K-MUL-HALFWORD |
| bench/matmul/matmul | 68000 | O2 | 618 | 620 | +2 | -4192 | rate better | M68K-MUL-HALFWORD |
| bench/matmul/matmul | 68000 | O3 | 618 | 620 | +2 | -4192 | rate better | M68K-MUL-HALFWORD |
| bench/matmul/matmul | 68000 | Os | 286 | 290 | +4 | -3936 | WORSE (strict bytes) | M68K-MUL-HALFWORD |
| bench/bintree/bintree | 68020 | O2 | 254 | 260 | +6 | -108220 | rate better | M68K-MUL-RATE |
| bench/bintree/bintree | 68020 | O3 | 254 | 260 | +6 | -108220 | rate better | M68K-MUL-RATE |
| bench/frames/frames | 68020 | O2 | 156 | 160 | +4 | -5139078 | rate better | M68K-MUL-RATE |
| bench/frames/frames | 68020 | O3 | 156 | 160 | +4 | -5139078 | rate better | M68K-MUL-RATE |
| bench/jpeg_fixed/jpeg_fixed | 68020 | O2 | 4854 | 4868 | +14 | 0 | unmeasured | M68K-MUL-RATE |
| bench/jpeg_fixed/jpeg_fixed | 68020 | O3 | 7572 | 7596 | +24 | 0 | unmeasured | M68K-MUL-RATE |
| bench/jpeg_single/jpeg_single | 68020 | O2 | 4630 | 4644 | +14 | 0 | unmeasured | M68K-MUL-RATE |
| bench/jpeg_single/jpeg_single | 68020 | O3 | 7414 | 7438 | +24 | 0 | unmeasured | M68K-MUL-RATE |
| bench/mandel/mandel | 68020 | O2 | 156 | 160 | +4 | -27720 | rate better | M68K-MUL-RATE |
| bench/mandel/mandel | 68020 | O3 | 156 | 160 | +4 | -27720 | rate better | M68K-MUL-RATE |
| bench/matmul/matmul | 68020 | O2 | 522 | 524 | +2 | -1600 | rate better | M68K-MUL-RATE |
| bench/matmul/matmul | 68020 | O3 | 522 | 524 | +2 | -1600 | rate better | M68K-MUL-RATE |
| bench/qbsp/qbsp | 68020 | O2 | 524 | 528 | +4 | n/a | unmeasured | M68K-MUL-RATE |
| bench/qbsp/qbsp | 68020 | O3 | 524 | 528 | +4 | n/a | unmeasured | M68K-MUL-RATE |
| bench/switch/switch | 68020 | O2 | 326 | 330 | +4 | -226000 | rate better | M68K-MUL-RATE |
| bench/switch/switch | 68020 | O3 | 326 | 330 | +4 | -226000 | rate better | M68K-MUL-RATE |
| run/call_clobbers_flags | 68020 | O2 | 126 | 128 | +2 | -39 | rate better | M68K-MUL-RATE |
| run/call_clobbers_flags | 68020 | O3 | 126 | 128 | +2 | -39 | rate better | M68K-MUL-RATE |
| run/compare_to_int64 | 68020 | O2 | 254 | 266 | +12 | -339 | rate better | M68K-MUL-RATE |
| run/compare_to_int64 | 68020 | O3 | 348 | 352 | +4 | -384 | rate better | M68K-MUL-RATE |
| run/compound_assignment_types | 68020 | O2 | 250 | 254 | +4 | -159 | rate better | M68K-MUL-RATE |
| run/compound_assignment_types | 68020 | O3 | 250 | 254 | +4 | -159 | rate better | M68K-MUL-RATE |
| run/copy_runs | 68020 | O2 | 1204 | 1208 | +4 | -3973 | rate better | M68K-MUL-RATE |
| run/copy_runs | 68020 | O3 | 1204 | 1208 | +4 | -3973 | rate better | M68K-MUL-RATE |
| run/div_constants_short | 68020 | O2 | 4502 | 4558 | +56 | -891504 | rate better | M68K-MUL-RATE |
| run/div_constants_short | 68020 | O3 | 4502 | 4558 | +56 | -891504 | rate better | M68K-MUL-RATE |
| run/div_constants_ulong | 68020 | O2 | 3602 | 3738 | +136 | -962128 | rate better | M68K-MUL-RATE |
| run/div_constants_ulong | 68020 | O3 | 3602 | 3738 | +136 | -962128 | rate better | M68K-MUL-RATE |
| run/huffman_decode | 68020 | O2 | 608 | 616 | +8 | -5432 | rate better | M68K-MUL-RATE |
| run/huffman_decode | 68020 | O3 | 608 | 616 | +8 | -5432 | rate better | M68K-MUL-RATE |
| run/i64_param_home | 68020 | O2 | 248 | 250 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/i64_param_home | 68020 | O3 | 248 | 250 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/ia16_abi | 68020 | O2 | 220 | 222 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/ia16_abi | 68020 | O3 | 220 | 222 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/int64_division | 68020 | O2 | 460 | 486 | +26 | -11312 | rate better | M68K-MUL-RATE |
| run/int64_division | 68020 | O3 | 460 | 486 | +26 | -11312 | rate better | M68K-MUL-RATE |
| run/loop_interchange | 68020 | O2 | 452 | 464 | +12 | -113472 | rate better | M68K-MUL-RATE |
| run/loop_interchange | 68020 | O3 | 452 | 464 | +12 | -113472 | rate better | M68K-MUL-RATE |
| run/memmove_overlap | 68020 | O2 | 288 | 294 | +6 | -1880 | rate better | M68K-MUL-RATE |
| run/memmove_overlap | 68020 | O3 | 288 | 294 | +6 | -1880 | rate better | M68K-MUL-RATE |
| run/mulconst_i64 | 68020 | O2 | 1480 | 1506 | +26 | -11990 | rate better | M68K-MUL-RATE |
| run/mulconst_i64 | 68020 | O3 | 1480 | 1506 | +26 | -11990 | rate better | M68K-MUL-RATE |
| run/old_style_extra_args | 68020 | O2 | 270 | 272 | +2 | -37 | rate better | M68K-MUL-RATE |
| run/old_style_extra_args | 68020 | O3 | 270 | 272 | +2 | -37 | rate better | M68K-MUL-RATE |
| run/os_locals_between_pushes | 68020 | O2 | 252 | 274 | +22 | -424 | rate better | M68K-MUL-RATE |
| run/os_locals_between_pushes | 68020 | O3 | 252 | 274 | +22 | -424 | rate better | M68K-MUL-RATE |
| run/phi_slot_unknown_arm | 68020 | O2 | 248 | 254 | +6 | -114404 | rate better | M68K-MUL-RATE |
| run/phi_slot_unknown_arm | 68020 | O3 | 248 | 254 | +6 | -114404 | rate better | M68K-MUL-RATE |
| run/rep_tails | 68020 | O2 | 1124 | 1146 | +22 | -216978 | rate better | M68K-MUL-RATE |
| run/rep_tails | 68020 | O3 | 1124 | 1146 | +22 | -216978 | rate better | M68K-MUL-RATE |
| run/rotate_small | 68020 | O2 | 270 | 282 | +12 | -7146 | rate better | M68K-MUL-RATE |
| run/rotate_small | 68020 | O3 | 270 | 282 | +12 | -7146 | rate better | M68K-MUL-RATE |
| run/select_of_constants | 68020 | O2 | 20440 | 21254 | +814 | -381350 | rate better | M68K-MUL-RATE |
| run/select_of_constants | 68020 | O3 | 20440 | 21254 | +814 | -381350 | rate better | M68K-MUL-RATE |
| run/shift_int64_variable | 68020 | O2 | 346 | 348 | +2 | -57594 | rate better | M68K-MUL-RATE |
| run/shift_int64_variable | 68020 | O3 | 346 | 348 | +2 | -57594 | rate better | M68K-MUL-RATE |
| run/spill_store_order | 68020 | O2 | 610 | 612 | +2 | -105255 | rate better | M68K-MUL-RATE |
| run/spill_store_order | 68020 | O3 | 800 | 802 | +2 | -89019 | rate better | M68K-MUL-RATE |
| run/struct_odd_sizes | 68020 | O2 | 176 | 178 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/struct_odd_sizes | 68020 | O3 | 176 | 178 | +2 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/switch_table_holes | 68020 | O2 | 158 | 160 | +2 | -10918 | rate better | M68K-MUL-RATE |
| run/switch_table_holes | 68020 | O3 | 158 | 160 | +2 | -10918 | rate better | M68K-MUL-RATE |
| run/sysv_abi | 68020 | O2 | 292 | 296 | +4 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/sysv_abi | 68020 | O3 | 292 | 296 | +4 | 0 | WORSE (rate) | M68K-MUL-RATE |
| run/tail_calls_edges | 68020 | O2 | 912 | 918 | +6 | -375 | rate better | M68K-MUL-RATE |
| run/tail_calls_edges | 68020 | O3 | 912 | 918 | +6 | -375 | rate better | M68K-MUL-RATE |
| run/variadic | 68020 | O2 | 348 | 350 | +2 | -37 | rate better | M68K-MUL-RATE |
| run/variadic | 68020 | O3 | 348 | 350 | +2 | -37 | rate better | M68K-MUL-RATE |
| run/wc_mixed | 68020 | O2 | 598 | 602 | +4 | n/a | unmeasured | M68K-MUL-RATE |
| run/wc_mixed | 68020 | O3 | 598 | 602 | +4 | n/a | unmeasured | M68K-MUL-RATE |
| run/wide_shift_by_one | 68020 | O2 | 270 | 280 | +10 | -52556 | rate better | M68K-MUL-RATE |
| run/wide_shift_by_one | 68020 | O3 | 270 | 280 | +10 | -52556 | rate better | M68K-MUL-RATE |
| run/select_of_constants | 68000 | O1 | 18652 | 19760 | +1108 | -4292240 | WORSE (strict) | M68K-MUL-SCHED |
| run/select_of_constants | 68000 | O2 | 18632 | 19740 | +1108 | -4292240 | rate better | M68K-MUL-SCHED |
| run/select_of_constants | 68000 | O3 | 18632 | 19740 | +1108 | -4292240 | rate better | M68K-MUL-SCHED |
| run/select_of_constants | 68000 | Os | 16916 | 19672 | +2756 | -4187664 | WORSE (strict bytes) | M68K-MUL-SCHED |

## 68020 -O1 per constant (mul_constants' constants): chain against `muls.l`, and gcc

Model of `crates/target/llrm-m68k/examples/mulprice.rs` (bytes / clocks from the m68k tables; the chain has no constant to load). "Picks" = the hook at -O1 strict / -Os / -O2. Function bytes = operand load (4 B) + sequence, without `rts`; gcc 15 `-m68020 -O1` single function `x*K`.

| x* | chain B / clk | muls.l B / clk | picks | ours muls.l function B | gcc form, B |
|---|---|---|---|---|---|
| 3 | 6 / 8 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 10 |
| 5 | 6 / 8 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 10 |
| 6 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 9 | 6 / 8 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 10 |
| 10 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 12 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 17 | 6 / 8 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 10 |
| 18 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 20 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 24 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 25 | none | 4 / 45 (+2 B constant load) | muls.l / muls.l / muls.l | 10 | chain 14 |
| 36 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 40 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 45 | none | 4 / 45 (+2 B constant load) | muls.l / muls.l / muls.l | 10 | chain 16 |
| 72 | 8 / 12 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 12 |
| 81 | none | 4 / 45 (+2 B constant load) | muls.l / muls.l / muls.l | 10 | chain 14 |
| 100 | none | 4 / 45 (+2 B constant load) | muls.l / muls.l / muls.l | 10 | chain 16 |
| 125 | none | 4 / 45 (+2 B constant load) | muls.l / muls.l / muls.l | 10 | chain 14 |
| 127 | 6 / 8 | 4 / 45 (+2 B constant load) | muls.l / muls.l / chain | 10 | chain 10 |
| 129 | 6 / 8 | 4 / 45 (+6 B constant load) | muls.l / muls.l / chain | 14 | chain 10 |
| 255 | 6 / 8 | 4 / 45 (+6 B constant load) | muls.l / muls.l / chain | 14 | chain 10 |

Reading: with the constant's register set per site, the chain is no larger (3, 5, 9, 17: tie; 129, 255: 4 B smaller) and 5x faster, so strict -O1 would take it; with the register shared across sites `muls.l` is 2 B smaller per extra site. The hook cannot know the sharing, so it assumes shared and keeps `muls.l`. gcc picks the chain for every constant (`synth_mult`, 25..125 too). Plan: M68K-MUL-LATE, a priced rewrite after selection that sees the constant register's real uses (like `calltarget::choose`), and `synth_mult` for the constants with no `(2^n±1)<<k` form. Owner m68k-b, exit 2026-11-14.
