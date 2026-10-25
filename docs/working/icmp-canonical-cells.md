# canonicalizeICmpPredicate: cells above the base

`icmp ne`/`ule`/`sle`/`uge`/`sge` become the inverse compare with the branch arms or select values swapped, as the pin's InstCombine makes them. Base 43f742d6b, release, d_faces -O2 Ir 1,014,910,241 -> 1,009,512,504 (-0.53%). Objects move in 18 to 36 sources per level.

| cell | bytes | cause |
|---|---|---|
| m32 -O3 x_crc32 | 283 -> 443 | open, not the fold: the inner `(c & 1) ? A : B` diamond is `test al, 1; je` before and `test eax, 1; jne` after, one jmp more per unrolled step. The byte `test` is not narrowed for the swapped compare. The pin rewrites this loop as a CRC table (LoopIdiom), so it has no like-for-like |
| m32 -O2 quicksort | 332 -> 348, 481 -> 497 | the epilogue block stands after the loop (`jl L0_26; epilogue` -> `jge L0_7; ...; epilogue`) |
| m32 -O3 div_constants_ulong, div_constants_short, spill_store_order, grep | 7238->7386, 9330->9456, 3942->4038, 713->745 | not diffed |
| m16 -Os div_constants_long, div_constants_short, grep | 5195->5317, 4015->4063, 508->551 | not diffed |
| m16 -O3 div_constants_short, div_constants_ulong | 7710->7845, 8248->8301 | not diffed |

CODE: m16 -O2 +56, -O3 +201, -Os +198; m32 -O1 -30, -O2 +138, -O3 +477, -Os +176.
