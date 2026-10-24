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

## Where the 63.3 M goes (by kind of work; the dist profile above)
| Kind | M Ir | % | What attacks it |
|---|--:|--:|---|
| the rule groups' generated walks (`x86_m32::generated::*`) | 14.29 | 22.6 | R45-2, one walk (about 13.7 M of it is walking; the rest is the shared `upper_zero` solution) |
| `machinecp::propagated` (copy propagation, 6 calls) | 10.23 | 16.2 | R45-7: per-instruction facts |
| shared liveness and effects facts (`_flags_dead_after`, `LiveRows::sets`, `_flags_live_out`, `dead_at_exit`, `live_rows`, the lane-effects cache) | 12.36 | 19.5 | inherent per body; shrinks with the sub-passes that ask |
| other hand-written sub-passes: `zeroes` 2.28, `popped_arguments` 1.69, `tested` 1.44, `zero_compares` 1.35, `upperzero::hoisted` 0.94, `secondary_bases` 0.92, `zeroed_loads` 0.59, the rest 3.3 | 12.50 | 19.7 | `zeroes`, `zero_compares`, `zeroed_loads` and part of the rest are `each` walks of the same kind and join R45-2 (up to 4.6 M more); `zeroes`/`zero_compares` leave to isel in R45-3 |
| post-RA sinks (`postrasink` 2.97, `copysink` 1.25) | 4.24 | 6.7 | row 44 #24 |
| post-RA `machinecse` | 3.86 | 6.1 | deleted by w-2's step 1 |
| `machinedce` | 2.27 | 3.6 | R45-5 |
| `Contents::has` and the once-cells that gate | 1.11 | 1.7 | |
| `once`'s own code | 2.46 | 3.9 | |

A single walk therefore reaches 13.7 M of the rule groups and, by taking in the other `each`/`window` sub-passes, up to 4.6 M more: about 18 M of 63 M. The other 45 M is analyses (31%), copy propagation (16%), sinks, `machinecse` and DCE (16%), and the hand-written passes that are not walks.

## What does not work (measured)
- Deleting the sub-passes the spike counted as never firing: `storecombine` and `restored_copies` fail two unit tests (adjacent word stores combine; `mov ax,es; mov es,ax` goes). They fire on rare shapes.
- One window per block instead of per instruction in the walkers: -2.0 M on a release build, +0.04 M on the dist build (not landed).

## Rows (one PR each; the ledger on the dist build before and after; the 568 sweep and the gate)
| # | Change | Removes (ours, upper bound) | Date |
|---|---|--:|---|
| R45-2 | one walk for the rule groups: each instruction is dispatched once to the groups whose first-instruction test it passes (the pin's `PeepholeOptimizer` visits each instruction once with all its rewriters) | up to 10 | design 2026-10-24 (below), count 2026-11-13, code 2026-11-20 |
| R45-3 | `zeroes`, `increments`, `zero_compares` are what isel emits (`xor`, `inc`, `test`) | up to 5.5 | 2026-11-27 (sdag-c's matcher, asked first) |
| R45-4 | `fused`, `fused_calls`, `addresses` become selection's addressing-mode matching | up to 4 | 2026-12-04 |
| R45-5 | read `constants`, `extensions`, `popped_arguments`, `overwritten`, `pushes`, `high_extracts`, `sharedstores`, `transferred`, `shuttles`, `zeroed_loads`, `compares_again` against LLVM; each gets a home or a premise | | 2026-11-06 (reading), then one PR each |
| R45-6 | `tested` into the translated `PeepholeOptimizer` (`optimizeCompareInstr`) | 1.4 | with row 45 #4, 2026-11-13 |
| R45-7 | `machinecp::propagated`: per-instruction facts cheaper (the pin's pass is 3.4 M); owner w-3 (row 84) | up to 6 | 2026-11-13 |

Each PR reports what it got on the dist build, as the F3 and TwoAddress rows did.

## The 45 M outside the walk: owner and date each
| Part | M | Owner | Date |
|---|--:|---|---|
| copy propagation | 10.2 | w-3, row 84 (R45-7) | 2026-11-13 |
| shared liveness/effects facts | 12.4 | w-1, R45-8: one `LiveRows` per body, read by the sub-passes that ask; profile first | design 2026-11-13 |
| post-RA sinks, `machinecse`, `machinedce` | 10.4 | w-2 (row 44 #24, `machinecse` step 1); `machinedce` R45-5 w-1 | 2026-11-06 |
| hand-written non-walk passes | 12.5 | w-1, R45-5 reading, then one PR each | 2026-11-06 |

## R45-2 design: a first-instruction index, one scan for all rule groups (2026-10-24)
The groups cannot share one dispatch loop as they are: each walk kind (`each`, `window`, `slide`, `gap`) decides which instructions its window skips and where it resumes, and a later group reads what an earlier one rewrote. Merging the loops changes the output. The pin's `PeepholeOptimizer` has the same order inside one visit, so the order of groups stays.

What is saved is the walking of instructions that cannot start a match. Each rule's first pattern fixes the operation, the name and the operand kinds of the instruction it starts at. One scan of a block builds, per first-pattern key, the positions that have it. A group then walks only its keys' positions and builds a window there; a group whose first pattern is a wildcard keeps walking everything. A block a group rewrote has its index rebuilt before the next group asks, so positions are never stale and the output is identical (objsweep 0 changed is the gate).

Steps, each with the dist ledger before and after:
1. **2026-11-13 - the count**: instrument one run to count, per group, instructions that pass the first-pattern key against all instructions walked. The saving is bounded by that ratio times the 13.7 M; if under 3 M, stop and record the dead end.
2. **2026-11-20 - the index**: `build.rs` emits each group's first-pattern key; `peep::Index` built per block and rebuilt on a changed block; the walkers read positions instead of every slot.
3. **2026-11-27 - the other `each`/`window` sub-passes** that are still hand-written join the index (up to 4.6 M), after R45-3 and R45-5 have moved the ones that leave.

Test (fail first): a body where a group's only candidate sits after a block another group rewrote must still match; reading a stale position misses it.

## R45-5 reading: where each remaining hand-written sub-pass goes (2026-10-24)
Read from each function's own comment; a home is the pin's pass that does the same work, a premise is why the pin has none. Each gets one PR (or a deletion), dated on its row.

| Sub-pass | What it does | Home or premise |
|---|---|---|
| `constants` | per block: reuses identical register contents (immediates, register copies) and folds an extension of a held constant | three parts, three homes: immediate reloads are `latecleanup.rs` already (global, slot 81); register-copy reuse is `MachineCopyPropagation` (row 84, w-3); the extension of a constant is a DAG fold in selection (R45-3). Delete `constants` once a test shows none of the three still fires only here |
| `overwritten` | removes overwritten moves, pure arithmetic, owned spill reloads | home: `machinedce` and `MachineLateInstrsCleanup`; fold into R45-5's `machinedce` row |
| `extensions`, `zeroed_loads` | load plus extension as one `movzx`; `xor r; mov r8,m8` | home: isel patterns (R45-3, with `zeroes`); `zeroed_loads` is priced by the target |
| `high_extracts` | `mov r,[slot]; shr r,16` as a word load, or `shld` | home: load narrowing in selection (`DAGCombiner::reduceLoadWidth`) |
| `pushes` | two immediate word pushes as one dword push | premise: 16-bit stack layout; belongs with the call-frame lowering |
| `popped_arguments` | `add sp,n` as pops at `-Os` | home: `X86FrameLowering::eliminateCallFramePseudoInstr`, which runs in prologue insertion after allocation (row 80, w-6, `framelowering.rs`; not `callframe`, which runs before allocation and cannot see a dead scratch register) |
| `transferred`, `shuttles` | tied work done in the source register when a copy restores the result | home: coalescer hints and `MachineCopyPropagation`; symptoms of allocation, measure after the allocator rows |
| `compares_again` | a compare repeated in the next block entered from one block | premise: gcc's `cse_condition_code_reg`; the pin has `optimizeCompareInstr` only inside a block |

First PRs, in order: `constants` (count what each of its three parts changes on the 568 sweep and QCport, then delete or hand over), `high_extracts` and `extensions` with R45-3; `popped_arguments` (1.69 M) waits for row 80's frame indices (w-6, 2026-11-20) and is asked of w-6 then.

### Count (2026-10-25, step 1)
On `d_faces -O2` the groups' first-pattern test (`Matcher::head`) ran 24,604 times and passed 856 (3.5%), counted at the one `run` all walks call. The walks cost 10.7 M outside the shared `upper_zero` solution, about 435 Ir per test, of which the 856 passes' tails are a part: an upper bound of the saving is most of 10.7 M, an estimate until the index exists. Step 2 (2026-11-20) measures it on the dist build; under 3 M it is dropped.

### `constants`, what each part changes (2026-10-25)
Counted in a throwaway build over the 65 QCport modules at -O2 (m16 and m32): 106 extensions folded to a constant, 36 loads or extensions removed because a register already holds the same known constant, 11 register copies removed, 7 immediate reloads removed (the part `latecleanup.rs` takes). The 41 bench kernels at -O1/-O2/-Os, both modes: 1 copy. Each `Peephole::once` run counts, so the totals include repeats; the proportions are what matter. The extension fold is most of the work, so `constants` is deleted only after the DAG fold (R45-3) covers it; the other parts are 18 of 160.

### `constants` under each selector (2026-10-25)
The 106 extension folds above are the old selector's (the default today). With `--isel=dag` the same count over the modules that compile is 0 folds and 1 removed load (old: 106 and 54): the DAG folds an extension of a constant at node creation (`fold_constant_arithmetic`), so R45-3 has no DAG work for it; `constants` goes when the DAG selector is the default (w12's flip), not before. Compiled: m16 65 of 65 modules (old) and 63 of 65 (dag, two refuse); m32 14 and 15 of 65 (the others fail in the front end on this include set), so the m32 figures say little. xor, inc/dec and test are the matcher's (w11): to be asked which are landed.

### `zeroes`, `increments`, `zero_compares` under each selector (2026-10-25)
w11 (matcher owner): under `--isel=dag` all three forms are pin table patterns, already selected. Checked, not assumed: a throwaway build counted the instructions each sub-pass changed over the 65 QCport modules at m16 -O2 (positions, so a rewrite that moved later instructions would inflate it; these rewrites keep the length):

| sub-pass | old selector | dag selector |
|---|--:|--:|
| `zero_compares` | 1731 | 0 |
| `increments` | 937 | 130 |
| `zeroes` | 1337 | 1190 |

So: `zero_compares` is dead under dag and goes with the default flip. `increments` leaves 130 the matcher does not make: all 130 are `sub r,1` (122 word, 5 byte, 3 dword) turned into `dec`, none an `add r,-1`; the pin's DAGCombiner turns `(sub x, C)` into `(add x, -C)` before its DEC32r pattern sees it, so the missing piece is that fold or a `sub x,1` pattern (DAG combiner, w10). `zeroes` still fires 1190 times under dag because it is the pin's own post-RA step, `X86InstrInfo::expandPostRAPseudo` of `MOV32r0` into `xor`; its home is a translation of that expansion, and its cost (2.28 M) is the walk, which R45-2's index reaches. R45-3 is therefore: delete `zero_compares` and `constants` at the flip, trace the 130, rename `zeroes` as the expansion.

