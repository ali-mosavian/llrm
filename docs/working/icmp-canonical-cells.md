# canonicalizeICmpPredicate and canonicalizeICmpBool: cells above the base

`icmp ne`/`ule`/`sle`/`uge`/`sge` become the inverse compare with the branch arms or select values swapped, and a compare of two `i1` becomes bitwise logic, as the pin's InstCombine makes them. Base bab8b6320 (with #4583, the byte-test narrowing under the DAG), release, d_faces -O2 Ir 988,146,749 -> 982,565,242 (-0.56%). Objects move in 18 to 43 sources per level.

| cell | bytes | cause |
|---|---|---|
| m32 -O3 x_crc32, old isel | 283 -> 443 | the old selector keeps `test eax, 1` (5 bytes, imm32) for the swapped `eq`, where it made `test al, 1` for `ne`; the unrolled diamond gets one more jmp a step. Under `--isel=dag` with #4583: 302 -> 350 at -O3 and 286 -> 270 at -O2, `test dl, 1`; the dag -O3 rest is the selects: the pin lowers each CRC step without a branch and our DAG makes a diamond, which the placement then duplicates with this polarity (w10, #4610). The pin rewrites the loop as a CRC table (LoopIdiom), so it has no like-for-like |
| m32 -O2 x_crc32, old isel | 267 -> 283 | as above |
| m32 -O2 quicksort | 332 -> 348, 481 -> 497 | the epilogue block stands after the loop (`jl L0_26; epilogue` -> `jge L0_7; ...; epilogue`) |
| m32 -O3 spill_store_order, grep, nbody_fixed, recpow | 3942->4038, 729->761, 1521->1537, 371->387 | not diffed |
| m32 -Os x_hash, grep, compare_to_int64 | 175->188, 551->567, 214->219 | not diffed |
| m16 -Os div_constants_long, div_constants_short, grep | 5195->5317, 4015->4063, 508->551 | not diffed |

CODE: m16 -O1 +82, -O2 +38, -O3 +43, -Os +198; m32 -O1 -14, -O2 +90, -O3 +133, -Os +181.

The nib zip loop (`test_a_zip_loop_tests_each_slices_length_as_llvm_does`) failed with the predicate fold alone: `icmp ne i1 %c, false` became `icmp eq i1 %c, false` with swapped arms and stayed, ConstraintElimination lost its guard and three bounds checks stayed. `canonicalizeICmpBool` (the pin's next step: `not %c`, which a branch takes as swapped arms) restores the pin's two-test loop.
