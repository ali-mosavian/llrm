| # | Pin pass | State | Ours | Pin switch | Why |
|---|---|---|---|---|---|
| 1 | ObjC ARC contraction | N/A |  |  | no Objective-C: the pass acts on retain/release calls the frontends never emit. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 2 | Pre-ISel Intrinsic Lowering | EXECUTED | isel/expand.rs, hir lowering |  | memory and math intrinsics lowered by the selector. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 3 | Expand IR instructions | N/A |  |  | acts on fp-to-int/div of more than 128 bits; MIR's widest integer is i64 (lower_int64.rs). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 4 | Expand Atomic instructions | N/A |  |  | MIR has no atomic or fence opcode (opcode.rs). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 5 | Lower AMX intrinsics | N/A |  |  | AMX is a 64-bit extension; target features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 6 | Lower AMX type for load/store | N/A |  |  | as above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 7 | Canonicalize natural loops | EXECUTED | loopsimplify.rs (mid-end) |  | loop-simplify runs in the mid-end; LSR relies on it. Pin: changed code in 59 (-O2) and 60 (-Os) of 77 IRs |
| 8 | Canonicalize Freeze Instructions in Loops | MISSING |  |  | funcattrs-port; 2026-11-13: MIR has `freeze`; LSR's canon-freeze is not run. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 9 | Induction Variable Users | MATCH | ivusers.rs |  | translated. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 10 | Loop Strength Reduction | MATCH | lsr_*.rs (registry `lsr`) | `-disable-lsr` | translated; runs at the end of the mid-end, the pin's runs after it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-lsr -O2: -37 instr, +40 B, 61 IRs differ; -disable-lsr -Os: +283 instr, +876 B, 63 IRs differ |
| 11 | Lower Garbage Collection Instructions | N/A |  |  | no GC strategy in any frontend. Pin: changed code in 62 (-O2) and 64 (-Os) of 77 IRs |
| 12 | Shadow Stack GC Lowering | N/A |  |  | as above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 13 | Remove unreachable blocks from the CFG | EXECUTED | simplifycfg.rs, adce.rs |  | removed in the mid-end before selection. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 14 | Constant Hoisting | N/A |  | `-disable-constant-hoisting` | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-constant-hoisting -O2: +0 instr, +0 B, 0 IRs differ; -disable-constant-hoisting -Os: +0 instr, +0 B, 0 IRs differ |
| 15 | Replace intrinsics with calls to vector library | N/A |  | `-disable-replace-with-vec-lib` | no vector library, no vectors in MIR (deferred). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 16 | Partially inline calls to library functions | N/A |  | `-disable-partial-libcall-inlining` | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-partial-libcall-inlining -O2: +0 instr, +0 B, 0 IRs differ; -disable-partial-libcall-inlining -Os: +0 instr, +0 B, 0 IRs differ |
| 17 | Instrument function entry/exit with calls to e.g. mcount() (post inlining) | N/A |  |  | acts on the `instrument-function-entry-inlined` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 18 | Scalarize Masked Memory Intrinsics | N/A |  |  | no masked intrinsics in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 19 | Expand reduction intrinsics | N/A |  | `-disable-expand-reductions` | no vector reductions in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 20 | Interleaved Access Pass | N/A |  |  | no vectors in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 21 | X86 Partial Reduction | N/A |  |  | SSE/AVX dot-product patterns; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 22 | Expand indirectbr instructions | N/A |  |  | MIR has no indirectbr. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 23 | CodeGen Prepare | DEVIATE | addresssink.rs (sinkCmpExpression, address phis), extload.rs (ext-load promotion), isel (address forms rebuilt in the user's block) | `-disable-cgp` | CG3 (C1 in addresssink.rs): the pass's work is done by isel, addresssink.rs and the select and switch lowering; reason: per function on the same IR the m32 corpus matches the pin (the pass changes 16-17 of about 160 functions; ours is at its level) and the lea count equals the pin's with the pass (2092 against 2139, 3983 without) (docs/working/codegenprepare.md); cost: at m16 up to 1544 of 4132 instructions of excess lie in functions the pin's address sinking helps (row AM-C2); exit: the 8086-form experiment 2026-11-06; owner w-4; review 2026-12-01. Pin: changed code in 73 (-O2) and 72 (-Os) of 77 IRs; -disable-cgp -O2: +4456 instr, +21337 B; sub-features measured by a patched scratch llc: optimizeMemoryInst +20712 B, sinkCmpExpression +468, optimizeCmp +469, select +4, dupRet -2, empty blocks -302 |
| 24 | Exception handling preparation | EXECUTED | ehprepare.rs |  | dwarf-eh-prepare equivalent. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 25 | Prepare inline asm insts | N/A |  |  | inline asm is handled by inline_asm.rs; the pass only rewrites constraints for callbr. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 26 | Safe Stack instrumentation pass | N/A |  |  | acts on the `safestack` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 27 | Insert stack protectors | N/A |  |  | acts on `ssp*` attributes, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 28 | X86 DAG->DAG Instruction Selection | DEVIATE | isel.rs, isel/matcher.rs |  | CG1: llrm selects from MIR with its own rule matcher, no SelectionDAG; reason: segmented targets and 16-bit pointers; cost: the rows; exit: none planned; owner oversight; review 2026-11-13. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 29 | Local Dynamic TLS Access Clean-up | N/A |  |  | no thread-local storage in any frontend. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 30 | X86 PIC Global Base Reg Initialization | N/A |  |  | no PIC code at m32 OMF/COFF or m16. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 31 | Argument Stack Rebase | N/A |  |  | acts on swiftasync frames only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 32 | Finalize ISel and expand pseudo-instructions | EXECUTED | isel.rs |  | the selector emits final instructions. Pin: changed code in 55 (-O2) and 54 (-Os) of 77 IRs |
| 33 | X86 Domain Reassignment Pass | N/A |  | `-disable-x86-domain-reassignment` | SSE/AVX mask domains; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-x86-domain-reassignment -O2: +0 instr, +0 B, 0 IRs differ; -disable-x86-domain-reassignment -Os: +0 instr, +0 B, 0 IRs differ |
| 34 | Early Tail Duplication | MISSING | jumps/ (late tail duplication only) | `-disable-early-taildup` | lir-cleanup-port; 2026-11-13: registry slot early-tailduplication. Pin: changed code in 23 (-O2) and 18 (-Os) of 77 IRs; -disable-early-taildup -O2: -34 instr, -131 B, 9 IRs differ; -disable-early-taildup -Os: +14 instr, -4 B, 8 IRs differ |
| 35 | Optimize machine instruction PHIs | MATCH | optimizephis.rs |  | translated (registry optimizephis@1). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 36 | Merge disjoint stack slots | EXECUTED | lifetimes.rs, slots.rs |  | slots of locals with non-overlapping lifetime markers share; the registry says Missing: reconcile at the ablation. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 37 | Local Stack Slot Allocation | N/A |  |  | X86 does not request virtual frame base registers (requiresVirtualBaseRegisters is false). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 38 | Remove dead machine instructions | EXECUTED | dead-code elimination in MIR; machinedce.rs after allocation | `-disable-machine-dce` | before register allocation MIR already removed the dead. Pin: changed code in 61 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-dce -O2: +11 instr, +50 B, 14 IRs differ; -disable-machine-dce -Os: +19 instr, +131 B, 14 IRs differ |
| 39 | Early If-Conversion | N/A |  | `-disable-early-ifcvt` | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-early-ifcvt -O2: +0 instr, +0 B, 0 IRs differ; -disable-early-ifcvt -Os: +0 instr, +0 B, 0 IRs differ |
| 40 | Machine InstCombiner | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 12 (-O2) and 6 (-Os) of 77 IRs |
| 41 | X86 cmov Conversion | N/A |  |  | the pass returns at once without CMOV, which the i486 does not have (selects.rs makes branches). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -x86-cmov-converter=0 -O2: +0 instr, +0 B, 0 IRs differ; -x86-cmov-converter=0 -Os: +0 instr, +0 B, 0 IRs differ |
| 42 | Early Machine Loop Invariant Code Motion | MISSING | licm (mid-end), loopslots.rs for reloads | `-disable-machine-licm` | funcattrs-port; 2026-11-13: changed code in 58 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-licm -O2: -242 instr, -1012 B, 38 IRs differ; -disable-machine-licm -Os: -304 instr, -1399 B, 42 IRs differ |
| 43 | Machine Common Subexpression Elimination | MISSING | precse.rs (before phi elimination), machinecse.rs (after allocation) | `-disable-machine-cse` | funcattrs-port; 2026-10-25: registry machinecse@4 is partial; untranslated PerformSimplePRE and invariant-load CSE. Pin: changed code in 57 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-cse -O2: +342 instr, +1082 B, 53 IRs differ; -disable-machine-cse -Os: +377 instr, +1151 B, 55 IRs differ |
| 44 | Machine code sinking | MISSING | machinesink.rs | `-disable-machine-sink` | funcattrs-port; 2026-10-25: registry machinesink@4 is partial; store and load sinking, SinkAndFold. Pin: changed code in 67 (-O2) and 65 (-Os) of 77 IRs; -disable-machine-sink -O2: +699 instr, +2474 B, 64 IRs differ; -disable-machine-sink -Os: +1107 instr, +3707 B, 60 IRs differ |
| 45 | Peephole Optimizations | MISSING | peephole.rs and peep/ | `-disable-peephole` | lir-cleanup-port; 2026-11-13: registry peephole@14 is ours-own, not a translation. Pin: changed code in 72 (-O2) and 72 (-Os) of 77 IRs; -disable-peephole -O2: +2189 instr, +7527 B, 67 IRs differ; -disable-peephole -Os: +1763 instr, +6048 B, 69 IRs differ |
| 46 | Remove dead machine instructions (run 2) | EXECUTED | machinedce.rs | `-disable-machine-dce` | as above, after the peephole. Pin: changed code in 61 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-dce -O2: +11 instr, +50 B, 14 IRs differ; -disable-machine-dce -Os: +19 instr, +131 B, 14 IRs differ |
| 47 | Live Range Shrink | DEVIATE | pressuresink.rs stands in |  | CG2: pressuresink.rs does the work and the pin's pass is not translated; reason: with the pass off the 77 IRs change by -19 B (-O2) and +7 B (-Os), the 71 vsgcc programs by 1-2 instructions, QCport m16 by 1 (docs/working/live-range-shrink.md); cost: nil; exit: none planned; owner w-4; review 2026-12-01 or with the Machine Scheduler work. Pin: changed code in 12 (-O2) and 12 (-Os) of 77 IRs |
| 48 | X86 Fixup SetCC | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 41 (-O2) and 40 (-Os) of 77 IRs |
| 49 | X86 LEA Optimize | MISSING |  | `-disable-x86-lea-opt` | funcattrs-port; 2026-11-13: changed code in 4 (-O2) and 34 (-Os) of 77 IRs; -disable-x86-lea-opt -O2: +6 instr, +32 B, 3 IRs differ; -disable-x86-lea-opt -Os: -60 instr, +707 B, 34 IRs differ |
| 50 | X86 Optimize Call Frame | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 59 (-O2) and 60 (-Os) of 77 IRs |
| 51 | X86 Avoid Store Forwarding Blocks | N/A |  |  | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 52 | X86 Suppress APX features for relocation | N/A |  |  | APX is 64-bit; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 53 | X86 speculative load hardening | N/A |  |  | acts on the `speculative_load_hardening` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 54 | X86 EFLAGS copy lowering | MISSING | flag lanes (liveness.rs), stepflags.rs |  | funcattrs-port; 2026-11-13: changed code in 2 (-O2) and 2 (-Os) of 77 IRs |
| 55 | X86 DynAlloca Expander | MISSING |  |  | funcattrs-port; 2026-11-09: dynamic alloca needs an HIR op (builtins row B2). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 56 | Tile Register Pre-configure | N/A |  |  | AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 57 | Detect Dead Lanes | N/A |  |  | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; ours has lanes.rs with no pin counterpart on the i486 |
| 58 | Init Undef Pass | N/A |  |  | acts only where a register class has a pseudo init-undef (RISC-V); X86 has none. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 59 | Process Implicit Definitions | MISSING |  |  | funcattrs-port; 2026-11-13: tied to MIR undef (LIR has no IMPLICIT_DEF; uninitialized reads are zero). Pin: changed code in 62 (-O2) and 63 (-Os) of 77 IRs |
| 60 | Remove unreachable machine basic blocks | EXECUTED | jumps.rs |  | removed with the layout. Pin: changed code in 64 (-O2) and 62 (-Os) of 77 IRs |
| 61 | Eliminate PHI nodes for register allocation | MISSING | phielim.rs, phielimination.rs |  | lir-cleanup-port; 2026-11-13: registry phielim@4 is ours-own. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 62 | Two-Address instruction pass | MISSING | twoaddr.rs |  | lir-cleanup-port; 2026-11-13: registry twoaddr@9. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 63 | Register Coalescer | MISSING | coalesce.rs, coalescer/ |  | regalloc-core-poc; 2026-11-13: registry coalesce@10 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 64 | Rename Disconnected Subregister Components | N/A |  |  | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 65 | Machine Instruction Scheduler | MISSING | pressuresink.rs stands in | `-enable-misched=0` | funcattrs-port; 2026-11-13: changed code in 55 (-O2) and 54 (-Os) of 77 IRs; -enable-misched=0 -O2: +0 instr, +154 B, 31 IRs differ; -enable-misched=0 -Os: +93 instr, +557 B, 30 IRs differ |
| 66 | Greedy Register Allocator | MISSING | allocate.rs, greedy/, splitkit | `-regalloc=fast` | regalloc-core-poc; 2026-11-13: registry allocate@11 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 67 | Tile Register Configure | N/A |  |  | AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 68 | Greedy Register Allocator (run 2) | N/A |  |  | the tile-register instance: AMX only; the change counts of run 1 are the same name's, and no i486 register is in the tile class |
| 69 | Virtual Register Rewriter | MISSING | allocate.rs, parcopy.rs |  | regalloc-core-poc; 2026-11-13. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 70 | Stack Slot Coloring | EXECUTED | slots.rs | `-disable-ssc` | frame slots of spilled values share when never live together; matched at the ablation. Pin: changed code in 48 (-O2) and 47 (-Os) of 77 IRs; -disable-ssc -O2: +94 instr, +2880 B, 37 IRs differ; -disable-ssc -Os: +6 instr, +1765 B, 33 IRs differ |
| 71 | Machine Copy Propagation Pass | MATCH | machinecp.rs | `-disable-copyprop` | translated, run inside the peephole. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs; -disable-copyprop -O2: +122 instr, +403 B, 38 IRs differ; -disable-copyprop -Os: +115 instr, +353 B, 34 IRs differ |
| 72 | Machine Loop Invariant Code Motion | N/A |  | `-disable-postra-machine-licm` | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 2 (-O2) and 3 (-Os) of 77 IRs; -disable-postra-machine-licm -O2: +0 instr, +0 B, 0 IRs differ; -disable-postra-machine-licm -Os: +0 instr, +0 B, 0 IRs differ; ours has loopslots.rs for reloads with no pin counterpart on the i486 |
| 73 | X86 Lower Tile Copy | N/A |  |  | AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 74 | X86 FP Stackifier | MATCH | floatalloc.rs |  | translated. Pin: changed code in 53 (-O2) and 53 (-Os) of 77 IRs |
| 75 | X86 Load Value Injection (LVI) Load Hardening | N/A |  |  | off unless -mlvi-hardening. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 76 | Remove Redundant DEBUG_VALUE analysis | N/A |  |  | debug values; llrm's debuginfo.rs works from source positions. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 77 | Fixup Statepoint Caller Saved | N/A |  |  | no statepoints. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 78 | PostRA Machine Sink | EXECUTED | postrasink.rs | `-disable-postra-machine-sink` | a copy or spill into the one successor that needs it. Pin: changed code in 28 (-O2) and 30 (-Os) of 77 IRs; -disable-postra-machine-sink -O2: -1 instr, +14 B, 4 IRs differ; -disable-postra-machine-sink -Os: -9 instr, -28 B, 8 IRs differ |
| 79 | Shrink Wrapping analysis | EXECUTED | saverestore.rs, shrinkwrap.rs | `-enable-shrink-wrap=0` | follows ShrinkWrapImpl; the registry says Missing: reconcile at the ablation. Pin: changed code in 4 (-O2) and 4 (-Os) of 77 IRs; -enable-shrink-wrap=0 -O2: -1 instr, -1 B, 2 IRs differ; -enable-shrink-wrap=0 -Os: +0 instr, +0 B, 0 IRs differ |
| 80 | Prologue/Epilogue Insertion & Frame Finalization | MISSING | prologue.rs, masm.rs |  | lir-cleanup-port; 2026-11-13: registry prologue@13. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 81 | Machine Late Instructions Cleanup Pass | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 13 (-O2) and 12 (-Os) of 77 IRs |
| 82 | Control Flow Optimizer | MISSING | jumps/branchfold.rs | `-disable-branch-fold` | lir-cleanup-port; 2026-11-13: registry jumps@17 is partial. Pin: changed code in 75 (-O2) and 75 (-Os) of 77 IRs; -disable-branch-fold -O2: +140 instr, +394 B, 40 IRs differ; -disable-branch-fold -Os: +134 instr, +323 B, 32 IRs differ |
| 83 | Tail Duplication | MISSING | jumps/  | `-disable-tail-duplicate` | lir-cleanup-port; 2026-11-13: registry jumps@17. Pin: changed code in 33 (-O2) and 20 (-Os) of 77 IRs; -disable-tail-duplicate -O2: -42 instr, -193 B, 17 IRs differ; -disable-tail-duplicate -Os: +0 instr, +0 B, 0 IRs differ |
| 84 | Machine Copy Propagation Pass (run 2) | MISSING |  | `-disable-copyprop` | funcattrs-port; 2026-10-25: llrm runs it once; the pin runs it again after tail duplication. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs; -disable-copyprop -O2: +122 instr, +403 B, 38 IRs differ; -disable-copyprop -Os: +115 instr, +353 B, 34 IRs differ |
| 85 | Post-RA pseudo instruction expansion pass | EXECUTED | isel.rs |  | the selector emits final instructions. Pin: changed code in 75 (-O2) and 75 (-Os) of 77 IRs |
| 86 | X86 pseudo instruction expansion pass | EXECUTED | isel.rs |  | as above. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs |
| 87 | Insert KCFI indirect call checks | N/A |  |  | acts on the `kcfi` module flag, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 88 | Post RA top-down list latency scheduler | N/A |  | `-disable-post-ra` | 0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-post-ra -O2: +0 instr, +0 B, 0 IRs differ; -disable-post-ra -Os: +0 instr, +0 B, 0 IRs differ; ours has schedule.rs (narrower) with no pin counterpart on the i486 |
| 89 | Analyze Machine Code For Garbage Collection | N/A |  |  | no GC strategy. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 90 | Branch Probability Basic Block Placement | MISSING | jumps/placement | `-disable-block-placement` | lir-cleanup-port; 2026-11-13: registry jumps@17, chains and loops (#2629). Pin: changed code in 74 (-O2) and 62 (-Os) of 77 IRs; -disable-block-placement -O2: +286 instr, +1995 B, 64 IRs differ; -disable-block-placement -Os: +729 instr, +2802 B, 57 IRs differ |
| 91 | Insert fentry calls | N/A |  |  | acts on the `fentry-call` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 92 | Insert XRay ops | N/A |  |  | acts on XRay attributes, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 93 | Implement the 'patchable-function' attribute | N/A |  |  | acts on `patchable-function`, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 94 | X86 Execution Dependency Fix | N/A |  |  | SSE execution domains; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 95 | BreakFalseDeps | N/A |  |  | partial-register and SSE false dependencies; the 486 and P5 have none. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 96 | X86 Indirect Branch Tracking | N/A |  |  | CET; off without the `cf-protection-branch` flag. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 97 | X86 vzeroupper inserter | N/A |  |  | AVX only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 98 | X86 Byte/Word Instruction Fixup | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 63 (-O2) and 62 (-Os) of 77 IRs |
| 99 | X86 Atom pad short functions | N/A |  |  | acts on Atom (`padShortFunctions`); not i486. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 100 | X86 LEA Fixup | MISSING |  |  | funcattrs-port; 2026-11-13: changed code in 37 (-O2) and 41 (-Os) of 77 IRs |
| 101 | X86 Fixup Inst Tuning | N/A |  |  | tunes SSE shuffles and blends; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 102 | X86 Fixup Vector Constants | N/A |  |  | vector constants; no vectors. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 103 | Compressing EVEX instrs when possible | N/A |  |  | AVX-512 only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 104 | X86 insert wait instruction | N/A |  |  | strict floating point, never emitted. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 105 | Contiguously Lay Out Funclets | N/A |  |  | Windows funclet EH only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 106 | Remove Loads Into Fake Uses | N/A |  |  | fake uses come from `-fextend-lifetimes`. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 107 | Live DEBUG_VALUE analysis | N/A |  |  | debug values; see above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 108 | Machine Sanitizer Binary Metadata | N/A |  |  | off without the sanitizer-metadata attribute. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 109 | X86 Speculative Execution Side Effect Suppression | N/A |  |  | off unless -mseses. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 110 | X86 Indirect Thunks | N/A |  |  | retpolines, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 111 | X86 Return Thunks | N/A |  |  | `-mfunction-return=thunk`, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 112 | Check CFA info and insert CFI instructions if needed | EXECUTED | cfi.rs | `-disable-cfi-fixup` | unwind info from cfi.rs. Pin: changed code in 33 (-O2) and 28 (-Os) of 77 IRs |
| 113 | X86 Load Value Injection (LVI) Ret-Hardening | N/A |  |  | off unless -mlvi-cfi. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 114 | Pseudo Probe Inserter | N/A |  |  | sample-profile probes, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 115 | Unpack machine instruction bundles | N/A |  |  | llrm has no instruction bundles. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |
| 116 | X86 Assembly Printer | EXECUTED | masm.rs, objbuild.rs |  | the object is written directly. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs |

{"DEVIATE": 3, "EXECUTED": 16, "MATCH": 5, "MISSING": 27, "N/A": 65}
