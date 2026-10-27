# Instructions against the pin's llc, on the same IR

Why: on the same IR, address folding equals the pin's (`docs/working/codegenprepare.md`), yet our backend has more instructions than the pin's `llc` with CodeGenPrepare on: +2.7% at -Os and +6.3% at -O2 over the 65 QCport modules at m16. This classifies them by mnemonic. Owner of the file: w-4; instrument `classes.py` (our IR at the switchconv stage; ours `llrm-c -S`, the pin's `llc -O2 -filetype=asm`; stamped base 25d838613b).

Caveats, for every table:
- Same IR for both, so the mid-end is not the difference; the code generators are.
- The pin's m16 target is code16 with 32-bit registers, scaled addressing and no segment registers in the IR: ours is real mode with BX/SI/DI/AX address and operation constraints and far pointers (`les`/`lfs`/`lgs`). The m16 tables are a trend, not a gap. The m32 tables are like-for-like in instruction set.
- Convention: our m32 uses `-mabi=sysv` and ours puts private functions on a register convention (`cc 1005`/`1002`) where the pin's llc sees an unknown calling convention number as C; the pin's `fastcc` for private functions is not reproduced (AM-CC rows). `LLRM_PIN_FASTCC` is not on this tree, so not used.
- Instruction counts, not bytes or clocks: a `lea` chain is more instructions than an `imul` and fewer 486 clocks.
- opt vs optsize: the pin's target is told `optsize` at -Os.

## Tables

### m32, the 71 vsgcc programs (like-for-like), -O2 (71 programs)

| class | ours | pin | delta |
|---|--:|--:|--:|
| mov | 2456 | 2429 | +27 |
| alu | 2822 | 2679 | +143 |
| branch | 724 | 699 | +25 |
| call/ret | 433 | 424 | +9 |
| push/pop/frame | 807 | 609 | +198 |
| lea | m32 +471 (-O2), -63 (-Os) | `recpow` 163 against 3, `x_switch` 78 against 6, `x_fir` 54 against 0: multiplications by constants are `lea` chains chosen by the CPU profile's clocks where the pin's `combineMul` leaves `imul` outside 3/5/9 and 2^n+-1 | DEVIATE M1 (AM-M1) in `arithmetic.rs`, priced: chains on vs off over the 71 programs, clocks -5.1% / -5.9% / -6.0% at -O1 / -O2 / -O3, code +3072 / +720 / +688 B, -Os identical | owner w-4, review 2026-12-01 |
| x87 | 1064 | 1137 | -73 |
| setcc/cmov | 6 | 13 | -7 |
| string | 43 | 0 | +43 |
| other | 2 | 5 | -3 |
| **all** | **9247** | **8414** | **+833** (+9.9%) |

Top 20 by delta: recpow.c +171, matmul.c +107, x_switch.c +65, x_fir.c +64, quicksort.c +55, x_mergesort.c +52, bintree.c +31, recchop.c +31, x_iir.c +26, x_isqrt.c +24, x_heapsort.c +22, x_popcount.c +22, queens.c +20, x_bubble.c +20, x_hash.c +20, x_life.c +20, x_switch_sparse.c +20, x_binsearch.c +19, x_transpose.c +18, x_radix.c +16.

### m32, the 71 vsgcc programs (like-for-like), -Os (71 programs)

| class | ours | pin | delta |
|---|--:|--:|--:|
| mov | 1252 | 1284 | -32 |
| alu | 1657 | 1895 | -238 |
| branch | 435 | 451 | -16 |
| call/ret | 335 | 347 | -12 |
| push/pop/frame | 780 | 573 | +207 |
| lea | 98 | 161 | -63 |
| x87 | 320 | 318 | +2 |
| setcc/cmov | 6 | 14 | -8 |
| string | 27 | 0 | +27 |
| other | 2 | 5 | -3 |
| **all** | **4912** | **5048** | **-136** (-2.7%) |

Top 20 by delta: lru.c +13, x_mergesort.c +13, x_transpose.c +13, queens.c +9, x_reverse.c +9, quicksort.c +8, x_ll_arith.c +8, x_life.c +7, x_popcount.c +7, matmul.c +6, crc.c +5, x_hanoi2.c +5, x_isqrt.c +5, x_binsearch.c +4, x_matmul_f.c +3, x_primes.c +3, x_radix.c +3, x_cordic.c +2, x_switch.c +2, ring.c +1.

### m16, the 65 QCport modules (a trend), -O2 (65 programs)

| class | ours | pin | delta |
|---|--:|--:|--:|
| mov | 27982 | 21060 | +6922 |
| alu | 17806 | 17733 | +73 |
| branch | 9110 | 9187 | -77 |
| call/ret | 4944 | 4467 | +477 |
| push/pop/frame | 12061 | 12411 | -350 |
| lea | 2682 | 2598 | +84 |
| x87 | 16765 | 20147 | -3382 |
| setcc/cmov | 1087 | 699 | +388 |
| string | 74 | 8 | +66 |
| other | 722 | 1 | +721 |
| **all** | **93233** | **88311** | **+4922** (+5.6%) |

Top 20 by delta: part.c +452, cmd.c +443, sc.c +420, savegame.c +338, triggers.c +281, ent.c +263, mdl_ai.c +262, weapons.c +234, keybind.c +203, menu.c +188, snd_mix.c +179, r_bsp.c +175, item.c +172, mdl.c +168, screen.c +166, d_alias.c +160, dl.c +143, d_faces.c +114, pl_trace.c +114, d_sprite.c +95.

### m16, the 65 QCport modules (a trend), -Os (65 programs)

| class | ours | pin | delta |
|---|--:|--:|--:|
| mov | 22697 | 16889 | +5808 |
| alu | 12501 | 15019 | -2518 |
| branch | 6211 | 6488 | -277 |
| call/ret | 4741 | 4519 | +222 |
| push/pop/frame | 12691 | 12562 | +129 |
| lea | 2092 | 2139 | -47 |
| x87 | 13978 | 16762 | -2784 |
| setcc/cmov | 887 | 573 | +314 |
| string | 43 | 0 | +43 |
| other | 616 | 1 | +615 |
| **all** | **76457** | **74952** | **+1505** (+2.0%) |

Top 20 by delta: sc.c +326, mdl_ai.c +180, savegame.c +170, ent.c +157, r_bsp.c +141, weapons.c +137, d_faces.c +120, dl.c +120, item.c +120, triggers.c +119, mdl.c +76, console.c +62, mod.c +60, doors.c +49, keybind.c +46, mod_tex.c +43, client.c +41, sb_build.c +36, pl_trace.c +34, d_beam.c +33.

## Per class: cause family, row, proposed owner

| class | where | what | cause family | row / owner |
|---|---|---|---|---|
| mov | m16: +5808 (-Os), +6922 (-O2) | in the five largest modules: register-to-register copies 1133 against the pin's 353 (3.2x), immediates loaded into registers 640 against 390 (the pin stores immediates to memory 346 against our 250), stores of registers 1235 against 1493; m32 like-for-like: +27 (-O2), -32 (-Os) | m16: register classes (BX/SI/DI/AX) force copies the pin's 32-bit-everywhere code16 does not need; coalescing and copy propagation. m32: none | trend only; splitkit (allocation, the Register Coalescer and Machine Copy Propagation rows of the slot table); the immediate store: w-2 (peephole) |
| alu | m16 -2518 (-Os); m32 +143 (-O2), -238 (-Os) | ours fewer at -Os | the pin spends more ALU instructions where ours folds a memory operand | none: not a gap |
| lea | m32 +471 (-O2), -63 (-Os) | `recpow` 163 against 3, `x_switch` 78 against 6, `x_fir` 54 against 0: multiplications by constants are `lea` chains (`fee9a73a2`, a multiply chain step is a lea where the target has the address form) where the pin uses `imul`; lea is 1 clock on an i486, imul 13-42 | a decision (clocks over instructions) | none; the clocks are in the rows |
| push/pop/frame | m32 +198 (-O2), +207 (-Os); m16 +129 / -350 | `quicksort` 38 against 12: callee-saved saves and stack-argument pushes | the register convention for private functions (the pin's fastcc is not reproduced) and D10 copies | w-6 (AM-CC2..5, D10) |
| x87 | m16 -2784 (-Os), -3382 (-O2); m32 +2 / -73 | ours fewer | memory-operand folds in place of separate loads | none: not a gap |
| setcc/cmov | m16 +314 / +388 | boolean values built as setcc chains | AM-1..3 | w-2 (setcc plan) |
| call/ret | m16 +222 / +477 | not examined | | open |
| string | ours 43 / 74, the pin 0 / 8 | `rep movs` inline for copies the pin lowers differently | I1, AM-I1 | w-4 (after the tuning phase) |
| other | m16 +615 / +721 | `les`, `lfs`, `lgs`: far-pointer loads of real mode | the pin's code16 has none in this IR | N/A |
| branch | m16 -277 (-Os), -77; m32 +25 / -16 | equal | | none |

What is left after the classes that are not gaps: the m16 `mov` excess (copies), `setcc`, and the calls at m16; at m32 only the register convention (`push/pop`). So the gap against the pin is mostly a property of the real-mode target and of two design choices, not a missing llc pass.
