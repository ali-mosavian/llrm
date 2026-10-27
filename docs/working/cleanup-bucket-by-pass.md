# The machine clean-up bucket, one row per pass (w-6, parity 69b60cafc)

Inclusive Ir of each of our backend passes (`LIRTransform::transform_raising`, d_faces -O2, dist, real `-o`) against the pin's counterpart
(`runOnMachineFunction` of the matching pass, clang d1106deb71cc, the same input); `tools/ledger/passtable.py` on the two profiles `ledger4.py`
reads. The totals are the bucket's of `docs/working/cost-centers.md`. The owner column is a proposal.

Two rows to read first: Peephole is 66.9 M against the pin's PeepholeOptimizer 2.1 M (it does the work of the pin's DAG combines and X86 passes as
well); the FP stackifier (`floatalloc`, `floatassign`, `floatfold`) is 28.6 M against the pin's 1.7 M.

## qbq-llrm
| our pass | ours M | pin M | ratio | proposed owner |
|---|--:|--:|--:|---|
| peephole::Peephole | 66.9 | 2.1 | 32.4 | w-1 |
| machinesched::MachineScheduler | 6.5 | 10.4 | 0.6 | w-2 |
| schedule::Scheduler | 0.0 | - | - | w-2 |
| machinesink::MachineSink | 10.8 | 4.5 | 2.4 | w-2 |
| pressuresink::PressureSink | 4.4 | - | - | w-2 |
| floatalloc::FloatAlloc | 19.1 | 1.7 | 11.5 | w-2 |
| floatassign::FloatPrep | 8.7 | - | - | w-2 |
| floatfold::FloatFold | 0.8 | - | - | w-2 |
| jumps::ControlFlow | 20.7 | 5.9 | 3.5 | w-3 |
| machinelicm::MachineLicm | 3.8 | 9.1 | 0.4 | w-3 |
| machinecp::CopyPropagation | 4.2 | 3.4 | 1.2 | w-3 |
| precse::MachineCse | 5.7 | 2.9 | 2.0 | w-2 (unassigned) |
| latecleanup::LateCleanup | 5.2 | 1.2 | 4.3 | funcattrs-port |
| combiner::MachineCombiner | 1.3 | 0.0 | - | w-8 |
| liverangeshrink::LiveRangeShrink | 2.7 | 1.3 | 2.1 | w-2 (unassigned) |
| loopslots::LoopSlots | 4.8 | - | - | w-3 (unassigned) |
| phiconst::SharedConstants | 1.7 | - | - | w-3 (unassigned) |
| farcall::FarIndirectCalls | 0.1 | - | - | (m16 only) |
| fixupsetcc::FixupSetCc | 1.9 | 0.0 | - | w-2 (unassigned) |
| fixupbw::FixupBw | 1.2 | - | - | w-1 (pin: with Peephole above) |
| pushfold::LoadFold | 0.8 | - | - | w-1 |
| cmpsub::CmpAsSub | 0.5 | - | - | w-1 |
| leaopt::OptimizeLeas | 0.4 | 0.0 | - | w-2 (lea fold) |
| fixupleas::FixupLeas | 0.0 | 0.0 | - | w-2 (lea fold) |
| flagscopy::FlagsCopy | 0.1 | 0.0 | - | w-2 (unassigned) |
| callframe::CallFrameOpt | 0.1 | 0.0 | - | w-2 (unassigned) |
| (none of ours) PEI (PrologEpilogInserter) | - | 1.0 | - | (unassigned) |
| (none of ours) ShrinkWrap | - | 0.3 | - | (unassigned) |
| (none of ours) ReachingDefAnalysis | - | 3.4 | - | (unassigned) |
| (none of ours) ExecutionDomainFix | - | 0.9 | - | (unassigned) |
| (none of ours) DeadMachineInstructionElim | - | 1.4 | - | (unassigned) |
| total | 172.3 | 49.4 | 3.48 | |

## QCport
| our pass | ours M | pin M | ratio | proposed owner |
|---|--:|--:|--:|---|
| peephole::Peephole | 47.4 | 1.8 | 26.2 | w-1 |
| machinesched::MachineScheduler | 5.5 | 8.5 | 0.6 | w-2 |
| schedule::Scheduler | 0.0 | - | - | w-2 |
| machinesink::MachineSink | 8.0 | 2.6 | 3.0 | w-2 |
| pressuresink::PressureSink | 3.3 | - | - | w-2 |
| floatalloc::FloatAlloc | 13.6 | 1.5 | 9.2 | w-2 |
| floatassign::FloatPrep | 5.8 | - | - | w-2 |
| floatfold::FloatFold | 0.7 | - | - | w-2 |
| jumps::ControlFlow | 13.4 | 5.6 | 2.4 | w-3 |
| machinelicm::MachineLicm | 2.8 | 8.5 | 0.3 | w-3 |
| machinecp::CopyPropagation | 1.4 | 3.1 | 0.4 | w-3 |
| precse::MachineCse | 4.7 | 2.3 | 2.0 | w-2 (unassigned) |
| latecleanup::LateCleanup | 4.5 | 1.2 | 3.8 | funcattrs-port |
| combiner::MachineCombiner | 1.0 | 0.0 | - | w-8 |
| liverangeshrink::LiveRangeShrink | 2.1 | 1.1 | 1.9 | w-2 (unassigned) |
| loopslots::LoopSlots | 3.4 | - | - | w-3 (unassigned) |
| phiconst::SharedConstants | 1.3 | - | - | w-3 (unassigned) |
| farcall::FarIndirectCalls | 0.1 | - | - | (m16 only) |
| fixupsetcc::FixupSetCc | 1.7 | 0.0 | - | w-2 (unassigned) |
| fixupbw::FixupBw | 0.9 | - | - | w-1 (pin: with Peephole above) |
| pushfold::LoadFold | 0.7 | - | - | w-1 |
| cmpsub::CmpAsSub | 0.4 | - | - | w-1 |
| leaopt::OptimizeLeas | 0.3 | 0.0 | - | w-2 (lea fold) |
| fixupleas::FixupLeas | 0.0 | 0.0 | - | w-2 (lea fold) |
| flagscopy::FlagsCopy | 0.0 | 0.0 | - | w-2 (unassigned) |
| callframe::CallFrameOpt | 0.1 | 0.0 | - | w-2 (unassigned) |
| (none of ours) PEI (PrologEpilogInserter) | - | 0.9 | - | (unassigned) |
| (none of ours) ShrinkWrap | - | 0.2 | - | (unassigned) |
| (none of ours) ReachingDefAnalysis | - | 3.0 | - | (unassigned) |
| (none of ours) ExecutionDomainFix | - | 0.8 | - | (unassigned) |
| (none of ours) DeadMachineInstructionElim | - | 1.3 | - | (unassigned) |
| total | 123.2 | 42.5 | 2.90 | |

## Peephole, measured again (w-1, 2026-10-27)

Peephole is 44.5 M on d_faces -O2 (callgrind of Peephole alone), not comparable to the pin's PeepholeOptimizer 2.1 M: it also does the pin's DAG combines
and X86 fixups. Flat: the walks cost about 4 M together, lane liveness 6.8 M, `upperzero::disturbed` 2.0 M, each of ~40 groups 0.3 to 1 M. No cut above
3 M (0.3% of the total); folding the walks cannot keep the output identical, since each group reads the one before. Not worth re-measuring.
