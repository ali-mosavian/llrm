# Row 45: the Peephole's compile cost, by sub-pass (w-1, 2026-10-24)

Owner: w-1 (handed over by w-2, coordinator ruling). Pin d1106deb71cc. [row45-plan](row45-plan.md) is the translation of `PeepholeOptimizer` itself; this file is the cost of ours, which is 44 post-allocation sub-passes (`peephole.rs`, `Peephole::once`), against the pin's `PeepholeOptimizer` 1.0 M. What each sub-pass is in LLVM, and how often it fires, is in [lir-cleanup-plan](../architecture/lir-cleanup-plan.md) ("The peephole: what fires, and where each rewrite lives in LLVM"); nothing is repeated here.

## Profile at the tip (d_faces -O2 m32, ledger command, symbolized `dist` build, callgrind inclusive)
The first version of this file read a symbolized `release` build: the dist build (fat LTO, one codegen unit) the ledger measures attributes differently, and a change that saved 2.0 M of `Peephole::once` there saved 0.04 M of the ledger (#4319's note). The numbers below are from a `dist` build with line tables kept (`CARGO_PROFILE_DIST_DEBUG=line-tables-only CARGO_PROFILE_DIST_STRIP=false`), the tip of 2026-10-24, total 922.8 M.
`Peephole::once` 63.3 M over 7 bodies, self 2.5 M; its callees:

| Sub-pass (callee) | M Ir | LLVM home | Step |
|---|--:|---|---|
| `machinecp::propagated` (6 calls) | 10.23 | `MachineCopyPropagation` (pin 3.4 M) | cheaper per-instruction facts (dead end in lir-cleanup-plan: the dataflow is not the cost) |
| `machinecse::eliminated` | 3.86 | MachineCSE runs before allocation only | delete, w-2's step 1 |
| `narrowed_arithmetic` | 3.77 | none | 3.6 M of it is `guards::upper_zero`, the zero-upper-half solution made once per body for the first group that asks (`upperzero::hoisted` 0.94 and the widening groups read it): an inherent analysis, not a walk |
| `postrasink::sunk` | 2.97 | `PostRAMachineSinking` | row 44 #24 |
| `fused` | 2.49 | addressing-mode matching, `foldMemoryOperandImpl` | isel (R45-4) |
| `_flags_dead_after` (612 calls) | 2.46 | | once per block: inherent |
| `LiveRows::sets` (14) | 2.43 | | twice per body |
| `_flags_live_out` | 2.39 | | once per body |
| `zeroes` | 2.28 | `MOV32r0` expanded after allocation | R45-3 |
| `machinedce::eliminated` | 2.27 | `DeadMachineInstructionElim` (pin 1.4 M) | R45-5 |
| `increments` | 1.87 | `UseIncDec` patterns | R45-3 |
| `popped_arguments` | 1.69 | none | read first, R45-5 |
| `tested` | 1.44 | `optimizeCompareInstr` | R45-6 |
| `zero_compares` | 1.35 | `X86InstrCompiler.td` | R45-3 |
| `copysink::sunk` | 1.25 | `PostRAMachineSinking` | row 44 #24 |
| `secondary_bases` | 0.92 | none | gated already (`needs`) |

The rule groups together are 17.4 M inclusive of the 63.3 M, 3.6 M of it the shared `upper_zero` solution: about 13.7 M is thirty-eight walks of the whole body, each building a window for every instruction (the walkers' per-window allocations were shown not to be the cost on the dist build).

## What does not work (measured)
- Deleting the sub-passes the spike counted as never firing: `storecombine` and `restored_copies` fail two unit tests (adjacent word stores combine; `mov ax,es; mov es,ax` goes). They fire on rare shapes.
- One window per block instead of per instruction in the walkers: -2.0 M on a release build, +0.04 M on the dist build (not landed).

## Rows (one PR each; the ledger on the dist build before and after; the 568 sweep and the gate)
| # | Change | Removes (ours, upper bound) | Date |
|---|---|--:|---|
| R45-2 | one walk for the rule groups: each instruction is dispatched once to the groups whose first-instruction test it passes (the pin's `PeepholeOptimizer` visits each instruction once with all its rewriters) | up to 10 | design 2026-11-06, code 2026-11-20 |
| R45-3 | `zeroes`, `increments`, `zero_compares` are what isel emits (`xor`, `inc`, `test`) | up to 5.5 | 2026-11-27 (sdag-c's matcher, asked first) |
| R45-4 | `fused`, `fused_calls`, `addresses` become selection's addressing-mode matching | up to 4 | 2026-12-04 |
| R45-5 | read `constants`, `extensions`, `popped_arguments`, `overwritten`, `pushes`, `high_extracts`, `sharedstores`, `transferred`, `shuttles`, `zeroed_loads`, `compares_again` against LLVM; each gets a home or a premise | | 2026-11-06 (reading), then one PR each |
| R45-6 | `tested` into the translated `PeepholeOptimizer` (`optimizeCompareInstr`) | 1.4 | with row 45 #4, 2026-11-13 |
| R45-7 | `machinecp::propagated`: per-instruction facts cheaper (the pin's pass is 3.4 M) | up to 6 | 2026-11-13 |

Each PR reports what it got on the dist build, as the F3 and TwoAddress rows did.
