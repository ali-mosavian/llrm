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
        "translated; runs at the end of the mid-end, the pin's runs after it. Pin: changed code in 0 (-O2) and 0 (-Os) of 77 IRs; -disable-lsr -O2: -37 instr, +40 B, 61 IRs differ; -disable-lsr -Os: +283 instr, +876 B, 63 IRs differ",
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
        "cgpaddr.rs (sinkCmpExpression, sinkAndCmp0Expression, optimizeMemoryInst), cgpretdup.rs (dupRetToEnableTailCallOpts), extload.rs (ext-load promotion)",
        "-disable-cgp",
        "w-4; 2026-10-23: address-mode sinking translated (cgpaddr.rs, DEVIATE CGP1: no promoted extensions, no phi combiner); compare sink in cgpaddr.rs; ext-load promotion in extload.rs; the rest (select2branch, branch opts, despeculate, store-extract) settled by the ablation. Pin: changed code in 73 (-O2) and 72 (-Os) of 77 IRs; -disable-cgp -O2: +4456 instr, +21337 B, 64 IRs differ; -disable-cgp -Os: +4061 instr, +19398 B, 63 IRs differ Landed: #3751 (optimizeMemoryInst); #3813 (addresssink retired into cgpaddr, select combiner). Not yet translated: promoted extensions [TypePromotionTransaction](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L3806), [optimizeExt](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7250) (w-4, 2026-11-13); the phi address combiner [AddressingModeCombiner](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L4180) (w-4, 2026-11-20); [combineToUSubWithOverflow](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1781) and [combineToUAddWithOverflow](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L1747) (w-3, 2026-11-13, #3786); [optimizeSelectInst](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L7766), [splitBranchCondition](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L9350), [despeculateCountZeros](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2629) (w-4, date per plan 10-28).",
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
        "CG3: translated (backend/earlytaildup.rs, slot earlytaildup@1, directly after selection, before farcall): shouldTailDuplicate and isSimpleBB / canCompletelyDuplicateBB / canTailDuplicate pre-RA, tailDuplicate with the SSA update of what the tail defines (processPHI, updateSuccessorsPHIs, MachineSSAUpdater), duplicateSimpleBB; DEVIATE ET1 (none), ET2 (duplicable: group, barrier, data; no EH pads or INLINEASM_BR), ET3 (the pin's merge into the layout predecessor is the same result: the jump to the next block is explicit here); owner w-1. Slot earlytaildup@1 (Partial in the registry). On our IR at m32 -O2 the pin's pass is worth 2431 of 32191215 clocks (0.008%), 559 taken branches and 912 instructions of 70 bench programs. Pin: changed code in 23 (-O2) and 18 (-Os) of 77 IRs; -disable-early-taildup -O2: -34 instr, -131 B, 9 IRs differ; -disable-early-taildup -Os: +14 instr, -4 B, 8 IRs differ Layout-mode part: placement/taildup.rs (late, after allocation).",
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
        "deadmi.rs after selection (registry deadmi@1); machinedce.rs after allocation",
        "-disable-machine-dce",
        "MIR removes the dead values; this one removes what selection leaves unread. Pin: changed code in 61 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-dce -O2: +11 instr, +50 B, 14 IRs differ; -disable-machine-dce -Os: +19 instr, +131 B, 14 IRs differ",
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
        Match,
        "combiner.rs",
        "",
        "translated (slot combiner@4, before leaopt): integer add/and/or/xor/imul reassociation only, the one pattern family i486 has; DEVIATE MC1 (trace is the block: no physical-register dependences, so call order is not a depth), MC2, MC3. Pin: changed code in 12 (-O2) and 6 (-Os) of 77 IRs. Ours on the 71 bench programs: 75 of 568 objects change; m32 32 objects (13 programs), 18 differ in size, net +1295 B (#3781)",
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
        Match,
        "machinelicm.rs",
        "-disable-machine-licm",
        "translated (slot machinelicm@4, ahead of MachineCSE; #3753, CG2 closes): a pure computation of values a loop does not change is made once in its preheader. DEVIATE ML1: the register-pressure gate is a proxy (w-3, review 2026-11-20). Numbered: QCport +150 B (-O1 +58, -O2 +47, -O3 +56, -Os -11; 22 of 260 modules). Pin: changed code in 58 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-licm -O2: -242 instr, -1012 B, 38 IRs differ; -disable-machine-licm -Os: -304 instr, -1399 B, 42 IRs differ",
    ),
    row(
        "Machine Common Subexpression Elimination",
        Executed,
        "precse.rs (before phi elimination)",
        "-disable-machine-cse",
        "precse.rs translates MachineCSE with PerformSimplePRE and invariant-load CSE (DEVIATE CSE1-CSE3, #3734); the post-allocation machinecse.rs is deleted (row CSE5: 7 of 568 objects grow, +22..+34 B, none shrinks; QCport +0.025..0.116%; d_faces -3.60 M Ir). Pin: changed code in 57 (-O2) and 61 (-Os) of 77 IRs; -disable-machine-cse -O2: +342 instr, +1082 B, 53 IRs differ; -disable-machine-cse -Os: +377 instr, +1151 B, 55 IRs differ",
    ),
    row(
        "Machine code sinking",
        Missing,
        "machinesink.rs",
        "-disable-machine-sink",
        "w-2; 2026-11-13: registry machinesink@4 is partial; store and load sinking, SinkAndFold. Pin: changed code in 67 (-O2) and 65 (-Os) of 77 IRs; -disable-machine-sink -O2: +699 instr, +2474 B, 64 IRs differ; -disable-machine-sink -Os: +1107 instr, +3707 B, 60 IRs differ Translated: [FindSuccToSinkTo](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1413), [isProfitableToSinkTo](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1270), [isWorthBreakingCriticalEdge](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1040), [isLegalToBreakCriticalEdge](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1111), [SinkInstruction](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1845) (partial, remainder unaudited). Also translated (#3912, #3959): [hasStoreBetween](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1660), edge odds and block frequency from MBPI/MBFI. Not translated (w-2, 2026-11-06): [PerformSinkAndFold](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L407) (no-op on x86), [aggressivelySinkIntoCycle](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineSink.cpp#L1754).",
    ),
    row(
        "Peephole Optimizations",
        Missing,
        "peephole.rs and peep/",
        "-disable-peephole",
        "w-2; 2026-11-13: registry peephole@14 is ours-own, not a translation. Pin: changed code in 72 (-O2) and 72 (-Os) of 77 IRs; -disable-peephole -O2: +2189 instr, +7527 B, 67 IRs differ; -disable-peephole -Os: +1763 instr, +6048 B, 69 IRs differ Landed piece: a mask only a branch reads is a test (#3743).",
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
        "w-4; translated 2026-10-23, registry liverangeshrink@4, before phi elimination. DEVIATE LRS1 in the header (class test, flags). Pin: changed code in 12 (-O2) and 12 (-Os) of 77 IRs (#3735)",
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
        "CG6: callframe.rs translates the pass function by function (registry callframe@5), tested on hand-built call frames against llc -run-pass=x86-cf-opt; its input, stores into a reserved call frame, is made by no selector yet (llrm pushes at selection), so it finds none; pushfold.rs keeps the load fold for selection's pushes (CF1); reason: the producer is the SelectionDAG call lowering (sdag-a) and the reserved frame is laid out by prologue insertion (row 80, 2026-11-13); cost: none; exit: both landed and pushfold.rs deleted; owner w-5; review 2026-11-13. Pin: changed code in 59 (-O2) and 60 (-Os) of 77 IRs",
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
        "CG9: translated function by function (registry flagscopy@5) over the pseudo pair `flags_copy`/`flags_restore` that stands for the two COPYs of $eflags (R2: isel makes none; exit: an isel producer; owner w-5; review 2026-11-19); all of runOnMachineFunction (cross-block copies, live-in flags, test-position hoisting, splitBlock) as the pin, checked against llc -run-pass=x86-flags-copy-lowering on three shapes; NF variants and fcmov N/A (APX / none pre-RA). Pin: changed code in 2 (-O2) and 2 (-Os) of 77 IRs (#3744)",
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
        Deviate,
        "processimpdefs.rs",
        "",
        "CG10: translated and wired (processimpdefs.rs, slot processimpdefs@2; isel emits `implicit_def` for poison and undef in a register, #3737, #3768, 8e6979c6b7); a phi of `implicit_def`s is one here and its users read undef (`canTurnIntoImplicitDef`); a phi with an undef operand that is no `implicit_def` is PHIElimination's (P2, `allPhiOperandsUndefined`, `SrcUndef`, `ImpDefs`); not translated: `INSERT_SUBREG` and `REG_SEQUENCE` users (PI-3, LIR has none); exit: PI-3 by 2026-11-13; owner w-8; review 2026-11-27. Pin: changed code in 62 (-O2) and 63 (-Os) of 77 IRs",
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
        Match,
        "phielimination.rs, phielim.rs (edge splitting)",
        "",
        "translated: `LowerPHINode`, `SplitPHIEdges`, `LoweredPHIs`, plain `findPHICopyInsertPoint`; a phi from a non-predecessor is an error (#3850); the old lowering and the non-RFP route (P1, P4) are deleted; `allPhiOperandsUndefined`, `ImpDefs`, `SrcUndef` (P2, done); DEVIATEs P3, P6, P7 (phielimination.rs header). P7: no EH-pad or INLINEASM_BR insert points: the unwind edge is the runtime's (not an LIR edge) and no front end emits callbr; exit: a front end that does. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Two-Address instruction pass",
        Missing,
        "twoaddr.rs, twoaddress.rs",
        "",
        "w-1; 2026-11-20: partial translation of [TwoAddressInstructionPass.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp): `tryInstructionCommute`/`isProfitableToCommute` (#3637), `convertInstTo3Addr` at m32 and m16 (#3680, #3729), kills as `isPlainlyKilled` on one `LiveIntervals` shared with the Coalescer (TA-6 retired), `rescheduleMIBelowKill` (#3830), `rescheduleKillAboveMI` (#3882, plan #3804); DEVIATEs TA-1 to TA-5, TA-7 and TA-8 (twoaddress.rs header). The maps (`processCopy`, `scanUses`, `removeClobberedSrcRegMap`, #4054), `collectTiedOperands`' undef rewrite (#4068), `processTiedPairs`' distance and map entry (#4100), `tryInstructionTransform`'s order and unfold (#4109) and `convertInstTo3Addr`'s map and distance update (this row's last PR) are translated; the pairs themselves are made by `_untied`. Not applicable: `eliminateRegSequence`, `processStatepoint`. The update-in-place `_reused` is ours; one `LiveIntervals` per function (the pin's), made lazily. Row 62b, `handleMove` translated and used by TwoAddress (#3916, liveness/liveintervals/handlemove.rs; no bundles, no regmask slots), the other incremental updates MISSING: [`LiveIntervals::handleMove`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervals.cpp) and the incremental updates a pass makes (w-1; 2026-11-20 with the TwoAddress sharing: translated, not patched around; TwoAddress edits one `LiveIntervals` and hands it to the Coalescer; used by TwoAddress, the coalescer and the greedy allocator). Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Register Coalescer",
        Missing,
        "coalesce.rs, coalescer/",
        "",
        "splitkit; 2026-11-13: registry coalesce@10 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs Translated: setRegisters, flip, isCoalescable, adjustCopiesBackFrom, hasOtherReachingDefs, removeCopyByCommutingDef, removePartialRedundancy, reMaterializeDef, updateRegDefsUses, joinCopy, JoinVals, joinVirtRegs, joinIntervals, joinAllIntervals, run. Not translated (all owner splitkit-port; dates after 11-03 are proposals): C1 no sub-register lanes, exit none while values have no lanes: [computeWriteLanes](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2746), [usesLanes](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3206), [taintExtent](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3166), [pruneSubRegValues](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3416), [isDefInSubRange](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3487), [pruneMainSegments](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3496), [joinSubRegRanges](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3616), [mergeSubRangeInto](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3678), [copyValueUndefInPredecessors](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2011), [setUndefOnPrunedSubRegUses](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2025); C2 no physical-register copies: [canJoinPhys](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1993), [joinReservedPhysReg](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2314); C3 no IMPLICIT_DEF in LIR (rows 59, 61 are w-1's): [eliminateUndefCopy](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1742), [removeImplicitDefs](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3510); C6 no undef flags: [addUndefFlag](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1865); C5 a dead copy is removed inline, not by LiveRangeEdit (2026-11-06: switch to greedy/liverangeedit.rs): [eliminateDeadDefs](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L613), [LRE_WillEraseInstruction](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L623); C7 terminal rule, off by default at the pin, no date: [isTerminalReg](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4142), [applyTerminalRule](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4152); debug info (LIR has no DBG_VALUE), no date: [buildVRegToDbgValueMap](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3908), [checkMergingChangesDbgValues](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L3952), [lateLiveIntervalUpdate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4096).",
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
        "machinesched.rs",
        "-enable-misched=0",
        "w-2; 2026-11-06: landed: the region graph (#3827, ScheduleDAGInstrs::buildSchedGraph); #3836 (bottom_up: [tryCandidate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3965), [tryPressure](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3805), [initPolicy](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3688)) landed; the pass runs as machinesched@5 after phielim (DEVIATE MS3-MS4; re-landed with the scheduling boundaries of isSchedulingBoundary). Left: [GenericScheduler::pickNode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4180), [pickNodeFromQueue](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4077), [schedNode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L4285), [registerRoots](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3786), [checkAcyclicLatency](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3758), [initialize](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L3657); [ScheduleDAGMILive::schedule](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L1691), [initRegPressure](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L1477), [updatePressureDiffs](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L1583), [computeCyclicCriticalPath](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L1811); [RegPressureTracker::recede](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterPressure.cpp#L772), [closeRegion](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterPressure.cpp#L335); [SchedBoundary::bumpNode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L2957), [releaseNode](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L2818), [checkHazard](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineScheduler.cpp#L2716) (MS2: resource model, a deviation); the pass wiring. Pin: changed code in 55 (-O2) and 54 (-Os) of 77 IRs; -enable-misched=0 -O2: +0 instr, +154 B, 31 IRs differ; -enable-misched=0 -Os: +93 instr, +557 B, 30 IRs differ",
    ),
    again(
        "Greedy Register Allocator",
        1,
        Missing,
        "allocate.rs, greedy/, splitkit",
        "-regalloc=fast",
        "splitkit; 2026-11-13: registry allocate@11 is partial. Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs Translated: enqueue, dequeue, tryAssign, evictInterference, tryEvict, addSplitConstraints, addThroughConstraints, growRegion, calcCompactRegion, calcBlockSplitCost, calcGlobalSplitCost, splitAroundRegion, tryRegionSplit, doRegionSplit, tryBlockSplit, tryInstructionSplit, calcGapWeights, tryLocalSplit, trySplit, mayRecolorAllInterferences, tryLastChanceRecoloring, tryRecoloringCandidates, selectOrSplit, selectOrSplitImpl. Not translated (owner splitkit-port; dates after 11-03 are proposals; greedy.rs header, until S2): hint recoloring, 2026-11-10: [trySplitAroundHintReg](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L1376), [collectHintInfo](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2471), [getBrokenHintFreq](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2516), [tryHintRecoloring](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2534), [tryHintsRecoloring](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2643), [aboutToRemoveInterval](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2418); CSR cost (the target's register costs are zero here, so the path is dropped; shrink wrapping owns callee-saved cost), 2026-11-12 after checking x86's CSR cost against the pin: [tryAssignCSRFirstTime](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2384), [initializeCSRCost](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2423), [calcSpillCost](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2356); statistics only, no date: [computeStats](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2786), [reportStats](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2878), [report](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2757); pass plumbing, not translated by design (the pass runs inside RegAlloc): RAGreedy ctor, [getAnalysisUsage](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L341), [releaseMemory](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L413), [runOnMachineFunction](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L289), [run](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2942), [hasVirtRegAlloc](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocGreedy.cpp#L2930). The other files of row 66 (InlineSpiller, SplitKit, LiveRangeEdit, LiveRegMatrix, InterferenceCache, SpillPlacement, EdgeBundles, VirtRegMap) are modules under greedy/, not listed here.",
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
        Match,
        "rewriter.rs",
        "",
        "translated (#3736, rewriter::rewrite); DEVIATE W1-W4: no sub-registers, no kill flags or live-ins, parcopy sequences parallel copies, x87 renamed by the stackifier (splitkit-port). Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs",
    ),
    row(
        "Stack Slot Coloring",
        Executed,
        "slots.rs",
        "-disable-ssc",
        "frame slots of spilled values share when never live together; matched at the ablation. Pin: changed code in 48 (-O2) and 47 (-Os) of 77 IRs; -disable-ssc -O2: +94 instr, +2880 B, 37 IRs differ; -disable-ssc -Os: +6 instr, +1765 B, 33 IRs differ",
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
        "follows ShrinkWrapImpl; the registry says Missing: reconcile at the ablation. Pin: changed code in 4 (-O2) and 4 (-Os) of 77 IRs; -enable-shrink-wrap=0 -O2: -1 instr, -1 B, 2 IRs differ; -enable-shrink-wrap=0 -Os: +0 instr, +0 B, 0 IRs differ",
    ),
    row(
        "Prologue/Epilogue Insertion & Frame Finalization",
        Deviate,
        "prologue.rs, masm.rs; frameobjects.rs (calculateFrameObjectOffsets, FO-1) and framelowering.rs (emitPrologue, emitEpilogue, emitSPUpdate), not yet called: CG7",
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
        "w-3; 2026-11-13: registry jumps@17 is partial. Pin: changed code in 75 (-O2) and 75 (-Os) of 77 IRs; -disable-branch-fold -O2: +140 instr, +394 B, 40 IRs differ; -disable-branch-fold -Os: +134 instr, +323 B, 32 IRs differ Done: TailMergeBlocks, OptimizeBlock L1451/L1503/L1551/L1586/L1685. Not yet translated (w-3, 2026-11-13): OptimizeBlock's block moves moveAfter/moveBefore (D20) [OptimizeBlock](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1426); a no-successor block moved to the end [L1601](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1601); forwarding across a branch-only block [L1702](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1702); fall-through creating moves [L1803](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1803), [L1868](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1868); [HoistCommonCodeInSuccs](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L2043): the run of moves and register arithmetic both ways begin with is done (hoist_common; flags dead, values renamed); not done: loads, a flag-setter before the compare that is not a compare, kill-flag fixes, debug-location merging.",
    ),
    row(
        "Tail Duplication",
        Deviate,
        "jumps/ ",
        "-disable-tail-duplicate",
        "CG4: late tail duplication is not translated; reason: -disable-tail-duplicate changes no clock, instruction or branch of 70 bench programs at m32 -O2 and -O3 (m16 -O2: 220 of 46865 in-loop instructions); cost: none measured; exit: re-measure with the ret duplication of STRISTMP-1; owner w-3; review 2026-11-20. Registry jumps@17. Pin: changed code in 33 (-O2) and 20 (-Os) of 77 IRs; -disable-tail-duplicate -O2: -42 instr, -193 B, 17 IRs differ; -disable-tail-duplicate -Os: +0 instr, +0 B, 0 IRs differ Layout-mode part only (placement/taildup.rs). Not yet translated (w-3, review 2026-11-20): [tailDuplicateBlocks](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L274) (the stand-alone late pass, a slot before placement); early-pass SSA update: [processPHI](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L350), [updateSuccessorsPHIs](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L482), [addSSAUpdateEntry](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L335), [duplicateInstruction](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L388), [appendCopies](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L1067), [tailDuplicateAndUpdate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L165), [removeDeadBlock](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L1081); [duplicateSimpleBB](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L753) ([isSimpleBB](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L717), [canCompletelyDuplicateBB](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L737)); the STRISTMP-1 ret-block rule in [shouldTailDuplicate](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TailDuplicator.cpp#L562). #3762 closes STRISTMP-1 for an and of two compares kept as a value. DEVIATE TD1 (owner w1, exit: key the callee table by something a copy keeps): a sibling call (a `jmp` return) is not copied by placement tail duplication or `duplicated_returns`; the pin copies it (the callee is an operand there), ours names the callee by the instruction id in a side table and a copy has none (dot.bas, #5964). Priced: objsweep old 00ee25d1fc (copying) vs new 8cd6209839, --isel=dag and old, O2 and Os, m32 and m16: 0 of 284 objects changed, because the corpus never makes such a copy (568 instrumented compiles: 0); where one is made (tests/check/c/sibling_call_tail_keeps_callee.c, m16 dag) the copy printed a bare `jmp` with no callee, wrong code, and the refusal is correct and 3 B smaller (195 -> 192). Other Return cloners (tail merge, loop rotation, early tail duplication) are not covered: follow-up row, owner w-3.",
    ),
    again(
        "Machine Copy Propagation Pass",
        2,
        Match,
        "machinecp.rs (flow.rs: after ControlFlow, slot copyprop@18)",
        "-disable-copyprop",
        "translated: the pass runs again after the control flow, as the pin after tail duplication (de8c6bc87; test in jumps_tests.rs). Pin: changed code in 77 (-O2) and 77 (-Os) of 77 IRs; -disable-copyprop -O2: +122 instr, +403 B, 38 IRs differ; -disable-copyprop -Os: +115 instr, +353 B, 34 IRs differ",
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
        "CG5: placement is translated (jumps/placement, #2629; its deviations D3-D6, D20-D22 in jumps-placement-check.md); reason: m32 -O2 taken branches ours 798165 against the pin's 786616 (+1.5%), -O3 +1.1%; the loops behind the residual are not placement: nbody_single = LSR's induction variable (w-5), x_adler = the compare sharing the subtract's flags (cmpsub@1, #3773) and the EmitLoweredSelect orientation (measured +3509 B, not landed, tail-dup-placement.md); cost: the 1.5% of taken branches; exit: those two causes, then re-measure with tools/pairs/bench_pin.py; owner w-3; review 2026-11-20. Pin: changed code in 74 (-O2) and 62 (-Os) of 77 IRs; -disable-block-placement -O2: +286 instr, +1995 B, 64 IRs differ; -disable-block-placement -Os: +729 instr, +2802 B, 57 IRs differ Translated with deviations D3 (duplication by bytes), D4 (loop headers after latches), D5 (merge before/after), D6 (Frequency vs MBFI), D20-D22 (jumps-placement-check.md): [buildCFGChains](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2799), [buildChain](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L1920), [buildLoopChains](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2706), [rotateLoop](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2423), [optimizeBranches](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L2964), [alignBlocks](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3003) (D2 selectBestSuccessor L1665 is retired). Review 2026-11-20 (w-3).",
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
        "fixupbw.rs",
        "",
        "CG8: a pass mirroring the pin's functions (fixupbw.rs) with one deviation, W1: real mode rewrites only if not longer (the 0x66 prefix); reason: encoding facts; cost: the real-mode words stay words; exit: none (W1 is a fact of the encoding); owner w-5; review 2026-11-19. Pin: changed code in 63 (-O2) and 62 (-Os) of 77 IRs (#3749, #3777)",
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
        "fixupleas.rs",
        "",
        "CG6: `optTwoAddrLEA` is translated in fixupleas.rs (permalinks in docs/working/lea-fixup-plan.md): cases 1 and 2 (add, inc, dec) for every spelling of the address, the pin's LEA32r rule applied to our word LEAs too, which the pin leaves alone; case 3 (`optLEAALU`) is translated in fixupleas.rs as the pin has it, the add or sub within five instructions (PR 3; the one-instruction peephole rule is gone); the objects it changes over the 71 bench programs x 2 targets x 4 levels are 5, -24 B in all (lru m16 O1 -2, nbody_fixed m16 O2 and O3 -10 each, x_list m16 O3 -2, matmul m32 O3 +0); owner w-8; review 2026-11-30. Pin: changed code in 37 (-O2) and 41 (-Os) of 77 IRs",
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

/// The table in docs/working/codegen-slot-table.md has each row once, in the
/// order of `ROWS`, under its own name: rows 23, 80 and 98 appeared twice after
/// merges of generated lines (23 as both EXECUTED and MISSING), and the board's
/// grep counts were wrong for it.
#[test]
fn the_slot_table_document_has_every_row_number_once_under_its_name() {
    let text = include_str!("../../../../docs/working/codegen-slot-table.md");
    let mut seen: Vec<(usize, String)> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("| ") else { continue };
        let mut cells = rest.split(" | ");
        let (Some(number), Some(name)) = (cells.next(), cells.next()) else { continue };
        if let Ok(number) = number.parse::<usize>() {
            seen.push((number, name.to_owned()));
        }
    }
    let numbers: Vec<usize> = seen.iter().map(|(number, _)| *number).collect();
    let expected: Vec<usize> = (1..=ROWS.len()).collect();
    assert_eq!(numbers, expected, "a row is missing, repeated or out of order");
    for ((number, name), row) in seen.iter().zip(ROWS) {
        let run = if row.nth > 1 { format!(" (run {})", row.nth) } else { String::new() };
        assert_eq!(name, &format!("{}{run}", row.pass), "row {number}");
    }
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
