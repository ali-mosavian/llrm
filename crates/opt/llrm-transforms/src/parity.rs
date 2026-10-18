//! The parity pipeline's slot registry: for each slot of LLVM's function
//! pipeline that `Options::llvm_order` builds, whether llrm's pass there is
//! translated from LLVM's, an older stand-in, or missing.
//!
//! # How it works
//! [`REGISTRY`] has one [`Entry`] per slot, named as in `llvm_order` (the pass
//! and the `PassBuilderPipelines.cpp` line it stands at). [`check_phase`]
//! compares one built list with it: a [`SlotKind::Translated`] slot of that
//! phase that the list does not run, or a slot the list runs that has no entry,
//! is an error. `Fixed::linear` asserts it in debug builds, so a translation
//! cannot be marked joined and left out of the pipeline. The slots built with a
//! translation are told to [`llrm_mir::counts::joined`], so `LLRM_DEBUG=counts`
//! prints, per analysis, how many times a joined slot made the manager compute
//! it and how many times an old one did (`callers` lines): an analysis with no
//! old caller can be deleted.
//!
//! A translation joins when its oracle passes (output identical to `opt` at the
//! pin), not when it is on by default; see
//! the parity pipeline plan page (PR 2288).
//!
//! # Example
//! `parity::check_phase(Phase::Simplify, &built)` is `Ok` when every Translated
//! Simplify slot is in `built` and every name in `built` has an entry.
//!
//! # Used by
//! `pipeline` (`Fixed::linear`), the registry tests.

/// What stands in a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotKind {
    /// Translated from LLVM's source at the pin.
    Translated,
    /// llrm's own pass does part of the job; `replaced_by` names the
    /// translation that will take the slot.
    Old,
    /// Part of the slot is translated and runs; the rest is still llrm's own.
    /// `replaced_by` says which part and the issue that holds the exit. Never
    /// counted as joined.
    Partial,
    /// LLVM runs a pass here and llrm nothing.
    Missing,
}

/// Where a list of LLVM's order stands: one list per phase, one mechanism for
/// all of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// The early function pipeline (l.1178-1180).
    Early,
    /// GlobalCleanup (l.1241-1244).
    Cleanup,
    /// `buildFunctionSimplificationPipeline` (l.630-837), once for a function.
    Simplify,
    /// The end of `buildModuleOptimizationPipeline`'s function list (l.1662).
    Late,
    /// The module pipeline: the inliner, the CGSCC members, the IPO passes.
    Module,
    /// llc's machine pipeline.
    Backend,
}

/// One slot of LLVM's pipeline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    /// As `llvm_order` names it (`pass@line`); a descriptive name when Missing.
    pub slot: &'static str,
    pub(crate) phase: Phase,
    /// LLVM's pass in the slot.
    pub llvm: &'static str,
    pub kind: SlotKind,
    /// For `Old`: the translation that will replace it; for a Translated slot
    /// that an off switch (`LLRM_NO_SROA`, `LLRM_NO_MEM2REG`) puts back to the
    /// stand-in, the translation's file.
    pub replaced_by: &'static str,
    /// For `Partial`, required: "criterion: ...; exit: ...".
    pub note: &'static str,
}

const fn entry(
    slot: &'static str,
    phase: Phase,
    llvm: &'static str,
    kind: SlotKind,
    replaced_by: &'static str,
) -> Entry {
    Entry { slot, phase, llvm, kind, replaced_by, note: "" }
}

const fn partial(
    slot: &'static str,
    phase: Phase,
    llvm: &'static str,
    note: &'static str,
) -> Entry {
    Entry { slot, phase, llvm, kind: Partial, replaced_by: "", note }
}

use Phase::{Backend, Cleanup, Early, Late, Simplify};
use SlotKind::{Missing, Old, Partial, Translated};

/// Every slot of the -O2 function pipeline in `llvm_order` (-O1 builds a
/// subset), then the slots LLVM has that llrm does not.
pub(crate) const REGISTRY: &[Entry] = &[
    entry("decide@1178", Early, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@1178", Early, "simplifycfg", Old, "simplifycfg-port"),
    entry("sroa@1179", Early, "sroa<modify-cfg>", Translated, "sroa.rs"),
    entry("earlycse@1180", Early, "early-cse", Translated, ""),
    entry("promote@1241", Cleanup, "mem2reg", Translated, "mem2reg.rs"),
    entry("fold@1242", Cleanup, "instcombine", Old, "instcombine-port"),
    entry("dead@1242", Cleanup, "instcombine", Old, "instcombine-port"),
    entry("decide@1244", Cleanup, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@1244", Cleanup, "simplifycfg", Old, "simplifycfg-port"),
    entry("sroa@646", Simplify, "sroa", Translated, "sroa.rs"),
    entry("earlycse@649", Simplify, "early-cse<memssa>", Translated, ""),
    entry("cvp@669", Simplify, "correlated-propagation", Translated, ""),
    entry("decide@675", Simplify, "simplifycfg, jump-threading, correlated-propagation", Old, "simplifycfg-port"),
    entry("phiopt@675", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("fold@677", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@677", Simplify, "instcombine", Old, "instcombine-port"),
    entry("tailrec@688", Simplify, "tailcallelim", Old, "tailcallelim-port"),
    entry("decide@690", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@690", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("algebraic@696", Simplify, "reassociate", Old, "reassociate-port"),
    entry("loopsimplify@764", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@764", Simplify, "lcssa", Old, "lcssa-port"),
    entry("floatloop@724", Simplify, "licm", Old, "licm-port"),
    entry("hoist@724", Simplify, "licm<no-allowspeculation>", Old, "licm-port"),
    entry("loopmotion@724", Simplify, "licm<no-allowspeculation>", Old, "licm-port"),
    entry("hoist@730", Simplify, "licm<allowspeculation>", Old, "licm-port"),
    entry("trivialunswitch@732", Simplify, "simple-loop-unswitch<trivial>", Old, "loop-passes-port"),
    entry("decide@766", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@766", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("fold@768", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@768", Simplify, "instcombine", Old, "instcombine-port"),
    entry("loopsimplify@772", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@772", Simplify, "lcssa", Old, "lcssa-port"),
    entry("loopidiom@737", Simplify, "loop-idiom", Translated, ""),
    entry("loopidioms@737", Simplify, "loop-idiom", Old, "loop-idiom-port (the pricing, noncountable half)"),
    entry("indvars@738", Simplify, "indvars", Old, "scev-port"),
    entry("loopdeletion@749", Simplify, "loop-deletion", Translated, ""),
    entry("unroll@753", Simplify, "loop-unroll-full", Translated, ""),
    entry("sroa@776", Simplify, "sroa", Translated, "sroa.rs"),
    entry("inferspace@787", Simplify, "infer-address-spaces (target)", Old, ""),
    entry("gvn@787", Simplify, "gvn", Old, "gvn-port"),
    entry("fold@801", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@801", Simplify, "adce", Old, "adce-port"),
    entry("decide@809", Simplify, "jump-threading, correlated-propagation", Old, "simplifycfg-port"),
    entry("cvp@810", Simplify, "correlated-propagation", Translated, ""),
    entry("memcpyopt@818", Simplify, "memcpyopt", Old, "backend-slopes"),
    partial(
        "merge@818",
        Simplify,
        "memcpyopt (tryMergingIntoMemset), MergeConsecutiveStores (backend)",
        "criterion: the memsets are MemCpyOpt's (memcpyoptimizer.rs, checked against opt by tools/llvmport/mccompare.py); exit: the wider constant stores of loopidioms::Merge go with the backend MergeConsecutiveStores (lir-cleanup-port)",
    ),
    entry("dse@820", Simplify, "dse", Old, "backend-slopes (after memcpyopt)"),
    entry("loopsimplify@823", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@823", Simplify, "lcssa", Old, "lcssa-port"),
    entry("hoist@823", Simplify, "licm<allowspeculation>", Old, "licm-port"),
    entry("loopmotion@823", Simplify, "licm<allowspeculation>", Old, "licm-port"),
    entry("decide@832", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@832", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("fold@837", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@837", Simplify, "instcombine", Old, "instcombine-port"),
    entry("constraint-elimination", Simplify, "constraint-elimination", Missing, ""),
    entry("instsimplify@1662", Late, "instsimplify", Translated, ""),
    entry("loop-instsimplify", Simplify, "loop-instsimplify", Missing, ""),
    entry("loop-simplifycfg", Simplify, "loop-simplifycfg", Missing, ""),
    entry("rotate@728", Simplify, "loop-rotate<header-duplication>", Translated, ""),
    entry("loopsimplify@728", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@728", Simplify, "lcssa", Old, "lcssa-port"),
    entry("sccp-bdce", Simplify, "sccp, bdce", Missing, ""),
    entry("dfa-jump-threading", Simplify, "dfa-jump-threading", Missing, ""),
    entry("vector-combine", Simplify, "vector-combine", Missing, ""),
    // The machine phases of `llc`, one slot per phase of `flow::machine_with`
    // (`module@row`, the row of the table in `backend/mod.rs`).
    entry("farcall@1", Backend, "x86 far-call lowering (target)", Old, ""),
    entry("phiconst@2", Backend, "none (shared phi constants)", Old, ""),
    entry("ssaspill@3", Backend, "none (spiller route)", Old, ""),
    entry("phielim@4", Backend, "phi-node-elimination", Old, "lir-cleanup-poc"),
    entry("pressuresink@5", Backend, "machine-sink", Old, "lir-cleanup-poc"),
    entry("floatfold@6", Backend, "x86 isel float folding (target)", Old, ""),
    entry("floatassign@7", Backend, "x87 stack slots (regalloc, target)", Old, "regalloc-core-poc"),
    entry("floatalloc@8", Backend, "x86-fp-stackifier (X86FloatingPoint.cpp)", Translated, ""),
    entry("twoaddr@9", Backend, "two-address-instruction", Old, "lir-cleanup-poc"),
    entry("coalesce@10", Backend, "register-coalescer", Old, "lir-cleanup-poc"),
    partial(
        "allocate@11",
        Backend,
        "greedy, spill-code-placement, virtregrewriter, stack-slot-coloring",
        "greedy by default under LLVM's order (#2312); criterion: LLRM_CHECK_LIVEINTERVALS and LLRM_GREEDY_CHECK pass; exit: SplitKit translated and rows at or below the old allocator per row",
    ),
    entry("parcopy@12", Backend, "phi-node-elimination copies, virtregrewriter", Old, ""),
    entry("prologue@13", Backend, "prologepilog", Old, ""),
    entry("peephole@14", Backend, "peephole-opt, machine-cse, machine-dce", Old, ""),
    entry("loopslots@15", Backend, "greedy region splitting (SplitKit)", Old, "regalloc-core-poc"),
    entry("schedule@16", Backend, "post-RA scheduler", Old, ""),
    partial(
        "jumps@17",
        Backend,
        "branch-folder, block-placement, tailduplication",
        "TailMergeBlocks only (#2241); criterion: rows vs min(gcc,pin) + oracle sweep (J1, #2307); exit: identity vs llc after placement and tail duplication",
    ),
    entry("early-tailduplication", Backend, "early-tailduplication", Missing, ""),
    entry("stack-coloring", Backend, "stack-coloring", Missing, ""),
    entry("shrink-wrap", Backend, "shrink-wrap", Missing, ""),
];

/// One analysis of the parity pipeline.
#[derive(Clone, Copy, Debug)]
pub struct AnalysisEntry {
    /// The manager's key for it (`Analysis::NAME`).
    pub key: &'static str,
    /// LLVM's analysis.
    pub llvm: &'static str,
    pub kind: SlotKind,
    /// Translated but not on by default.
    pub off: bool,
    /// The analyses it asks for (`mssa` depends on alias analysis, #2163).
    pub deps: &'static [&'static str],
    /// What lets it join.
    pub oracle: &'static str,
}

/// The analyses with a translation or a stand-in the registry follows. The
/// result of `mssa` is handed out mutable (the walker lives inside it).
pub const ANALYSES: &[AnalysisEntry] = &[
    AnalysisEntry {
        key: "loops",
        llvm: "LoopAnalysis",
        kind: Translated,
        off: false,
        deps: &["dominators"],
        oracle: "tools/loops_oracle.py: print<loops> of opt at the pin",
    },
    AnalysisEntry {
        key: "mssa",
        llvm: "MemorySSAAnalysis",
        kind: Translated,
        off: true,
        deps: &["aa"],
        oracle: "shadow_mssa equality on dumped pass inputs",
    },
];

/// Vocabulary the manager does not have yet. `PreservedAnalyses` keeps an
/// analysis or drops it; LLVM's passes that keep MemorySSA do so by updating it
/// (`MemorySSAUpdater`), which the manager cannot tell from "unchanged".
pub const TODO: &[&str] =
    &["preserved, kept updated by the pass (MemorySSAUpdater): not a PreservedAnalyses variant yet"];

/// How many times LLVM runs a slot's pass over one body in one visit of the
/// function (`PassBuilderPipelines.cpp`: the function simplification pipeline
/// runs once per function, so `gvn` once; llvm-difftest counted 11 on d_faces
/// -O2 for its 11 functions). The slot name is as in `llvm_order`.
pub const RUNS_PER_BODY: &[(&str, [usize; 4])] = &[("gvn@787", [0, 1, 1, 1])];

/// The level a list is built for: the pin's `default<O1>`, `default<O2>`,
/// `default<O3>`; -Os is the -O2 list on optsize IR (`opt` rejects `-Os`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O1,
    O2,
    O3,
    Os,
}

impl Level {
    fn index(self) -> usize {
        self as usize
    }
}

/// Registry slots the pin's `default<O1>` does not have (no GVN at -O1:
/// `buildO1FunctionSimplificationPipeline`), so a Translated one is not
/// required in a -O1 list.
pub const ABSENT_AT_O1: &[&str] = &["gvn@787"];

/// Run counts LLVM does not match yet, stated and not checked:
/// `loopsimplify`/`lcssa` run at three unconditional slots per body at every
/// level (x_hanoi2: ours 9/9/15/9 at -O1/-O2/-O3/-Os against LLVM's
/// 4/10/10/10).
pub const RUN_COUNTS_OPEN: &[&str] = &[
    "loopsimplify@764,772,823 and lcssa: one pair per loop-pass adaptor",
    "-O3 reruns a body's whole list after `ipa-cp.` and `ipa-deadargs.` (d_faces: _d_draw_faces runs the 52 steps three times, 17 gvn runs over 12 bodies against -O2's 12): LLVM runs IPSCCP and deadargelim before the function pipelines, so a body is not run twice (cgscc-poc's single pass-list run per SCC visit)",
];

/// `name` without the slot it stands in (`gvn@787` is `gvn`); the rounds name
/// their passes without a slot.
fn base_of(name: &str) -> &str {
    name.split_once('@').map_or(name, |split| split.0)
}

/// `lines`, the `LLRM_DEBUG=runs` lines of a compile of `bodies` functions,
/// against [`RUNS_PER_BODY`]: a slot that ran over more bodies' worth of
/// steps than its allowance is the error. Linear in the lines.
pub fn check_run_counts(
    lines: &[String],
    bodies: usize,
    level: Level,
) -> Result<(), String> {
    for (slot, per_level) in RUNS_PER_BODY {
        let per_body = &per_level[level.index()];
        let ran = lines
            .iter()
            .filter(|line| line.starts_with("step "))
            .filter_map(|line| line.split_whitespace().nth(1))
            .filter(|name| base_of(name.split_once('-').map_or(*name, |split| split.1)) == base_of(slot))
            .count();
        if ran > *per_body * bodies {
            return Err(format!(
                "{slot} ran {ran} times over {bodies} bodies at {level:?}, LLVM runs it {per_body} per body"
            ));
        }
    }
    Ok(())
}

/// `built`, the slot names a list of `phase` runs, against the registry: the
/// first Translated slot of the phase that it lacks, or slot it has that the
/// registry does not know, is the error. The tests call it on registries of
/// their own; the pipeline goes through [`check_phase`].
#[cfg(test)]
pub(crate) fn check(
    registry: &[Entry],
    phase: Phase,
    built: &[&str],
) -> Result<(), String> {
    check_without(registry, phase, built, &[])
}

/// [`check`] where the slots in `absent` are not LLVM's at this level.
fn check_without(
    registry: &[Entry],
    phase: Phase,
    built: &[&str],
    absent: &[&str],
) -> Result<(), String> {
    for one in registry
        .iter()
        .filter(|one| one.phase == phase && matches!(one.kind, Translated | Partial) && !absent.contains(&one.slot))
    {
        if !built.contains(&one.slot) {
            return Err(format!("slot {} is Translated ({}) and not run in {phase:?}", one.slot, one.llvm));
        }
    }
    for name in built {
        if !registry.iter().any(|one| one.slot == *name) {
            return Err(format!("slot {name} runs in {phase:?} and is not in the registry"));
        }
    }
    Ok(())
}

/// The Translated slots of `REGISTRY` that `built` runs: a Partial slot is not
/// one of them, it is never joined.
pub(crate) fn joined_slots<'a>(
    registry: &'a [Entry],
    built: &'a [&str],
) -> impl Iterator<Item = &'static str> + 'a {
    registry.iter().filter(|one| one.kind == Translated && built.contains(&one.slot)).map(|one| one.slot)
}

/// [`check`] against [`REGISTRY`]; the built slots that are Translated are told
/// to the counts as joined.
pub fn check_phase(
    phase: Phase,
    built: &[&str],
) -> Result<(), String> {
    check_phase_at(phase, built, Level::O2)
}

/// [`check_phase`] for the list of `level`: the slots the pin's list of that
/// level lacks are not required.
pub fn check_phase_at(
    phase: Phase,
    built: &[&str],
    level: Level,
) -> Result<(), String> {
    check_phase_with(phase, built, level, true, true, crate::pipeline::Options::default().rotate_slot)
}

/// [`check_phase_at`] with the explicit off switches: the slots whose
/// translation is `sroa.rs` are Old when `LLRM_NO_SROA` is set, those of
/// `mem2reg.rs` when `LLRM_NO_MEM2REG` is, and the old stand-in is what runs;
/// with `rotate` false (`LLRM_NO_ROTATE`) the rotate slot is not run.
pub(crate) fn check_phase_with(
    phase: Phase,
    built: &[&str],
    level: Level,
    sroa: bool,
    mem2reg: bool,
    rotate: bool,
) -> Result<(), String> {
    let absent = if level == Level::O1 { ABSENT_AT_O1 } else { &[] };
    // `LLRM_NO_ROTATE` takes the translated `loop-rotate` out of the order.
    let registry: Vec<Entry> = REGISTRY
        .iter()
        .map(|one| {
            let off = (!sroa && one.replaced_by == "sroa.rs")
                || (!mem2reg && one.replaced_by == "mem2reg.rs")
                || (!rotate && one.slot == "rotate@728");
            Entry { kind: if off { Old } else { one.kind }, ..*one }
        })
        .collect();
    check_without(&registry, phase, built, absent)?;
    llrm_mir::counts::joined(joined_slots(&registry, built));
    Ok(())
}

/// [`check_phase`] for the machine phases `llc` runs: `built` is the slot of
/// each phase of `flow::machine_with` that runs.
pub fn check_backend(built: &[&str]) -> Result<(), String> {
    check_phase(Phase::Backend, built)
}

/// How many slots are in each state; Partial is counted apart from Translated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tally {
    pub translated: usize,
    pub partial: usize,
    pub old: usize,
    pub missing: usize,
}

fn count(kinds: impl Iterator<Item = SlotKind>) -> Tally {
    let mut out = Tally::default();
    for kind in kinds {
        match kind {
            Translated => out.translated += 1,
            Partial => out.partial += 1,
            Old => out.old += 1,
            Missing => out.missing += 1,
        }
    }
    out
}

pub(crate) fn tally(registry: &[Entry]) -> Tally {
    count(registry.iter().map(|one| one.kind))
}

const PHASES: [(Phase, &str); 6] = [
    (Phase::Early, "early"),
    (Phase::Cleanup, "cleanup"),
    (Phase::Simplify, "simplify"),
    (Phase::Late, "late"),
    (Phase::Module, "module"),
    (Phase::Backend, "backend"),
];

/// The recount of `registry` and `analyses` as the `[parity]` lines that
/// `LLRM_DEBUG=counts` prints: all slots, each phase's slots, the analyses.
pub(crate) fn tally_lines_of(
    registry: &[Entry],
    analyses: &[AnalysisEntry],
) -> Vec<String> {
    let line = |what: &str, count: Tally| {
        format!(
            "[parity] {what} translated {} partial {} old {} missing {}",
            count.translated, count.partial, count.old, count.missing
        )
    };
    let mut out = vec![line("slots", tally(registry)) + &format!(" (of {})", registry.len())];
    for (phase, name) in PHASES {
        out.push(line(
            &format!("phase {name}"),
            count(registry.iter().filter(|one| one.phase == phase).map(|one| one.kind)),
        ));
    }
    out.push(line("analyses", count(analyses.iter().map(|one| one.kind))));
    out
}

/// [`tally_lines_of`] for [`REGISTRY`] and [`ANALYSES`].
pub fn tally_lines() -> Vec<String> {
    tally_lines_of(REGISTRY, ANALYSES)
}

/// The registry as text: each Old slot with the translation that takes it, each
/// analysis with its oracle.
pub fn report() -> String {
    let mut out = String::new();
    let count = tally(REGISTRY);
    out += &format!(
        "slots: {} translated, {} partial, {} old, {} missing\n",
        count.translated, count.partial, count.old, count.missing
    );
    for one in REGISTRY.iter().filter(|one| one.kind == Partial) {
        out += &format!("{} ({}) partial: {}\n", one.slot, one.llvm, one.note);
    }
    for one in REGISTRY.iter().filter(|one| one.kind == Old && !one.replaced_by.is_empty()) {
        out += &format!("{} ({}) -> {}\n", one.slot, one.llvm, one.replaced_by);
    }
    for one in ANALYSES {
        out += &format!("{} {:?} deps {:?} oracle: {}\n", one.key, one.kind, one.deps, one.oracle);
    }
    out
}
