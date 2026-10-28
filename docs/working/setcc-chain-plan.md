# setcc chain vs branch (x_mergesort, stft bq32, nhstutil leaves)

TL;DR: our IR equals the pin's; both lower `x + zext(icmp)` to `cmp; setcc; movzx; add` on i486, where main (gcc) keeps the branch. A numbered DEVIATE, priced from `timings.times`, lowers it as a branch where the target has no cmov.

target: i486-unknown-linux-gnu, cpu=i486, no sse (tools/pairs/pin.py), repro `merge.c` with the arrays made external (static ones are dead and the function vanishes).

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

## The nhstutil leaf is another chain

`static int is_tmp(const void *p){return p>=temps && p<temps+240;}` (repro `nh2.c`; w-6's dumps `nh-main.s` against `nh-fix.s`): both the pin's llc and ours emit `cmp; setae cl; cmp; setb al; and cl,al; movzx` (no `zext`-add), where main branches (`cmp; jae; ...; xor eax,eax / mov al,1`). The IR is an `and i1` of two compares used as a value: a different pattern with the same cost shape. With a variable bound our own output already branches (6 instructions, `jb` over a `setb`): the chain appears when the bounds are link-time constants.

Price (i486 / P5): two `setcc` 4+4 / 1+1, `and` 1, `movzx` 3 / 3, two `cmp` 2 = 14 / 8; the branched form `cmp; jb; cmp; setb` is 1 + 3 + 1 + 4 = 9 on a taken branch, 6 not taken (i486), 5 / 3 on P5.

## Plan

1. One cost-based rule over setcc chains, one DEVIATE (next free number), not a rule per shape: on a target without cmov, a chain of `setcc`s and their combiners (`and`/`or` of compares used as a value, `add x, zext(c)`, a `select` of those) is expanded to `select`s before `selects::lowered` when the chain costs more than the branched form. Both prices are sums over `timings.times` entries (`setcc`, `movzx`, `alu_rr`, `jcc`, `jcc_not_taken`) read through the target description. Exit: the pin's lowering equals or beats main on the rows. If the one rule cannot hold both shapes, it becomes two numbered rows.
2. First PR 2026-10-30: the expansion and a fail-first test (this repro under `LLRM_DEBUG`), rows pair on x_mergesort, stft bq32, qsort, bsearch and every bench row that uses a short-circuit boolean (found by a grep of the sources for `&&`/`||` used as values; nothing worse than main per row); `qcport.py` -Os (bytes decide there: whether the rule applies at -Os is read from those bytes, not assumed).
3. Priced against #3544's numbers before the PR.

