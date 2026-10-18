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

use Phase::{Cleanup, Early, Simplify};
use SlotKind::{Missing, Old, Translated};

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
    entry("loopidioms@737", Simplify, "loop-idiom", Old, "loop-idiom-port"),
    entry("indvars@738", Simplify, "indvars", Old, "scev-port"),
    entry("unroll@753", Simplify, "loop-unroll-full", Old, "loop-passes-port"),
    entry("sroa@776", Simplify, "sroa", Old, "sroa.rs"),
    entry("inferspace@787", Simplify, "infer-address-spaces (target)", Old, ""),
    entry("gvn@787", Simplify, "gvn", Old, "gvn-port"),
    entry("sccp@792", Simplify, "sccp", Missing, ""),
    entry("bdce@797", Simplify, "bdce", Translated, ""),
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
    entry("dfa-jump-threading", Simplify, "dfa-jump-threading", Missing, ""),
    entry("vector-combine", Simplify, "vector-combine", Missing, ""),
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
    for one in registry.iter().filter(|one| one.phase == phase && one.kind == Translated) {
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

/// [`check`] against [`REGISTRY`]; the built slots that are Translated are told
/// to the counts as joined.
pub fn check_phase(
    phase: Phase,
    built: &[&str],
) -> Result<(), String> {
    check(REGISTRY, phase, built)?;
    llrm_mir::counts::joined(
        REGISTRY.iter().filter(|one| one.kind == Translated && built.contains(&one.slot)).map(|one| one.slot),
    );
    Ok(())
}

/// The registry as text: each Old slot with the translation that takes it, each
/// analysis with its oracle.
pub fn report() -> String {
    let mut out = String::new();
    for one in REGISTRY.iter().filter(|one| one.kind == Old && !one.replaced_by.is_empty()) {
        out += &format!("{} ({}) -> {}\n", one.slot, one.llvm, one.replaced_by);
    }
    for one in ANALYSES {
        out += &format!("{} {:?} deps {:?} oracle: {}\n", one.key, one.kind, one.deps, one.oracle);
    }
    out
}
