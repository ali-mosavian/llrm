# Plan: Machine CSE and Machine InstCombiner rows (funcattrs-port)

Rows 43 and 40 of docs/working/codegen-slot-table.md, moved to funcattrs-port. Land 11-06 each, as soon as it builds and passes the runner; size, clock and compile effects are numbered in the PR, never blocking.

## 43. Machine Common Subexpression Elimination
[`MachineCSE.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp) (976 lines). `precse.rs` is already its translation of `ProcessBlockCSE`, `isCSECandidate`, `isProfitableToCSE` with three open DEVIATEs (E1 no `PerformSimplePRE` and no invariant-load CSE, E2 reader counts from the old body, E3 constants not candidates); `machinecse.rs` (581 lines, after allocation) is ours alone: the pin runs MachineCSE once, before register allocation.
1. Translate what is missing, in the pin's order: `isDereferenceableInvariantLoad` candidates (`isCSECandidate` loads), `PerformSimplePRE` (`ProcessBlockPRE`, `isProfitableToHoistFrom`), the live use-list reader counts (E2), and the constant case (E3) with rematerialization where TwoAddress/RA wants it.
2. Number each DEVIATE (E1-E3 get ids and exits) and drop the "no number" ones.
3. Retire the stand-in: run the rows with `machinecse.rs` off; keep it only where rows rise (numbered), else delete it (a `Deletes:` line, tests ported or classed as mechanism-only).
- Premise (m32, 12 bench programs, same IR to both backends): ours 897 / 2780 B, pin with CSE 811 / 2652 B, without 832 / 2716 B at -O2 (+86 instructions over the pin): the gap is ours.
- Tests: fail-first on each translated case (a load of a constant global repeated across blocks, two sibling blocks computing the same expression, a value read in a loop) at the LIR level as precse's own tests do.

## 40. Machine InstCombiner
[`MachineCombiner.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCombiner.cpp) (776 lines) with X86's `getMachineCombinerPatterns`/`genAlternativeCodeSequence` in X86InstrInfo.cpp (reassociation of associative and commutative integer and float operations, `isAssociativeAndCommutative`): it rewrites a chain of operations to shorten the critical path when the target's scheduling model says it pays (`MachineTraceMetrics`, `TargetSchedModel`).
1. Find what fires at i486: diff `-print-before=machine-combiner` / `-print-after` over the 12 bench IRs (it changes code in 12 of 77 IRs at -O2, 6 at -Os in the pin); name the patterns and the model the i486 CPU gives.
2. Translate the pass and exactly those X86 patterns as an LIR phase before `Peephole`, using ours `liveness`/`timing` for the trace metrics (`MachineTraceMetrics` is not translated: a block-local critical path first, the trace later, DEVIATE numbered).
3. Premise: measure ours vs pin-with vs pin-without per pattern before starting code, as for the other rows; land where ours exceeds pin-with.
- Tests: per pattern, the before/after chain and a chain the model says does not pay (kept).
