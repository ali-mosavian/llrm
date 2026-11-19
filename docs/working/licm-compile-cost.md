# LICM and loop-pass compile cost (w-3, plan note 2026-10-22)

target: d_faces -O2, `llrm-c -m32 -mabi=sysv -march=i486`, callgrind Ir, ledger `ledger-c4fade734-O2.txt` (loop bucket: ours 192.7 M, pin 86.8 M). Own callgrind: dist d7d4e23a4 (parity plus the D12 revert), `w3-cg/cg.out`, total 1435 M. The pin side is the ledger's.

## Where LICM's 57-60 M goes

`licm::run` is 60.2 M inclusive; the pin's `LICMPass::run` is 14.0 M.

| callee of `licm::run` | Ir | note |
|---|---|---|
| `Analyses::get::<MemorySsa>` (13 calls) | 43.3 M (72%) | an analysis, shared: 41 builds in all, 59.9 M |
| `sink_and_hoist` (59 loops) | 13.1 M | the pass proper |
| `promotion_round` | 1.3 M | |
| the rest | < 1 M | |

The pass proper (14.5 M with promotion) is at parity with the pin's 14.0 M. The excess is the MemorySSA it asks for.

## Where the MemorySSA cost goes

`MemorySsa::run` (41 builds, 59.9 M) is `PlainAccesses` (52.1 M): `PlainAccesses::update` 43.5 M in 39 calls and `::run` 15.0 M in 71 calls. `update` recomputes two analyses from scratch each time a pass has changed the function: `Annotated` 24.7 M and `Writes` 17.9 M. The pin keeps MemorySSA up to date with `MemorySSAUpdater` (insertions, moves, removals), not by rebuilding. So the lever is not in LICM.

## Levers (pin-faithful first), owners, dates

1. `Annotated` and `Writes` kept across a pass that preserves them or updated for the instructions it moved (LICM moves, `MemorySSAUpdater::moveToPlace`): the 42.6 M above, less what a real change costs. Owner w-5 (analysis manager, SE persistence step 1 is the same machinery), plan 10-27, first Ir drop as a PR by 11-02, measured with d_faces -O2 callgrind Ir and objects identical.
2. loopdeletion: 34 fresh `ScalarEvolution::new` for 17 loops, 7.57 M of its 11.87 M (pin 7.3 M); one SE per loop saves at most 3.8 M (under the 4 M bar), the persistent SE removes the rest: w-5's list.
3. LICM itself: none (parity). Not a row.

Evidence: `w3-cg/{cg.out,ann_all.txt,tree2.txt}` in w-3's scratch; reproduce with `valgrind --tool=callgrind` on the command line above and `callgrind_annotate --tree=calling --inclusive=yes`.

## Re-take 2026-11-11 (softfloat): AM-MS-1..3 closed

d_faces -O2 callgrind, `llrm-llc` of dist xid/dist-rc1 (release/next d6bfc5f306; llrm-cc ccc93b452ff2, llc a65cb0f156a9), total 1298.7 M Ir (llc alone). Source `qbq-llrm/src/render/d_faces.c` built under llrm-cc with `-Dpascal= -Dfar= -Dnear= -Dhuge=` (the Watcom keywords; not the same input as the 1435 M run, so shares, not totals, are comparable).
`PlainAccesses`, `Annotated` and `Writes` do not appear in the profile: the MemorySSA is built by `mssa` (memoryssa-port; `manager_tests` asserts the build asks no `PlainAccesses`). `MemorySsa::run` is 4.68 M inclusive (0.36%; was 59.9 M in 41 builds), `licm::run` 28.9 M (was 60.2 M). Positive control: the same profile holds `MemorySsaUpdater::move_to_place` and `Analyses::get::<MemorySsa>`, so the reader sees MemorySSA code. Raw: `~/scratch/sf-fix32/cg/cg.1131350.out`.
