# Row 45: the Peephole's compile cost, by sub-pass (w-1, 2026-10-24)

Owner: w-1 (handed over by w-2, coordinator ruling). Pin d1106deb71cc. [row45-plan](row45-plan.md) is the translation of `PeepholeOptimizer` itself; this file is the cost of ours, which is 44 post-allocation sub-passes (`peephole.rs`, `Peephole::once`), against the pin's `PeepholeOptimizer` 1.0 M. What each sub-pass is in LLVM, and how often it fires, is in [lir-cleanup-plan](../architecture/lir-cleanup-plan.md) ("The peephole: what fires, and where each rewrite lives in LLVM"); nothing is repeated here.

## Profile at the tip (d_faces -O2 m32, ledger command, symbolized release, callgrind inclusive)
`Peephole::once` 72.5 M Ir over 7 bodies; its callees:

| Sub-pass (callee) | M Ir | Fires (lir-cleanup-plan) | LLVM home | Step |
|---|--:|---|---|---|
| `machinecp::propagated` (2 calls) | 11.50 | 23% | `MachineCopyPropagation` (pin 3.4 M) | row 62's neighbour: cheaper per-instruction facts, not the dataflow (dead end recorded there) |
| `machinecse::eliminated` | 4.15 | 3.7% | MachineCSE runs before allocation only | delete, w-2's step 1 (11-02) |
| `narrowed_arithmetic` | 4.11 | never | none | delete: R45-1 |
| `fused` | 3.23 | 34% | addressing-mode matching and `foldMemoryOperandImpl` | isel, R45-4 |
| `postrasink::sunk` | 3.18 | 14% | `PostRAMachineSinking` | row 44 #24 (after w-2's row 65 piece 2) |
| `constants` | 2.53 | not read | not read | read first, R45-5 |
| `machinedce::eliminated` | 2.48 | 2.4% | `DeadMachineInstructionElim` (pin 1.4 M, before allocation) | R45-5 |
| `zeroes` | 2.43 | 56% | `MOV32r0` expanded after allocation | isel emits the pseudo, R45-3 |
| `increments` | 2.25 | 38% | `UseIncDec` patterns | isel, R45-3 |
| `zero_compares` | 1.75 | 40% | `X86InstrCompiler.td` patterns | isel, R45-3 |
| `secondary_bases` | 1.58 | never | none | delete: R45-1 |
| `popped_arguments` | 1.72 | rare | none | read first, R45-5 |
| `tested` | 1.59 | 27% | `optimizeCompareInstr` (the translated `PeepholeOptimizer`) | R45-6 |
| `copysink::sunk` | 1.43 | 7% | `PostRAMachineSinking` | with row 44 #24 |
| `extensions` | 1.32 | 13% | not read | R45-5 |
| `fused_calls` | 1.14 | 6 fires | addressing-mode matching | R45-4 |
| `restored_copies` | 0.78 | never | none | delete: R45-1 |
| the shared facts the sub-passes ask: `allocate::live` 3.34, `_flags_live_out` 2.57, `_loaded_addresses` 1.98, `dead_at_exit` 1.70, `_flags_dead_after` 1.26 | 10.85 | | | shrink with the sub-passes that ask them |

## Rows (one PR each; ledger Ir before and after; the 568 sweep and the gate as the check that a sub-pass that "never fires" does not)
| # | Change | Removes (ours) | Date |
|---|---|--:|---|
| R45-1 | delete the sub-passes that never fire: `narrowed_arithmetic`, `secondary_bases`, `restored_copies`, `waits`, `storecombine`, `frame` (the spike counted 0 fires in 6,760 runs; the ablation changed no row) | up to 6.5 | 2026-10-31 |
| R45-2 | the shared facts asked once per body, not once per sub-pass | up to 4 | 2026-11-06 |
| R45-3 | `zeroes`, `increments`, `zero_compares` become what isel emits (`xor`, `inc`, `test`) | up to 6.4 | 2026-11-13 |
| R45-4 | `fused`, `fused_calls`, `addresses` become selection's addressing-mode matching | up to 4.4 | 2026-11-20 (sdag-c's matcher, asked first) |
| R45-5 | read `constants`, `extensions`, `popped_arguments`, `overwritten`, `pushes`, `high_extracts`, `sharedstores`, `transferred`, `shuttles`, `zeroed_loads`, `compares_again` against LLVM; each gets a home or a premise | 11 combined | 2026-11-06 (reading), then one PR each |
| R45-6 | `tested` into the translated `PeepholeOptimizer` (`optimizeCompareInstr`) | 1.6 | with row 45 #4, 2026-11-13 |

The sum of the upper bounds is more than the 72.5 M less the pin's counterparts' share; each PR reports what it got, as the F3 rows did.
