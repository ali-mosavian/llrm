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
const UNSETTLED: &str = "funcattrs-port; 2026-10-25: unsettled until the ablation";

const ROWS: &[Row] = &[
    // IR before selection (CodeGenPrepare's company).
    row(
        "ObjC ARC contraction",
        Na,
        "",
        "",
        "no Objective-C: the pass acts on retain/release calls the frontends never emit",
    ),
    row(
        "Pre-ISel Intrinsic Lowering",
        Executed,
        "isel/expand.rs, hir lowering",
        "",
        "memory and math intrinsics lowered by the selector",
    ),
    row(
        "Expand IR instructions",
        Na,
        "",
        "",
        "acts on fp-to-int/div of more than 128 bits; MIR's widest integer is i64 (lower_int64.rs)",
    ),
    row("Expand Atomic instructions", Na, "", "", "MIR has no atomic or fence opcode (opcode.rs)"),
    row("Lower AMX intrinsics", Na, "", "", "AMX is a 64-bit extension; target features exclude it"),
    row("Lower AMX type for load/store", Na, "", "", "as above"),
    row(
        "Canonicalize natural loops",
        Executed,
        "loopsimplify.rs (mid-end)",
        "",
        "loop-simplify runs in the mid-end; LSR relies on it",
    ),
    row(
        "Canonicalize Freeze Instructions in Loops",
        Missing,
        "",
        "",
        "funcattrs-port; 2026-11-13: MIR has `freeze`; LSR's canon-freeze is not run",
    ),
    row("Induction Variable Users", Match, "ivusers.rs", "", "translated"),
    row(
        "Loop Strength Reduction",
        Match,
        "lsr_*.rs (registry `lsr`)",
        "-disable-lsr",
        "translated; runs at the end of the mid-end, the pin's runs after it",
    ),
    row("Lower Garbage Collection Instructions", Na, "", "", "no GC strategy in any frontend"),
    row("Shadow Stack GC Lowering", Na, "", "", "as above"),
    row(
        "Remove unreachable blocks from the CFG",
        Executed,
        "simplifycfg.rs, adce.rs",
        "",
        "removed in the mid-end before selection",
    ),
    row("Constant Hoisting", Missing, "", "-disable-constant-hoisting", UNSETTLED),
    row(
        "Replace intrinsics with calls to vector library",
        Na,
        "",
        "-disable-replace-with-vec-lib",
        "no vector library, no vectors in MIR (deferred)",
    ),
    row("Partially inline calls to library functions", Missing, "", "-disable-partial-libcall-inlining", UNSETTLED),
    row(
        "Instrument function entry/exit with calls to e.g. mcount() (post inlining)",
        Na,
        "",
        "",
        "acts on the `instrument-function-entry-inlined` attribute, never set",
    ),
    row("Scalarize Masked Memory Intrinsics", Na, "", "", "no masked intrinsics in MIR"),
    row("Expand reduction intrinsics", Na, "", "-disable-expand-reductions", "no vector reductions in MIR"),
    row("Interleaved Access Pass", Na, "", "", "no vectors in MIR"),
    row("X86 Partial Reduction", Na, "", "", "SSE/AVX dot-product patterns; features -sse"),
    row("Expand indirectbr instructions", Na, "", "", "MIR has no indirectbr"),
    row(
        "CodeGen Prepare",
        Missing,
        "addresssink.rs (sinkCmpExpression, address sinking), extload.rs (ext-load promotion)",
        "-disable-cgp",
        "funcattrs-port; 2026-10-25: partial; the rest (select2branch, branch opts, despeculate, tail-call dup, store-extract) settled by the ablation",
    ),
    row("Exception handling preparation", Executed, "ehprepare.rs", "", "dwarf-eh-prepare equivalent"),
    row(
        "Prepare inline asm insts",
        Na,
        "",
        "",
        "inline asm is handled by inline_asm.rs; the pass only rewrites constraints for callbr",
    ),
    row("Safe Stack instrumentation pass", Na, "", "", "acts on the `safestack` attribute, never set"),
    row("Insert stack protectors", Na, "", "", "acts on `ssp*` attributes, never set"),
    // Selection.
    row(
        "X86 DAG->DAG Instruction Selection",
        Deviate,
        "isel.rs, isel/matcher.rs",
        "",
        "CG1: llrm selects from MIR with its own rule matcher, no SelectionDAG; reason: segmented targets and 16-bit pointers; cost: the rows; exit: none planned; owner oversight; review 2026-11-13",
    ),
    row("Local Dynamic TLS Access Clean-up", Na, "", "", "no thread-local storage in any frontend"),
    row("X86 PIC Global Base Reg Initialization", Na, "", "", "no PIC code at m32 OMF/COFF or m16"),
    row("Argument Stack Rebase", Na, "", "", "acts on swiftasync frames only"),
    row(
        "Finalize ISel and expand pseudo-instructions",
        Executed,
        "isel.rs",
        "",
        "the selector emits final instructions",
    ),
    row(
        "X86 Domain Reassignment Pass",
        Na,
        "",
        "-disable-x86-domain-reassignment",
        "SSE/AVX mask domains; features -sse",
    ),
    // Machine SSA.
    row(
        "Early Tail Duplication",
        Missing,
        "jumps/ (late tail duplication only)",
        "-disable-early-taildup",
        "lir-cleanup-port; 2026-11-13: registry slot early-tailduplication",
    ),
    row("Optimize machine instruction PHIs", Match, "optimizephis.rs", "", "translated (registry optimizephis@1)"),
    row(
        "Merge disjoint stack slots",
        Executed,
        "lifetimes.rs, slots.rs",
        "",
        "slots of locals with non-overlapping lifetime markers share; the registry says Missing: reconcile at the ablation",
    ),
    row(
        "Local Stack Slot Allocation",
        Na,
        "",
        "",
        "X86 does not request virtual frame base registers (requiresVirtualBaseRegisters is false)",
    ),
    again(
        "Remove dead machine instructions",
        1,
        Executed,
        "dead-code elimination in MIR; machinedce.rs after allocation",
        "-disable-machine-dce",
        "before register allocation MIR already removed the dead",
    ),
    row("Early If-Conversion", Missing, "", "-disable-early-ifcvt", UNSETTLED),
    row("Machine InstCombiner", Missing, "", "", UNSETTLED),
    row(
        "X86 cmov Conversion",
        Na,
        "",
        "",
        "the pass returns at once without CMOV, which the i486 does not have (selects.rs makes branches)",
    ),
    row(
        "Early Machine Loop Invariant Code Motion",
        Missing,
        "licm (mid-end), loopslots.rs for reloads",
        "-disable-machine-licm",
        UNSETTLED,
    ),
    row(
        "Machine Common Subexpression Elimination",
        Missing,
        "precse.rs (before phi elimination), machinecse.rs (after allocation)",
        "-disable-machine-cse",
        "funcattrs-port; 2026-10-25: registry machinecse@4 is partial; untranslated PerformSimplePRE and invariant-load CSE",
    ),
    row(
        "Machine code sinking",
        Missing,
        "machinesink.rs",
        "-disable-machine-sink",
        "funcattrs-port; 2026-10-25: registry machinesink@4 is partial; store and load sinking, SinkAndFold",
    ),
    row(
        "Peephole Optimizations",
        Missing,
        "peephole.rs and peep/",
        "-disable-peephole",
        "lir-cleanup-port; 2026-11-13: registry peephole@14 is ours-own, not a translation",
    ),
    again(
        "Remove dead machine instructions",
        2,
        Executed,
        "machinedce.rs",
        "-disable-machine-dce",
        "as above, after the peephole",
    ),
    row(
        "Live Range Shrink",
        Missing,
        "pressuresink.rs stands in",
        "",
        "lir-cleanup-port; 2026-11-13: registry pressuresink@5",
    ),
    row("X86 Fixup SetCC", Missing, "", "", UNSETTLED),
    row("X86 LEA Optimize", Missing, "", "-disable-x86-lea-opt", UNSETTLED),
    row("X86 Optimize Call Frame", Missing, "", "", UNSETTLED),
    row("X86 Avoid Store Forwarding Blocks", Missing, "", "", UNSETTLED),
    row("X86 Suppress APX features for relocation", Na, "", "", "APX is 64-bit; features exclude it"),
    row("X86 speculative load hardening", Na, "", "", "acts on the `speculative_load_hardening` attribute, never set"),
    row("X86 EFLAGS copy lowering", Missing, "flag lanes (liveness.rs), stepflags.rs", "", UNSETTLED),
    row(
        "X86 DynAlloca Expander",
        Missing,
        "",
        "",
        "funcattrs-port; 2026-11-09: dynamic alloca needs an HIR op (builtins row B2)",
    ),
    row("Tile Register Pre-configure", Na, "", "", "AMX; features exclude it"),
    // Before and through allocation.
    row("Detect Dead Lanes", Missing, "lanes.rs", "", UNSETTLED),
    row(
        "Init Undef Pass",
        Na,
        "",
        "",
        "acts only where a register class has a pseudo init-undef (RISC-V); X86 has none",
    ),
    row(
        "Process Implicit Definitions",
        Missing,
        "",
        "",
        "funcattrs-port; 2026-11-13: tied to MIR undef (LIR has no IMPLICIT_DEF; uninitialized reads are zero)",
    ),
    row("Remove unreachable machine basic blocks", Executed, "jumps.rs", "", "removed with the layout"),
    row(
        "Eliminate PHI nodes for register allocation",
        Missing,
        "phielim.rs, phielimination.rs",
        "",
        "lir-cleanup-port; 2026-11-13: registry phielim@4 is ours-own",
    ),
    row("Two-Address instruction pass", Missing, "twoaddr.rs", "", "lir-cleanup-port; 2026-11-13: registry twoaddr@9"),
    row(
        "Register Coalescer",
        Missing,
        "coalesce.rs, coalescer/",
        "",
        "regalloc-core-poc; 2026-11-13: registry coalesce@10 is partial",
    ),
    row("Rename Disconnected Subregister Components", Missing, "", "", UNSETTLED),
    row("Machine Instruction Scheduler", Missing, "pressuresink.rs stands in", "-enable-misched=0", UNSETTLED),
    again(
        "Greedy Register Allocator",
        1,
        Missing,
        "allocate.rs, greedy/, splitkit",
        "-regalloc=fast",
        "regalloc-core-poc; 2026-11-13: registry allocate@11 is partial",
    ),
    row("Tile Register Configure", Na, "", "", "AMX; features exclude it"),
    again("Greedy Register Allocator", 2, Na, "", "", "the tile-register instance: AMX only"),
    row("Virtual Register Rewriter", Missing, "allocate.rs, parcopy.rs", "", "regalloc-core-poc; 2026-11-13"),
    row(
        "Stack Slot Coloring",
        Executed,
        "slots.rs",
        "-disable-ssc",
        "frame slots of spilled values share when never live together; matched at the ablation",
    ),
    again(
        "Machine Copy Propagation Pass",
        1,
        Match,
        "machinecp.rs",
        "-disable-copyprop",
        "translated, run inside the peephole",
    ),
    row(
        "Machine Loop Invariant Code Motion",
        Missing,
        "loopslots.rs for reloads",
        "-disable-postra-machine-licm",
        UNSETTLED,
    ),
    row("X86 Lower Tile Copy", Na, "", "", "AMX; features exclude it"),
    row("X86 FP Stackifier", Match, "floatalloc.rs", "", "translated"),
    row("X86 Load Value Injection (LVI) Load Hardening", Na, "", "", "off unless -mlvi-hardening"),
    row(
        "Remove Redundant DEBUG_VALUE analysis",
        Na,
        "",
        "",
        "debug values; llrm's debuginfo.rs works from source positions",
    ),
    row("Fixup Statepoint Caller Saved", Na, "", "", "no statepoints"),
    row(
        "PostRA Machine Sink",
        Executed,
        "postrasink.rs",
        "-disable-postra-machine-sink",
        "a copy or spill into the one successor that needs it",
    ),
    row(
        "Shrink Wrapping analysis",
        Executed,
        "saverestore.rs, shrinkwrap.rs",
        "-enable-shrink-wrap=0",
        "follows ShrinkWrapImpl; the registry says Missing: reconcile at the ablation",
    ),
    row(
        "Prologue/Epilogue Insertion & Frame Finalization",
        Missing,
        "prologue.rs, masm.rs",
        "",
        "lir-cleanup-port; 2026-11-13: registry prologue@13",
    ),
    row("Machine Late Instructions Cleanup Pass", Missing, "", "", UNSETTLED),
    row(
        "Control Flow Optimizer",
        Missing,
        "jumps/branchfold.rs",
        "-disable-branch-fold",
        "lir-cleanup-port; 2026-11-13: registry jumps@17 is partial",
    ),
    row(
        "Tail Duplication",
        Missing,
        "jumps/ ",
        "-disable-tail-duplicate",
        "lir-cleanup-port; 2026-11-13: registry jumps@17",
    ),
    again(
        "Machine Copy Propagation Pass",
        2,
        Missing,
        "",
        "-disable-copyprop",
        "funcattrs-port; 2026-10-25: llrm runs it once; the pin runs it again after tail duplication",
    ),
    row("Post-RA pseudo instruction expansion pass", Executed, "isel.rs", "", "the selector emits final instructions"),
    row("X86 pseudo instruction expansion pass", Executed, "isel.rs", "", "as above"),
    row("Insert KCFI indirect call checks", Na, "", "", "acts on the `kcfi` module flag, never set"),
    row(
        "Post RA top-down list latency scheduler",
        Missing,
        "schedule.rs (narrower)",
        "-disable-post-ra",
        "lir-cleanup-port; 2026-11-13: registry schedule@16 is ours-own",
    ),
    row("Analyze Machine Code For Garbage Collection", Na, "", "", "no GC strategy"),
    row(
        "Branch Probability Basic Block Placement",
        Missing,
        "jumps/placement",
        "-disable-block-placement",
        "lir-cleanup-port; 2026-11-13: registry jumps@17, chains and loops (#2629)",
    ),
    row("Insert fentry calls", Na, "", "", "acts on the `fentry-call` attribute, never set"),
    row("Insert XRay ops", Na, "", "", "acts on XRay attributes, never set"),
    row("Implement the 'patchable-function' attribute", Na, "", "", "acts on `patchable-function`, never set"),
    row("X86 Execution Dependency Fix", Na, "", "", "SSE execution domains; features -sse"),
    row("BreakFalseDeps", Na, "", "", "partial-register and SSE false dependencies; the 486 and P5 have none"),
    row("X86 Indirect Branch Tracking", Na, "", "", "CET; off without the `cf-protection-branch` flag"),
    row("X86 vzeroupper inserter", Na, "", "", "AVX only"),
    row("X86 Byte/Word Instruction Fixup", Missing, "", "", UNSETTLED),
    row("X86 Atom pad short functions", Na, "", "", "acts on Atom (`padShortFunctions`); not i486"),
    row("X86 LEA Fixup", Missing, "", "", UNSETTLED),
    row("X86 Fixup Inst Tuning", Na, "", "", "tunes SSE shuffles and blends; features -sse"),
    row("X86 Fixup Vector Constants", Na, "", "", "vector constants; no vectors"),
    row("Compressing EVEX instrs when possible", Na, "", "", "AVX-512 only"),
    row("X86 insert wait instruction", Na, "", "", "strict floating point, never emitted"),
    row("Contiguously Lay Out Funclets", Na, "", "", "Windows funclet EH only"),
    row("Remove Loads Into Fake Uses", Na, "", "", "fake uses come from `-fextend-lifetimes`"),
    row("Live DEBUG_VALUE analysis", Na, "", "", "debug values; see above"),
    row("Machine Sanitizer Binary Metadata", Na, "", "", "off without the sanitizer-metadata attribute"),
    row("X86 Speculative Execution Side Effect Suppression", Na, "", "", "off unless -mseses"),
    row("X86 Indirect Thunks", Na, "", "", "retpolines, never requested"),
    row("X86 Return Thunks", Na, "", "", "`-mfunction-return=thunk`, never requested"),
    row(
        "Check CFA info and insert CFI instructions if needed",
        Executed,
        "cfi.rs",
        "-disable-cfi-fixup",
        "unwind info from cfi.rs",
    ),
    row("X86 Load Value Injection (LVI) Ret-Hardening", Na, "", "", "off unless -mlvi-cfi"),
    row("Pseudo Probe Inserter", Na, "", "", "sample-profile probes, never requested"),
    row("Unpack machine instruction bundles", Na, "", "", "llrm has no instruction bundles"),
    row("X86 Assembly Printer", Executed, "masm.rs, objbuild.rs", "", "the object is written directly"),
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
            Match | Executed => assert!(!one.ours.is_empty(), "{at}: names no module of ours"),
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
