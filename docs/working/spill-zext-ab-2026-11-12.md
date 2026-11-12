# SPILL-ZEXT A/B, 2026-11-12

Base and new are built from one tree at release/next f8fb985b44 (merge 3a4f5d0bbe): base with the four source files of the PR reverted to release/next, new as committed; the 11 later release/next commits are merged without a rebuild. Executed instructions are `perf stat -e instructions:u` of the whole run, min of 3, so start-up is in both (noise about +-2). x86_64-linux, 188 programs (`bench/*/*.c`, `tests/run/c/*.c`) at -Os -O1 -O2 -O3, code bytes of the object (`tools/ab/object_bytes.py`, padding-free). 3 programs (`huge`, `lru`, `textfill`) do not compile on either side without their own flags.

## Cells whose bytes changed (35 of 752)

| program | level | bytes base | bytes new | delta | executed instr. base | new | delta |
|---|---|---|---|---|---|---|---|
| jpeg_fixed | -O2 | 8189 | 8135 | -54 | 154704 | 154705 | +1 |
| jpeg_fixed | -O1 | 3760 | 3713 | -47 | 154776 | 154776 | +0 |
| jpeg_fixed | -Os | 3456 | 3413 | -43 | 154776 | 154776 | +0 |
| jpeg_single | -O1 | 3622 | 3582 | -40 | 17562619 | 17562628 | +9 |
| jpeg_single | -O2 | 8970 | 8931 | -39 | 11466616 | 11466815 | +199 |
| jpeg_single | -O3 | 12206 | 12173 | -33 | 11100307 | 11100497 | +190 |
| jpeg_fixed | -O3 | 9487 | 9459 | -28 | 154706 | 154704 | -2 |
| jpeg_single | -Os | 3378 | 3355 | -23 | 18370492 | 18371476 | +984 |
| long_compare_chain | -O2 | 758 | 742 | -16 | 277928 | 277868 | -60 |
| long_compare_chain | -O3 | 758 | 742 | -16 | 277928 | 277868 | -60 |
| sfm16_float | -Os | 2116 | 2102 | -14 | 184473 | 184473 | +0 |
| sfm16_float_add | -Os | 1862 | 1848 | -14 | 158463 | 158463 | +0 |
| sfm16_float_cmp | -Os | 1935 | 1921 | -14 | 167609 | 167608 | -1 |
| sfm16_float_div | -Os | 1866 | 1852 | -14 | 158863 | 158863 | +0 |
| sfm16_float_f2i | -Os | 1869 | 1855 | -14 | 158862 | 158863 | +1 |
| sfm16_float_i2f | -Os | 1846 | 1832 | -14 | 157682 | 157682 | +0 |
| sfm16_float_mul | -Os | 1866 | 1852 | -14 | 158865 | 158866 | +1 |
| sfm16_float_sub | -Os | 1866 | 1852 | -14 | 158829 | 158828 | -1 |
| spill_store_order | -O1 | 534 | 521 | -13 | 200088 | 200112 | +24 |
| spill_store_order | -Os | 469 | 459 | -10 | 197326 | 197352 | +26 |
| queens | -O3 | 1541 | 1537 | -4 | 270909 | 270672 | -237 |
| stft_fixed | -O2 | 1212 | 1208 | -4 | 163706 | 163707 | +1 |
| stft_fixed | -O3 | 1212 | 1208 | -4 | 163705 | 163707 | +2 |
| stft_fixed | -O1 | 1142 | 1139 | -3 | 173563 | 173563 | +0 |
| recchop | -O3 | 1828 | 1826 | -2 | 159123 | 159125 | +2 |
| stft_fixed | -Os | 1131 | 1129 | -2 | 173562 | 173562 | +0 |
| stft_single | -Os | 1043 | 1041 | -2 | 172156 | 172156 | +0 |
| idioms | -Os | 1205 | 1203 | -2 | 393356 | 393356 | +0 |
| memfuncs_runtime | -O2 | 968 | 966 | -2 | 132084293 | 132084294 | +1 |
| memfuncs_runtime | -O3 | 968 | 966 | -2 | 132084295 | 132084295 | +0 |
| over_aligned_local_after_even_pushes | -O1 | 854 | 852 | -2 | 204327 | 204327 | +0 |
| over_aligned_local_after_even_pushes | -O2 | 798 | 796 | -2 | 203799 | 203799 | +0 |
| over_aligned_local_after_even_pushes | -O3 | 798 | 796 | -2 | 203798 | 203800 | +2 |
| spill_store_order | -O2 | 653 | 652 | -1 | 185140 | 185175 | +35 |
| spill_store_order | -O3 | 883 | 886 | +3 | 182482 | 182482 | +0 |

## Cells whose object changed at the same size (executed output equal)

- recpow -O2: instr 214395 -> 214394 (-1)
- recpow -O3: instr 214395 -> 214394 (-1)
- rectwo -O2: instr 316079 -> 316079 (+0)
- rectwo -O3: instr 316078 -> 316079 (+1)
- stft_single -O1: instr 172157 -> 172157 (+0)
- stft_single -O2: instr 162364 -> 162364 (+0)
- stft_single -O3: instr 162364 -> 162364 (+0)

## Program output
42 cells whose object differs were linked with the POSIX layer and run: output equal to base on all 42 (`MISMATCH` 0). 710 cells have a byte-identical object.

## Other targets (object md5, base vs new)

| target | cells | identical |
|---|---|---|
| 6502-cc65 | 84 | 84 |
| arm64-linux | 92 | 92 |
| arm64-macos | 92 | 92 |
| m68k-68000 | 92 | 92 |
| m68k-68020 | 92 | 92 |
| wasm32 | 92 | 92 |
| x86_16-dos | 90 | 90 |
| x86_32-linux | 92 | 92 |
| x86_64-linux | 92 | 88 (positive control: the 4 differing are m64 cells) |
| z80-sdcc | 84 | 84 |

Identity ran on the first 24 programs of each corpus at -Os and -O2. Not run: wasm/arm64/m68k/6502/z80 beyond that sample. The fact is read only by `X86InstrInfo` at `bits == 64`.
