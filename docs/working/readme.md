# Working on llrm

For contributors: running tests, finding a miscompile, measuring, and what has broken before.

| Doc | Holds |
|---|---|
| [llrm-cc-measure](llrm-cc-measure.md) | What llrm-cc costs and emits beside the pin: `tools/cc_cost.py`, method, rows, withdrawn tables |
| [early-taildup-plan](early-taildup-plan.md) | Early Tail Duplication translation: pin functions and lines, what LIR gives it, three steps with dates (2026-11-03..08) |
| [constraint-elimination-plan](constraint-elimination-plan.md) | ConstraintElimination translation: why, the order CE-1 to CE-4 with dates, the oracle |
| [knownbits-plan](knownbits-plan.md) | ValueTracking known bits (qbq-llrm d_faces): per caller against the pin, twelve dated rows (2026-10-24) |
| [f3-plan](f3-plan.md) | F3, `translate::program`: profile by bucket, the pin's counterpart, four rows with dates (2026-10-24) |
| [optimizer-gap-summed](optimizer-gap-summed.md) | Optimizer gap on d_faces -O2: the pin's Ir summed over every call site, ours against it |
| [scheduler-live-intervals](scheduler-live-intervals.md) | MachineScheduler on the shared LiveIntervals: design note, 16.9 M Ir against the pin's 10.4 M |
| [pass-table-dist](pass-table-dist.md) | Optimizer passes on dist, ours against the pin: Ir, runs, excess and owners |
| [presize-plan](presize-plan.md) | Vectors that grow by doubling to a size known up front: sites, growth Ir, capacities (2026-10-24) |
| [f2-plan](f2-plan.md) | F2 `emit_module`: profile, pin counterpart, four rows |
| [alias-stack-plan](alias-stack-plan.md) | Alias stack: module-by-module route to the pin's BasicAA/TBAA/CaptureTracking structure, Ir budget per stage |
| [count-to-zero-plan](count-to-zero-plan.md) | Count-to-zero cost term in LSR, read from the target cost table: findings and rows AM-CZ-1 (x_bubble m32), AM-CZ-2 (PLASMA m16) |
| [module-ipo-plan](module-ipo-plan.md) | Module walk and IPO against the pin: what `_sift` lacks in `-print-after-all`, rows by 2026-11-09 |
| [frontend-bucket-plan](frontend-bucket-plan.md) | Front end compile-time bucket (qbq-llrm d_faces): attribution and rows F1-F4 (2026-10-23) |
| [loop-bucket-callees](loop-bucket-callees.md) | Loop and scalar buckets: top 10 callees on d_faces -O2 with owner and date |
| [scev-predicates-plan](scev-predicates-plan.md) | SCEV predicates (S3 rank 5, DEVIATE 1): plan (2026-10-23) |
| [stable-loads](stable-loads.md) | D-I: stable-load rematerialization off by default; AM-DI rows, the argument rule, the exit |
| [regclass-on-body](regclass-on-body.md) | register classes carried on the body, as the pin's MRI: the rescans, the creators, the steps |
| [dag-refusals-2026-10-25](dag-refusals-2026-10-25.md) | Every corpus under `--isel=dag`: refusals and failed compiles by cause (0 at m16/m32; m64 `vla`) |
| [x86-64-dag-plan](x86-64-dag-plan.md) | The DAG path at m64, LEA first: D64-0..4 and the D64-1 design (2026-10-24) |
| [clocks-sweep](clocks-sweep.md) | Clocks of every runnable program at m16 -Os, main vs parity: the slower list, method, DOSBox check (2026-10-26) |
| [qb-runtime-contract-plan](qb-runtime-contract-plan.md) | The QB runtime's contract in the DAG path: five dated PRs (2026-10-24) |
| [bench-table-a33fdfff7](bench-table-a33fdfff7.md) | vsgcc bench table at tip a33fdfff7: old and dag selectors against min(gcc, clang) |
| [flip-packet](flip-packet.md) | The default flip to `--isel=dag`: the five conditions, status and evidence per condition |
| [greedy-switchover-plan](greedy-switchover-plan.md) | Switching over to the translated greedy allocator: what the old modules still provide (2026-10-22) |
| [dagisel-target-seam](dagisel-target-seam.md) | The `DagTarget` seam: shared `select_units`, the x86 and m68k implementations, the proof |
| [dagisel-seam-merge](dagisel-seam-merge.md) | Merge order and conflicts of the DAG seam with the shared legalizer stack |
| [m68k-enabler-series](m68k-enabler-series.md) | The shared enablers the m68k backend needs, in landing order |
| [m68k-block-placement-plan](m68k-block-placement-plan.md) | Block placement for m68k: the pin's `MachineBlockPlacement` over the spike's machine function |
| [m68k parity](../../crates/target/llrm-m68k/PARITY.md) | m68k against the pin: numbered deviations, what the removed overlay bought per piece, open rows |
| [m68k-isa](m68k-isa.md) | The 68k family opcode table: how it is checked against binutils and the manual, numbered limits, sources |
| [m68k-timing](m68k-timing.md) | Clocks per CPU of the 68k family: tables used, how they were checked, numbered limits |
| [m68k-timing-cycles](m68k-timing-cycles.md) | The kernels priced on each CPU of the 68k family |
| [testing](testing.md) | Running the tests |
| [pr-rules](pr-rules.md) | The goal, what a PR carries, how it lands, how to measure, safety |
| [gate](gate.md) | What CI and the gate run, the tiers, what must pass |
| [ci](ci.md) | CI as the gate: parity with `gate.py run`, private tools, instruction counters |
| [pgo](pgo.md) | the profile-guided llrm-c: how it is built, when its profile is renewed, how to measure with it |
| [debugging](debugging.md) | Finding a miscompile in a DOS program, the debug socket and debug symbols |
| [measurement](measurement/readme.md) | How we measure, the instruments, compile time, comparison with gcc |
| [fragility](fragility.md) | What broke, or hid a break, and what guards it |
| [figures](figures.md) | Drawing a figure as mermaid, and the generated pass pages |
| [c1-lanes-plan](c1-lanes-plan.md) | C1: sub-register lanes in live intervals: what differs, the lane model, five steps |
| [codegen-improvements](codegen-improvements.md) | Backend ideas drawn from other compilers |
| [fold](fold.md) | What the Fold pass costs and carries, and the plan to remove it |
| [fold-split](fold-split.md) | The plan splitting the fold work between valuetracking-port and fold-poc |
| [value-tracking-port](value-tracking-port.md) | Evidence for the ValueTracking, KnownBits and ConstantRange port: agreement with LLVM, deviations, owed work |
| [undef-table](undef-table.md) | MIR `undef`: each pass and pin function that treats it apart from poison, and its state |
| [llrm-only-inventory](llrm-only-inventory.md) | Passes and analyses with no pin counterpart in the d_faces -O2 profile: Ir, why, delete or translate first (2026-10-25) |
| [residual-split](residual-split.md) | The 59 M of ledger residual outside the buckets, per item Ir, pin counterpart, verdict (2026-10-25) |
| [llrm-only-rules](llrm-only-rules.md) | One rule per llrm-only site over 2 M: DELETE, REPLACE or DEVIATE, owner, expected M (2026-10-25) |
| [instcombine-like-for-like](instcombine-like-for-like.md) | InstCombine on the same IR against the pin: per function, per run, the cost outside the loop (d_faces -O2, 2026-10-25) |
| [icmp-canonical-cells](icmp-canonical-cells.md) | `canonicalizeICmpPredicate`: the cells that grew when compares took the pin's canonical form, with causes |
| [layout-fidelity](layout-fidelity.md) | Where the pin lays out the blocks its helpers create, and which of ours match; the cells that grew when ours moved |
| [qb-float-rows](qb-float-rows.md) | The 41 QB float rows above main after BC's per-assignment rounding (#3352): the numbers, owner and exit |
| [qb-lalr-handoff](qb-lalr-handoff.md) | The QB front takeover: what landed and what is left |
| [qb-unsupported-lowering](qb-unsupported-lowering.md) | BC output for the lowerings the QB front refused: PALETTE USING, ON ERROR RESUME NEXT, REDIM PRESERVE |
| [qb-currency](qb-currency.md) | The CURRENCY type: what PDS BC calls, the design, three phases |
| [qb-vbdos-forms](qb-vbdos-forms.md) | VBDOS forms: what the front needs for FORM and CONTROL types, and what stays deferred |
| [qcport-os-rows](qcport-os-rows.md) | The QCport modules bigger at -Os after #3369 and after its re-land: numbers, owner and exit |
| [phi-pre-ra-translation](phi-pre-ra-translation.md) | PHIElimination, TwoAddressInstruction and the pass order before greedy, against the pin: same, differs, missing |
| [livevariables-plan](livevariables-plan.md) | The pin's LiveVariables translated for TwoAddress and PHIElimination: functions, callers, dates (2026-10-24) |
| [twoaddress-translation-plan](twoaddress-translation-plan.md) | Translating TwoAddressInstructionPass: the copy-count premise, the data LIR lacks, seven pieces in the pin's order |
| [process-implicit-defs-premise](process-implicit-defs-premise.md) | ProcessImplicitDefs: what the pin's pass does (IMPLICIT_DEF to undef on 16 to 32 widening), ours, the extends compared |
| [vreg-class-field-plan](vreg-class-field-plan.md) | A register class on each virtual register: pin sites, touched types, caller counts, order |
| [local-reassign](local-reassign.md) | `canReassign` in the greedy allocator: the QCport -Os modules that change, vs parity and main |
| [qcport-vs-main](qcport-vs-main.md) | QCport -Os and -O2, parity against main, per module, with the owner of each module above main |
| [flip-ambt-table](flip-ambt-table.md) | The 22 AM-BT rows by dag/old and dag/pin at m32 -O2: verdict, cause and owner per row (2026-10-25) |
| [flip-ambt-rows-8-14](flip-ambt-rows-8-14.md) | Rows 8-14 re-run on tip c76f9a6a5 with the pin's i486 triple: old, dag and pin llc clocks, instructions and code bytes (2026-10-26) |
| [bsf-bsr-cost-row](bsf-bsr-cost-row.md) | BSF/BSR cost row and the pin's `rep` prefix rule on BSF (2026-10-26) |
| [combine-forward-load-rows.md](combine-forward-load-rows.md) | m16 -Os growers of the load forwarding in combine (#5008) |
| [combine-fsub-rows.md](combine-fsub-rows.md) | QCport growers of `fsub x, C` -> `fadd x, -C` (#5642) |
| [simplifycfg-resimplify-rows.md](simplifycfg-resimplify-rows.md) | QCport growers of SimplifyCFG's repeated block visit (#5662) |
| [icmp-mul-constant-rows.md](icmp-mul-constant-rows.md) | QCport growers of `foldICmpMulConstant` (#5670) |
| [lsr-pin-faithful-rows.md](lsr-pin-faithful-rows.md) | loop-reduce rows at m16 -Os that equal the pin on the same IR: TL-1 tile i6, TL-2 crc.c (#5013) |
| [compile-ledger-w1](compile-ledger-w1.md) | Plan for the regalloc-glue and scalar buckets of the compile ledger: ours vs the pin per pass, callees, levers and dates |
| [unroll-o1-rows](unroll-o1-rows.md) | x_life and x_isqrt against main: the -O1 unroll mechanism and the select diamond, both the pin's |
| [pin-comparison-checklist](pin-comparison-checklist.md) | Five lines every pin comparison states (input stage, target header, optsize, convention, pre-RA difference), and the internal-function convention as a DEVIATE |
| [above-main-rows](above-main-rows.md) | Rows above main |
| [accesses-basicaa-plan](accesses-basicaa-plan.md) | MemorySSA Accesses: BasicAA per query (exit of DEVIATE AN1) |
| [backend-rows-plan](backend-rows-plan.md) | Backend rows to translate: 82, 34/83, 90, 84, 42 — w-3, plan note 2026-10-22 |
| [bpi-plan](bpi-plan.md) | BranchProbabilityInfo, MachineBranchProbabilityInfo, MBFI: plan (2026-10-22) |
| [callframe-translation-plan](callframe-translation-plan.md) | Translating X86CallFrameOptimization (slot 50) |
| [codegen-ablation](codegen-ablation.md) | Codegen ablation |
| [codegen-rows-freeze-dynalloca-latecleanup](codegen-rows-freeze-dynalloca-latecleanup.md) | Plan: three codegen rows (funcattrs-port) |
| [codegen-rows-machinecse-combiner](codegen-rows-machinecse-combiner.md) | Plan: Machine CSE and Machine InstCombiner rows (funcattrs-port) |
| [codegenprepare](codegenprepare.md) | CodeGenPrepare: the pin's transforms against ours |
| [commute-load-clock-rows](commute-load-clock-rows.md) | commute-load clock rows (#3874) |
| [count-to-zero-xbubble-trial](count-to-zero-xbubble-trial.md) | x_bubble zero-end counter generator: trial (2026-10-22) |
| [early-machinelicm](early-machinelicm.md) | Early MachineLICM, pressure-gated (codegen row 42) — w-3, plan note 2026-11-06 |
| [fold-retirement](fold-retirement.md) | Retiring `fold` slot by slot |
| [post-parity-tuning](post-parity-tuning.md) | Where llrm may differ from the pin after parity, and how: -Os inlining |
| [forged-sel-2](forged-sel-2.md) | FORGED-SEL-2: what a forged selector reaches |
| [fp-constant-fold-plan](fp-constant-fold-plan.md) | FP binary constant fold: plan (2026-10-22) |
| [fpround-pricing](fpround-pricing.md) | FPRound is priced as one instruction |
| [floatprep-into-selection-plan](floatprep-into-selection-plan.md) | FloatPrep into selection: the plan |
| [gate-coverage](gate-coverage.md) | What the landing gate runs, and what it does not |
| [instruction-gap-vs-pin](instruction-gap-vs-pin.md) | Instructions against the pin's llc, on the same IR |
| [irreducible-counts](irreducible-counts.md) | Irreducible cycles in the corpora (BRIDGE-1) |
| [joins-window](joins-window.md) | joins/fjoins steps over the slope limit at N=128 and N=256 (2026-10-28); the N=64 window rows are in `tools/budgets.toml` |
| [homes-spares](homes-spares.md) | Homes and Spares against the pin's InlineSpiller: what each decides, measured effect, plan to 11-17 |
| [intervals-once](intervals-once.md) | Live intervals built once, as the pin does: three builders today, steps, owners, dates |
| [ivusers-order](ivusers-order.md) | IVUsers user order (IU1) |
| [lazy-annotated-plan](lazy-annotated-plan.md) | Lazy Annotated and the MemorySSA cost: plan |
| [lea-fixup-plan](lea-fixup-plan.md) | X86 LEA Fixup (row 100): plan |
| [ledger-command](ledger-command.md) | The ledger command |
| [cost-centers](cost-centers.md) | Self Ir by symbol (hashing, memcpy, Vec growth, malloc), ours against the pin |
| [algebraic-696-note](algebraic-696-note.md) | algebraic@696 deleted: the pair by bucket, the costs and what is owed |
| [qcport-o2-size](qcport-o2-size.md) | AM-QC-1: QCport -O2 size by cause (unrolling, inlining) and module |
| [cleanup-bucket-by-pass](cleanup-bucket-by-pass.md) | The machine clean-up bucket (172.3 M vs the pin's 49.4) one row per pass, with a proposed owner |
| [gap-like-for-like](gap-like-for-like.md) | The d_faces m32 -O2 gap to the pin by group, with the isel and combine work counted once, and an owner per row |
| [compile-time-plan-scalar-ipo](compile-time-plan-scalar-ipo.md) | Scalar and module/IPO compile time: dated plan to the pin |
| [emit-baselines](emit-baselines.md) | Compile costs measured to an object: old and new |
| [emit-plan](emit-plan.md) | Emit compile-time bucket: dated plan to the pin's AsmPrinter and MC |
| [loops-compile-plan](loops-compile-plan.md) | Loops compile-time bucket: dated plan to 1.0x of the pin |
| [licm-compile-cost](licm-compile-cost.md) | LICM and loop-pass compile cost (w-3, plan note 2026-10-22) |
| [live-range-shrink](live-range-shrink.md) | Live Range Shrink (codegen slot 47) |
| [liveness-reuse-plan](liveness-reuse-plan.md) | One lane solve serving flags: built and dropped, 0.50 M on `d_faces` |
| [llrm-c-reds](llrm-c-reds.md) | The llrm-c lib reds (`cargo test --release -p llrm-c --lib --features toolchain`) |
| [llrm-only-passes](llrm-only-passes.md) | llrm-only MIR passes bucket (d_faces -O2 m32, tip 3b5ac93b68) |
| [lsr-address-registers](lsr-address-registers.md) | LSR address-register term (LU2): tried, not landed |
| [lsr-callees](lsr-callees.md) | LSR callees against the pin, same IR — w-3, 2026-10-22 |
| [machine-scheduler-plan](machine-scheduler-plan.md) | Machine Instruction Scheduler (codegen row 65) |
| [machine-cleanup-compile-plan](machine-cleanup-compile-plan.md) | Machine clean-up compile time: plan to 1.0x of the pin |
| [shared-liveness-poc](shared-liveness-poc.md) | Shared liveness across the scheduler and allocator: worth under 0.5% of the compile |
| [machinecombiner-translation-plan](machinecombiner-translation-plan.md) | Translating MachineCombiner (slot 40) |
| [old-allocator-modules](old-allocator-modules.md) | Old allocator modules: last callers and the PR that deletes each |
| [peephole-vs-pin](peephole-vs-pin.md) | Peephole cost by sub-pass against the pin on d_faces -O2 (2026-10-25) |
| [pei-plan](pei-plan.md) | Prologue/Epilogue Insertion and Frame Finalization (row 80): plan |
| [phielimination-completion-plan](phielimination-completion-plan.md) | PHIElimination: completing the translation (row 61) |
| [pin-slot-table](pin-slot-table.md) | Pin pipeline slots against llrm's pipeline |
| [premise-peephole-sink-scheduler](premise-peephole-sink-scheduler.md) | Premise tests: Peephole, MachineSink, Machine Scheduler |
| [process-implicit-defs-plan](process-implicit-defs-plan.md) | ProcessImplicitDefs: the translation |
| [qb-host](qb-host.md) | The QB runtime on a Linux or macOS host with SDL: layout, build, what was checked, what is missing (#4669) |
| [x-base64-ties](x-base64-ties.md) | x_base64: the two ties that flip in the shared-LiveIntervals landing |
| [twoaddress-cost-plan](twoaddress-cost-plan.md) | TwoAddress and LiveIntervals compile cost against the pin: callgrind attribution, four steps |
| [twoaddress-rest-plan](twoaddress-rest-plan.md) | Row 62: what of TwoAddressInstructionPass is left |
| [row23-plan](row23-plan.md) | Row 23: CodeGenPrepare translation plan |
| [row43-plan](row43-plan.md) | Row 43: Machine CSE, what is left |
| [earlycse-snapshot-deviate](earlycse-snapshot-deviate.md) | EarlyCse forks the function and context where the pin mutates in place: DEVIATE EC-1, about 6.8 M |
| [first-divergence-m32](first-divergence-m32.md) | First divergence from the pin for 71 programs, m32 -O2, counted by cause (2026-10-25) |
| [row44-plan](row44-plan.md) | Row 44: MachineSink translation plan |
| [row45-cost-plan](row45-cost-plan.md) | The Peephole's compile cost by sub-pass at the tip, the LLVM home of each, rows R45-1 to R45-6 with dates (2026-10-24) |
| [row45-plan](row45-plan.md) | Row 45: PeepholeOptimizer translation plan |
| [fp-stackifier-plan](fp-stackifier-plan.md) | X86FPStackifier: compile-cost plan |
| [mssa-build-plan](mssa-build-plan.md) | MemorySSA build cost: the pin builds as often, 11x cheaper per build |
| [mssa-lazy-refs-design](mssa-lazy-refs-design.md) | MemorySSA clients: references made when asked (design, priced) |
| [row61-deleted-tests](row61-deleted-tests.md) | Row 61: the tests deleted with the non-RFP route |
| [row63-66-plan](row63-66-plan.md) | Rows 63 (RegisterCoalescer) and 66 (RAGreedy): translation plan |
| [row82-plan](row82-plan.md) | Row 82, Control Flow Optimizer: translation plan |
| [scev-predicates-plan](scev-predicates-plan.md) | SCEV predicates (S3 rank 5): plan |
| [scev-s3-stubs](scev-s3-stubs.md) | SCEV stubs (S3), ranked |
| [segment-cast-audit](segment-cast-audit.md) | Casts through the selector space: audit |
| [selectiondag-c2-plan](selectiondag-c2-plan.md) | C2: m16 patterns for the DAG selector |
| [selectiondag-switch-plan](selectiondag-switch-plan.md) | SelectionDAG switch plan, revised 2026-10-22 |
| [selectiondag-builder-gaps](selectiondag-builder-gaps.md) | What the pin's `visit*` has that the DAG builder lacks (sdag-a) |
| [selectiondag-far-builder](selectiondag-far-builder.md) | Far and huge pointers in the DAG builder (sdag-a) |
| [old-selector-reds](old-selector-reds.md) | The 11 old-selector tests ignored since #4992, with the delta and m16 size of each |
| [old-selector-retirement](old-selector-retirement.md) | What the old selector leaves to retire after the DAG flip, surveyed by reading |
| [selectiondag-default-readiness](selectiondag-default-readiness.md) | What must be true to make `--isel=dag` the default (sdag-a) |
| [jumps-profile-d_faces](jumps-profile-d_faces.md) | Line profile of ControlFlow (`jumps`) on d_faces -O2 against the pin's BranchFolder (2026-10-25) |
| [landing-batch-time](landing-batch-time.md) | Where a landing batch's wall clock goes: stages, the long pole, changes that keep the gate (2026-10-25) |
| [d64-4-s3-inventory](d64-4-s3-inventory.md) | D64-4 S3 float programs for tests/llc and the pin asm each must match at m64 (sdag-a) |
| [selectiondag-tail-calls](selectiondag-tail-calls.md) | Tail calls in the DAG path: pin functions, steps, dates (sdag-a) |
| [selectiondag-huge-memmove](selectiondag-huge-memmove.md) | memmove of huge pointers: the pin's behaviour, options, decision date 10-30 (sdag-a) |
| [selectiondag-x87-contract](selectiondag-x87-contract.md) | What the DAG legalizer hands the x87 emitter and stackifier (sdag-b) |
| [setcc-chain-plan](setcc-chain-plan.md) | setcc chain vs branch (x_mergesort, stft bq32, nhstutil leaves) |
| [shared-liveintervals-plan](shared-liveintervals-plan.md) | TwoAddress on a shared LiveIntervals (row 62, row 62b) — owner w-1, due 2026-11-20 |
| [shared-se](shared-se.md) | One ScalarEvolution across the loop passes — w-3, plan note 2026-10-28 |
| [simplifycfg-jt-cvp-cost](simplifycfg-jt-cvp-cost.md) | SimplifyCFG + JumpThreading + CVP compile cost: ranked callees |
| [struct-copy-rows](struct-copy-rows.md) | Struct-copy rows R2 and R3 (#3621): cause |
| [tail-dup-placement](tail-dup-placement.md) | Tail duplication and block placement: premise tests — w-3, 2026-10-22 |
| [twoaddress-reschedule-plan](twoaddress-reschedule-plan.md) | TwoAddress: rescheduleMIBelowKill and rescheduleKillAboveMI |
| [x86-fixup-passes-plan](x86-fixup-passes-plan.md) | X86 Fixup SetCC (48), EFLAGS copy lowering (54), Byte/Word Instruction Fixup (98): plan (2026-10-22) |

## Docs with the change

A change to a pass, analysis, flag, level, tool, target or runtime updates its doc in the same PR. A pass's `//!` header is its doc; after editing one, run `tools/fmt.sh` (it rewraps header comments), then `python3 tools/docs/docsbuild/build.py --sync` and commit what it writes. A page of `docs/architecture/` joins its readme's Contents list by front matter (`architecture-index:` and `architecture-order:`, tools/docs/docsbuild/archindex.py); `build.py --sync` rewrites the list, and `--check` fails on a stale one. The tests (`pytest tools/docs/docsbuild`, `tests/header_mir.rs`) fail on a stale page, a missing section or a malformed MIR example.
| [selectiondag-plan](selectiondag-plan.md) | Translating SelectionDAG selection (slot row 28): the pin's structure, what replaces each part, the generated matcher, shadow mode then switch then delete, with owners and dates |
| [dag-assert-ext](dag-assert-ext.md) | `AssertZext`/`AssertSext` for extended arguments and results: why the DAG needs them, what drops them, SD43 |
| [regalloc-matrix-profile](regalloc-matrix-profile.md) | LiveRegMatrix cost on d_faces m32 -O2 against the pin: ~10.8 M of the 77.6 M allocator, ~4.5 M over the pin |
| [regpressure-plan](regpressure-plan.md) | Per-set register pressure for the pre-RA scheduler: tracker, limits, deltas; the half w2 calls |
| [alloc-by-caller](alloc-by-caller.md) | Allocation and copy cost of d_faces -O2 by caller: flat, 9.4% of Ir, no single cut; successors/predecessors do not allocate |
| [cgp-switch-type](cgp-switch-type.md) | `optimizeSwitchType`: a switch on i8/i16 is widened to i32 as the pin does; m16 keeps it narrow; byte evidence |
| [scaling-gate-rebaseline-2026-10-28](scaling-gate-rebaseline-2026-10-28.md) | The 43 `test_scaling_gate` tests re-baselined per test against the current step names: old to new mapping, bounds |
| [scev-persistent-se](scev-persistent-se.md) | One persistent SE across the loop passes: trip-count computes 7 -> 3, plan, files, risks |
