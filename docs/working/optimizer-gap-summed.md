# Optimizer gap, pin summed over every call site

Inclusive Ir, d_faces `-m32 -march=i486 -O2`. Ours: dist build with line tables at 59c834686 plus #5355 (`callgrind-check` clean), pass `run` functions. Pin: clang 24, same recipe. Both sides are a function's inclusive cost summed over every caller (`calls=` lines of the raw profile), so passes run at several pipeline sites are already totals. The sum is over the `cfn=`/`calls=` records of the raw callgrind file.

Calls are not comparable: ours counts function runs, the pin's loop passes count loops.

| Pass | ours M | pin M | gap M | note |
|---|--:|--:|--:|---|
| SimplifyCfg | 21.25 | 10.58 | +10.7 | w4 |
| CVP | 20.22 | 9.54 | +10.7 | w4, LVI |
| EarlyCSE | 14.46 | 6.95 | +7.5 | fork parked |
| JumpThreading | 12.45 | 7.33 | +5.1 | w4 |
| Combine | 57.70 | 52.61 | +5.1 | pin InstCombine 52.03 + AggressiveInstCombine 0.58 |
| IndVars | 27.42 | 23.34 | +4.1 | ours `IndVarsOnCreated` 4.56 not included |
| LoopDeletion | 8.37 | 4.34 | +4.0 | two sites ours (6 + 11 runs), 65 loop runs in the pin; domtree rebuilt from scratch 0.54 |
| LoopLoadElim | 5.85 | 2.35 | +3.5 | SCEV |
| DSE | 6.19 | 2.84 | +3.4 | diffuse |
| Licm | 17.31 | 14.02 | +3.3 | w3 |
| Sccp | 6.88 | 4.23 | +2.7 | pin IPSCCP 4.15 is a separate module pass |
| InstSimplify | 3.46 | 1.09 | +2.4 | |
| LoopInstSimplify | 4.58 | 2.81 | +1.8 | |
| ConstraintElim | 5.56 | 4.24 | +1.3 | |
| LoopIdiom | 4.39 | 4.08 | +0.3 | |
| Sroa | 9.18 | 9.81 | -0.6 | |
| GVN | 45.67 | 49.19 | -3.5 | |
| LoopRotate | 2.00 | 7.83 | -5.8 | pin's includes work ours does elsewhere |
| LoopFullUnroll | 8.92 | 9.16 | -0.2 | pin LoopFullUnroll 3.60 + LoopUnroll 5.56 |

Not comparable: LCSSA (4.91 vs 0.75, the pin runs it inside other passes), LSR (pin runs it in llc).

The earlier table's pin column already matched these sums; the 4.3 M LoopDeletion figure is the all-sites total (65 calls), so +4.0 survives. Largest unowned gaps: EarlyCSE (parked), Combine, IndVars, LoopDeletion.
