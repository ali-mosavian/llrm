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
| 6502-IDIOM-LIBCALL | fill_nest O1 O2; idioms O1 O2; memcpy_libcall O1 O2 Os; pattern_fill O1; rep_count O1; stack_arrays_copy O1 (10) | ours turns the loop into a memset/memcpy call (1-12 calls per program), gcc keeps it inline (0); the call costs more than the loop below some trip count. Counted in `-O1` asm of each program | 6502 | 2026-11-24 |
| 6502-CALLS-TAIL | call_keeps O1 O2; tail_calls O1 Os; memmove_overlap O1 (5) | cause not examined (no mem calls on either side) | 6502 | cause card 2026-11-17 |

The lever for the first row is the pin's cost for the idiom (a target hook on when the libcall is worth it), not a per-program switch.
