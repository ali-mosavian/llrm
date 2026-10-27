# Local reassignment in the greedy allocator

`canReassign` and the `EnableLocalReassign` clause of `canEvictInterferenceBasedOnCost` are translated at d1106deb71cc: the pin turns it on by default (`RegAllocEvictionAdvisor.cpp#L184-L188`, `TargetSubtargetInfo.cpp#L59-L62`, no x86 override). Before, a cheaper local range was never taken out of a register to make room for a heavier local one.

Measured on parity `7f75d65222` and main `origin/main` (clean builds, one target dir each), `llrm-c -m16 -march=i386 -Os`, QCport's 65 modules, emitted bytes. The 15 modules that change; every other module is equal:

| module | main B | parity B | with change B |
|---|---|---|---|
| game/combat | 5639 | 5636 | 5634 (-2) |
| game/mdl_ai | 20451 | 21089 | 21081 (-8) |
| game/triggers | 4553 | 5071 | 5068 (-3) |
| game/weapons | 12793 | 12945 | 12947 (+2) |
| host/bench | 2261 | 2305 | 2278 (-27) |
| host/demo | 5404 | 5609 | 5606 (-3) |
| host/host | 23316 | 24514 | 24511 (-3) |
| model/mdl | fails | 6281 | 6277 (-4) |
| render/r_bsp | fails | 4456 | 4453 (-3) |
| render/sb_build | 1158 | 1136 | 1123 (-13) |
| render/sc | fails | 8971 | 8968 (-3) |
| ui/console | fails | 3560 | 3559 (-1) |
| ui/menu | 4892 | 4970 | 4974 (+4) |
| ui/sbar | fails | 3887 | 3885 (-2) |
| ui/screen | 6635 | 6993 | 6997 (+4) |

Main fails on those modules only with the flags this table was made with (`-m16 -march=i386` and a keyword wrapper: "value#1 has no word destination to promote"); with tools/rows/qcport.py's compile (no flags) it builds all 65 (`qcport-vs-main.md`, 2026-10-21), so the `fails` cells are not a property of main. Over the 54 it builds: main 172,605 B, parity 178,157, with the change 178,108. All 65: parity 223,891, with the change 223,829 (-62 B).

Three modules are bigger (+2, +4, +4): game/weapons, ui/menu, ui/screen; ui/menu and ui/screen are already above main on parity. The change moves host/demo and render/d_alias by -3 and 0 B: it does not close the base gap to main.

## Rows above parity (oversight's sweep of d3f08d2438: 6062 compiles, 143 objects change by -25..+5 B; QCport -Os 329,187 -> 329,098 B)

| row | module | change | owner, date |
|---|---|---|---|
| LR-1 | render/dl | +3 B (0.06%, over the 0.05% bar) | w-1 (greedy), 2026-10-31 |
| LR-2 | ui/screen | +3 B | w-1 (greedy), 2026-10-31 |
| LR-3 | game/weapons | +2 B | w-1 (greedy), 2026-10-31 |

Not held conditions (oversight). Exit: the allocation order of the evicted local range, the same family as QO-1 (`qcport-os-rows.md`).

## Oversight's sweep of d3f08d2438: QCport -Os object bytes, main / parent / head

Source: `qcport-os-3446.tsv` (OMF object-file bytes of `llrm-c -Os`, 65 modules; llrm-c md5 main 56f120bb59fa, parent 804e487fd4c9, head 0eff6c50903f). Modules whose bytes differ between parent and head; the other 50 are equal.

| module | main | parent | head | head - parent |
|---|--:|--:|--:|--:|
| bench | 3068 | 3120 | 3092 | -28 |
| combat | 7499 | 7456 | 7446 | -10 |
| demo | 7005 | 7427 | 7424 | -3 |
| dl | 4625 | 4878 | 4881 | +3 |
| ent | 10251 | 10492 | 10490 | -2 |
| host | 33681 | 34934 | 34933 | -1 |
| loadscr | 2171 | 2219 | 2217 | -2 |
| mdl | 8204 | 8510 | 8508 | -2 |
| mdl_ai | 25850 | 26522 | 26514 | -8 |
| mod_tex | 2299 | 2445 | 2412 | -33 |
| sbar | 4656 | 5008 | 5006 | -2 |
| sc | 11678 | 12272 | 12268 | -4 |
| screen | 8899 | 9294 | 9297 | +3 |
| triggers | 6604 | 7137 | 7135 | -2 |
| weapons | 15478 | 15442 | 15444 | +2 |
| TOTAL (65) | 321280 | 329187 | 329098 | -89 |
