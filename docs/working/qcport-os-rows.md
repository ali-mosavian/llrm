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
