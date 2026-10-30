# Homes and Spares against the pin's InlineSpiller (splitkit; note 2026-10-24, exit 2026-11-17)

TL;DR: both passes are llrm-only module passes that cost 16.4 M Ir (d_faces -O2 m32: Homes 8.4, Spares 8.0). Their effect on emitted bytes is nil on QCport and 0.1% on the m32 kernels; their effect on executed instructions is not measured yet. Proposal: measure that by 11-05; remove them unless it shows more than 0.5%.

## What each decides, and what the pin does instead
| pass | decides | the pin |
|---|---|---|
| `Homes` (`!llrm.home`) | a phi that also sits in a stored memory cell is spilled to that cell: no slot, no store, restores read the cell (gcc `REG_EQUIV`) | spills the phi to its own slot (`InlineSpiller::insertSpill`; `StackSlotColoring` may share slots); no value-in-memory fact |
| `Spares` (`!llrm.spares`) | a fixed-cell load is rematerialized past the writes `MemorySSA` proves disjoint | rematerializes only loads that are invariant (`isDereferenceableInvariantLoad`, no alias query: `TargetInstrInfo::isReallyTriviallyReMaterializable`); a load with a write in between is spilled and reloaded |

So the pin has no counterpart for either decision. `InlineSpiller` and `LiveRangeEdit` take their facts from the instruction (`!invariant.load`, memory operands), never from an alias analysis.

## Measured (llrm-c at origin/feat/parity, passes switched off by a local hack)
- QCport, 64 of 65 modules, m16: `-Os` 290,470 B with and without; `-O2` 351,266 B with, 351,261 B without. The passes buy no bytes there.
- qbq-llrm render and game, 7 files, m32 `-O2`: 40,272 B with, 40,316 B without (+44 B, 0.1%).
- Cost: 16.4 M Ir of the 1063 M ledger (1.5%).
- Not measured: executed instructions and clocks (the passes' purpose: fewer slot stores and reloads in loops); the rows that motivated them (`homes_tests.rs`, `spares_tests.rs`) are loop kernels on `@_g`.

## Plan
1. By 11-05: executed-instruction counts with and without on the bench set (the vsgcc kernels and the QCport loop rows), same binary. If the gain is under 0.5% of executed instructions, the passes go (one PR, registry row).
2. If it is more: translate the decision into the allocator where the pin's own mechanisms are, in this order: (a) `Spares` as a rematerialization query on the spiller (LIR level: `storedhomes::apart` already proves a cell untouched; the spiller asks it for the writes between a load and its restore, which is the pin's `allUsesAvailableAt` question); (b) `Homes` as `hoistSpill`-style slot reuse: a spilled phi stored to the cell that already holds it. Both stay a numbered DEVIATE (M = the 16.4 above while module passes; exit = the translation) until then.
3. Exit 11-17: either gone, or computed in the backend with the cost re-measured on the ledger command, objsweep and QCport numbered.
