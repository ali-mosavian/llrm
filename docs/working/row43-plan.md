# Row 43: Machine CSE — what is left (owner w-1, from funcattrs-port)

Pin d1106deb71cc, `llvm/lib/CodeGen/MachineCSE.cpp`. Ours: `backend/precse.rs` (before phi elimination, #3734) and `backend/machinecse.rs` (after allocation, not the pin's). Translated: `isCSECandidate`, `isProfitableToCSE`, `ProcessBlockCSE`, `PerformCSE`, `EnterScope`/`ExitScope`, `isPRECandidate`, `ProcessBlockPRE`, `PerformSimplePRE`, `isProfitableToHoistInto`, `run`. The DEVIATEs below are the rest, in the order they leave:

| step | DEVIATE | pin function | what | date |
|---|---|---|---|---|
| 1 | CSE4 | `PerformTrivialCopyPropagation` | the rename moves the instruction's read to the copy's source, so heuristics 2 and 3 see the pin's use lists | done, this PR |
| 2 | CSE3 | `isCSECandidate`, `isDereferenceableInvariantLoad` | the invariant loads the pin takes (`!invariant.load`, GOT and constant-pool, immutable fixed stack objects); needs the flag on the LIR load | 2026-11-08 |
| 3 | CSE1 | `isProfitableToHoistInto`, `isLegalToHoistInto` | block frequencies from `MachineBlockFrequencyInfo` of the current CFG (w-5's MBFI), no table of selection's estimate; the landing-pad test where LIR has one | 2026-11-10 |
| 4 | CSE2 | `isCSECandidate` | the operation with no register operand: after the allocator remakes a value with two defs (its row) | with that row |
| 5 | CSE5 | none: `machinecse.rs` | removed once what it finds after allocation is found before it | with step 4 |

Each step: ledger d_faces Ir and the 568 objects before and after; a changed row is numbered; a source test with a premise assertion; the DEVIATE it retires named in the PR.
