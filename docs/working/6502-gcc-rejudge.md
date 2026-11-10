# 6502 vs gcc: the re-judge after the flag fix

Owner: 6502. Opened 2026-11-10. The gcc progs column was built with other flags than ours ([#8715](https://github.com/ali-mosavian/llrm/pull/8715)); the 11-04 gcc-vs-ours cells of 19 programs are withdrawn and redone here (gcc cc1 build-6502, ours dzB llc md5 88b4cf59f7, executed on sim65, pads 0,37,101,200).

Priced as the matrix does: O2 clocks + 3 B, Os strict bytes, O1 per axis. Cells: `scratch/handoff/matrix/6502-gcc-progs-flagfix-2026-11-09/`.

**57 cells (19 programs x O1/O2/Os), corrected gcc column, ours vs gcc per axis:**

| level | cells | bytes worse | clocks worse | either axis | both axes | priced (clocks + 3 B) worse |
|---|---|---|---|---|---|---|
| O1 | 19 | 1 | 8 | 9 | 0 | 7 |
| O2 | 19 | 7 | 4 | 8 | 3 | 4 |
| Os | 19 | 2 | 8 | 9 | 1 | 8 |
| all | 57 | 10 | 20 | 26 | 4 | 19 |

The scoreboard numbers are the last row's bytes and clocks: 10 bigger, 20 slower (26 cells with either). "Worse" below is the matrix rule above (O1 either axis, O2 priced, Os bytes): 15 cells, 21 under the old column.

- Worse -> better (rows to close): frameless O1 Os; licm_graph_callee_footprint O1 O2 Os; mem_length_boundaries_m32 O2; memmove_overlap O2; pattern_fill O2; tail_calls O2.
- Better -> worse (newly open): memcpy_libcall Os; tail_calls O1 Os.

## Open cells and owners

| row | cells | cause | owner | exit |
|---|---|---|---|---|
| 6502-LSR-POINTER | fill_nest O1; pattern_fill O1; rep_count O1; stack_arrays_copy O1 (4) | executed (sim65 pc profile, dzB): 55-78% of the clocks sit in one inline loop that recomputes its address every trip (`asl`/`rol` of the 16-bit index, `clc`/`adc` of the base, 11-17 instructions); gcc walks a pointer. LSR leaves `base + i*size` where a pointer increment is cheaper: the same cause as k_strnlen | 6502 (with w12's `lsr_model` seam) | 2026-11-24 |
| 6502-LSR-WORSE | O1: byval_inline (+27 B), function_table (+80 B, +16% clk), idioms (+64 B), memcpy_libcall (+15 B, +2% clk), sat_builtins (+43 B), split_loops (+4 B), static_pointers (+42 B), switch_peeled_table (+55 B, +23% clk); O2: idioms (+254 B), memcpy_libcall (+27 B), sat_builtins (+43 B), switch_peeled_table (+61 B, +23% clk) vs the release/next tip ad929191cb | executed (pad 0, pass82): with 16-bit widths LSR reduces loops with an address use into several 16-bit counters and pointers (switch_peeled_table: 3 induction variables); the 6502 cost weighs a counter step at 3 and a base add at 7, not the shl and table add it removes. Dropping the keep-address-solutions hook fixes switch_peeled_table/byval_inline/memcpy_libcall but loses rep_count, loop_interchange, fill_nest (measured) | 6502-d | 2026-11-24 |
| 6502-MUL-CONST | call_keeps O1 O2; idioms O1 O2 (4) | executed: `__mulsi3` takes 13.1k of 30.0k clocks in call_keeps (6 calls, `ldy #11`: 3, 5, 7 are expanded as 2^n-1, 11 is not; gcc synthesizes it) and 78% of idioms' 30.9M. The 32-iteration shift-add loop is the cost | 6502 | 2026-11-24 |
| 6502-STACKARG-COPY | tail_calls O1 Os (bytes) | static, per function: `main` 377 insns against gcc's 245, the callees 58 against 30. Each outgoing stack-argument group copies `rs0` into a pointer pair (8 B, 4 times in main) instead of using `(rs0),y`; a 2-byte stack-to-stack copy loads both bytes before storing either (11 instructions through X and a temp, gcc 4). The ABI is not a cause to fix: the mos ABI passes 8 bytes in registers, gcc's passes more | 6502 | 2026-11-24 |
| 6502-MEMMOVE-OS | memcpy_libcall O1 O2 Os; memmove_overlap O1 (4) | Os is strict bytes: 644 B against gcc's 633. Whole-program static: 277 instructions against 263; ours keeps `same` (a 3-argument loop) as two calls, gcc inlines it. O1/O2 clocks +0.09%/+0.9%. The loop bodies are not yet compared | 6502 | cause card 2026-11-17 |

The first correction of this table: the earlier row 6502-IDIOM-LIBCALL named the libcall as the cause from a static count of `memset`/`memcpy` calls (ours 1-12 per program, gcc 0). The executed profile does not support it: the libcall routines hold 10-25% of the clocks in those programs, the loops 55-78%. No idiom hook (`memory_idioms_at_optsize`, m68k) is the lever here.
