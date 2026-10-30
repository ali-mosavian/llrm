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

use Phase::{Backend, Cleanup, Early, Late, Module, Simplify};
use SlotKind::{Missing, Old, Partial, Translated};

/// Every slot of the -O2 function pipeline in `llvm_order` (-O1 builds a
/// subset), then the slots LLVM has that llrm does not.
pub(crate) const REGISTRY: &[Entry] = &[
    entry("lowerexpect@1177", Early, "lower-expect", Translated, "lowerexpect.rs"),
    partial(
        "decide@1178",
        Early,
        "simplifycfg",
        "translated SimplifyCfg at the pin's default options runs here and Decide does not (#2907); untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@1178) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    entry("sroa@1179", Early, "sroa<modify-cfg>", Translated, "sroa.rs"),
    entry("earlycse@1180", Early, "early-cse", Translated, ""),
    entry("promote@1241", Cleanup, "mem2reg", Translated, "mem2reg.rs"),
    partial(
        "combine@1242",
        Cleanup,
        "instcombine",
        "criterion: the families of combine.rs `visitors` (add, icmp, gep, store, and what they call: phi, select) match `opt -passes=instcombine` on the cases of their tests; exit: the remaining visit* (sub, and/or/xor, mul/div/rem, shifts, casts, select, phi, load, alloca, calls) translated, then fold@N and dead@N deleted (instcombine-port); the opcodes of foldICmpBinOpWithConstant not yet translated (xor, or, mul, shl, shr, srem, div, sub, add: InstCombineCompares.cpp:4124; only and is, combine_icmp_const.rs) are owed by llrm-w-4, 2026-10-24 onward; on by default, LLRM_NO_COMBINE turns it off",
    ),
    entry("dead@1242", Cleanup, "instcombine", Old, "instcombine-port"),
    partial(
        "decide@1244",
        Cleanup,
        "simplifycfg",
        "translated SimplifyCfg at the default options (convertSwitchRangeToICmp is read by simplifySwitch, untranslated) runs here and Decide does not; untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@1244) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    entry("sroa@646", Simplify, "sroa", Translated, "sroa.rs"),
    entry("earlycse@649", Simplify, "early-cse<memssa>", Translated, ""),
    entry(
        "jumpthreading@668",
        Simplify,
        "jump-threading",
        Translated,
        "jumpthreading.rs + jumpthreading/loadpre.rs; deviations J2 (guards: no deopt bundles in MIR), J3 (!prof/BFI/BPI), J15 (isSafeToSpeculativelyExecute reuse); see docs/architecture/jumpthreading-translation.md",
    ),
    entry("cvp@669", Simplify, "correlated-propagation", Translated, ""),
    entry("jumptableswitch@672", Simplify, "jump-table-to-switch", Translated, "jumptableswitch.rs"),
    partial(
        "decide@675",
        Simplify,
        "simplifycfg, jump-threading, correlated-propagation",
        "translated SimplifyCfg at the default options (convertSwitchRangeToICmp is read by simplifySwitch, untranslated) runs here and Decide does not (the jump threading and CVP before it have their own slots; what Decide decided here by value facts they do not is now lost until rows say it matters); untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@675) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    partial(
        "combine@677",
        Simplify,
        "instcombine",
        "criterion: the families of combine.rs `visitors` (add, icmp, gep, store, and what they call: phi, select) match `opt -passes=instcombine` on the cases of their tests; exit: the remaining visit* (sub, and/or/xor, mul/div/rem, shifts, casts, select, phi, load, alloca, calls) translated, then fold@N and dead@N deleted (instcombine-port); the opcodes of foldICmpBinOpWithConstant not yet translated (xor, or, mul, shl, shr, srem, div, sub, add: InstCombineCompares.cpp:4124; only and is, combine_icmp_const.rs) are owed by llrm-w-4, 2026-10-24 onward; on by default, LLRM_NO_COMBINE turns it off",
    ),
    partial(
        "aggressive-instcombine@678",
        Simplify,
        "aggressive-instcombine",
        "TruncInstCombine runs here; criterion: the trunc graphs of tools/difftest/difftest.py pair on synthetic inputs match opt -passes=aggressive-instcombine; exit: foldUnusualPatterns translated (the folds of AggressiveInstCombine.cpp: bit-pattern, popcount, funnel shift, table, load and store merging, lib calls), no date; L2194 is the LTO pipeline's and not a slot",
    ),
    entry("tailrec@688", Simplify, "tailcallelim", Translated, ""),
    partial(
        "decide@690",
        Simplify,
        "simplifycfg",
        "translated SimplifyCfg at the default options (convertSwitchRangeToICmp is read by simplifySwitch, untranslated) runs here and Decide does not; untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@690) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    entry("reassociate@696", Simplify, "reassociate", Translated, ""),
    entry("constraint-elimination@699", Simplify, "constraint-elimination", Translated, ""),
    entry("loopsimplify@764", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@764", Simplify, "lcssa", Old, "lcssa-port"),
    entry("loop-instsimplify@715", Simplify, "loop-instsimplify", Translated, ""),
    entry("loop-simplifycfg@716", Simplify, "loop-simplifycfg", Translated, ""),
    entry("floatloop@724", Simplify, "licm", Old, "licm-port"),
    entry("licm@724", Simplify, "licm<no-allowspeculation>", Translated, ""),
    entry("licm@730", Simplify, "licm<allowspeculation>", Translated, ""),
    entry("trivialunswitch@732", Simplify, "simple-loop-unswitch<trivial> (-O3: nontrivial;trivial)", Translated, ""),
    partial(
        "decide@766",
        Simplify,
        "simplifycfg",
        "translated SimplifyCfg at the default options (convertSwitchRangeToICmp is read by simplifySwitch, untranslated) runs here and Decide does not; untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@766) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    partial(
        "combine@768",
        Simplify,
        "instcombine",
        "criterion: the families of combine.rs `visitors` (add, icmp, gep, store, and what they call: phi, select) match `opt -passes=instcombine` on the cases of their tests; exit: the remaining visit* (sub, and/or/xor, mul/div/rem, shifts, casts, select, phi, load, alloca, calls) translated, then fold@N and dead@N deleted (instcombine-port); the opcodes of foldICmpBinOpWithConstant not yet translated (xor, or, mul, shl, shr, srem, div, sub, add: InstCombineCompares.cpp:4124; only and is, combine_icmp_const.rs) are owed by llrm-w-4, 2026-10-24 onward; on by default, LLRM_NO_COMBINE turns it off",
    ),
    entry("loopsimplify@772", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@772", Simplify, "lcssa", Old, "lcssa-port"),
    entry("loopidiom@737", Simplify, "loop-idiom", Translated, ""),
    entry("loopidioms@737", Simplify, "loop-idiom", Old, "loop-idiom-port (the pricing, noncountable half)"),
    partial(
        "indvars@738",
        Simplify,
        "indvars",
        "SimplifyIndvar, WidenIV, exit values, congruent IVs, canonicalizeExitCondition; optimizeLoopExits and predicateLoopExits once indvars.rs calls exitopt/exitpredicate; LFTR on (D22 closed); criterion: identical to opt on d_faces -O2; exit: exitsink deleted or DEVIATE-numbered; congruent/twin_phis, rewound, exitfold and the old widen path are deleted",
    ),
    entry("loopdeletion@749", Simplify, "loop-deletion", Translated, ""),
    Entry {
        slot: "unroll@753",
        phase: Simplify,
        llvm: "loop-unroll-full",
        kind: Translated,
        replaced_by: "",
        note: "CLOSED row planets -Os (found 2026-10-20 by w-6, withdrawn the same day): the pin's -Os (O2 plus the optsize attribute) keeps the 4-trip f32 sum over four constant stores to a frame slot, as head does (opt -passes='default<O2>' on that IR with optsize keeps the loop, without it folds); clang folded a C array literal only because its frontend initializes the array from a constant global by memcpy, which the unroll cost reads as a large saving. A frontend-shape difference (Nib/C array literal), not the unroll cost or LFTR (21aadae64 only stopped the -O1 fold, where the pin keeps the loop too)",
    },
    entry("sroa@776", Simplify, "sroa", Translated, "sroa.rs"),
    entry("mldst-motion@783", Simplify, "mldst-motion", Translated, "mldstmotion.rs"),
    entry("inferspace@787", Simplify, "infer-address-spaces (target)", Old, ""),
    entry("gvn@787", Simplify, "gvn", Translated, ""),
    entry("sccp@792", Simplify, "sccp", Translated, ""),
    entry("bdce@797", Simplify, "bdce", Translated, ""),
    partial(
        "combine@801",
        Simplify,
        "instcombine",
        "criterion: the families of combine.rs `visitors` (add, icmp, gep, store, and what they call: phi, select) match `opt -passes=instcombine` on the cases of their tests; exit: the remaining visit* (sub, and/or/xor, mul/div/rem, shifts, casts, select, phi, load, alloca, calls) translated, then fold@N and dead@N deleted (instcombine-port); the opcodes of foldICmpBinOpWithConstant not yet translated (xor, or, mul, shl, shr, srem, div, sub, add: InstCombineCompares.cpp:4124; only and is, combine_icmp_const.rs) are owed by llrm-w-4, 2026-10-24 onward; on by default, LLRM_NO_COMBINE turns it off",
    ),
    entry("dfajumpthreading@806", Simplify, "dfa-jump-threading", Translated, ""),
    entry(
        "jumpthreading@809",
        Simplify,
        "jump-threading",
        Translated,
        "jumpthreading.rs + jumpthreading/loadpre.rs; deviations J2 (guards: no deopt bundles in MIR), J3 (!prof/BFI/BPI), J15 (isSafeToSpeculativelyExecute reuse); see docs/architecture/jumpthreading-translation.md",
    ),
    entry("cvp@810", Simplify, "correlated-propagation", Translated, ""),
    entry("adce@815", Simplify, "adce", Translated, ""),
    entry("memcpyopt@818", Simplify, "memcpyopt", Translated, ""),
    partial(
        "merge@818",
        Simplify,
        "memcpyopt (tryMergingIntoMemset), MergeConsecutiveStores (backend)",
        "criterion: the memsets are MemCpyOpt's (memcpyoptimizer.rs, checked against opt by tools/llvmport/mccompare.py); exit: the wider constant stores of loopidioms::Merge go with the backend MergeConsecutiveStores (lir-cleanup-port)",
    ),
    partial(
        "dse@820",
        Simplify,
        "dse",
        "criterion: DSE stage 1 (deadstore.rs) removes what opt -passes=dse does on near pointers (tools/llvmport/dsecompare.py, 9000 cases); exit: stages 2-3 (free as a terminator, calloc folding, dominating conditions)",
    ),
    entry("loopsimplify@823", Simplify, "loop-simplify", Old, "loopsimplify-port"),
    entry("lcssa@823", Simplify, "lcssa", Old, "lcssa-port"),
    entry("licm@823", Simplify, "licm<allowspeculation>", Translated, ""),
    partial(
        "combine@1403",
        Late,
        "instcombine",
        "criterion: as combine@1242 (the same translated families), a late InstCombine of the pin's default pipeline that cleans what SimplifyCFG@1438 and the unroll leave; exit: as combine@1242",
    ),
    partial(
        "combine@1464",
        Late,
        "instcombine",
        "criterion: as combine@1242 (the same translated families), a late InstCombine of the pin's default pipeline that cleans what SimplifyCFG@1438 and the unroll leave; exit: as combine@1242",
    ),
    partial(
        "combine@1499",
        Late,
        "instcombine",
        "criterion: as combine@1242 (the same translated families), a late InstCombine of the pin's default pipeline that cleans what SimplifyCFG@1438 and the unroll leave; exit: as combine@1242",
    ),
    entry("licm@1508", Late, "licm<allowspeculation>", Translated, ""),
    partial(
        "decide@832",
        Simplify,
        "simplifycfg",
        "translated SimplifyCfg at the option set of that slot (hoist and sink common code on; convertSwitchToArithmetic and convertSwitchRangeToICmp are read by simplifySwitch, untranslated) runs here and Decide does not; untranslated: SimplifyCondBranchToCondBranch, simplifyUncondBranch's remainder, simplifySwitch, simplifyUnreachable's noreturn and assume folds; criterion: opt -passes=simplifycfg on the same inputs (tools/difftest/difftest.py inputs decide@832) is identical on the rows corpus; exit: those pieces translated (simplifycfg-fold-2148, 2026-10-23)",
    ),
    partial(
        "combine@837",
        Simplify,
        "instcombine",
        "criterion: the families of combine.rs `visitors` (add, icmp, gep, store, and what they call: phi, select) match `opt -passes=instcombine` on the cases of their tests; exit: the remaining visit* (sub, and/or/xor, mul/div/rem, shifts, casts, select, phi, load, alloca, calls) translated, then fold@N and dead@N deleted (instcombine-port); the opcodes of foldICmpBinOpWithConstant not yet translated (xor, or, mul, shl, shr, srem, div, sub, add: InstCombineCompares.cpp:4124; only and is, combine_icmp_const.rs) are owed by llrm-w-4, 2026-10-24 onward; on by default, LLRM_NO_COMBINE turns it off",
    ),
    // Not slots: SimplifyCFG at PassBuilderPipelines.cpp#L873
    // (`addPreInlinerPasses`, PGO instrumentation or use only) and #L1424
    // (`ExtraVectorizerPasses`, after the vectorizer) are not in the default
    // pipeline, and llrm has neither a profile nor a vectorizer.
    entry(
        "loop-interchange@1631",
        Late,
        "loop-interchange",
        Translated,
        "loopinterchange.rs, loopinterchange_transform.rs; deviations D1-D6 of its header (LoopInfo made again after a swap, no LoopCacheAnalysis so the cache rule decides nothing, no remarks); output equal to opt's on 108 runs of the pin's LoopInterchange tests (tools/loopinterchange_oracle.py)",
    ),
    // l.1474: a loop pass manager of its own, before LoopUnrollPass.
    entry(
        "loop-unroll-and-jam@1474",
        Late,
        "loop-unroll-and-jam",
        Translated,
        "loopunrollandjam.rs, loopunrollruntime.rs (the epilog subset of UnrollRuntimeLoopRemainder); parity with the pin: output equal to opt's on 16 of the 24 runs of the pin's LoopUnrollAndJam tests, the other 8 classified in tools/loopunrollandjam_expected.toml (tools/loopunrollandjam_oracle.py); deviations D1-D7 of its header: D1 on at -O3 and -Omax only (the pin's default pipelines have it off), exit none, the particle -O3 row; D2 no loop metadata; D3 LoopInfo made again; D4 epilog only; D5 no remarks; D6 BasicAA lacks aliasPHI (exit: BasicAA); D7 isKnownViaInduction is a stub (exit: S5)",
    ),
    // l.1476: LoopUnrollPass, after the jam.
    partial(
        "unroll@1476",
        Late,
        "loop-unroll",
        "the full unroll of loopunrollpass.rs runs here; partial, runtime and peel unrolling are not in this slot (X86 sets no partial or runtime unrolling; peeling is the peel step); criterion: tile -O2 clocks within 5% of the pin's; exit: peeling in this slot",
    ),
    entry("sroa@1494", Late, "sroa<preserve-cfg>", Translated, "sroa.rs"),
    // l.1400: after `infer-alignment`; LoopLoadElimination at the target's
    // vector width (no vector registers on llrm's targets).
    partial(
        "loop-load-elim@1400",
        Late,
        "loop-load-elim",
        "translated LoopLoadElimination (looploadelim.rs) runs here, output equal to opt's on its tests; criterion: x_prefix clocks at or below main at -O1/-O2/-O3/-Os (-O2: 47985 against 44454 clocks, 395 against 174 bytes; base was 50377 before the slot); exit: the assembly of the three x_prefix loops against main's tells which pass differs (laa-runtime-checks-loadelim, 2026-11-06)",
    ),
    partial(
        "simplifycfg@1438",
        Late,
        "simplifycfg",
        "translated SimplifyCfg at hoistCommonInsts, sinkCommonInsts and no canonical loops runs here; The jam follows at L1474 as at the pin; untranslated: speculateUnpredictables and hoistLoadsStoresWithCondFaulting; criterion: opt -passes=simplifycfg on the same inputs is identical on the rows corpus; exit: those translated (llrm-w-4, 2026-10-30)",
    ),
    partial(
        "slp@1455",
        Late,
        "slp-vectorizer",
        "slpcompares.rs makes the wide compare the pin's SLP, VectorCombine and InstCombine leave of an and-chain of equalities of adjacent bytes; no vectors; criterion: the chain of eight bytes is one ldr and one cmp as the pin's (tests/slp_compares.rs); exit: vector types in the backend",
    ),
    entry("infer-alignment@1362", Late, "infer-alignment", Translated, ""),
    entry("infer-alignment@1498", Late, "infer-alignment", Translated, ""),
    entry("float2int@1594", Late, "float2int", Translated, ""),
    entry("lower-constant-intrinsics@1597", Late, "lower-constant-intrinsics", Translated, ""),
    entry("instsimplify@1662", Late, "instsimplify", Translated, ""),
    entry("divrempairs@1667", Late, "div-rem-pairs", Translated, ""),
    entry("tailrec@1677", Late, "tailcallelim", Translated, ""),
    partial(
        "simplifycfg@1682",
        Late,
        "simplifycfg",
        "translated SimplifyCfg at the default options runs here; untranslated: speculateUnpredictables and hoistLoadsStoresWithCondFaulting; criterion: opt -passes=simplifycfg on the same inputs is identical on the rows corpus; exit: those translated (llrm-w-4, 2026-10-30)",
    ),
    // LSR runs in llc's IR preparation (`TargetPassConfig::addIRPasses`,
    // l.874), after this pipeline, so a Module slot (it follows the
    // interprocedural step):
    // `lsr_pass::LoopStrengthReduce`.
    entry("lsr", Module, "loop-reduce (LoopStrengthReduce.cpp)", Translated, ""),
    // Runs in the strength slot,
    // before `lsr`: `canonfreeze::CanonicalizeFreezeInLoops`.
    entry("canonfreeze", Module, "canon-freeze", Translated, ""),
    entry("rotate@728", Simplify, "loop-rotate<header-duplication>", Translated, ""),
    entry("indvars@753", Simplify, "indvars (LPM2 worklist: the loops the unroll made)", Translated, ""),
    entry("vector-combine", Simplify, "vector-combine", Missing, ""),
    // The machine phases of `llc`, one slot per phase of `flow::machine_with`
    // (`module@row`, the row of the table in `backend/mod.rs`).
    entry("farcall@1", Backend, "x86 far-call lowering (target)", Old, ""),
    entry("cmpsub@1", Backend, "x86 EmitCmp: SUB shares flags with the difference", Translated, ""),
    entry("optimizephis@1", Backend, "opt-phis (OptimizePHIs.cpp)", Translated, ""),
    entry("deadmi@1", Backend, "dead-mi-elimination (DeadMachineInstructionElim.cpp)", Translated, ""),
    entry("phiconst@2", Backend, "none (shared phi constants)", Old, ""),
    entry("processimpdefs@2", Backend, "process-implicit-defs (ProcessImplicitDefs.cpp)", Translated, ""),
    entry("ssaspill@3", Backend, "none (spiller route)", Old, ""),
    entry("machinelicm@4", Backend, "early-machinelicm", Translated, ""),
    partial(
        "machinecse@4",
        Backend,
        "machine-cse",
        "criterion: a repeated pure computation before phi elimination is made once, rows identical or better, the post-RA machinecse step deleted (row CSE5); exit: PerformSimplePRE and the invariant-load candidate",
    ),
    partial(
        "machinesink@4",
        Backend,
        "machine-sink",
        "criterion: hanoi, x_switch_* and mandel keep a phi constant off the path that does not read it; exit: store and load sinking along critical edges (hasStoreBetween), SinkAndFold, the pressure test of isProfitableToSinkTo",
    ),
    partial(
        "peepholeopt@4",
        Backend,
        "peephole-opt (PeepholeOptimizer.cpp)",
        "criterion: a SUB nothing reads is a CMP before allocation (optimizeCompareInstr, first switch), x_heapsort cmp/sub as the pin's; exit: the rest of optimizeCompareInstr (flags of an earlier instruction reused, NewCC, InstsToUpdate) and the other PeepholeOptimizer rewrites",
    ),
    entry("combiner@4", Backend, "machine-combiner (MachineCombiner.cpp)", Translated, ""),
    entry("liverangeshrink@4", Backend, "lrshrink (LiveRangeShrink.cpp)", Translated, ""),
    entry("phielim@4", Backend, "phi-node-elimination", Old, "lir-cleanup-poc"),
    partial(
        "machinesched@5",
        Backend,
        "machine-scheduler (MachineScheduler.cpp)",
        "criterion: a block region longer than half the allocatable registers is ordered bottom-up by pressure (DEVIATE MS3: no pass-through pressure, one limit for all register sets); exit: per-set limits and tracking (row 65 piece 2)",
    ),
    entry("fixupsetcc@5", Backend, "x86-fixup-setcc (X86FixupSetCC.cpp)", Translated, ""),
    entry("callframe@5", Backend, "x86-cf-opt (X86CallFrameOptimization.cpp)", Translated, ""),
    entry("flagscopy@5", Backend, "x86-flags-copy-lowering (X86FlagsCopyLowering.cpp)", Translated, ""),
    entry("leaopt@5", Backend, "x86-optimize-leas (X86OptimizeLEAs.cpp)", Translated, ""),
    entry("floatfold@6", Backend, "x86 isel float folding (target)", Old, ""),
    partial(
        "pushfold@6",
        Backend,
        "x86-cf-opt (X86CallFrameOptimization.cpp)",
        "criterion: a pushed value a load made and nothing else reads is pushed from memory, rows identical or better; exit: none, the pass's reserved-frame input is not made (CG6)",
    ),
    entry("floatassign@7", Backend, "x87 stack slots (regalloc, target)", Old, "regalloc-core-poc"),
    entry("floatalloc@8", Backend, "x86-fp-stackifier (X86FloatingPoint.cpp)", Translated, ""),
    entry("twoaddr@9", Backend, "two-address-instruction", Old, "lir-cleanup-poc"),
    partial(
        "coalesce@10",
        Backend,
        "register-coalescer",
        "RegisterCoalescer translated and the only coalescer (902b53576); graph helpers stay in coalesce.rs for twoaddr and the spiller; untranslated: adjustCopiesBackFrom, removeCopyByCommutingDef, removePartialRedundancy (DEVIATE C4); reMaterializeDef in #2807; join_copy costs 2-3.6x the pin per join; criterion: identical to llc on the rows, bodies with phis included; exit: the interference graph of coalesce.rs gone from twoaddr and the spiller",
    ),
    partial(
        "allocate@11",
        Backend,
        "greedy, spill-code-placement, virtregrewriter, stack-slot-coloring",
        "greedy by default under LLVM's order (#2312) with SplitKit (SplitEditor, trySplit) and InlineSpiller; criterion: LLRM_CHECK_LIVEINTERVALS and LLRM_GREEDY_CHECK pass and rows at or below the old allocator per row; exit: rematerialization at a split (#2513), sub-registers, tryInstructionSplit, hint splitting translated (trySplitAroundHintReg, tryHintsRecoloring: splitkit-port, D-RS3); the fallback gone: greedy_rewritten's rounds and the old `rewritten` (allocate.rs, taken when greedy fails a range); `spiller::_folded_reads` replaced by MachineSink + PeepholeOptimizer::foldLoad (lir-cleanup-port)",
    ),
    entry("parcopy@12", Backend, "phi-node-elimination copies, virtregrewriter", Old, ""),
    entry(
        "prologue@13",
        Backend,
        "prologepilog; shrink-wrap is translated (shrinkwrap.rs) and runs inside it",
        Old,
        "",
    ),
    entry("latecleanup@13", Backend, "machine-latecleanup (MachineLateInstrsCleanup.cpp)", Translated, ""),
    entry("peephole@14", Backend, "peephole-opt, machine-cse, machine-dce", Old, ""),
    entry("fixupbw@14", Backend, "x86-fixup-bw-insts (X86FixupBWInsts.cpp)", Translated, ""),
    entry("fixupleas@14", Backend, "x86-fixup-leas (X86FixupLEAs.cpp)", Translated, ""),
    entry("loopslots@15", Backend, "greedy region splitting (SplitKit)", Old, "regalloc-core-poc"),
    entry("schedule@16", Backend, "post-RA scheduler", Old, ""),
    partial(
        "jumps@17",
        Backend,
        "branch-folder, block-placement, tailduplication",
        "TailMergeBlocks (#2241), block placement chains and loops (#2629); criterion: rows vs min(gcc,pin) per bench pair + oracle sweep (J1, #2307); exit: identity vs llc after placement, branch folding and tail duplication; open: trellises and tail duplication (#2745) and alignBlocks/optimizeBranches (#2760) once landed, irreducible loops after tail dup (D16), BranchFolder merge parts, MBFI/MBPI replaced by BlockOdds (D7)",
    ),
    entry("copyprop@18", Backend, "machine-cp (run 2, after tail duplication)", Translated, ""),
    partial(
        "earlytaildup@1",
        Backend,
        "early-tailduplication",
        "criterion: a join of at most two instructions (its jump counted) goes into every predecessor that jumps to it alone, a one-jump block is bypassed, phis and read-outside values get an SSA update; exit: the pin's merge into the layout predecessor is the same result here (ET3); EH pads and INLINEASM_BR have no counterpart (ET2)",
    ),
    // X86TargetLowering::EmitLoweredSelect: the diamond at selection
    // (lir-cleanup-port, 2026-11-13).
    entry("emit-lowered-select", Backend, "x86-emit-lowered-select", Missing, ""),
    entry("stack-coloring", Backend, "stack-coloring", Missing, ""),
];

/// One analysis of the parity pipeline.
#[derive(Clone, Copy, Debug)]
pub struct AnalysisEntry {
    /// The manager's key for it (`Analysis::NAME`): the name
    /// `LLRM_DEBUG=counts` prints.
    pub key: &'static str,
    /// The Rust type that implements it, which `tools/old_analysis_callers.py`
    /// looks for in the sources.
    pub symbol: &'static str,
    /// LLVM's analysis.
    pub llvm: &'static str,
    pub kind: SlotKind,
    /// Translated but not on by default.
    pub off: bool,
    /// The analyses it asks for (`mssa` depends on alias analysis, #2163).
    pub deps: &'static [&'static str],
    /// What lets it join.
    pub oracle: &'static str,
    /// For a row that is not Translated: what takes its place and who owns that
    /// ("none: DEVIATE or delete candidate" where LLVM has no counterpart).
    pub replaced_by: &'static str,
}

/// An analysis of llrm's own that LLVM's counterpart (`llvm`, or "none")
/// replaces once `replaced_by` lands.
const fn old_analysis(
    key: &'static str,
    symbol: &'static str,
    llvm: &'static str,
    replaced_by: &'static str,
) -> AnalysisEntry {
    AnalysisEntry { key, symbol, llvm, kind: Old, off: false, deps: &[], oracle: "", replaced_by }
}

/// The analyses with a translation or a stand-in the registry follows. The
/// result of `mssa` is handed out mutable (the walker lives inside it).
pub const ANALYSES: &[AnalysisEntry] = &[
    AnalysisEntry {
        key: "dominators",
        symbol: "Dominators",
        llvm: "DominatorTreeAnalysis",
        kind: Translated,
        off: false,
        deps: &[],
        oracle: "tools/llvmport/domcompare.py: DomTree against the pinned LLVM on the corpus (#2292)",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "globals-modref",
        symbol: "GlobalsModRefAnalysis",
        llvm: "GlobalsAA (GlobalsModRef.cpp)",
        kind: Translated,
        off: false,
        deps: &[],
        oracle: "globalsmodref_tests.rs against the rules of GlobalsModRef.cpp at the pin; opt -O2 on noinline callees (pt1n/pt1in cases)",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "loops",
        symbol: "Loops",
        llvm: "LoopAnalysis",
        kind: Translated,
        off: false,
        deps: &["dominators"],
        oracle: "tools/loops_oracle.py: print<loops> of opt at the pin",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "mssa",
        symbol: "MemorySsa",
        llvm: "MemorySSAAnalysis",
        kind: Translated,
        off: false,
        deps: &["aa"],
        oracle: "shadow_mssa equality on dumped pass inputs",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "scev",
        symbol: "Scevs",
        llvm: "ScalarEvolutionAnalysis",
        kind: Translated,
        off: false,
        deps: &["dominators", "loops", "assumes"],
        oracle: "tools/scev/golden.py: expressions against the text the pinned LLVM prints",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "assumes",
        symbol: "Assumes",
        llvm: "AssumptionAnalysis",
        kind: Translated,
        off: false,
        deps: &[],
        oracle: "the llvm.assume calls of the function (scev/analysis.rs)",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "lvi",
        symbol: "LviCache",
        llvm: "LazyValueAnalysis",
        kind: Translated,
        off: false,
        deps: &["dominators", "assumes"],
        oracle: "tools/llvmport/lvi_dump.cpp: LazyValueInfo of the pinned LLVM (#2344, #2551)",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "branch-probabilities",
        symbol: "BranchProbabilityInfo",
        llvm: "BranchProbabilityAnalysis",
        kind: Translated,
        off: false,
        deps: &["dominators", "loops"],
        oracle: "print<branch-prob> of opt at the pin, edge for edge on QCport (#2704)",
        replaced_by: "",
    },
    AnalysisEntry {
        key: "block-frequencies",
        symbol: "BlockFrequencies",
        llvm: "BlockFrequencyAnalysis",
        kind: Partial,
        off: false,
        deps: &["loops"],
        oracle: "blockfreq_golden.ll",
        replaced_by: "BlockFrequencyInfoImpl is translated; its edge probabilities are still branchprob::estimated (D1b), exit: read branch-probabilities",
    },
    // The rest are llrm's own analyses. Their callers count as old callers.
    old_analysis(
        "scalar-evolution",
        "ScalarEvolution",
        "ScalarEvolutionAnalysis",
        "scev (add recurrences alone; passes.rs): its callers move to scev",
    ),
    old_analysis(
        "assumptions",
        "AssumptionCache",
        "AssumptionAnalysis",
        "assumes: two analyses stand for one LLVM analysis; callers move to assumes",
    ),
    old_analysis(
        "demanded",
        "DemandedBits",
        "DemandedBitsAnalysis",
        "InstCombine SimplifyDemandedBits translation (instcombine-port); deletes it with algebraic::_unread_bits",
    ),
    old_analysis(
        "globals-aa",
        "GlobalsAA",
        "GlobalsAA (GlobalsModRef.cpp)",
        "globalsmodref.rs (funcattrs-port, #2636), caching on top (parity-pipeline)",
    ),
    old_analysis(
        "forged-selectors",
        "ForgedSelectors",
        "none",
        "none: DEVIATE or delete candidate (a data-segment pointer forged from an integer is a real-mode fact; globals-aa reads it)",
    ),
    old_analysis(
        "summaries",
        "Summaries",
        "FunctionAttrs memory effects + GlobalsModRef FunctionInfo",
        "funcattrs-port (FunctionAttrs inference) and globalsmodref FunctionInfo",
    ),
    old_analysis(
        "summaries from the log",
        "counted(\"summaries from the log\")",
        "none",
        "with summaries: funcattrs-port",
    ),
    old_analysis("summaries updated", "counted(\"summaries updated\")", "none", "with summaries: funcattrs-port"),
    old_analysis("callee-effects", "CalleeEffects", "none", "none: DEVIATE or delete candidate (passes.rs)"),
    old_analysis(
        "call-graph",
        "CallGraphAnalysis",
        "CallGraphAnalysis",
        "no oracle yet: its doc names the LLVM analysis, unproven",
    ),
    old_analysis(
        "call-registers",
        "CallRegisters",
        "none",
        "none: backend facts in a mid-end analysis (callgraph / regalloc owner)",
    ),
    old_analysis(
        "global-sizes",
        "GlobalSizes",
        "none",
        "none: DEVIATE or delete candidate (DataLayout / GlobalVariable sizes are read directly at the pin)",
    ),
    old_analysis(
        "type-ancestry",
        "TypeAncestry",
        "none",
        "none: DEVIATE; TypeBasedAA is the pin's counterpart (not translated)",
    ),
    old_analysis("declarations", "Declarations", "none", "none: DEVIATE or delete candidate"),
    old_analysis(
        "callees",
        "counted(\"callees\")",
        "none",
        "none: DEVIATE or delete candidate (a scan of the body's callees, memory.rs)",
    ),
    old_analysis(
        "declared names",
        "counted(\"declared names\")",
        "none",
        "none: DEVIATE or delete candidate (the name table of declarations, passes.rs)",
    ),
    old_analysis("shape", "Shape", "none", "bundle of dominators, loops and reaching; goes with its callers"),
    old_analysis(
        "exposed-frames",
        "ExposedFrames",
        "CaptureTracking (PointerMayBeCaptured)",
        "capturetracking.rs is translated; callers move to it (frameescape::exposed_allocas goes)",
    ),
    old_analysis(
        "registers",
        "Registers",
        "ScalarEvolution add recurrences",
        "scev (indvars chain, scev-createscev-2239)",
    ),
    old_analysis("followers", "Followers", "ScalarEvolution add recurrences", "scev (induction::followers_all goes)"),
    old_analysis("counted", "Counted", "ScalarEvolution trip counts", "scev (induction::counted_all goes)"),
    old_analysis(
        "dominated-edges",
        "DominatedEdges",
        "LazyValueInfo / ConstraintElimination",
        "LVI and CVP callers; ranges::dominated_edges goes (vt-constantfolding)",
    ),
    old_analysis(
        "pointer-values",
        "PointerValues",
        "BasicAA + CaptureTracking",
        "basicaa-port (aa.rs): alias::point_values goes",
    ),
    old_analysis("points-to", "Pointers", "BasicAA", "basicaa-port (aa.rs): alias::pointers goes"),
    old_analysis("annotated", "Annotated", "MemoryLocation", "basicaa-port: references.rs MemoryLocation callers"),
    old_analysis("writes", "Writes", "AAResults::getModRefInfo", "basicaa-port with GlobalsModRef"),
    old_analysis(
        "call-footprints",
        "CallFootprints",
        "AAResults::getModRefInfo(Call)",
        "basicaa-port with GlobalsModRef: one walk of a body's calls for Writes and Accesses",
    ),
    old_analysis("plain-accesses", "PlainAccesses", "MemorySSA", "mssa (memoryssa-port)"),
    old_analysis("accesses", "Accesses", "MemorySSA", "mssa (memoryssa-port)"),
    old_analysis(
        "base-accesses",
        "BaseAccesses",
        "MemorySSA",
        "mssa (memoryssa-port): MemorySSA's clients read the AA stack, not provenance",
    ),
    old_analysis("through-memory", "ThroughMemory", "MemorySSA + GVN value numbering", "mssa and the GVN translation"),
    old_analysis(
        "float-facts",
        "FloatFacts",
        "ValueTracking (computeKnownFPClass) + MemorySSA",
        "vt-constantfolding (floating-point InstSimplify) and mssa",
    ),
    old_analysis("table-loads", "TableLoads", "none", "none: DEVIATE or delete candidate (consts::is_table_load)"),
    old_analysis(
        "loop-products",
        "Products",
        "none",
        "none: lsr's own prices; block-frequencies once lsr is translated",
    ),
    old_analysis(
        "pressure",
        "Pressure",
        "RegisterPressure / LiveIntervals",
        "the backend live-interval translation (regalloc owner)",
    ),
];

/// What is still open in the parity pipeline's use of the manager. A pass that
/// keeps MemorySSA by updating it says `PreservedAnalyses::preserve_updated`
/// and puts the result back (`Analyses::replace`); EarlyCSE does. LLVM's LICM
/// and GVN keep MemorySSA the same way at the pin; here they do not yet.
pub const TODO: &[&str] = &[
    "LICM updates its clone of MemorySSA (MemorySSAUpdater) and the manager drops it with the pass: it does not `replace` it yet",
    "GVN (memoryssa mode) does not ask for MemorySSA",
];

/// How many times LLVM runs a slot's pass over one body in one visit of the
/// function (`PassBuilderPipelines.cpp`: the function simplification pipeline
/// runs once per function, so `gvn` once; llvm-difftest counted 11 on d_faces
/// -O2 for its 11 functions). The slot name is as in `llvm_order`.
pub const RUNS_PER_BODY: &[(&str, [usize; 4])] = &[("gvn@787", [0, 1, 1, 1]), ("dfajumpthreading@806", [0, 1, 1, 1])];

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

/// Registry slots the pin's `default<O1>` does not have (no GVN, JumpThreading,
/// ConstraintElimination or TailCallElim at -O1:
/// `buildO1FunctionSimplificationPipeline`), so a Translated one is not
/// required in a -O1 list.
pub const ABSENT_AT_O1: &[&str] = &[
    "gvn@787",
    "jumpthreading@668",
    "jumpthreading@809",
    "dfajumpthreading@806",
    "tailrec@688",
    "aggressive-instcombine@678",
    "jumptableswitch@672",
    "constraint-elimination@699",
    "mldst-motion@783",
    "slp@1455",
];

/// Run counts LLVM does not match yet, stated and not checked:
/// `loopsimplify`/`lcssa` run at three unconditional slots per body at every
/// level (x_hanoi2: ours 9/9/15/9 at -O1/-O2/-O3/-Os against LLVM's
/// 4/10/10/10).
pub const RUN_COUNTS_OPEN: &[&str] = &["loopsimplify@764,772,823 and lcssa: one pair per loop-pass adaptor"];

/// The phase and kind of the slot named `slot` (`sroa@1179`), if the registry
/// has it as a function list's slot: what `llrm-pass --order` reports before it
/// runs.
pub fn describe(slot: &str) -> Option<(Phase, SlotKind, &'static str)> {
    REGISTRY
        .iter()
        .find(|one| one.slot == slot && !matches!(one.phase, Phase::Module | Phase::Backend))
        .map(|one| (one.phase, one.kind, one.llvm))
}

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

/// A slot the registry calls Old (or Missing) that runs a translated pass
/// (`FunctionPass::translation`): the row lags the wiring. `held` is each built
/// slot with the translations it runs.
pub(crate) fn check_old_slots(
    registry: &[Entry],
    held: &[(&str, Vec<&'static str>)],
) -> Result<(), String> {
    let lagging: Vec<String> = held
        .iter()
        .filter_map(|(slot, translations)| {
            let row = registry.iter().find(|one| one.slot == *slot)?;
            let pass = translations.first()?;
            matches!(row.kind, Old | Missing)
                .then(|| format!("{slot} ({}) runs the translated {pass}", row.kind.name()))
        })
        .collect();
    if lagging.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "slots the registry calls unfinished run translated passes, mark them Partial or Translated: {}",
            lagging.join("; ")
        ))
    }
}

/// (file stem, pass) pairs of [`check_missing_has_no_file`], as `letters` makes
/// them.
#[cfg(test)]
const SOURCE_NAMES: &[(&str, &str)] = &[("constraintelim", "constraintelimination")];

/// A Missing entry whose pass has a source file (`shrinkwrap.rs` for
/// `shrink-wrap`): the row undercounts. `stems` are the file stems of every
/// `.rs` under `crates`; names compare by their letters and digits, without a
/// leading `x86`; `SOURCE_NAMES` lists the stems that spell their pass
/// differently.
#[cfg(test)]
pub(crate) fn check_missing_has_no_file(
    registry: &[Entry],
    stems: &[String],
) -> Result<(), String> {
    let letters = |name: &str| -> String {
        let name: String = name.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase();
        name.strip_prefix("x86").unwrap_or(&name).to_string()
    };
    let lying: Vec<String> = registry
        .iter()
        .filter(|one| one.kind == Missing)
        .filter(|one| {
            stems
                .iter()
                .any(
                    |stem| {
                        let (stem, name) = (letters(stem), letters(one.llvm));
                        stem == name || SOURCE_NAMES.contains(&(stem.as_str(), name.as_str()))
                    },
                )
        })
        .map(|one| format!("{} ({})", one.slot, one.llvm))
        .collect();
    if lying.is_empty() {
        Ok(())
    } else {
        Err(format!("registry entries marked Missing whose pass has a source file: {}", lying.join("; ")))
    }
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
    check_phase_with(phase, built, level, true, true, crate::pipeline::Options::default().rotate_slot, &[], &[])
}

/// [`check_phase_at`] with the explicit off switches: the slots whose
/// translation is `sroa.rs` are Old when `LLRM_NO_SROA` is set, those of
/// `mem2reg.rs` when `LLRM_NO_MEM2REG` is, and the old stand-in is what runs;
/// with `rotate` false (the default) the rotate slot is not run. `off` are
/// the slots an option switches off, which are not required.
pub(crate) fn check_phase_with(
    phase: Phase,
    built: &[&str],
    level: Level,
    sroa: bool,
    mem2reg: bool,
    rotate: bool,
    off: &[&str],
    held: &[(&str, Vec<&'static str>)],
) -> Result<(), String> {
    let absent: Vec<&str> =
        if level == Level::O1 { ABSENT_AT_O1.iter().chain(off).copied().collect() } else { off.to_vec() };
    // The translated `loop-rotate` is in the order unless the option is off.
    let registry: Vec<Entry> = REGISTRY
        .iter()
        .map(|one| {
            let off = (!sroa && one.replaced_by == "sroa.rs")
                || (!mem2reg && one.replaced_by == "mem2reg.rs")
                || (!rotate && one.slot == "rotate@728");
            Entry { kind: if off { Old } else { one.kind }, ..*one }
        })
        .collect();
    check_without(&registry, phase, built, &absent)?;
    check_old_slots(&registry, held)?;
    llrm_mir::counts::joined(joined_slots(&registry, built));
    Ok(())
}

/// [`check_phase`] for the machine phases `llc` runs: `built` is the slot of
/// each phase of `flow::machine_with` that runs, and `off` the slots the
/// options leave out (-O0 drops the optional phases).
pub fn check_backend(
    built: &[&str],
    off: &[&str],
) -> Result<(), String> {
    check_without(REGISTRY, Phase::Backend, built, off)?;
    llrm_mir::counts::joined(joined_slots(REGISTRY, built));
    Ok(())
}

impl SlotKind {
    /// The word the `[parity]` lines print.
    pub const fn name(self) -> &'static str {
        match self {
            Translated => "translated",
            Partial => "partial",
            Old => "old",
            Missing => "missing",
        }
    }
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
    out.extend(analyses.iter().map(|one| format!("[parity] analysis {} {} {}", one.key, one.kind.name(), one.symbol)));
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
