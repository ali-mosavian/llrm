# Rows of `foldICmpDivConstant` and `foldCmpLoadFromIndexedGlobal` (#8438)

Both folds are the pin's. Measured base `5087459800` against head `b219a19dd6`, 227 programs, O2/Os, ten targets. Bytes are the assembled object's code: z80 `sdasz80` `_CODE`, 6502 `ca65` CODE, ELF `.text`, m16 OMF code segments, wasm Code section.

Cause, shared mid-end: `icmp slt (sdiv X, 1024), 0` becomes `icmp slt X, -1023` (and, over an add, `icmp slt %s1, -1535`); a table compare over a constant table becomes the pin's magic bit-vector test (`shl 1, i; and 9361`). The pin emits the same IR; the targets pay for a wide compare constant and a longer live range.

Exit 2026-11-20 for every row. Fix: a Tti-priced hook (default = the pin) or a backend undo shared by targets.

| row | owner | cells (bytes, base -> head) |
|---|---|---|
| WIDE-CMP-Z80 | llrm-target-z80 | jpeg_fixed O2 7669 -> 7844 (+175, +83 instrs, one function); spill_store_order O2 +6 |
| WIDE-CMP-6502 | llrm-target-6502 | jpeg_fixed O2 13532 -> 13632 (+100), Os +87 |
| DIGITS-6502 | llrm-6502-b (proposed by 6502-c, unconfirmed) | tests/bench/kernels.c K_digits Os 104 -> 129 B (3403 cy at 129 B); the fold compares the dividend (`icmp ugt %5, 99`), so n and the quotient stay live across the udiv call: 3 callee-saved cells pushed and popped. llvm-mos has the same IR (314 B). Reported by 6502-c, not re-measured here |
| WIDE-CMP-M68K | llrm-target-m68k | 68000 jpeg O2 +48, Os +34, idioms O2 +72, Os +64; 68020 jpeg O2 +54, Os +36, idioms O2 +32, Os +62; pattern_fill +2 |
| MID-END-FOLD-COST-M32 | llrm-target-x86-m32 | jpeg_fixed Os +42; memfuncs_runtime O2 +16, Os +7; spill_store_order O2 +16 |
| MID-END-FOLD-COST-M64 | llrm-target-x86-m64 | memfuncs_runtime O2 +127, Os +20; idioms O2 +32, Os +3; jpeg_fixed Os +23 |
| MID-END-FOLD-COST-ARM64 | w15-arm64 | memfuncs_runtime O2 +20, Os +8 |
| MID-END-FOLD-COST-M16 | llrm-target-x86-m16 | dos/wat jpeg_fixed O2 +34; idiom_edges O2 +17/+23; idioms O2 +17/+20, Os +10/+14; backward_move O2 +9/+2, Os +9/+7 |
| MID-END-FOLD-COST-WASM | llrm-wasm-b | jpeg_fixed O2 +20, Os +1; memfuncs_runtime O2 +31, Os +20; pattern_fill O2 +18; idioms Os +12 |

Better in the same run: idioms O2 wasm -7, arm64 -44/-32, z80 -31/-62, 6502 -28/-13; jpeg_fixed Os m16 -165/-163; spill_store_order 6502 -43; pattern_fill m32 -16.

Compile time (callgrind, x86_64 O2): jpeg_fixed -0.31%, select_of_constants +0.03%, memfuncs_runtime +5.2%.

Executed, head matches the `.out` (m68k, 6502, arm64 and m32/m64 jpeg_fixed also on base): wasm, m32, m64, m68k, z80, 6502, arm64, m16 (idioms, pattern_fill, memfuncs_runtime, memcpy_runtime, os_execute, idiom_edges, backward_move where the target builds them); bench/jpeg_fixed on m32, m64, m68k, arm64, m16. Not run: bench/jpeg_fixed on wasm (no file input), 6502 and z80 (no file layer in the harness). spill_store_order is WRONG on z80 and 6502 on both sides (16-bit int hash).
