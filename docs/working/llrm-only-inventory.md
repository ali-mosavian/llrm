# llrm-only passes and analyses in the d_faces -O2 profile

Parity 0aea1e46a, d_faces -O2 callgrind, total 936.9 M Ir. Inclusive Ir in M; rows nest (an analysis is also inside the pass that asked for it), so they do not add. "Pin" is the counterpart by the module's own doc; rows marked `check` were not read against the pin's source. Raw list: `ledger4.py` on the same profile (buckets) and `callgrind_annotate --inclusive=yes`.

## The residual

The ledger's residual is ours 114.0 M against the pin's 21.1 M. 55 M of it is passes that sit in no bucket of `ledger4.py` and all have a pin counterpart: MachineScheduler 10.7, ConstraintElimination 6.3, IndVarsOnCreated 5.3 (`check`), LoopInstSimplify 5.2, MachineLICM 3.6, MachineCP 3.4, LiveRangeShrink 3.1, CodeGenPrepare 2.6, CanonicalizeFreezeInLoops 2.5, FixupLEAs 2.1, others 10.3. That is a gap in the buckets, not llrm-only code. The other 59 M (pass-manager glue, analyses asked outside a bucketed pass, allocation, lowering) is not split here.

## Passes with no pin counterpart, or made another way

| pass | Ir | why it exists | verdict |
|---|---|---|---|
| Decide | 11.5 | branches with a known way decided (JumpThreading and SimplifyCFG on known conditions) | Decide@809 is being deleted (w5); the rest go as the SimplifyCFG and JT slots reach the pin |
| Dead | 5.7 | ADCE and DCE in one | candidate: delete where Adce 1.7, Bdce 1.7 and InstCombine's DCE cover it |
| Interprocedural + Stamp | 5.2 + 4.1 | facts across direct calls written into the bodies (constant actuals, returns, purity) | translate first: IPSCCP, FunctionAttrs, ArgumentPromotion |
| FloatAlloc + FloatPrep | 14.8 + 7.8 | flat x87 registers, then stack form (pin: X86FloatingPoint, another design) | translate first |
| FloatFold, FloatLoop | 0.9, 0.05 | exact float values and loops by `floatfacts` | no counterpart; keep, small |
| PressureSink | 4.6 | a load moved to its one reader when pressure drops | candidate: measure against MachineSink and MachineLICM |
| LoopSlots, SharedConstants | 1.3, 2.0 | invariant spill-slot loads out of loops; one value for a phi's repeated constant | candidates, same test |
| CmpAsSub, LoadFold | 0.6, 0.8 | translated from `EmitCmp` and the pin's load folds | not llrm-only |
| Ports, Window, FarIndirectCalls, CalleePop | 0.6, 0.0, 0.2, 0.2 | the 16-bit target: port calls, huge pointers, far calls, callee-pop | keep, target-specific |
| Rotate, Differences, GepOffset, FixedNarrow, ExtLoad | 0.3, 0.3, 0.3, 0.1, 0.1 | gcc's `-ftree-ch`; identities InstCombine has; address-mode displacement; fixed-point product; extending load | small; delete as the pin's pass covers each |
| Merge, LoopIdioms | 2.0, 0.5 | store merging and idioms beyond LoopIdiomRecognize | check against LoopIdiom and MemCpyOpt |
| Together, Fixed, Slot, Walk | 11.0, 417.2, 391.5, 346.1 | the pipeline driver: SROA plus Promote as one slot; per-body fixed point; slot naming; the CGSCC-shaped walk | glue; keep |

## Analyses

| analysis | Ir | pin counterpart | verdict |
|---|---|---|---|
| MemorySsa | 20.6 | MemorySSA, built on an alias stack of our own | keep; its inputs are the cost |
| CallFootprints | 17.5 | `getModRefInfo` of calls (attributes, GlobalsModRef) | translate the AA stack, then delete |
| Writes | 15.0 | what each call writes (stores of CallFootprints) | same |
| Accesses, BaseAccesses | 14.8, 9.3 | `MemoryLocation::get` and the AA queries | same |
| PointerValues, Pointers | 13.6 | BasicAA underlying objects and CaptureTracking | same |
| Summaries | 12.6 | FunctionAttrs and GlobalsAA | same |
| Annotated | 11.0 | provenance on each load and store (TBAA, scoped-noalias metadata) | same |
| ExposedFrames | 5.3 | CaptureTracking on allocas | same |
| Shape | 6.0 | DominatorTree and LoopInfo as block ids; Dominators 0.6 and Loops 1.5 exist | candidate: read Dominators and Loops directly |
| Counted, Followers | 5.2, 0.2 | the counted proofs ScalarEvolution gives | translate first: retire as SCEV covers each client |
| CallRegisters | 4.2 | the register mask of a call | llrm-only; keep |
| GlobalsAA, GlobalsModRef | 1.7, 1.1 | GlobalsAA | at the pin |

The alias stack (CallFootprints, Writes, Accesses, BaseAccesses, PointerValues, Annotated, ExposedFrames, Summaries) is the one large llrm-only mass. It stands where BasicAA, TBAA, CaptureTracking and `AAResults` stand in the pin, and nothing deletes it short of translating them. The passes marked candidate need a bench and objsweep run each before they go.
