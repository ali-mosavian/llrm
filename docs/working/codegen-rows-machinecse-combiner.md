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
w-8's plan note docs/working/machinecombiner-translation-plan.md (#3687) is the plan: pin functions in order with permalinks (`run`, `combineInstructions`, `getDepth`, `getLatency`, `improvesCriticalPathLen`, TargetInstrInfo reassociation helpers, X86 `isAssociativeAndCommutative`); on i486 only the generic REASSOC patterns apply and the pass needs trace depth and default latencies. Code follows pushfold.rs's template (pre-RA LIR phase, flow.rs SLOTS, parity.rs REGISTRY, the parity_tests check_backend list, the slot-table row). Step 0 here: premise per pattern (ours vs pin with and without) before code; land where ours exceeds pin-with.

Step 0 result (m32, `-print-before/after=machine-combiner` over the 12 bench IRs, pin d1106deb71cc): the pass changes bintree, crc and matmul and only by reassociating chains of one integer operation: `ADD32rr` (matmul's nsw adds, bintree's `(a+b)+c` to `a+(b+c)`) and `XOR32rr` (crc), the generic REASSOC_AX_BY/AX_YB family on associative and commutative operations (`X86InstrInfo::isAssociativeAndCommutative`: add, and, or, xor, imul, no float at -sse); no FMA, no LEA pattern. So the translation is `MachineCombiner::combineInstructions` + `TargetInstrInfo::getMachineCombinerPatterns`/`genAlternativeCodeSequence` for those, with depth from a block-local critical path.
