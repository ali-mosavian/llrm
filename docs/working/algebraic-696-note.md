# algebraic@696 deleted: the pair, by bucket (w-6, 2026-10-24; branch note, not a PR)

d_faces -O2 m32, the ledger command, dists of ff320e4511 (parity c3a880d9e) and da80da0ad9 (the deletion), qbq-llrm, ours M:

| bucket | base | new | delta |
|---|--:|--:|--:|
| SimplifyCFG, JumpThreading, CVP | 75.8 | 82.7 | +6.9 |
| InstCombine | 79.2 | 79.0 | -0.2 |
| loop passes | 139.0 | 133.2 | -5.8 |
| scalar | 111.6 | 110.7 | -0.9 |
| llrm-only | 11.4 | 1.2 | -10.2 |
| module/IPO | 22.8 | 22.6 | -0.2 |
| isel | 38.4 | 38.5 | +0.1 |
| regalloc | 145.0 | 136.0 | -9.0 |
| machine clean-up | 169.2 | 152.7 | -16.5 |
| emit, front end | 64.5 | 64.6 | +0.1 |
| residual | 65.5 | 63.3 | -2.2 |
| total | 922.2 | 884.5 | -37.7 |

QCport d_faces: llrm-only -7.9, regalloc -6.9, loops -6.2, clean-up -3.2, SimplifyCFG +1.1, InstCombine +0.9; total 703.1 -> 682.3.

Not an accounting artefact: the d_faces object shrinks (8114 -> 7857 B, 1859 -> 1792 instructions of assembly), so the backend buckets (regalloc -9.0, clean-up -16.5) do less because they get less code; SimplifyCFG grows by 6.9 M for what `algebraic` no longer folded before it. Whether the 27 M outside the llrm-only bucket survives is open: the shrink may be `algebraic`'s own non-pin rewrites costing code on d_faces, and the combine translations still owed (strlen idiom shape, zext/sext icmp folds, x_mergesort) will add some of the work back. Re-measure after each (dated 11-02).

Costs of the deletion (71-program pair, 50 of 568 objects change): x_strlen +25.8% clocks (the `slen` loop is no longer a `strlen` call), x_mergesort +4.5% (+14% -Os), nbody_fixed code +3%, QCport 65 modules +0.63% -O1, +0.79% -O2, +0.97% -O3, +0.20% -Os; unit test an_extension_keeps_whether_a_value_is_zero fails (combine lacks `icmp ne (zext x), 0` -> the narrow icmp, as the pin does).

## Final pair (2026-10-25, with the icmp-of-extension fold and the power-of-two division)
Dists f204349dd -> 439af0656, 71 programs / 568 objects (69 change), ledger command: d_faces qbq-llrm 886.8 -> 862.4 M (-24.4), QCport 685.7 -> 670.2 (-15.5); llrm-only 11.3 -> 1.2 M.
Clocks: x_strlen back to base (the fold), nbody_fixed -3.0% (-O1..-O3, code -3%; the power-of-two division in the backend), x_switch_dense -3.3%, shellsort -12.7% (-O1/-O2) but +13.3% -O3, nbody -O3 -0.7%, x_mergesort +0.5% (+14.1% -Os), quicksort +1.0%, bintree +0.4%; -Os nbody_fixed +47.7% clocks, code -2.7% (the pin keeps `idiv` at minsize, `isIntDivCheap`; the pin-faithful result). QCport 65 modules -0.08% -O1, -0.09% -O2, -0.20% -O3, -0.14% -Os; no new tests/run failure.
