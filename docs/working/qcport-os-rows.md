# QCport -Os rows above base after #3369

`phielimination::lowered` lowers every phi with the translated PHIElimination, where the 8-byte and x87-touched ones went through the old priced stage (#3369, `bd499e91ca`). It saves 25.7 M instructions on d_faces -O2 (-1.5%), and costs bytes at -Os, where bytes bind. Oversight ruled it keep-and-fix-forward on 2026-10-21 (revert backstop 2026-10-28). Owner w-1, exit 2026-10-28: the pin's mechanism for the wide values (copy placement, or the isel split into two 4-byte values), then these rows are 0.

Measured: `tools/rows/qcport.py`'s way (`llrm-cc -Os -march=i386`, QCport 65 modules, emitted LEDATA bytes), base = the parent of #3369, then #3369. 48 modules are equal, 5 are smaller (game/mdl_ai -27, game/triggers -8, ui/screen -2, game/weapons -1, model/mdl -1); total 269,867 -> 269,986 B (+119 B, +0.04%). Cause seen in game/gib: the phis left to the old stage are in `ssaspill::floating` (touched by an x87 instruction, or in a web with a wide value), not wide themselves (width 2 or 10); the translated lowering allocates their far-pointer and segment values differently (`les bx` for `mov es`, extra spill pairs).

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

Base = parity without the change, then with it (`llrm-cc -m16 -Os -march=i386`, same 65 modules, emitted bytes). With the DS-selector fix (#3434) in, 58 are equal, 5 smaller (game/mdl_ai -2, game/weapons -7, model/mdl -1, ui/console -4, ui/screen -2) and two bigger: QO-1 and QO-6. The other ten rows are at base. Total 223,756 -> 223,879 B (+123, 0.055%).

| row | module | main B | base B | with change B | owner, date |
|---|---|---|---|---|---|
| QO-1 | host/demo | 5404 | 5609 | 5742 (+133) | splitkit (base gap to main), evidence w-1 |
| QO-6 | render/d_alias | 11537 | 12393 | 12399 (+6) | splitkit (base gap to main), evidence w-1 |

Both are above main already on parity (cause not examined); the change adds to it.

QO-1 is one function, `_demo_snapshot_apply@3` (+45 lines): the pointer argument is kept in `di` on base and reloaded from its cell before each use with the change. `LLRM_GREEDY_TRACE`: values 220 and spilled 25 -> 26, but reloads 14 -> 47, stores 9 -> 21, new ranges 129 -> 171. The allocator decides differently, so the cause is splitkit's allocation row, not the phi lowering. Traced with a temporary `tryEvict`/spill print on both builds: the first events are the same (`v2`, `v98`, `v86`, `v242` evict in turn), then one range (4 segments, weight 0.15935233; `v238` on base, `v361` with the change) takes `ebx` from `v105` on base and `edi` from `v86` with the change; the rest of the evictions and the whole-range spills of the split pieces differ from there. The extra phi-copy ranges change which register is the cheapest to evict. Exit: the module at or below base, by the eviction choice and not by the phi lowering.

Status: both rows are blocked by the base gap, not by the change: host/demo main 5404 vs parity 5609 (+205) and render/d_alias main 11537 vs parity 12393 (+856) exist before it. Once splitkit's allocation rows close that gap, the +133 and +6 land under main. The pin comparison found `canReassign` untranslated (separate PR, -62 B in all, host/demo -3, d_alias 0) and no deviation in the eviction cost, tie-break, hints or weights; the first divergence is the interference pattern of one range, `ebx` on base and `edi` with the change.

## Re-measure on parity 3b70c16822 (2026-10-22)

Base = parity 3b70c16822, head = 1988d65aff (parity merged into the change); both built here, each in a clean worktree and its own target (HEAD checked before and after: rp-d, rp-c; `llrm-cc` ab0b9f0e5ec6 and fac8d0f1e309). QCport 65 modules, no flags, emitted bytes (the `tools/rows/qcport.py` method). Main from `qcport-vs-main.md` (854bb7efc4).

-Os: 8 modules change, total 262691 -> 262647 (-44 B). Larger: host/demo 5817 -> 5852 (+35, 0.60%; main 5673), host/host +4 (0.02%). Smaller: mdl_ai -37, d_alias -27, weapons -10, mdl -4, savegame -3, screen -2.
-O2: 12 change, 320336 -> 320251 (-85 B). Larger: render/dl 5164 -> 5193 (+29, 0.56%), host/host +4, gib/weapons/console +2 each.

| row | module | level | main B | parity B | with change B | owner, date |
|---|---|---|---|---|---|---|
| QO-1 | host/demo | -Os | 5673 | 5817 | 5852 (+35, 0.60%) | exit QO-13b, splitkit, 2026-10-30 |
| QO-13 | render/dl | -O2 | see qcport-vs-main | 5164 | 5193 (+29, 0.56%) | exit QO-13b, splitkit, 2026-10-30 |

Instructions (from the -S listings, parity 3b70c16822 vs head 62c12c1305): host/demo -Os 1984 -> 1993 (+9, 0.45%, all in `_demo_snapshot_apply`), render/dl -O2 1637 -> 1651 (+14 in 5 functions). The pin's llc on the same MIR: demo 2003 instructions (at or above ours, taken at the `addresssink` stage), dl 1508 (below ours).

QO-13b, cause (2026-10-22, observed, not inferred): the phi's temporary has no register class of its own. In `Coalescer::llvm_env` a value's palette comes from how it is used, so the temporary (only copied) got the general registers {EAX..EDI} and the result, the far pointer's selector, {ES,FS,GS}: `can_share` refused every copy between them (splitkit's instrumented build; mine: a debug print of the disjoint pairs, render/dl -O2). The pin makes the temporary in the result's class (PHIElimination.cpp#L414-L415). Giving a copy-only value the palette of the value it is copied into takes render/dl -O2 from 1651 back to 1637 instructions (parity's), and -29 B (5193 -> 5164).

That fix is a partial answer and is not landed (tuning freeze; branch `mir-undef/phi-temp-class`, test `test_a_far_pointers_phi_is_not_copied_through_ax`). Priced on parity 3ce2ce70ab (pair ex7, `llrm-cc` dc805365b661 -> de89a6c3371b, QCport qcport.py bytes): -Os 262521 -> 262505, host/demo only -6 of its +35 B (about 29 B remain, not explained), snd_mix +6; -O2 319978 -> 319891, render/dl -29, r_bsp -43, but host/demo +43 B (+0.59%) and sc +28 B (+0.19%). So the rows stay open; their exit is the translated lowering and the allocator work (steps 2 and 4 of the plan), not the palette.

QO-6 (render/d_alias) is now smaller with the change (-27 at -Os, -13 at -O2). The earlier rows' numbers above were taken on older parities and are kept as history.

## QO-SIZE-1: where the -Os bytes are (w-3, 2026-10-22)

target: llrm-cc -Os (m16), 65 QCport modules, code bytes from `jwasm -Fl` listings of `-S` output, calibrated against the OMF CODE bytes. base: main 854bb7efc4, llrm-cc md5 56f120bb59fa. new: sha 97623a29d689 (stamp clean), md5 e52b0cf85b26, built with `pair_build.sh` (`cargo build --profile dist --bin llrm-cc ...`, `CARGO_INCREMENTAL=0`). Script: `bytes.py`/`bytes2.py` (w-3 scratch `w3-qo`; the method is in this paragraph). Pin counterpart: none; the pin's x86 backend has no 16-bit code generation.

Calibration: main 215,484 B in the listings against 215,034 B in the objects (+0.2%); delta 4,908 against 4,879 (no-inline arm), 2,072 against 1,986 (default arm): residual -29 and -86 B.

| class (bytes, parity - main) | no-inline arm | default arm |
|---|---|---|
| mov (integer) | +4,449 | +4,896 |
| x87 (all `f*`) | +3,800 | +3,617 |
| frame (push/pop/leave/ret) | +1,363 | -602 |
| other | +270 | -410 |
| alu/lea | -367 | -1,050 |
| branch/cmp/setcc | -4,607 | -4,379 |
| listing total | +4,908 | +2,072 |
| OMF CODE | +4,879 | +1,986 |
| residual | -29 | -86 |

Data adds +457 B in the default arm (LEDATA +2,443 in all). The inliner is a net saving: without it the CODE growth is +4,879, with it +1,986.

Modules carrying the `mov` growth (default arm, bytes): host/host +1,287, render/sc +872, game/mdl_ai +442, game/triggers +442, host/cmd +328, render/dl +268, r_bsp +248, game/item +244; down: host/savegame -633, d_faces -285. x87: render/d_alias +667, game/triggers +369, mdl_ai +358. Function lists (top 30 by `mov` and by x87 bytes, both arms) are in `w3-qo/byfn-*.json`; a function that main inlined but parity keeps shows as 0 -> N in the no-inline arm, so rank by module when inlining differs.
