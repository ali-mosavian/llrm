# What the rotate slot (rotate@728, header copy) changes, llvm_order, branch alim/feat/llvm-order-rotate

Corpus: 71 programs at -O2 and -O3 (-m32 -mabi=sysv -march=i486) + d_faces -O1/-O2/-Os (145 compiles, none failed).
Counts: `LLRM_DEBUG=runs` `step … changed` lines per pass base name, slot ON vs OFF (`LLRM_EXP_NOROTATE`, measurement only).
Only passes whose count differs are listed; the rest are equal.

| pass | guard (header-shaped loop, no "rotated" test: the shape is the guard) | changed ON | OFF |
|---|---|---|---|
| unroll | unroll.rs L5-9: header holds the exit test; body a chain of single-successor blocks header→latch | 0 diff (equal) | |
| peel | peel.rs L315: header phis with the two arms | 13 | 23 |
| indvars | indvars.rs: counted exit test in the header | 26 | 34 |
| fill | fill.rs L43-51: loop kept as one trip behind the header test | 16 | 18 |
| trivialunswitch | trivialunswitch.rs L1-10: branch in the HEADER on an invariant cond | 0 | 2 |
| loopmotion | loopmotion.rs L19: `source` is the header | 0 diff (equal) | |
| lsr | not in the function pipeline | - | - |
| floatloop, lcssa-guard | none | - | - |
| (made by the slot) rotate | | 156 | 0 |
| loopsimplify / lcssa re-runs | | 170 / 450 | 16 / 298 |
| fold / decide / dead | cleanup after the copy | 183 / 214 / 293 | 76 / 176 / 248 |
| hoist / gvn / algebraic | | 110 / 40 / 110 | 99 / 33 / 103 |

Total changed steps 2,590 ON, 1,935 OFF.

Unroll reason lines: loops evaluated for a full unroll 116 ON vs 83 OFF; "not innermost" 13 vs 12; max-completely-peel-times 203 vs 204. Unroll changed the same number of bodies.
Reading: peel (-10), indvars (-8), fill (-2), trivialunswitch (-2) act on fewer loops with the slot; unroll is unchanged.
