# QCport -Os rows above base after #3369

`phielimination::lowered` lowers every phi with the translated PHIElimination, where the 8-byte and x87-touched ones went through the old priced stage (#3369, `bd499e91ca`). It saves 25.7 M instructions on d_faces -O2 (-1.5%), and costs bytes at -Os, where bytes bind. Oversight ruled it keep-and-fix-forward on 2026-10-21 (revert backstop 2026-10-28). Owner w-1, exit 2026-10-28: the pin's mechanism for the wide values (copy placement, or the isel split into two 4-byte values), then these rows are 0.

Measured: `tools/rows/qcport.py`'s way (`llrm-c -Os -march=i386`, QCport 65 modules, emitted LEDATA bytes), base = the parent of #3369, then #3369. 48 modules are equal, 5 are smaller (game/mdl_ai -27, game/triggers -8, ui/screen -2, game/weapons -1, model/mdl -1); total 269,867 -> 269,986 B (+119 B, +0.04%). Cause seen in game/gib: the phis left to the old stage are in `ssaspill::floating` (touched by an x87 instruction, or in a web with a wide value), not wide themselves (width 2 or 10); the translated lowering allocates their far-pointer and segment values differently (`les bx` for `mov es`, extra spill pairs).

| row | module | base B | #3369 B | delta |
|---|---|---|---|---|
| QO-1 | host/demo | 5954 | 5995 | +41 |
| QO-2 | game/gib | 1962 | 1992 | +30 |
| QO-3 | game/doors | 2736 | 2752 | +16 |
| QO-4 | game/item | 5135 | 5149 | +14 |
| QO-5 | render/dl | 4219 | 4233 | +14 |
| QO-6 | render/d_alias | 12874 | 12883 | +9 |
| QO-7 | game/ent | 8751 | 8759 | +8 |
| QO-8 | ui/console | 4071 | 4079 | +8 |
| QO-9 | sound/snd_mix | 3909 | 3915 | +6 |
| QO-10 | host/savegame | 7953 | 7958 | +5 |
| QO-11 | render/r_bsp | 4959 | 4963 | +4 |
| QO-12 | render/d_faces | 4901 | 4904 | +3 |

## Re-land on parity 9e7f33edd3 (2026-10-21)

Base = parity without the change, then with it (`llrm-c -m16 -Os -march=i386`, same 65 modules, emitted bytes). With the DS-selector fix (#3434) in, 58 are equal, 5 smaller (game/mdl_ai -2, game/weapons -7, model/mdl -1, ui/console -4, ui/screen -2) and two bigger: QO-1 and QO-6. The other ten rows are at base. Total 223,756 -> 223,879 B (+123, 0.055%).

| row | module | main B | base B | with change B | owner, date |
|---|---|---|---|---|---|
| QO-1 | host/demo | 5404 | 5609 | 5742 (+133) | splitkit (base gap to main), evidence w-1 |
| QO-6 | render/d_alias | 11537 | 12393 | 12399 (+6) | splitkit (base gap to main), evidence w-1 |

Both are above main already on parity (cause not examined); the change adds to it.

QO-1 is one function, `_demo_snapshot_apply@3` (+45 lines): the pointer argument is kept in `di` on base and reloaded from its cell before each use with the change. `LLRM_GREEDY_TRACE`: values 220 and spilled 25 -> 26, but reloads 14 -> 47, stores 9 -> 21, new ranges 129 -> 171. The allocator decides differently, so the cause is splitkit's allocation row, not the phi lowering. Traced with a temporary `tryEvict`/spill print on both builds: the first events are the same (`v2`, `v98`, `v86`, `v242` evict in turn), then one range (4 segments, weight 0.15935233; `v238` on base, `v361` with the change) takes `ebx` from `v105` on base and `edi` from `v86` with the change; the rest of the evictions and the whole-range spills of the split pieces differ from there. The extra phi-copy ranges change which register is the cheapest to evict. Exit: the module at or below base, by the eviction choice and not by the phi lowering.

Status: both rows are blocked by the base gap, not by the change: host/demo main 5404 vs parity 5609 (+205) and render/d_alias main 11537 vs parity 12393 (+856) exist before it. Once splitkit's allocation rows close that gap, the +133 and +6 land under main. The pin comparison found `canReassign` untranslated (separate PR, -62 B in all, host/demo -3, d_alias 0) and no deviation in the eviction cost, tie-break, hints or weights; the first divergence is the interference pattern of one range, `ebx` on base and `edi` with the change.
