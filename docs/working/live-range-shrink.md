# Live Range Shrink (codegen slot 47)

The pin's `LiveRangeShrink` (LiveRangeShrink.cpp, x86 `addPreRegAlloc`, d1106deb71cc) moves a single-def instruction down next to its first user inside a block, so the def's live range is shorter before register allocation. It has no switch of its own. Ours: `pressuresink.rs` stands in for it and for the scheduler.

## Premise test (2026-10-23)

The pin's `llc` with the pass off by a patched scratch copy (`LiveRangeShrink::runOnMachineFunction` returns at once under `LRS_OFF`; the assert build's object replaced and relinked, `llc-abl`; same method as `docs/working/codegenprepare.md`), same IR as before.

| input | pin with the pass | pin without it | functions that differ |
|---|--:|--:|--:|
| 77 IRs (funcattrs-port's), -O2, .text bytes | 370340 | 370321 (-19) | 2 IRs |
| 77 IRs, -Os, .text bytes | 312407 | 312414 (+7) | 3 IRs |
| 71 vsgcc programs, m32 -O2, instructions | 8414 | 8415 | 2 functions |
| 71 vsgcc programs, m32 -Os, instructions | 5048 | 5050 | 3 functions |
| 65 QCport modules, m16 -Os, instructions | 74952 | 74953 | 1 function |

The pass changes the code of 12 of the 77 IRs (the slot table's count) but the size of 2 to 3: it reorders, and the allocator then finds the same number of spills and copies. So its value in bytes and instructions on these inputs is nil (-0.01% to +0.00%). Clocks on an in-order i486 are not measured; the order changes are inside one block.

## Pressure re-take (2026-10-23)

LRS exists to cut register pressure, so bytes undersell it. Same patched `llc`, `-asm-verbose`, counted from the pin's asm (`abl_lrs2.py`): instructions, `Spill`/`Reload` comments, register-to-register moves.

| input | instr | spills | reloads | copies | IRs that differ |
|---|--:|--:|--:|--:|--:|
| 65 QCport IRs m16 -O2, without LRS vs with | +1 of 88748 | -2 of 2073 | -2 of 3020 | +3 of 1669 | 2 |
| 65 QCport IRs m16 -Os | +2 of 76280 | 0 of 1766 | -2 of 2536 | +7 of 1425 | 3 |
| 12 IRs m32 -O2 / -Os | 0 | 0 | 0 | 0 | 0 |

Without LRS the pin spills, reloads and copies the same, to within 7 instructions in 88 000. Clocks, 71 rows programs in the 486 clock model (`tools/pairs/pinclocks.sh` with the pin's clang IR through the patched `llc`, `pinclocks_lrs.sh`), LRS on / off:

| level | clocks on | clocks off | code B | programs that differ | spills / reloads / copies on, off |
|---|--:|--:|--:|---|---|
| -O2 | 40417250 | 40415224 (-0.005%) | 25660, 25660 | 3: recchop -31, x_bigadd +5, x_popcount -2000 | 211/243/326, 211/243/325 |
| -O3 | 40378319 | 40376293 (-0.005%) | 30892, 30892 | the same 3 | 344/472/419, 344/472/418 |

Without the pass the code is the same size and the clocks are lower by 0.005%; the pass is not faster here.

## Decision

Clocks and counts both say nil. Proposed: DEVIATE L1 stands, now with the pressure counts; Live Range Shrink is not translated; `pressuresink.rs` is the stand-in; owner w-4; review 2026-12-01 or with the Machine Scheduler work (w-2), whichever comes first.
