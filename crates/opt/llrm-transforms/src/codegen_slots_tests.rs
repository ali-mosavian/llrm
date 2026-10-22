//! The pin's `llc` pipeline (`-debug-pass=Structure`,
//! tests/fixtures/pin-codegen, made by tools/pairs/pin_codegen.py) against
//! llrm's backend: every pass that transforms has one row with a state, and
//! every state says why. The pipeline is the same at -O1, -O2, -O3 and -Os and
//! for m32 and m16 (-O0 alone differs); the passes decide inside.
//! `LLRM_CODEGEN_SLOT_TABLE=FILE` writes the table.

use std::collections::BTreeMap;

const FIXTURES: [(&str, &str); 8] = [
    ("m32 -O1", include_str!("../../../../tests/fixtures/pin-codegen/m32-O1.txt")),
    ("m32 -O2", include_str!("../../../../tests/fixtures/pin-codegen/m32-O2.txt")),
    ("m32 -O3", include_str!("../../../../tests/fixtures/pin-codegen/m32-O3.txt")),
    ("m32 -Os", include_str!("../../../../tests/fixtures/pin-codegen/m32-Os.txt")),
    ("m16 -O1", include_str!("../../../../tests/fixtures/pin-codegen/m16-O1.txt")),
    ("m16 -O2", include_str!("../../../../tests/fixtures/pin-codegen/m16-O2.txt")),
    ("m16 -O3", include_str!("../../../../tests/fixtures/pin-codegen/m16-O3.txt")),
    ("m16 -Os", include_str!("../../../../tests/fixtures/pin-codegen/m16-Os.txt")),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    /// Translated: `ours` names the module.
    Match,
    /// Run by a stage of ours that is not a translation of it; `ours` says
    /// which.
    Executed,
    /// A numbered deliberate difference (`CGn`), stated in the row.
    Deviate,
    /// Not translated: `why` is `owner; date` and what stands in.
    Missing,
    /// Not applicable at i486 and the 16-bit target; `why` is the evidence.
    Na,
}
use State::{Deviate, Executed, Match, Missing, Na};

struct Row {
    pass: &'static str,
    /// Which run of a pass the pipeline repeats, from 1.
    nth: usize,
    state: State,
    ours: &'static str,
    /// The pin's own switch that turns it off, for the ablation.
    switch: &'static str,
    why: &'static str,
}

const fn row(
    pass: &'static str,
    state: State,
    ours: &'static str,
    switch: &'static str,
    why: &'static str,
) -> Row {
    Row { pass, nth: 1, state, ours, switch, why }
}

const fn again(
    pass: &'static str,
    nth: usize,
    state: State,
    ours: &'static str,
    switch: &'static str,
    why: &'static str,
) -> Row {
    Row { pass, nth, state, ours, switch, why }
}

/// Analyses and managers: no output of their own, a row's input.
const ANALYSES: [&str; 33] = [
    "Analysis",
    "Tree Construction",
    "Information",
    "Info Analysis",
    "Verifier",
    "Frequency",
    "Probability Analysis",
    "Cycle Info",
    "Natural Loop",
    "Alias",
    "Tracker",
    "Slot index numbering",
    "Live Interval",
    "Live Variable",
    "Live Stack Slot",
    "Virtual Register Map",
    "Live Register Matrix",
    "Bundle Machine CFG Edges",
    "Spill Code Placement",
    "Optimization Remark Emitter",
    "Debug Variable",
    "Assignment Tracking",
    "Dominance Frontier",
    "Trace Metrics",
    "Class Info",
    "Dominator",
    "Free MachineFunction",
    "Reaching Definitions",
    "StackMap Liveness",
    "Stack Frame Layout",
    "Pass Manager",
    "Register Allocation Pass Scoring",
    "Library Function Lowering",
];

fn is_analysis(name: &str) -> bool {
    ANALYSES.iter().any(|one| name.contains(one)) && name != "Branch Probability Basic Block Placement"
}

/// The registry slot each translated or stood-in row cites exists.
const REGISTRY_OWNER: &str = "lir-cleanup-port";

const ROWS: &[Row] = &[
    // IR before selection (CodeGenPrepare's company).
    row(
        "ObjC ARC contraction",
        Na,
        "",
        "",
        "no Objective-C: the pass acts on retain/release calls the frontends never emit. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Pre-ISel Intrinsic Lowering",
        Executed,
        "isel/expand.rs, hir lowering",
        "",
        "memory and math intrinsics lowered by the selector. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Expand IR instructions",
        Na,
        "",
        "",
        "acts on fp-to-int/div of more than 128 bits; MIR's widest integer is i64 (lower_int64.rs). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Expand Atomic instructions",
        Na,
        "",
        "",
        "MIR has no atomic or fence opcode (opcode.rs). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Lower AMX intrinsics",
        Na,
        "",
        "",
        "AMX is a 64-bit extension; target features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row("Lower AMX type for load/store", Na, "", "", "as above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs"),
    row(
        "Canonicalize natural loops",
        Executed,
        "loopsimplify.rs (mid-end)",
        "",
        "loop-simplify runs in the mid-end; LSR relies on it. Pin: changed code in 59 (-O2) and 60 (-Os) of 77 IRs",
    ),
    row(
        "Canonicalize Freeze Instructions in Loops",
        Match,
        "canonfreeze.rs",
        "",
        "translated. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Induction Variable Users",
        Match,
        "ivusers.rs",
        "",
        "translated. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Loop Strength Reduction",
        Match,
        "lsr_*.rs (registry `lsr`)",
        "-disable-lsr",
        "translated; runs at the end of the mid-end, the pin's runs after it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-lsr -O2: -37 instr, +40 B, 61 IRs differ; -disable-lsr -Os: +283 instr, +876 B, 63 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 241 instr 1173 B, pin-with 255 instr 942 B, pin-without 251 instr 942 B (2 programs); -Os: ours 309 instr 959 B, pin-with 303 instr 925 B, pin-without 299 instr 922 B (3 programs)",
    ),
    row(
        "Lower Garbage Collection Instructions",
        Na,
        "",
        "",
        "no GC strategy in any frontend. Pin: changed code in 62 (-O2) and 64 (-Os) of 77 IRs",
    ),
    row("Shadow Stack GC Lowering", Na, "", "", "as above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs"),
    row(
        "Remove unreachable blocks from the CFG",
        Executed,
        "simplifycfg.rs, adce.rs",
        "",
        "removed in the mid-end before selection. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Constant Hoisting",
        Na,
        "",
        "-disable-constant-hoisting",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-constant-hoisting -O2: +0 instr, +0 B, 0 IRs differ; -disable-constant-hoisting -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    row(
        "Replace intrinsics with calls to vector library",
        Na,
        "",
        "-disable-replace-with-vec-lib",
        "no vector library, no vectors in MIR (deferred). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Partially inline calls to library functions",
        Na,
        "",
        "-disable-partial-libcall-inlining",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-partial-libcall-inlining -O2: +0 instr, +0 B, 0 IRs differ; -disable-partial-libcall-inlining -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    row(
        "Instrument function entry/exit with calls to e.g. mcount() (post inlining)",
        Na,
        "",
        "",
        "acts on the `instrument-function-entry-inlined` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Scalarize Masked Memory Intrinsics",
        Na,
        "",
        "",
        "no masked intrinsics in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Expand reduction intrinsics",
        Na,
        "",
        "-disable-expand-reductions",
        "no vector reductions in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row("Interleaved Access Pass", Na, "", "", "no vectors in MIR. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs"),
    row(
        "X86 Partial Reduction",
        Na,
        "",
        "",
        "SSE/AVX dot-product patterns; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Expand indirectbr instructions",
        Na,
        "",
        "",
        "MIR has no indirectbr. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "CodeGen Prepare",
        Executed,
        "addresssink.rs (sinkCmpExpression, address-phi sink), cgpaddr.rs (optimizeMemoryInst), extload.rs (ext-load promotion)",
        "-disable-cgp",
        "w-4; 2026-10-23: address-mode sinking translated (cgpaddr.rs, DEVIATE CGP1: no promoted extensions, no phi/select combiner); compare sink in addresssink.rs; ext-load promotion in extload.rs; the rest (select2branch, branch opts, despeculate, tail-call dup, store-extract) settled by the ablation. Pin: changed code in 73 (-O2) and 72 (-Os) of 77 IRs; -disable-cgp -O2: +4456 instr, +21337 B, 64 IRs differ; -disable-cgp -Os: +4061 instr, +19398 B, 63 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 1611 instr 6318 B, pin-with 1412 instr 5482 B, pin-without 1482 instr 5642 B (6 programs); -Os: ours 486 instr 1392 B, pin-with 463 instr 1378 B, pin-without 488 instr 1474 B (5 programs)",
    ),
    row(
        "Exception handling preparation",
        Executed,
        "ehprepare.rs",
        "",
        "dwarf-eh-prepare equivalent. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Prepare inline asm insts",
        Na,
        "",
        "",
        "inline asm is handled by inline_asm.rs; the pass only rewrites constraints for callbr. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Safe Stack instrumentation pass",
        Na,
        "",
        "",
        "acts on the `safestack` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Insert stack protectors",
        Na,
        "",
        "",
        "acts on `ssp*` attributes, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    // Selection.
    row(
        "X86 DAG->DAG Instruction Selection",
        Deviate,
        "isel.rs, isel/matcher.rs",
        "",
        "CG1: llrm selects from MIR with its own rule matcher, no SelectionDAG; reason: segmented targets and 16-bit pointers; cost: the rows; exit: none planned; owner oversight; review 2026-11-13. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Local Dynamic TLS Access Clean-up",
        Na,
        "",
        "",
        "no thread-local storage in any frontend. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 PIC Global Base Reg Initialization",
        Na,
        "",
        "",
        "no PIC code at m32 OMF/COFF or m16. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Argument Stack Rebase",
        Na,
        "",
        "",
        "acts on swiftasync frames only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Finalize ISel and expand pseudo-instructions",
        Executed,
        "isel.rs",
        "",
        "the selector emits final instructions. Pin: changed code in 55 (-O2) and 54 (-Os) of 77 IRs",
    ),
    row(
        "X86 Domain Reassignment Pass",
        Na,
        "",
        "-disable-x86-domain-reassignment",
        "SSE/AVX mask domains; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-x86-domain-reassignment -O2: +0 instr, +0 B, 0 IRs differ; -disable-x86-domain-reassignment -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    // Machine SSA.
    row(
        "Early Tail Duplication",
        Deviate,
        "jumps/ (late tail duplication only)",
        "-disable-early-taildup",
        "CG3: early tail duplication is not translated; reason: on our IR at m32 -O2 the pin's pass is worth 2431 of 32191215 clocks (0.008%), 559 taken branches and 912 instructions of 70 bench programs (-O3 the same); cost: that; exit: decide with the ret duplication of STRISTMP-1 (main duplicates `ret` into both arms, ours does not) and re-measure; owner w-3; review 2026-11-20. Registry slot early-tailduplication. Pin: changed code in 23 (-O2) and 18 (-Os) of 77 IRs; -disable-early-taildup -O2: -34 instr, -131 B, 9 IRs differ; -disable-early-taildup -Os: +14 instr, -4 B, 8 IRs differ",
    ),
    row(
        "Optimize machine instruction PHIs",
        Match,
        "optimizephis.rs",
        "",
        "translated (registry optimizephis@1). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Merge disjoint stack slots",
        Executed,
        "lifetimes.rs, slots.rs",
        "",
        "slots of locals with non-overlapping lifetime markers share; the registry says Missing: reconcile at the ablation. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Local Stack Slot Allocation",
        Na,
        "",
        "",
        "X86 does not request virtual frame base registers (requiresVirtualBaseRegisters is false). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    again(
        "Remove dead machine instructions",
        1,
        Executed,
        "dead-code elimination in MIR; machinedce.rs after allocation",
        "-disable-machine-dce",
        "before register allocation MIR already removed the dead. Pin: changed code in 61 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-dce -O2: +11 instr, +50 B, 14 IRs differ; -disable-machine-dce -Os: +19 instr, +131 B, 14 IRs differ",
    ),
    row(
        "Early If-Conversion",
        Na,
        "",
        "-disable-early-ifcvt",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-early-ifcvt -O2: +0 instr, +0 B, 0 IRs differ; -disable-early-ifcvt -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    row(
        "Machine InstCombiner",
        Missing,
        "",
        "",
        "w-2; 2026-11-13: (premise below: start only where ours exceeds pin-with) changed code in 12 (-O2) and 6 (-Os) of 77 IRs",
    ),
    row(
        "X86 cmov Conversion",
        Na,
        "",
        "",
        "the pass returns at once without CMOV, which the i486 does not have (selects.rs makes branches). Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -x86-cmov-converter=0 -O2: +0 instr, +0 B, 0 IRs differ; -x86-cmov-converter=0 -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    row(
        "Early Machine Loop Invariant Code Motion",
        Deviate,
        "licm (mid-end), loopslots.rs for reloads",
        "-disable-machine-licm",
        "CG2: the mid-end `licm` hoists, `loopslots.rs` hoists reloads, and no machine pass with a register-pressure gate follows; reason: the premise test (docs/working/early-machinelicm.md) shows the pin's pass worth 7 of 4190 (m32 -O2) and 12 of 2675 (m32 -Os) in-loop instructions, 754 of 47619 (1.6%) at m16 -O2 and 204 of 36905 at m16 -Os; cost: that m16 -O2 residual; attribution (tools/pairs/mlicm_kinds.py): the net 657 in mdl_ai, cmd, sc, ent and weapons is diffuse (moves 82, loads 80, push/add-esp/call 136, lea 23, x87 loads and stores 70): no kind the mid-end `licm` could take; exit: none planned; owner w-3; review 2026-11-13. Pin: changed code in 58 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-licm -O2: -242 instr, -1012 B, 38 IRs differ; -disable-machine-licm -Os: -304 instr, -1399 B, 42 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 584 instr 1788 B, pin-with 506 instr 1645 B, pin-without 504 instr 1645 B (3 programs); -Os: ours 188 instr 494 B, pin-with 180 instr 511 B, pin-without 177 instr 506 B (2 programs)",
    ),
    row(
        "Machine Common Subexpression Elimination",
        Executed,
        "precse.rs (before phi elimination), machinecse.rs (after allocation)",
        "-disable-machine-cse",
        "precse.rs translates MachineCSE with PerformSimplePRE and invariant-load CSE (DEVIATE CSE1-CSE4, #3734); machinecse.rs (after allocation) is ours alone, DEVIATE CSE5: removed, 10 of 568 objects grow (+15..+187 B), none shrinks. Pin: changed code in 57 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-cse -O2: +342 instr, +1082 B, 53 IRs differ; -disable-machine-cse -Os: +377 instr, +1151 B, 55 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 897 instr 2780 B, pin-with 811 instr 2652 B, pin-without 832 instr 2716 B (4 programs)",
    ),
    row(
        "Machine code sinking",
        Missing,
        "machinesink.rs",
        "-disable-machine-sink",
        "w-2; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry machinesink@4 is partial; store and load sinking, SinkAndFold. Pin: changed code in 67 (-O2) and 65 (-Os) of 77 IRs; -disable-machine-sink -O2: +699 instr, +2474 B, 64 IRs differ; -disable-machine-sink -Os: +1107 instr, +3707 B, 60 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 1041 instr 3171 B, pin-with 943 instr 3059 B, pin-without 933 instr 3043 B (6 programs); -Os: ours 359 instr 973 B, pin-with 347 instr 964 B, pin-without 346 instr 966 B (5 programs)",
    ),
    row(
        "Peephole Optimizations",
        Missing,
        "peephole.rs and peep/",
        "-disable-peephole",
        "w-2; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry peephole@14 is ours-own, not a translation. Pin: changed code in 72 (-O2) and 72 (-Os) of 77 IRs; -disable-peephole -O2: +2189 instr, +7527 B, 67 IRs differ; -disable-peephole -Os: +1763 instr, +6048 B, 69 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 1923 instr 7289 B, pin-with 1728 instr 6478 B, pin-without 1788 instr 6654 B (11 programs); -Os: ours 806 instr 2220 B, pin-with 786 instr 2271 B, pin-without 831 instr 2382 B (11 programs)",
    ),
    again(
        "Remove dead machine instructions",
        2,
        Executed,
        "machinedce.rs",
        "-disable-machine-dce",
        "as above, after the peephole. Pin: changed code in 61 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-dce -O2: +11 instr, +50 B, 14 IRs differ; -disable-machine-dce -Os: +19 instr, +131 B, 14 IRs differ",
    ),
    row(
        "Live Range Shrink",
        Match,
        "liverangeshrink.rs",
        "",
        "w-4; translated 2026-10-23, registry liverangeshrink@4, before phi elimination. DEVIATE LRS1 in the header (class test, flags). pressuresink.rs stays until the machine scheduler. Pin: changed code in 12 (-O2) and 12 (-Os) of 77 IRs",
    ),
    row(
        "X86 Fixup SetCC",
        Match,
        "fixupsetcc.rs",
        "",
        "translated (registry fixupsetcc@5). Pin: changed code in 41 (-O2) and 40 (-Os) of 77 IRs",
    ),
    row(
        "X86 LEA Optimize",
        Match,
        "leaopt.rs",
        "-disable-x86-lea-opt",
        "translated (slot leaopt@5, before phi elimination); DEVIATE LO-1: only register-and-number addresses (no symbol or frame displacement), LO-2: 32-bit addresses only (exit w-6, 2026-11-09). Pin: changed code in 4 (-O2) and 34 (-Os) of 77 IRs; -disable-x86-lea-opt -O2: +6 instr, +32 B, 3 IRs differ; -disable-x86-lea-opt -Os: -60 instr, +707 B, 34 IRs differ",
    ),
    row(
        "X86 Optimize Call Frame",
        Deviate,
        "pushfold.rs",
        "",
        "CG6: the pass's input, stores into a reserved call frame, is not made: llrm pushes at selection, and pushfold.rs is the pass's one effect left, a once-read load pushed from memory (canFoldIntoRegPush); owner w-8; exit: none, the form is not produced. Pin: changed code in 59 (-O2) and 60 (-Os) of 77 IRs",
    ),
    row(
        "X86 Avoid Store Forwarding Blocks",
        Na,
        "",
        "",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Suppress APX features for relocation",
        Na,
        "",
        "",
        "APX is 64-bit; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 speculative load hardening",
        Na,
        "",
        "",
        "acts on the `speculative_load_hardening` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 EFLAGS copy lowering",
        Deviate,
        "flagscopy.rs",
        "",
        "CG9: translated (registry flagscopy@5) over an explicit flags copy, the pseudo pair standing for the pin's COPY $eflags search (R1 one block, R2); reason: isel makes no flags copy; cost: none; exit: an isel producer of a flags copy; owner w-5; review 2026-11-19. Pin: changed code in 2 (-O2) and 2 (-Os) of 77 IRs",
    ),
    row(
        "X86 DynAlloca Expander",
        Na,
        "",
        "",
        "the pin's DYN_ALLOCA pseudo is made only under MSVC-style stack probing: for `alloca i8, i32 %n` on i486-linux its llc emits the rounded `sub esp` with no pseudo and no probe (a `probe-stack` function gets an inline probe loop, no frontend sets it); `hook_dynamic_alloca` makes the same rounded `sub sp`, test test_a_dynamic_alloca_is_a_rounded_stack_pointer_subtraction_with_no_probe; 0 of 77 IRs change in the pin",
    ),
    row(
        "Tile Register Pre-configure",
        Na,
        "",
        "",
        "AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    // Before and through allocation.
    row(
        "Detect Dead Lanes",
        Na,
        "",
        "",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; ours has lanes.rs with no pin counterpart on the i486",
    ),
    row(
        "Init Undef Pass",
        Na,
        "",
        "",
        "acts only where a register class has a pseudo init-undef (RISC-V); X86 has none. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Process Implicit Definitions",
        Missing,
        "",
        "",
        "w-1; 2026-11-13: (premise below: start only where ours exceeds pin-with) tied to MIR undef (LIR has no IMPLICIT_DEF; uninitialized reads are zero). Pin: changed code in 62 (-O2) and 63 (-Os) of 77 IRs",
    ),
    row(
        "Remove unreachable machine basic blocks",
        Executed,
        "jumps.rs",
        "",
        "removed with the layout. Pin: changed code in 64 (-O2) and 62 (-Os) of 77 IRs",
    ),
    row(
        "Eliminate PHI nodes for register allocation",
        Missing,
        "phielim.rs, phielimination.rs",
        "",
        "w-1; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry phielim@4 is ours-own. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Two-Address instruction pass",
        Missing,
        "twoaddr.rs",
        "",
        "w-1; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry twoaddr@9. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Register Coalescer",
        Missing,
        "coalesce.rs, coalescer/",
        "",
        "splitkit; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry coalesce@10 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Rename Disconnected Subregister Components",
        Na,
        "",
        "",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Machine Instruction Scheduler",
        Missing,
        "pressuresink.rs stands in",
        "-enable-misched=0",
        "w-2; 2026-11-13: (premise below: start only where ours exceeds pin-with) changed code in 55 (-O2) and 54 (-Os) of 77 IRs; -enable-misched=0 -O2: +0 instr, +154 B, 31 IRs differ; -enable-misched=0 -Os: +93 instr, +557 B, 30 IRs differ",
    ),
    again(
        "Greedy Register Allocator",
        1,
        Missing,
        "allocate.rs, greedy/, splitkit",
        "-regalloc=fast",
        "splitkit; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry allocate@11 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Tile Register Configure",
        Na,
        "",
        "",
        "AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    again(
        "Greedy Register Allocator",
        2,
        Na,
        "",
        "",
        "the tile-register instance: AMX only; the change counts of run 1 are the same name's, and no i486 register is in the tile class",
    ),
    row(
        "Virtual Register Rewriter",
        Missing,
        "allocate.rs, parcopy.rs",
        "",
        "splitkit; 2026-11-13 (premise below: start only where ours exceeds pin-with). Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Stack Slot Coloring",
        Executed,
        "slots.rs",
        "-disable-ssc",
        "frame slots of spilled values share when never live together; matched at the ablation. Pin: changed code in 48 (-O2) and 47 (-Os) of 77 IRs; -disable-ssc -O2: +94 instr, +2880 B, 37 IRs differ; -disable-ssc -Os: +6 instr, +1765 B, 33 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -Os: ours 106 instr 343 B, pin-with 100 instr 289 B, pin-without 100 instr 291 B (1 programs)",
    ),
    again(
        "Machine Copy Propagation Pass",
        1,
        Match,
        "machinecp.rs",
        "-disable-copyprop",
        "translated, run inside the peephole. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs; -disable-copyprop -O2: +122 instr, +403 B, 38 IRs differ; -disable-copyprop -Os: +115 instr, +353 B, 34 IRs differ",
    ),
    row(
        "Machine Loop Invariant Code Motion",
        Na,
        "",
        "-disable-postra-machine-licm",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 2 (-O2) and 3 (-Os) of 77 IRs; -disable-postra-machine-licm -O2: +0 instr, +0 B, 0 IRs differ; -disable-postra-machine-licm -Os: +0 instr, +0 B, 0 IRs differ; ours has loopslots.rs for reloads with no pin counterpart on the i486",
    ),
    row(
        "X86 Lower Tile Copy",
        Na,
        "",
        "",
        "AMX; features exclude it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 FP Stackifier",
        Match,
        "floatalloc.rs",
        "",
        "translated. Pin: changed code in 53 (-O2) and 53 (-Os) of 77 IRs",
    ),
    row(
        "X86 Load Value Injection (LVI) Load Hardening",
        Na,
        "",
        "",
        "off unless -mlvi-hardening. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Remove Redundant DEBUG_VALUE analysis",
        Na,
        "",
        "",
        "debug values; llrm's debuginfo.rs works from source positions. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Fixup Statepoint Caller Saved",
        Na,
        "",
        "",
        "no statepoints. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "PostRA Machine Sink",
        Executed,
        "postrasink.rs",
        "-disable-postra-machine-sink",
        "a copy or spill into the one successor that needs it. Pin: changed code in 28 (-O2) and 30 (-Os) of 77 IRs; -disable-postra-machine-sink -O2: -1 instr, +14 B, 4 IRs differ; -disable-postra-machine-sink -Os: -9 instr, -28 B, 8 IRs differ",
    ),
    row(
        "Shrink Wrapping analysis",
        Executed,
        "saverestore.rs, shrinkwrap.rs",
        "-enable-shrink-wrap=0",
        "follows ShrinkWrapImpl; the registry says Missing: reconcile at the ablation. Pin: changed code in 4 (-O2) and 4 (-Os) of 77 IRs; -enable-shrink-wrap=0 -O2: -1 instr, -1 B, 2 IRs differ; -enable-shrink-wrap=0 -Os: +0 instr, +0 B, 0 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 29 instr 104 B, pin-with 32 instr 111 B, pin-without 33 instr 111 B (1 programs); -Os: ours 20 instr 48 B, pin-with 21 instr 61 B, pin-without 21 instr 62 B (1 programs)",
    ),
    row(
        "Prologue/Epilogue Insertion & Frame Finalization",
        Deviate,
        "prologue.rs, masm.rs; frameobjects.rs (calculateFrameObjectOffsets, not yet called: FO-1)",
        "",
        "CG7: the pin lays out frame objects and fills offsets after allocation (frame indices, `replaceFrameIndices`); `frame.rs` hands out a displacement at spill time; so PEI is translated as pure functions (frameobjects.rs: calculateFrameObjectOffsets; emitPrologue/emitEpilogue and the callee-saved spills to follow) that nothing calls; cost: none measured; exit: row FI-1 (frame indices in `frame.rs`, docs/working/pei-plan.md), then the functions replace prologue.rs and masm::parts; owner w-6; review 2026-11-20. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Machine Late Instructions Cleanup Pass",
        Match,
        "latecleanup.rs",
        "",
        "translated (slot latecleanup@13, after prologue); DEVIATE LC1: immediates only, no frame-register load-address. Pin: changed code in 13 (-O2) and 12 (-Os) of 77 IRs. Ours on the 71 bench programs: 6 objects change, all smaller (-17..-24 B)",
    ),
    row(
        "Control Flow Optimizer",
        Missing,
        "jumps/branchfold.rs",
        "-disable-branch-fold",
        "w-3; 2026-11-13: (premise below: start only where ours exceeds pin-with) registry jumps@17 is partial. Pin: changed code in 75 (-O2) and 75 (-Os) of 77 IRs; -disable-branch-fold -O2: +140 instr, +394 B, 40 IRs differ; -disable-branch-fold -Os: +134 instr, +323 B, 32 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 114 instr 296 B, pin-with 94 instr 271 B, pin-without 95 instr 271 B (1 programs); -Os: ours 234 instr 573 B, pin-with 217 instr 604 B, pin-without 221 instr 612 B (3 programs)",
    ),
    row(
        "Tail Duplication",
        Deviate,
        "jumps/ ",
        "-disable-tail-duplicate",
        "CG4: late tail duplication is not translated; reason: -disable-tail-duplicate changes no clock, instruction or branch of 70 bench programs at m32 -O2 and -O3 (m16 -O2: 220 of 46865 in-loop instructions); cost: none measured; exit: re-measure with the ret duplication of STRISTMP-1; owner w-3; review 2026-11-20. Registry jumps@17. Pin: changed code in 33 (-O2) and 20 (-Os) of 77 IRs; -disable-tail-duplicate -O2: -42 instr, -193 B, 17 IRs differ; -disable-tail-duplicate -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    again(
        "Machine Copy Propagation Pass",
        2,
        Missing,
        "",
        "-disable-copyprop",
        "w-2; 2026-11-13: (premise below: start only where ours exceeds pin-with) llrm runs it once; the pin runs it again after tail duplication. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs; -disable-copyprop -O2: +122 instr, +403 B, 38 IRs differ; -disable-copyprop -Os: +115 instr, +353 B, 34 IRs differ",
    ),
    row(
        "Post-RA pseudo instruction expansion pass",
        Executed,
        "isel.rs",
        "",
        "the selector emits final instructions. Pin: changed code in 75 (-O2) and 75 (-Os) of 77 IRs",
    ),
    row(
        "X86 pseudo instruction expansion pass",
        Executed,
        "isel.rs",
        "",
        "as above. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Insert KCFI indirect call checks",
        Na,
        "",
        "",
        "acts on the `kcfi` module flag, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Post RA top-down list latency scheduler",
        Na,
        "",
        "-disable-post-ra",
        "0 of 77 IRs differ with the pin's switch off or the pass on (-O2 and -Os): changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-post-ra -O2: +0 instr, +0 B, 0 IRs differ; -disable-post-ra -Os: +0 instr, +0 B, 0 IRs differ; ours has schedule.rs (narrower) with no pin counterpart on the i486",
    ),
    row(
        "Analyze Machine Code For Garbage Collection",
        Na,
        "",
        "",
        "no GC strategy. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Branch Probability Basic Block Placement",
        Deviate,
        "jumps/placement",
        "-disable-block-placement",
        "CG5: placement is translated in part (jumps/placement, #2629); reason: m32 -O2 taken branches ours 798165 against the pin's 786616 (+1.5%) and 917988 without the pass; clocks without it +0.84% of 32.19 M; -O3 733888 against 726139 (+1.1%); executed `jmp` 112791 against 94444 at -O2; cost: the 1.5% of taken branches (nbody_single, queens, x_adler, x_ll_div, x_crc32 the largest); exit: per-program attribution by w-2 (docs/working/tail-dup-placement.md), translate what it names; owner w-3; review 2026-11-20. Pin: changed code in 74 (-O2) and 62 (-Os) of 77 IRs; -disable-block-placement -O2: +286 instr, +1995 B, 64 IRs differ; -disable-block-placement -Os: +729 instr, +2802 B, 57 IRs differ. Premise (the one IR llrm-c hands its backend, to both; m32, the programs the pin's switch changes): -O2: ours 1366 instr 4616 B, pin-with 1275 instr 4256 B, pin-without 1257 instr 4000 B (9 programs); -Os: ours 635 instr 1776 B, pin-with 612 instr 1779 B, pin-without 623 instr 1800 B (7 programs)",
    ),
    row(
        "Insert fentry calls",
        Na,
        "",
        "",
        "acts on the `fentry-call` attribute, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Insert XRay ops",
        Na,
        "",
        "",
        "acts on XRay attributes, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Implement the 'patchable-function' attribute",
        Na,
        "",
        "",
        "acts on `patchable-function`, never set. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Execution Dependency Fix",
        Na,
        "",
        "",
        "SSE execution domains; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "BreakFalseDeps",
        Na,
        "",
        "",
        "partial-register and SSE false dependencies; the 486 and P5 have none. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Indirect Branch Tracking",
        Na,
        "",
        "",
        "CET; off without the `cf-protection-branch` flag. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row("X86 vzeroupper inserter", Na, "", "", "AVX only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs"),
    row(
        "X86 Byte/Word Instruction Fixup",
        Deviate,
        "peephole.peep group byte_word_fixup",
        "",
        "CG8: a peephole group (W2), one rule per pin function with its permalink, not a pass; reason: the liveness and encoding facts are the peephole's; cost: none known; exit: a Rust pass once those facts reach LIRTransforms; owner w-5; review 2026-11-19. W1: real mode only if not longer. Pin: changed code in 63 (-O2) and 62 (-Os) of 77 IRs",
    ),
    row(
        "X86 Atom pad short functions",
        Na,
        "",
        "",
        "acts on Atom (`padShortFunctions`); not i486. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 LEA Fixup",
        Deviate,
        "peephole.peep `lea_fixup`; peephole.rs (`lea_fixed`)",
        "",
        "CG6: `optTwoAddrLEA` cases 1 and 2 are translated (peephole.peep `lea_fixup`, permalinks in docs/working/lea-fixup-plan.md); case 3 (`optLEAALU`, an LEA before a `sub`) is not; reason: no site found yet; cost: none measured; exit: translate it by 2026-11-09; owner w-6; review 2026-11-09. Pin: changed code in 37 (-O2) and 41 (-Os) of 77 IRs",
    ),
    row(
        "X86 Fixup Inst Tuning",
        Na,
        "",
        "",
        "tunes SSE shuffles and blends; features -sse. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Fixup Vector Constants",
        Na,
        "",
        "",
        "vector constants; no vectors. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Compressing EVEX instrs when possible",
        Na,
        "",
        "",
        "AVX-512 only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 insert wait instruction",
        Na,
        "",
        "",
        "strict floating point, never emitted. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Contiguously Lay Out Funclets",
        Na,
        "",
        "",
        "Windows funclet EH only. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Remove Loads Into Fake Uses",
        Na,
        "",
        "",
        "fake uses come from `-fextend-lifetimes`. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Live DEBUG_VALUE analysis",
        Na,
        "",
        "",
        "debug values; see above. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Machine Sanitizer Binary Metadata",
        Na,
        "",
        "",
        "off without the sanitizer-metadata attribute. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Speculative Execution Side Effect Suppression",
        Na,
        "",
        "",
        "off unless -mseses. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Indirect Thunks",
        Na,
        "",
        "",
        "retpolines, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Return Thunks",
        Na,
        "",
        "",
        "`-mfunction-return=thunk`, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Check CFA info and insert CFI instructions if needed",
        Executed,
        "cfi.rs",
        "-disable-cfi-fixup",
        "unwind info from cfi.rs. Pin: changed code in 33 (-O2) and 28 (-Os) of 77 IRs",
    ),
    row(
        "X86 Load Value Injection (LVI) Ret-Hardening",
        Na,
        "",
        "",
        "off unless -mlvi-cfi. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Pseudo Probe Inserter",
        Na,
        "",
        "",
        "sample-profile probes, never requested. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "Unpack machine instruction bundles",
        Na,
        "",
        "",
        "llrm has no instruction bundles. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
    row(
        "X86 Assembly Printer",
        Executed,
        "masm.rs, objbuild.rs",
        "",
        "the object is written directly. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs",
    ),
];

/// One pass of the pin's pipeline, with which run of it this is.
fn passes(fixture: &str) -> Vec<(String, usize)> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    fixture
        .lines()
        .filter(|line| line.starts_with(' ') && !line.trim().is_empty())
        .map(|line| {
            let name = line.trim().to_owned();
            let nth = seen.entry(name.clone()).and_modify(|one| *one += 1).or_insert(1);
            (name, *nth)
        })
        .collect()
}

fn transforming(fixture: &str) -> Vec<(String, usize)> {
    passes(fixture).into_iter().filter(|(name, _)| !is_analysis(name)).collect()
}

/// m32 and m16, -O1 to -Os: the same passes in the same order. -O0 is the only
/// level that differs (84 lines against 222), so the pipeline is not a function
/// of the level.
#[test]
fn the_pin_codegen_pipeline_is_the_same_at_every_level_and_target() {
    let first = passes(FIXTURES[0].1);
    assert!(first.len() > 150, "an empty fixture is no pipeline");
    for (name, text) in FIXTURES {
        assert_eq!(passes(text), first, "{name}");
    }
}

/// A pass added to the pin's pipeline with no row, or a row for a pass the
/// pipeline does not run, fails here: the table is the pipeline.
#[test]
fn every_pin_codegen_pass_has_exactly_one_row() {
    let pipeline = transforming(FIXTURES[1].1);
    for (name, nth) in &pipeline {
        let found = ROWS.iter().filter(|one| one.pass == name && one.nth == *nth).count();
        assert_eq!(found, 1, "`{name}` (run {nth}) has {found} rows");
    }
    for one in ROWS {
        assert!(
            pipeline.contains(&(one.pass.to_owned(), one.nth)),
            "row `{}` ({}) names no pass of the pipeline",
            one.pass,
            one.nth
        );
    }
}

/// A row says why: MATCH and EXECUTED name ours, DEVIATE is numbered, MISSING
/// has an owner and a date, N/A has evidence.
#[test]
fn every_codegen_row_states_its_reason() {
    for one in ROWS {
        let at = format!("`{}`", one.pass);
        assert!(!one.why.trim().is_empty(), "{at}: no reason");
        match one.state {
            Match | Executed => assert!(
                !one.ours.is_empty(),
                "{at}: names no module of ours (the count is Loop Strength Reduction's: a loop pass prints no dump under -print-after-all, so its change shows on the next pass that does) The pin's switches inside it (branch opts, select-to-branch, store-extract, delete-phis, complex address modes) are 0 to 0.05% of the 5.6% (-disable-cgp costs +21337 B at -O2): the effect is address-mode and compare sinking, which addresssink.rs does in part"
            ),
            Deviate => assert!(
                one.why.starts_with("CG") && one.why.contains("owner"),
                "{at}: a DEVIATE is numbered CGn with an owner"
            ),
            Missing => assert!(one.why.contains("; 2026-"), "{at}: a MISSING row has `owner; date`"),
            Na => assert!(one.ours.is_empty(), "{at}: N/A names no module of ours"),
        }
    }
    assert!(REGISTRY_OWNER.starts_with("lir"));
}

#[test]
fn the_codegen_slot_table_is_written_on_request() {
    let Ok(path) = std::env::var("LLRM_CODEGEN_SLOT_TABLE") else { return };
    let name = |state: State| match state {
        Match => "MATCH",
        Executed => "EXECUTED",
        Deviate => "DEVIATE",
        Missing => "MISSING",
        Na => "N/A",
    };
    let mut out = String::from("| # | Pin pass | State | Ours | Pin switch | Why |\n|---|---|---|---|---|---|\n");
    for (at, one) in ROWS.iter().enumerate() {
        let run = if one.nth > 1 { format!(" (run {})", one.nth) } else { String::new() };
        out.push_str(&format!(
            "| {} | {}{} | {} | {} | {} | {} |\n",
            at + 1,
            one.pass,
            run,
            name(one.state),
            one.ours,
            if one.switch.is_empty() { String::new() } else { format!("`{}`", one.switch) },
            one.why
        ));
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for one in ROWS {
        *counts.entry(name(one.state)).or_default() += 1;
    }
    out.push_str(&format!("\n{counts:?}\n"));
    std::fs::write(path, out).expect("writes the table");
}
