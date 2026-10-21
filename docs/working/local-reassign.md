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

Main fails to compile 11 of the 65 modules ("value#1 has no word destination to promote"). Over the 54 it builds: main 172,605 B, parity 178,157, with the change 178,108. All 65: parity 223,891, with the change 223,829 (-62 B).

Three modules are bigger (+2, +4, +4): game/weapons, ui/menu, ui/screen; ui/menu and ui/screen are already above main on parity. The change moves host/demo and render/d_alias by -3 and 0 B: it does not close the base gap to main.
