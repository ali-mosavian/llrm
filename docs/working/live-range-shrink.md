# Live Range Shrink (codegen slot 47)

The pin's `LiveRangeShrink` (LiveRangeShrink.cpp, x86 `addPreRegAlloc`, d1106deb71cc) moves a single-def instruction down next to its first user inside a block, so the def's live range is shorter before register allocation. It has no switch of its own. Ours: `pressuresink.rs` stands in for it and for the scheduler.

## Premise test (2026-10-23)

The pin's `llc` with the pass off by a patched scratch copy (`LiveRangeShrink::runOnMachineFunction` returns at once under `LRS_OFF`; the assert build's object replaced and relinked, `~/scratch/w4-cgp/llc-abl`; same method as `docs/working/codegenprepare.md`), same IR as before.

| input | pin with the pass | pin without it | functions that differ |
|---|--:|--:|--:|
| 77 IRs (funcattrs-port's), -O2, .text bytes | 370340 | 370321 (-19) | 2 IRs |
| 77 IRs, -Os, .text bytes | 312407 | 312414 (+7) | 3 IRs |
| 71 vsgcc programs, m32 -O2, instructions | 8414 | 8415 | 2 functions |
| 71 vsgcc programs, m32 -Os, instructions | 5048 | 5050 | 3 functions |
| 65 QCport modules, m16 -Os, instructions | 74952 | 74953 | 1 function |

The pass changes the code of 12 of the 77 IRs (the slot table's count) but the size of 2 to 3: it reorders, and the allocator then finds the same number of spills and copies. So its value in bytes and instructions on these inputs is nil (-0.01% to +0.00%). Clocks on an in-order i486 are not measured; the order changes are inside one block.

## Decision

No pass to write. Numbered DEVIATE L1: Live Range Shrink is not translated; `pressuresink.rs` is the stand-in; the pin's own value on 77 IRs is -19 / +7 B; owner w-4; review 2026-12-01 or with the Machine Scheduler work (w-2), whichever comes first.
