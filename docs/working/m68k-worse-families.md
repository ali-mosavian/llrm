# m68k: the 478 clock-worse cells by family (2026-11-11 01:25Z, m68k-b)

TL;DR: the 478 cells where ours is slower than the best of gcc/clang (#8970) fall in 15 families; the top 10 hold 461 of them. The two largest are register/immediate moves (193 cells, mostly small) and `muls/mulu/divs/divu` executed where the best external avoids them (90 cells, median +21%, 24 over +50%).

What a family is: for each cell, executed clocks of ours and of the best external column by instruction class, the class with the largest excess. Cost inside a routine of the linked runtime (compiler-rt archive, harness stubs) is charged to that routine, found by symbol. A family names where the extra clocks are, not why: it is a pull list, not a root cause.

Method: tip 62d11819cf (llrm-cc 49a365db, cycles c8907d60). The cells and their worse/best figures are #8970's (def16209); each cell's two exes were rebuilt on the tip and listed with `CYCLES_LISTING` (instruction x count x clocks). Branches are listed untaken, so class sums are an approximation: the listing covers a median 96% of the cell's clocks (71% to 102%; 6 cells under 85%). 68000 and 68020, O1 to Os, all 478 cells, each with the family and the top two classes in the tsv. The 285 cells measured against one external are included (flagged partial in #8970), their best is the one that exists.

| rank | family | cells | over 5% | over 50% | median % | WORSE after bytes | first program (largest %) |
|---|---|---|---|---|---|---|---|
| 1 | register/immediate moves (copies, constants, reloads) | 193 | 72 | 6 | 2.3 | 173 | shift_int64_masked_amount (68000 O3, +110%) |
| 2 | `muls`/`mulu`/`divs`/`divu` executed | 90 | 76 | 24 | 21.5 | 71 | shift_int64_variable (68020 O1, +134%) |
| 3 | multiply by library call (`__mulsi3`...) | 50 | 49 | 16 | 24.9 | 41 | loop_versioning (68000 O3, +366%) |
| 4 | alu/ext/clr/swap | 42 | 28 | 9 | 13.3 | 36 | byte_multiply (68000 O3, +328%) |
| 5 | lea/pea | 27 | 0 | 0 | 0.3 | 14 | static_pointers (68020 O3, +2%) |
| 6 | branches/returns | 17 | 10 | 0 | 6.4 | 13 | long_double_constant_store (68020 O3, +45%) |
| 7 | soft-float runtime | 14 | 13 | 8 | 36068.8 | 14 | float_in_memory_loop (68000 O3, +220503%) |
| 8 | movem save/restore | 10 | 3 | 0 | 3.4 | 7 | u64_shift_select_arms (68020 Os, +10%) |
| 9 | compare/test | 10 | 7 | 3 | 15.9 | 10 | switch_peeled_table (68000 O3, +81%) |
| 10 | call/return overhead | 8 | 4 | 0 | 4.3 | 8 | tail_calls (68020 O3, +10%) |

The other 5 families (17 cells): divide/modulo by library call (7), memcpy/memset (5), other runtime: `sf_cmp_bits_64` (2), shifts/rotates (2), other runtime: `pack` (1).

Top-10 total: 461 of 478 cells.
