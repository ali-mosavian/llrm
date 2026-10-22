# Codegen slot table

The pin's `llc` pipeline (d1106deb71cc, `-debug-pass=Structure`) against llrm's backend. i486 at m32 and code16 at m16, -O1, -O2, -O3 and -Os: the eight fixtures (tests/fixtures/pin-codegen, made by tools/pairs/pin_codegen.py) list the same passes in the same order, so there is one table; the passes decide by level inside. Analyses are left out (their consumers are rows). `codegen_slots_tests.rs` refuses a pass without a row and a row without its reason. Regenerate: `LLRM_CODEGEN_SLOT_TABLE=docs/working/codegen-slot-table.md cargo test -p llrm-transforms --lib codegen_slots_tests`, then re-add this header.

States: MATCH (a translation), EXECUTED (ours-own code does the work), DEVIATE (numbered), MISSING (owner; date), N/A (evidence). A MISSING row owned by `funcattrs-port; 2026-10-25` is unsettled: the ablation (the pin's own `-disable-*` switch on the 12 bench IRs and 64 QCport modules, object bytes and instructions) turns it N/A or gives it an owner. Two rows disagree with the parity registry (`Merge disjoint stack slots`, `Shrink Wrapping analysis`: ours exist, the registry says Missing); the ablation reconciles them.

| # | Pin pass | State | Ours | Pin switch | Why |
|---|---|---|---|---|---|
| 1 | ObjC ARC contraction | N/A |  |  | no Objective-C: the pass acts on retain/release calls the frontends never emit |
| 2 | Pre-ISel Intrinsic Lowering | EXECUTED | isel/expand.rs, hir lowering |  | memory and math intrinsics lowered by the selector |
| 3 | Expand IR instructions | N/A |  |  | acts on fp-to-int/div of more than 128 bits; MIR's widest integer is i64 (lower_int64.rs) |
| 4 | Expand Atomic instructions | N/A |  |  | MIR has no atomic or fence opcode (opcode.rs) |
| 5 | Lower AMX intrinsics | N/A |  |  | AMX is a 64-bit extension; target features exclude it |
| 6 | Lower AMX type for load/store | N/A |  |  | as above |
| 7 | Canonicalize natural loops | EXECUTED | loopsimplify.rs (mid-end) |  | loop-simplify runs in the mid-end; LSR relies on it |
| 8 | Canonicalize Freeze Instructions in Loops | MISSING |  |  | funcattrs-port; 2026-11-13: MIR has `freeze`; LSR's canon-freeze is not run |
| 9 | Induction Variable Users | MATCH | ivusers.rs |  | translated |
| 10 | Loop Strength Reduction | MATCH | lsr_*.rs (registry `lsr`) | `-disable-lsr` | translated; runs at the end of the mid-end, the pin's runs after it |
| 11 | Lower Garbage Collection Instructions | N/A |  |  | no GC strategy in any frontend |
| 12 | Shadow Stack GC Lowering | N/A |  |  | as above |
| 13 | Remove unreachable blocks from the CFG | EXECUTED | simplifycfg.rs, adce.rs |  | removed in the mid-end before selection |
| 14 | Constant Hoisting | MISSING |  | `-disable-constant-hoisting` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 15 | Replace intrinsics with calls to vector library | N/A |  | `-disable-replace-with-vec-lib` | no vector library, no vectors in MIR (deferred) |
| 16 | Partially inline calls to library functions | MISSING |  | `-disable-partial-libcall-inlining` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 17 | Instrument function entry/exit with calls to e.g. mcount() (post inlining) | N/A |  |  | acts on the `instrument-function-entry-inlined` attribute, never set |
| 18 | Scalarize Masked Memory Intrinsics | N/A |  |  | no masked intrinsics in MIR |
| 19 | Expand reduction intrinsics | N/A |  | `-disable-expand-reductions` | no vector reductions in MIR |
| 20 | Interleaved Access Pass | N/A |  |  | no vectors in MIR |
| 21 | X86 Partial Reduction | N/A |  |  | SSE/AVX dot-product patterns; features -sse |
| 22 | Expand indirectbr instructions | N/A |  |  | MIR has no indirectbr |
| 23 | CodeGen Prepare | MISSING | addresssink.rs (sinkCmpExpression, address sinking), extload.rs (ext-load promotion) | `-disable-cgp` | funcattrs-port; 2026-10-25: partial; the rest (select2branch, branch opts, despeculate, tail-call dup, store-extract) settled by the ablation |
| 24 | Exception handling preparation | EXECUTED | ehprepare.rs |  | dwarf-eh-prepare equivalent |
| 25 | Prepare inline asm insts | N/A |  |  | inline asm is handled by inline_asm.rs; the pass only rewrites constraints for callbr |
| 26 | Safe Stack instrumentation pass | N/A |  |  | acts on the `safestack` attribute, never set |
| 27 | Insert stack protectors | N/A |  |  | acts on `ssp*` attributes, never set |
| 28 | X86 DAG->DAG Instruction Selection | DEVIATE | isel.rs, isel/matcher.rs |  | CG1: llrm selects from MIR with its own rule matcher, no SelectionDAG; reason: segmented targets and 16-bit pointers; cost: the rows; exit: none planned; owner oversight; review 2026-11-13 |
| 29 | Local Dynamic TLS Access Clean-up | N/A |  |  | no thread-local storage in any frontend |
| 30 | X86 PIC Global Base Reg Initialization | N/A |  |  | no PIC code at m32 OMF/COFF or m16 |
| 31 | Argument Stack Rebase | N/A |  |  | acts on swiftasync frames only |
| 32 | Finalize ISel and expand pseudo-instructions | EXECUTED | isel.rs |  | the selector emits final instructions |
| 33 | X86 Domain Reassignment Pass | N/A |  | `-disable-x86-domain-reassignment` | SSE/AVX mask domains; features -sse |
| 34 | Early Tail Duplication | MISSING | jumps/ (late tail duplication only) | `-disable-early-taildup` | lir-cleanup-port; 2026-11-13: registry slot early-tailduplication |
| 35 | Optimize machine instruction PHIs | MATCH | optimizephis.rs |  | translated (registry optimizephis@1) |
| 36 | Merge disjoint stack slots | EXECUTED | lifetimes.rs, slots.rs |  | slots of locals with non-overlapping lifetime markers share; the registry says Missing: reconcile at the ablation |
| 37 | Local Stack Slot Allocation | N/A |  |  | X86 does not request virtual frame base registers (requiresVirtualBaseRegisters is false) |
| 38 | Remove dead machine instructions | EXECUTED | dead-code elimination in MIR; machinedce.rs after allocation | `-disable-machine-dce` | before register allocation MIR already removed the dead |
| 39 | Early If-Conversion | MISSING |  | `-disable-early-ifcvt` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 40 | Machine InstCombiner | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 41 | X86 cmov Conversion | N/A |  |  | the pass returns at once without CMOV, which the i486 does not have (selects.rs makes branches) |
| 42 | Early Machine Loop Invariant Code Motion | MISSING | licm (mid-end), loopslots.rs for reloads | `-disable-machine-licm` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 43 | Machine Common Subexpression Elimination | MISSING | precse.rs (before phi elimination), machinecse.rs (after allocation) | `-disable-machine-cse` | funcattrs-port; 2026-10-25: registry machinecse@4 is partial; untranslated PerformSimplePRE and invariant-load CSE |
| 44 | Machine code sinking | MISSING | machinesink.rs | `-disable-machine-sink` | funcattrs-port; 2026-10-25: registry machinesink@4 is partial; store and load sinking, SinkAndFold |
| 45 | Peephole Optimizations | MISSING | peephole.rs and peep/ | `-disable-peephole` | lir-cleanup-port; 2026-11-13: registry peephole@14 is ours-own, not a translation |
| 46 | Remove dead machine instructions (run 2) | EXECUTED | machinedce.rs | `-disable-machine-dce` | as above, after the peephole |
| 47 | Live Range Shrink | MISSING | pressuresink.rs stands in |  | lir-cleanup-port; 2026-11-13: registry pressuresink@5 |
| 48 | X86 Fixup SetCC | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 49 | X86 LEA Optimize | MISSING |  | `-disable-x86-lea-opt` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 50 | X86 Optimize Call Frame | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 51 | X86 Avoid Store Forwarding Blocks | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 52 | X86 Suppress APX features for relocation | N/A |  |  | APX is 64-bit; features exclude it |
| 53 | X86 speculative load hardening | N/A |  |  | acts on the `speculative_load_hardening` attribute, never set |
| 54 | X86 EFLAGS copy lowering | MISSING | flag lanes (liveness.rs), stepflags.rs |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 55 | X86 DynAlloca Expander | MISSING |  |  | funcattrs-port; 2026-11-09: dynamic alloca needs an HIR op (builtins row B2) |
| 56 | Tile Register Pre-configure | N/A |  |  | AMX; features exclude it |
| 57 | Detect Dead Lanes | MISSING | lanes.rs |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 58 | Init Undef Pass | N/A |  |  | acts only where a register class has a pseudo init-undef (RISC-V); X86 has none |
| 59 | Process Implicit Definitions | MISSING |  |  | funcattrs-port; 2026-11-13: tied to MIR undef (LIR has no IMPLICIT_DEF; uninitialized reads are zero) |
| 60 | Remove unreachable machine basic blocks | EXECUTED | jumps.rs |  | removed with the layout |
| 61 | Eliminate PHI nodes for register allocation | MISSING | phielim.rs, phielimination.rs |  | lir-cleanup-port; 2026-11-13: registry phielim@4 is ours-own |
| 62 | Two-Address instruction pass | MISSING | twoaddr.rs |  | lir-cleanup-port; 2026-11-13: registry twoaddr@9 |
| 63 | Register Coalescer | MISSING | coalesce.rs, coalescer/ |  | regalloc-core-poc; 2026-11-13: registry coalesce@10 is partial |
| 64 | Rename Disconnected Subregister Components | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 65 | Machine Instruction Scheduler | MISSING | pressuresink.rs stands in | `-enable-misched=0` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 66 | Greedy Register Allocator | MISSING | allocate.rs, greedy/, splitkit | `-regalloc=fast` | regalloc-core-poc; 2026-11-13: registry allocate@11 is partial |
| 67 | Tile Register Configure | N/A |  |  | AMX; features exclude it |
| 68 | Greedy Register Allocator (run 2) | N/A |  |  | the tile-register instance: AMX only |
| 69 | Virtual Register Rewriter | MISSING | allocate.rs, parcopy.rs |  | regalloc-core-poc; 2026-11-13 |
| 70 | Stack Slot Coloring | EXECUTED | slots.rs | `-disable-ssc` | frame slots of spilled values share when never live together; matched at the ablation |
| 71 | Machine Copy Propagation Pass | MATCH | machinecp.rs | `-disable-copyprop` | translated, run inside the peephole |
| 72 | Machine Loop Invariant Code Motion | MISSING | loopslots.rs for reloads | `-disable-postra-machine-licm` | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 73 | X86 Lower Tile Copy | N/A |  |  | AMX; features exclude it |
| 74 | X86 FP Stackifier | MATCH | floatalloc.rs |  | translated |
| 75 | X86 Load Value Injection (LVI) Load Hardening | N/A |  |  | off unless -mlvi-hardening |
| 76 | Remove Redundant DEBUG_VALUE analysis | N/A |  |  | debug values; llrm's debuginfo.rs works from source positions |
| 77 | Fixup Statepoint Caller Saved | N/A |  |  | no statepoints |
| 78 | PostRA Machine Sink | EXECUTED | postrasink.rs | `-disable-postra-machine-sink` | a copy or spill into the one successor that needs it |
| 79 | Shrink Wrapping analysis | EXECUTED | saverestore.rs, shrinkwrap.rs | `-enable-shrink-wrap=0` | follows ShrinkWrapImpl; the registry says Missing: reconcile at the ablation |
| 80 | Prologue/Epilogue Insertion & Frame Finalization | MISSING | prologue.rs, masm.rs |  | lir-cleanup-port; 2026-11-13: registry prologue@13 |
| 81 | Machine Late Instructions Cleanup Pass | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 82 | Control Flow Optimizer | MISSING | jumps/branchfold.rs | `-disable-branch-fold` | lir-cleanup-port; 2026-11-13: registry jumps@17 is partial |
| 83 | Tail Duplication | MISSING | jumps/  | `-disable-tail-duplicate` | lir-cleanup-port; 2026-11-13: registry jumps@17 |
| 84 | Machine Copy Propagation Pass (run 2) | MISSING |  | `-disable-copyprop` | funcattrs-port; 2026-10-25: llrm runs it once; the pin runs it again after tail duplication |
| 85 | Post-RA pseudo instruction expansion pass | EXECUTED | isel.rs |  | the selector emits final instructions |
| 86 | X86 pseudo instruction expansion pass | EXECUTED | isel.rs |  | as above |
| 87 | Insert KCFI indirect call checks | N/A |  |  | acts on the `kcfi` module flag, never set |
| 88 | Post RA top-down list latency scheduler | MISSING | schedule.rs (narrower) | `-disable-post-ra` | lir-cleanup-port; 2026-11-13: registry schedule@16 is ours-own |
| 89 | Analyze Machine Code For Garbage Collection | N/A |  |  | no GC strategy |
| 90 | Branch Probability Basic Block Placement | MISSING | jumps/placement | `-disable-block-placement` | lir-cleanup-port; 2026-11-13: registry jumps@17, chains and loops (#2629) |
| 91 | Insert fentry calls | N/A |  |  | acts on the `fentry-call` attribute, never set |
| 92 | Insert XRay ops | N/A |  |  | acts on XRay attributes, never set |
| 93 | Implement the 'patchable-function' attribute | N/A |  |  | acts on `patchable-function`, never set |
| 94 | X86 Execution Dependency Fix | N/A |  |  | SSE execution domains; features -sse |
| 95 | BreakFalseDeps | N/A |  |  | partial-register and SSE false dependencies; the 486 and P5 have none |
| 96 | X86 Indirect Branch Tracking | N/A |  |  | CET; off without the `cf-protection-branch` flag |
| 97 | X86 vzeroupper inserter | N/A |  |  | AVX only |
| 98 | X86 Byte/Word Instruction Fixup | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 99 | X86 Atom pad short functions | N/A |  |  | acts on Atom (`padShortFunctions`); not i486 |
| 100 | X86 LEA Fixup | MISSING |  |  | funcattrs-port; 2026-10-25: unsettled until the ablation |
| 101 | X86 Fixup Inst Tuning | N/A |  |  | tunes SSE shuffles and blends; features -sse |
| 102 | X86 Fixup Vector Constants | N/A |  |  | vector constants; no vectors |
| 103 | Compressing EVEX instrs when possible | N/A |  |  | AVX-512 only |
| 104 | X86 insert wait instruction | N/A |  |  | strict floating point, never emitted |
| 105 | Contiguously Lay Out Funclets | N/A |  |  | Windows funclet EH only |
| 106 | Remove Loads Into Fake Uses | N/A |  |  | fake uses come from `-fextend-lifetimes` |
| 107 | Live DEBUG_VALUE analysis | N/A |  |  | debug values; see above |
| 108 | Machine Sanitizer Binary Metadata | N/A |  |  | off without the sanitizer-metadata attribute |
| 109 | X86 Speculative Execution Side Effect Suppression | N/A |  |  | off unless -mseses |
| 110 | X86 Indirect Thunks | N/A |  |  | retpolines, never requested |
| 111 | X86 Return Thunks | N/A |  |  | `-mfunction-return=thunk`, never requested |
| 112 | Check CFA info and insert CFI instructions if needed | EXECUTED | cfi.rs | `-disable-cfi-fixup` | unwind info from cfi.rs |
| 113 | X86 Load Value Injection (LVI) Ret-Hardening | N/A |  |  | off unless -mlvi-cfi |
| 114 | Pseudo Probe Inserter | N/A |  |  | sample-profile probes, never requested |
| 115 | Unpack machine instruction bundles | N/A |  |  | llrm has no instruction bundles |
| 116 | X86 Assembly Printer | EXECUTED | masm.rs, objbuild.rs |  | the object is written directly |

{"DEVIATE": 1, "EXECUTED": 16, "MATCH": 5, "MISSING": 37, "N/A": 57}
