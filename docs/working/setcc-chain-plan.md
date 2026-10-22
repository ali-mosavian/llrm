# setcc chain vs branch (x_mergesort, stft bq32, nhstutil leaves)

TL;DR: our IR equals the pin's; both lower `x + zext(icmp)` to `cmp; setcc; movzx; add` on i486, where main (gcc) keeps the branch. A numbered DEVIATE, priced from `timings.times`, lowers it as a branch where the target has no cmov.

target: i486-unknown-linux-gnu, cpu=i486, no sse (tools/pairs/pin.py), repro `~/scratch/cause/repro/merge.c` with the arrays made external (static ones are dead and the function vanishes).

## Cause

The merge loop `b[k++] = a[i] <= a[j] ? a[i++] : a[j++]`. Our first `combine@677` turns the diamond that increments `i` or `j` into `i + zext(c)` and `j + zext(!c)` plus `smin`/`select` (stage diff `05-pipeline.ll` against `walk/01/001-r01-combine@677.ll`). The pin's `opt default<O2>` makes the same IR for this loop (`zext i1`, `add nsw`, `llvm.smin`), so the IR is pin-faithful.

Lowering, per iteration of the merge loop:

| | instructions | note |
|---|---|---|
| ours | 17 | `cmp; setle; movzx; add; test; je` and a `xor dl,1; movzx; add` |
| pin llc | 21 | the same chain, the two flags spilled to the stack (`setg 2(%esp)`) |
| main (gcc) | 9 | `cmp; jle`, `inc` in each arm (count from the coordinator's row, not re-measured here) |

## Price (`timings.times`, i486 / P5)

`setcc` 4 / 1, `movzx` 3 / 3, `alu_rr` 1 / 1, `jcc` taken 3 / 1, `jcc_not_taken` 1 / 1.

- chain for one index: setcc + movzx + add = 8 (i486), 5 (P5); the second index adds the `xor`: 5 / 5 more.
- branch: `jcc` 1..3 + `inc` 1 = 2..4 (i486), 2 (P5) for both indices together.

The branch wins on both CPUs for this shape.

## Plan

1. DEVIATE (next free number): on a target without cmov, `add x, zext(c)` and `zext(c)` feeding only an add are expanded to `select c, x+1, x` before `selects::lowered`, when `setcc + movzx + add` costs more than `jcc + alu_rr` from the cost table. The term is read from the target description, not from this loop's shape. Exit: the pin's lowering equals or beats main on the rows.
2. First PR 2026-11-02: the expansion and a fail-first test (this repro under `LLRM_DEBUG`), rows pair on x_mergesort, qsort, bsearch, stft bq32, nhstutil; `qcport.py` -Os (bytes decide there: whether the rule applies at -Os is read from those bytes, not assumed).
3. Priced against #3544's numbers before the PR.

Not checked yet: the stft bq32 and nhstutil leaves; they are named by the row and share the cause only if their stage diff shows the same `zext` chain.
