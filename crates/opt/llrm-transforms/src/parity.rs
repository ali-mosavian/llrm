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
//! `docs/architecture/parity-pipeline.md`.
//!
//! # Used by
//! `pipeline` (`Fixed::linear`), the registry tests.

use crate::pipeline::Phase;

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

/// One slot of LLVM's pipeline.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    /// As `llvm_order` names it (`pass@line`); a descriptive name when Missing.
    pub slot: &'static str,
    pub(crate) phase: Phase,
    /// LLVM's pass in the slot.
    pub llvm: &'static str,
    pub kind: SlotKind,
    /// For `Old`: the translation that will replace it.
    pub replaced_by: &'static str,
}

const fn entry(
    slot: &'static str,
    phase: Phase,
    llvm: &'static str,
    kind: SlotKind,
    replaced_by: &'static str,
) -> Entry {
    Entry { slot, phase, llvm, kind, replaced_by }
}

use Phase::{Backend, Cleanup, Early, Simplify};
use SlotKind::{Missing, Old, Partial, Translated};

/// Every slot of the -O2 function pipeline in `llvm_order` (-O1 builds a
/// subset), then the slots LLVM has that llrm does not.
pub(crate) const REGISTRY: &[Entry] = &[
    entry("decide@1178", Early, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@1178", Early, "simplifycfg", Old, "simplifycfg-port"),
    entry("sroa@1179", Early, "sroa<modify-cfg>", Old, "sroa.rs"),
    entry("earlycse@1180", Early, "early-cse", Translated, ""),
    entry("promote@1241", Cleanup, "mem2reg", Old, "mem2reg.rs"),
    entry("fold@1242", Cleanup, "instcombine", Old, "instcombine-port"),
    entry("dead@1242", Cleanup, "instcombine", Old, "instcombine-port"),
    entry("decide@1244", Cleanup, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@1244", Cleanup, "simplifycfg", Old, "simplifycfg-port"),
    entry("sroa@646", Simplify, "sroa", Old, "sroa.rs"),
    entry("earlycse@649", Simplify, "early-cse<memssa>", Translated, ""),
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
    entry("fill@737", Simplify, "loop-idiom", Old, "loop-idiom-port"),
    entry("indvars@738", Simplify, "indvars", Old, "scev-port"),
    entry("unroll@753", Simplify, "loop-unroll-full", Old, "loop-passes-port"),
    entry("sroa@776", Simplify, "sroa", Old, "sroa.rs"),
    entry("inferspace@787", Simplify, "infer-address-spaces (target)", Old, ""),
    entry("gvn@787", Simplify, "gvn", Old, "gvn-port"),
    entry("fold@801", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@801", Simplify, "adce", Old, "adce-port"),
    entry("decide@809", Simplify, "jump-threading, correlated-propagation", Old, "simplifycfg-port"),
    entry("memcpyopt@818", Simplify, "memcpyopt", Old, "memcpyopt-port"),
    entry("merge@818", Simplify, "mldst-motion", Old, ""),
    entry("dse@820", Simplify, "dse", Old, "dse-port"),
    entry("loopsimplify@823", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@823", Simplify, "lcssa", Old, "lcssa-port"),
    entry("hoist@823", Simplify, "licm<allowspeculation>", Old, "licm-port"),
    entry("loopmotion@823", Simplify, "licm<allowspeculation>", Old, "licm-port"),
    entry("decide@832", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("phiopt@832", Simplify, "simplifycfg", Old, "simplifycfg-port"),
    entry("fold@837", Simplify, "instcombine", Old, "instcombine-port"),
    entry("dead@837", Simplify, "instcombine", Old, "instcombine-port"),
    entry("constraint-elimination", Simplify, "constraint-elimination", Missing, ""),
    entry("loop-instsimplify", Simplify, "loop-instsimplify", Missing, ""),
    entry("loop-simplifycfg", Simplify, "loop-simplifycfg", Missing, ""),
    entry("loop-rotate@728", Simplify, "loop-rotate<header-duplication>", Missing, ""),
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
    entry("floatalloc@8", Backend, "x86-fp-stackifier (X86FloatingPoint.cpp)", Old, "backend-slopes"),
    entry("twoaddr@9", Backend, "two-address-instruction", Old, "lir-cleanup-poc"),
    entry("coalesce@10", Backend, "register-coalescer", Old, "lir-cleanup-poc"),
    entry(
        "allocate@11",
        Backend,
        "greedy, spill-code-placement, virtregrewriter, stack-slot-coloring",
        Partial,
        "greedy under the parity switch (#2312); criterion: LLRM_CHECK_LIVEINTERVALS and LLRM_GREEDY_CHECK pass; exit: SplitKit translated and rows at or below the old allocator per row",
    ),
    entry("parcopy@12", Backend, "phi-node-elimination copies, virtregrewriter", Old, ""),
    entry("prologue@13", Backend, "prologepilog", Old, ""),
    entry("peephole@14", Backend, "peephole-opt, machine-cse, machine-dce", Old, ""),
    entry("loopslots@15", Backend, "greedy region splitting (SplitKit)", Old, "regalloc-core-poc"),
    entry("schedule@16", Backend, "post-RA scheduler", Old, ""),
    entry(
        "jumps@17",
        Backend,
        "branch-folder, block-placement, tailduplication",
        Partial,
        "TailMergeBlocks only (#2241); placement and tail duplication are gcc's; exit in #2307",
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
pub const ANALYSES: &[AnalysisEntry] = &[AnalysisEntry {
    key: "mssa",
    llvm: "MemorySSAAnalysis",
    kind: Translated,
    off: true,
    deps: &["aa"],
    oracle: "shadow_mssa equality on dumped pass inputs",
}];

/// Vocabulary the manager does not have yet. `PreservedAnalyses` keeps an
/// analysis or drops it; LLVM's passes that keep MemorySSA do so by updating it
/// (`MemorySSAUpdater`), which the manager cannot tell from "unchanged".
pub const TODO: &[&str] = &[
    "preserved, kept updated by the pass (MemorySSAUpdater): not a PreservedAnalyses variant yet",
    "LoopInfo is invalidated by CFG changes only (CFGAnalyses): the manager keys by (analysis, function) already but has no CFG-preserved set",
];

/// `built`, the slot names a list of `phase` runs, against the registry: the
/// first Translated slot of the phase that it lacks, or slot it has that the
/// registry does not know, is the error.
pub(crate) fn check(
    registry: &[Entry],
    phase: Phase,
    built: &[&str],
) -> Result<(), String> {
    for one in registry.iter().filter(|one| one.phase == phase && matches!(one.kind, Translated | Partial)) {
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
pub(crate) fn check_phase(
    phase: Phase,
    built: &[&str],
) -> Result<(), String> {
    check(REGISTRY, phase, built)?;
    llrm_mir::counts::joined(joined_slots(REGISTRY, built));
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

pub(crate) fn tally(registry: &[Entry]) -> Tally {
    let mut out = Tally::default();
    for one in registry {
        match one.kind {
            Translated => out.translated += 1,
            Partial => out.partial += 1,
            Old => out.old += 1,
            Missing => out.missing += 1,
        }
    }
    out
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
        out += &format!("{} ({}) partial: {}\n", one.slot, one.llvm, one.replaced_by);
    }
    for one in REGISTRY.iter().filter(|one| one.kind == Old && !one.replaced_by.is_empty()) {
        out += &format!("{} ({}) -> {}\n", one.slot, one.llvm, one.replaced_by);
    }
    for one in ANALYSES {
        out += &format!("{} {:?} deps {:?} oracle: {}\n", one.key, one.kind, one.deps, one.oracle);
    }
    out
}
