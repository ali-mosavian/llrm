# Where gcc beats ours: cause cards, 2026-11-03

Asm and words only (no gcc source). Listings: `gcc-vs-ours-asm-{6502,65c02,z80}-2026-11-03.md` (same dir). gcc = spike
backends (naive ZP-register machine); ours = llrm-cc / z80c at release/next 0428edc613 (matrix numbers are from an older
build). Rows: 6502 NMOS 39, 65C02 40 (same causes), Z80 12 (pre-charge counts). "Not read" = the listing is there, the cause is not yet classified.

## Convention (fixed 2026-11-03 03:30)

gcc returns results in rc2:rc3 (Z80: HL), ours in A:X (Z80: DE). The gcc cells are now charged the caller's move: 6502 QI
+2 B +3 clk +1 insn, wider +4 B +6 clk +2 insn; Z80 16-bit +1 B +4 T +1 insn (`ex de,hl`). Cells that call my hand helpers
(mul/div) or carry a 512 B table (crc16) have `note` = "hand helper, not codegen" / "table, not codegen" and are not counted
as compiler wins. The M1 "A:X return" wins below are therefore partly gone; numbers in the row table are pre-charge
and must be re-read from `matrix/6502-gcc.tsv`.

## Mechanisms

**M1 A:X return duplicates work.** To hand back a 16-bit value in A:X, ours computes the low byte twice or shuffles through
tay/tax/tya. `k_aslax3` ours: `lda rc2; asl a; asl a; asl a` (value for A) then `asl rc2; rol rc3` x3 (carries into rc3), `ldx rc3`.
gcc: `asl rc2; rol rc3` x3; `rts` (13 B / 52 clk vs 17 / 64). Same shape: `k_shrax3` (ours also computes the high byte twice),
`k_asrax4`, `k_mulax6`, `k_mulax10`, `k_mul20`, `k_mul40` (ours ends `clc; adc rc4; tay; lda rc3; adc rc5; tax; tya`; gcc adds in
place in rc2/rc3). Part ABI, part a real cost of A:X returns; a peephole that adds into the A/X pair directly would remove it.

**M2 Booleans built by select blocks.** `k_isspace`, `k_ispunct`, `k_toupper`: ours materialises each `||` operand as 0/1
(`lda #1 / lda #0` joined by `cmt/cmd` blocks, `sta rc2; ... sta rc3`) and recombines; gcc keeps the compare-and-branch chain and
returns from the matching exit (`isspace` 47 clk vs 81; `ispunct` 146 vs 152; `toupper` 70 vs 72). Ours is smaller on isspace
(51 B vs 59 B) because gcc repeats the 16-bit `adc #247 / sbc #0` range check per arm.

**M3 Zero-init duplicated on the guarded-loop entry.** `popcount` (ours 34 B vs 30), `k_memchr`, `k_strnlen`, `k_strspn`,
`k_strcspn`, `crc16` entry: ours emits `lda #0; sta rc3` (or `lda #255; sta rc5; lda #255; sta rc6`) once on the zero-trip path
and again before the loop (`popcount_b13` / `popcount_b11`); gcc has one init and a second `rts`. 4-6 B per occurrence, no clock cost.

**M4 Constant offset folded into abs,y.** `fill` tail `return buf[n-1]`: gcc `ldy rc3; lda _buf-1,y`. Ours builds `&buf + n`
(`clc/adc`, `tay/txa/adc #0/tax`) then adds `-1` as two `adc #255` and loads through `(rc2),y` - about 24 B and 60 clk more.
(ours wins fill clocks, 201 vs 310: its loop is `sta buf,x`.)

**M5 Pointer copies for post-increment / lookahead.** `k_stpcpy`: ours copies the destination pointer to a temp
(`lda rc2; sta rc6; lda rc3; sta rc7`) and uses `ldy #1; lda (rc4),y; sta (rc6),y` to pipeline; gcc: one `ldy #0` loop, two 16-bit
`inc` pairs. `k_strchr` (ours 42 B vs 30) pipelines with `ldy #1` and a second block; `k_memmove` ours opens with four pointer
copies (`rc16..19 <- rc2..5`, 16 B) before the direction test; `k_strcmp`, `k_scmp`, `schr`: ours keeps a pointer plus an X index
(`inx; bne; inc rc3`, `txa; tay; lda (rc2),y`) where gcc walks one pointer (bytes only, 1-7 B).

**M6 Loop idiom -> library call.** `k_memset` ours: `jsr memset` with the return pointer spilled to two static cells
(`stx s_k_memset_0 ...`) and counted runtime bytes (67 B vs 47) - gcc -Os keeps the byte loop. (gcc -O2 also makes the call;
the harness links my plain `memset`, 60 B / 1318 clk, and rejects any cell that links cc65's.) Clocks go the other way: ours 530 vs 985.

**M7 Helper size, not codegen.** `mul8`/`mul16` (gcc 23 B / 47 B = call + my 19 B / 43 B shift-add loop; slower: 192 vs 196 clk,
570 vs 297), `k_tos{div,mod,udiv,umod}ax` (gcc 42-107 B, 807-856 clk; ours 89-149 B, 502-552 clk: ours helper is larger and faster),
`k_imul8x8r16` (69 vs 70 B). gcc also leaves `jsr ___udivhi3; rts` where ours emits `jmp` (tail call, 1 B + 6 clk).
Do not chase these: compare the helpers.

**M8 Full unroll of a constant trip count.** `sum4` (36 B / 64 clk vs ours 22 B / 98 clk): gcc -O1 unrolls the 4-iteration
loop into `lda _buf+i; sta rc3; clc; adc rc2; sta rc2` x4; ours keeps `ldx; lda buf,x; ... inx; cpx #4; bne`. Clocks win, bytes lose.

**M9 crc16 table.** gcc -O2 replaces the bit loop by a 256-entry table (512 B of `.word`s, 678 B total, 1999 clk vs ours 3438).
Excluded by the standing rule "crc16 stays the 88 B loop, no table". gcc -Os (loop, 102 B, ~6700 clk) loses to ours (70 B).

**M10 End-pointer bound instead of a 16-bit down-counter.** `lz4`/`lz4c` (28113 vs 37937 clk; 55568 vs 88722): the byte-copy loop
`while (n) { *out++ = *in++; --n; }`. Ours updates the counter every iteration
(`clc; lda rc4; adc #255; sta rc4; txa; adc #255; tax; lda rc4; bne; txa; bne`, ~9 insns) next to two pointer increments;
gcc (ivopts) drops the counter and compares one pointer with a precomputed end (`lda rc8; cmp rc16; bne; lda rc9; cmp rc17; bne`).
`k_memchr`, `k_strnlen`, `k_memmove` loops show the same bound-by-pointer shape. Ours is smaller on both lz4 rows (375 vs 437 B).

**M11 Plain ZP compare/subtract vs register shuffles.** `gcd`: ours keeps `a` in Y and `b` in X, so each round needs
`stx rc3`, `ldy rc2`, `cpy rc3`, `txa/tax` to compare and subtract; gcc's `lda rc2; cmp rc4; sec; lda; sbc; sta` loop has none
(33 B vs 40, 147 clk vs 164).

**M12 Static-cell spills.** `k_strncpy`, `k_memset`, `scmp` tail: ours parks live values in absolute static cells
(`s_k_strncpy_0`, 3 B + 4 clk per access) where free zero-page rc registers exist (gcc used rc8-rc13).

**M13 Value copied every iteration, used only on a match.** `k_strrchr`: ours copies the pointer (`ldy rc2; lda rc3; sta rc6`, with a
`pha/pla` around it to free A) and the loaded char on every trip; only the matching trip needs the copy. gcc copies `rc6 -> rc8` inside
the match block only (436 clk vs 603). Missing sinking of the copy into the conditional block.

**M14 Both operands lowercased by unconditional select.** `k_stricmp`, `k_strnicmp`: same shape as M2. Ours computes `c+159`, `c+224`, compares, then
picks with a branchless select for both strings on every trip; gcc branches, so a non-letter pays one range check (1462 vs 1872 clk, ours
101 B vs 103 B).

**M15 Inner-loop pointer copy.** `k_strpbrk` (ours 83 B vs 63): the set pointer is copied to rc16/rc17 (with `txa/pha ... pla`) before the inner scan, and the
null result `ldx #0; ldy #0; stx rc2; sty rc3` is emitted twice (M3). Ours is 50 clk faster (263 vs 312).

## Rows (6502 NMOS; 65C02 adds memcmp, memcpy, strncmp and drops scmp/schr/strchr clocks)

| row | gcc B/clk vs ours B/clk (best each) | cause |
|---|---|---|
| fill | 43/310 vs 46/201 | M4 (bytes only) |
| popcount | 30/245 vs 34/222 | M3 |
| mul8 | 23/192 vs 11/196 | M7 (clocks by 4 only) |
| gcd | 33/147 vs 40/164 | M11 |
| sum4 | 36/64 vs 22/98 | M8 |
| scmp | 65/106 vs 68/90 | M5 (bytes) |
| schr | 48/192 vs 49/178 | M5 (bytes, 1 B) |
| crc16 | 102/1999 vs 70/3438 | M9 |
| lz4, lz4c | 437/28113, 523/55568 vs 375/37937, 464/88722 | M10 |
| k_aslax3, k_asrax4, k_shrax3, k_mulax6, k_mulax10 | 13/52, 31/73, 13/52, 30/74, 34/84 vs 17/64, 37/88, 22/66, 33/84, 34/88 | M1 |
| k_mul20, k_mul40 | 42/94, 46/104 vs 42/105, 43/119 | M1 |
| k_imul8x8r16 | 69/646 vs 70/412 | M7 |
| k_isspace, k_ispunct, k_toupper | 59/47, 109/146, 37/70 vs 51/81, 109/152, 37/72 | M2 |
| k_memchr | 51/278 vs 63/352 | M3, M10 |
| k_memmove | 142/488 vs 180/885 | M5, M10 |
| k_memset | 47/985 vs 67/530 | M6, M12 |
| k_stpcpy | 28/482 vs 44/531 | M5 |
| k_strchr | 30/162 vs 42/152 | M5 (bytes) |
| k_strcmp | 62/840 vs 69/561 | M5 (bytes) |
| k_strncpy | 96/993 vs 116/904 | M12, M3 (bytes) |
| k_strnlen | 57/889 vs 81/1113 | M3, M10 |
| k_strspn, k_strcspn | 84/186, 84/370 vs 92/142, 88/306 | M3, M5 (bytes; ours faster) |
| k_strpbrk | 63/312 vs 83/263 | M15, M3 (bytes) |
| k_strrchr | 46/436 vs 41/603 | M13 (clocks) |
| k_stricmp, k_strnicmp | 133/1462 vs 101/1872; 150/1435 vs 127/2137 | M14 (clocks) |
| k_tosdivax, k_tosmodax, k_tosudivax, k_tosumodax | 103/850 ... 42/807 vs 149/552 ... 89/502 | M7 (bytes only) |

## Z80 (gcc wins, charged convention, helper/table cells excluded: bytes 1, clocks 10; ours = z80c -O2/-Oz, gcc results in A / HL, ours 16-bit in DE)

Instrument fix 04:10: the gcc crc16 -O2 cell had a 256-entry `.dw` table in `_DATA` that the byte count skipped (120 B, now 633 B,
noted "table, not codegen"). The row is no longer a gcc win.

**Z1 Base + index recomputed every trip.** `fill`: ours `ld hl,#_buf; add hl,bc; ld (hl),c` inside the loop; gcc walks a pointer (`ex de,hl; ld (hl),c; ex de,hl; inc de`)
and keeps the array base in IY for the tail (464 vs 508 T; ours 24 B vs 31).
**Z2 Pair copies through HL.** `scmp`, `reverse`: only HL addresses `cp (hl)`/`ld r,(hl)` on ours, so each string pointer is copied (`ld l,e; ld h,d` / `ld l,c; ld h,b`) every trip;
gcc swaps with one `ex de,hl` pair or reads `(de)`. `scmp` 150 vs 192 T, 26 vs 27 B.
**Z3 Loop state in the frame.** `reverse`, `mcopy`, `bsearch8`: ours parks the base pointer / counter / lo-hi in `-n(ix)` slots and reloads them each trip (`ld a,-2(ix); dec a; ... ld -2(ix),a` in `mcopy`);
gcc keeps the base in IY and bounds `mcopy` by an end pointer (M10) instead of a counter. `bsearch8` ours also merges the two arms with selects (`ld d,b; inc d` / `ld c,-3(ix)`) where gcc branches (M2).
**Z4 Return in DE vs HL.** `add16`: both are 3 B after the charge; gcc's `add hl,de; ex de,hl; ret` is 78 T vs ours 82 T; 4 T, not a real gap.
**Z5 Accumulate in IY / unroll.** `sum16` (ours 20 B `ld l,c; ld h,b; ld l,(hl); ld h,#0; add hl,de` vs gcc IY accumulate, 1059 vs 1094 T), `sum4` (M8, unrolled, clocks only), `gcd`, `fib` (pair moves through BC, 1-2 B, 10-60 T).

| row | gcc B/T vs ours B/T | cause |
|---|---|---|
| add16 | 3/78 vs 3/82 | Z4 |
| fill | 27/464 vs 24/508 | Z1 (clocks) |
| scmp | 27/154 vs 27/192 | Z2 |
| reverse | 57/644 vs 63/735 | Z2, Z3 |
| mcopy | 55/786 vs 55/969 | Z3, M10 |
| bsearch8 | 71/756 vs 68/790 | Z3, M2 |
| sum16 | 42/1059 vs 20/1094 | Z5 |
| sum4 | 20/142 vs 11/160 | M8 |
| gcd | 22/274 vs 21/285 | Z5 |
| fib | 23/676 vs 19/738 | Z5 |
| mul8, mul16, digits | helper cells | hand helper, not codegen |
| crc16 | 633 B at -O2 | table, not codegen (loop cells lose) |

All rows read from the listings (`gcc-vs-ours-asm-z80-2026-11-03.md`).

## Step 2 estimate (2026-11-03): code-quality ideas stop under the 1% rule

Upper bounds from a dataflow over the emitted -O2 asm of the 30 C kernels (labels merge predecessors; scripts `bench/proto_{y,a}_label.py`):
- Y known across labels: 15 of 46 `ldy` could go (lz4 4, lz4c 5, scmp 3, 3 others); 2 clk + 2 B each, one per loop trip.
- A kept live across labels: 8 `lda` could go (fletcher, gcd, lz4, lz4c, max8, popcount), most flags-dependent.
- Branch relaxation: 17 `b?? skip; jmp` pairs, 16 of them in lz4/lz4c/crc16 whose bodies exceed the window. No `bra` on NMOS, so they stay.
Each moves under 1% of the geomean over 69 programs (only 4-6 programs change, by 2-5% each). Not implemented. Instrument: static count on
`-O2` asm without the 51 hand kernels; if oversight wants the hand kernels in the bound, it is a rerun of the same script.

## Step 2 measured with the hand kernels (2026-11-03, replaces the estimate above)

The static estimate covered the 30 C kernels only. Measured: the full 6502 bench (81 programs x O1/O2/Os, hand kernels included) run through
a scratch wrapper (`cc1_peep.py`, a 6502-register tool, kept out of the repo), which deletes the `ldy #n` / `lda rcN` the dataflow over labels proves redundant (flags unread) after the real cc1.
Every cell still passes the result oracle. Geomean over the cells that change: best-per-program bytes -0.9%, clocks -1.1%; 19 of 81 programs change
(strpbrk, strcspn, strlen, popcount, max8 and lz4-like loops lead, 3-9% each). That is at the 1% line, so it is oversight's call; it is an upper bound
for these two patterns (no NMOS `bra`, so relaxation is not in it).
