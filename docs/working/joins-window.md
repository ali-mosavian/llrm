# joins / fjoins: steps over the slope limit at N=128 and N=256, 2026-10-28

`tools/test_budgets.py` reads `joins` and `fjoins` at N=64. These are the steps
`test_no_step_grows_faster_than_the_program_does` flags when the axes are read at 128 and 256
(parity d5551fd0ed, limit 2.3 per doubling).
Kept here so the readings are not lost; no budget rows are registered for them.
The N=64 window rows in `tools/budgets.toml` (`[[window]]`) are a different thing: see their `why`.

## joins, N=128

| step | Minstr N -> 2N | factor |
|---|---|---|
| mir gvn@787 | 441.8 -> 1119.1 | x2.53 |
| mir dse@820 | 234.2 -> 576.2 | x2.46 |
| mir earlycse@649 | 216.4 -> 572.7 | x2.65 |
| mir ipo-early | 61.7 -> 163.0 | x2.64 |
| object write | 44.2 -> 125.2 | x3.11 |
| mir interprocedural | 27.9 -> 105.2 | x3.77 |
| regalloc apply | 12.5 -> 33.2 | x2.65 |
| lir machinesched | 7.1 -> 32.4 | x4.53 |
| lir phielim | 9.7 -> 23.3 | x2.41 |
| regalloc prepare | 8.7 -> 22.0 | x2.53 |
| mir merge@818 | 8.7 -> 20.9 | x2.42 |
| lir optimizephis | 2.9 -> 8.2 | x2.83 |
| lir earlytaildup | 1.9 -> 5.2 | x2.82 |

## fjoins, N=128

| step | Minstr N -> 2N | factor |
|---|---|---|
| dag isel | 569.5 -> 1276.4 | x2.47 |
| mir gvn@787 | 416.5 -> 1074.7 | x2.58 |
| mir dse@820 | 234.2 -> 576.1 | x2.46 |
| mir earlycse@649 | 210.2 -> 565.5 | x2.69 |
| mir ipo-early | 51.0 -> 140.6 | x2.76 |
| object write | 51.9 -> 126.8 | x2.44 |
| mir interprocedural | 26.5 -> 102.5 | x3.87 |
| mir combine@677 | 13.2 -> 30.7 | x2.33 |
| mir combine@1403 | 13.1 -> 30.6 | x2.34 |
| mir combine@801 | 13.0 -> 30.6 | x2.35 |
| regalloc apply | 12.6 -> 29.0 | x2.31 |
| lir machinesched | 6.9 -> 26.3 | x3.80 |
| lir phielim | 9.8 -> 23.3 | x2.38 |
| regalloc prepare | 8.4 -> 19.6 | x2.34 |
| lir optimizephis | 3.0 -> 8.2 | x2.74 |
| lir earlytaildup | 1.9 -> 5.2 | x2.82 |

## joins, N=256

| step | Minstr N -> 2N | factor |
|---|---|---|
| mir ipo-early | 163.0 -> 488.0 | x2.99 |
| object write | 125.2 -> 389.7 | x3.11 |
| mir interprocedural | 105.2 -> 374.3 | x3.56 |
| lir phielim | 23.3 -> 65.1 | x2.79 |
| mir merge@818 | 20.9 -> 54.2 | x2.59 |
| lir optimizephis | 8.2 -> 27.3 | x3.34 |
| lir earlytaildup | 5.2 -> 16.0 | x3.05 |

## fjoins, N=256

| step | Minstr N -> 2N | factor |
|---|---|---|
| dag isel | 1276.4 -> 3152.1 | x2.47 |
| mir ipo-early | 140.6 -> 441.6 | x3.14 |
| object write | 126.8 -> 387.0 | x3.05 |
| mir interprocedural | 102.5 -> 369.7 | x3.61 |
| mir combine@1242 | 34.2 -> 86.9 | x2.54 |
| mir combine@677 | 30.7 -> 81.2 | x2.64 |
| mir combine@1403 | 30.6 -> 81.2 | x2.65 |
| lir phielim | 23.3 -> 65.1 | x2.79 |
| lir optimizephis | 8.2 -> 27.3 | x3.33 |
| lir earlytaildup | 5.2 -> 16.0 | x3.04 |
