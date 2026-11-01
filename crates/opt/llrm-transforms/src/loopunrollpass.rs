//! `LoopFullUnrollPass`: a loop whose trip count is known and small enough, or
//! whose unrolled form simplifies enough, is copied out completely.
//!
//! # How it works
//! For each loop, innermost first, `try_to_unroll_loop` gathers the unrolling
//! preferences (the thresholds, as the target and the optimization level set
//! them), prices the loop with `CodeMetrics` (`UnrollCostEstimator`), reads the
//! trip count from ScalarEvolution (the smallest exact trip count of any
//! exiting block, else the maximum), and asks `compute_unroll_count` for a
//! count. The count is the trip count when
//! - the unrolled size is under the threshold (`should_full_unroll`), or
//! - it is not, but `analyze_loop_unroll_cost` simulates every iteration with
//!   [`UnrolledInstAnalyzer`](llrm_analysis::loopunrollanalyzer) and finds the
//!   unrolled cost under the threshold boosted by the dynamic cost saved.
//!
//! A count that is not the whole trip count would be partial or runtime
//! unrolling, which this pass (`OnlyFullUnroll`) does not do. The loop is then
//! unrolled by [`crate::loopunroll::unroll_loop_blocks`].
//!
//! # Deviations
//! - U3 `computePeelCount` is `looppeel.rs` (S2); its deviations are PL1 to PL5
//!   there.
//! - U4 The pragma metadata (`llvm.loop.unroll.*`) is read from the loop ID on
//!   the latch's branch; no frontend of llrm writes it yet. Remarks (the
//!   `OptimizationRemarkEmitter`), `UniformityInfo` and profile data are left
//!   out: D3 of the plan page, and MIR has no convergence tokens or PSI.
//! - U5 `Function::hasOneLiveUse` of an internal callee is found by scanning
//!   the module for the callee's uses; no use list is kept for globals.
//! - U6 The loop pass manager's update of loops (`addSiblingLoops`) is the
//!   driver's: the loops a complete unroll makes of the inner loops are visited
//!   next, in program order.
//! - U8 `simplifyLoopAfterUnroll` simplifies without a dominator tree or
//!   assumptions.
//! - U9 Blocks an unroll merges away are erased once, when the pass ends.
//! - U1 At `optsize` a loop of at most the target's few trips is unrolled under
//!   the target's bar (`Tti::unroll_size_few_trips`; m16: 3 trips, 30) where
//!   the pin's `OptSizeThreshold` is 0 on x86. float_in_place_update.c at -Os
//!   m16: 132 bytes to 14 (main 76); the corpus of 258 programs at m16 -Os
//!   changes in no other. logical_value.c (main 25, here 155), split_loops.c
//!   and int64_division.c need the cost analysis held to a bar too, which grew
//!   matmul.nib 390 to 562 bytes: open.
//!
//! # Example
//! A loop of four trips becomes four copies; the induction variable's
//! arithmetic on constants is simplified away.
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
//!   %next = add i16 %i, 1
//!   %go = icmp slt i16 %next, 4
//!   br i1 %go, label %b1, label %b2
//! ```
//! becomes, after simplification, copies of the body on `0`, `1`, `2` and `3`
//! with no `phi`, no `icmp` and no backedge.
//!
//! # See also
//! [`tryToUnrollLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1213),
//! [`computeUnrollCount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1016),
//! [`analyzeLoopUnrollCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L336),
//! [`LoopFullUnrollPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1640).

use llrm_analysis::loopunrollanalyzer::{Env, UnrolledInstAnalyzer};
use llrm_analysis::scev::ScalarEvolution;
use llrm_mir::codemetrics::{CodeMetrics, Query, collect_ephemeral_values};
use llrm_mir::context::{ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::domtree::DomTree;
use llrm_mir::instructioncost::{instruction_cost, instruction_cost_with};
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::memory::Callees;
use llrm_mir::module::{
    BlockId, Function, GlobalKind, GlobalValue, InstId, MetadataNode, MetadataOperand, Operand, ValueDef,
};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::{Cost, CostKind, Tti, UnrollingPreferences};
use llrm_mir::valuetracking::AssumptionCache;

use crate::looppeel::PeelingPreferences;
use crate::loopunroll::{Exit, Options as UnrollOptions, Unrolled, unroll_loop_blocks};

const UNROLL_THRESHOLD_DEFAULT: u32 = 150;
const UNROLL_THRESHOLD_AGGRESSIVE: u32 = 300;
const UNROLL_MAX_PERCENT_THRESHOLD_BOOST: u32 = 400;
const UNROLL_MAX_ITERATIONS_COUNT_TO_ANALYZE: u32 = 10;
const UNROLL_MAX_UPPER_BOUND: u32 = 8;
const PRAGMA_UNROLL_THRESHOLD: u32 = 16 * 1024;
const PRAGMA_UNROLL_FULL_MAX_ITERATIONS: u32 = 1_000_000;
const NO_THRESHOLD: u32 = u32::MAX;

/// What the pass is asked: `OptLevel` and the function's size attributes.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// `OptLevel`: above 2, the aggressive threshold.
    pub opt_level: u32,
    /// `hasOptSize`.
    pub opt_size: bool,
    /// `hasMinSize`.
    pub min_size: bool,
    /// `OnlyWhenForced`.
    pub only_when_forced: bool,
    pub prepare_for_lto: bool,
}

/// What the pass reads and changes.
pub struct Run<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub callees: &'a Callees,
    pub this: Option<GlobalId>,
    pub tti: &'a dyn Tti,
    pub function: &'a mut Function,
    pub options: Options,
    /// Blocks an unroll merged away, erased when the pass is done.
    pub emptied: Vec<BlockId>,
    /// The module's analyses, and what the pass declares in it, for the IV
    /// simplifier (PL5).
    pub outer: &'a llrm_mir::passes::Outer,
    pub declared: &'a mut llrm_mir::passes::Declared,
    /// The iterations peeled off the loop headed by a block in this run (PL1).
    pub peeled: IdMap<BlockId, u32>,
    /// The headers of the loops this run made: those that were no loop's header
    /// when it began (`LPMUpdater::addSiblingLoops`, for the passes after it).
    pub created: Vec<BlockId>,
    /// The expressions of the passes before (`Scevs`), current while the
    /// function is as they left it: dropped at the first change.
    pub scev: ScalarEvolution,
}

/// `TransformationMode`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Unspecified,
    Disable,
    SuppressedByUser,
    ForcedByUser,
}

impl Mode {
    pub(crate) fn forced(self) -> bool {
        self == Mode::ForcedByUser
    }
}

/// The options of a loop's `llvm.loop` ID: `(name, operands after it)`.
pub(crate) struct LoopOptions(Vec<(String, Vec<MetadataOperand>)>);

impl LoopOptions {
    /// The loop ID of `lp` (a node whose first operand is itself), as
    /// `Loop::getLoopID`.
    pub(crate) fn of(
        run: &Run,
        lp: Loop,
    ) -> Self {
        let function = &*run.function;
        let id = lp.get_loop_latch(function).and_then(|latch| function.terminator(latch)).and_then(|term| {
            function.instruction(term).metadata.iter().find(|(kind, _)| kind == "llvm.loop").map(|(_, id)| *id)
        });
        let node = |at: llrm_mir::module::MetadataId| run.metadata.get(at.0 as usize);
        let Some(id) = id else { return Self(Vec::new()) };
        let Some(loop_id) = node(id) else { return Self(Vec::new()) };
        if !matches!(
            loop_id.operands.first(),
            Some(MetadataOperand::Node(first)) if *first == id
        ) {
            return Self(Vec::new());
        }
        let options = loop_id.operands[1..]
            .iter()
            .filter_map(|one| match one {
                MetadataOperand::Node(option) => node(*option),
                _ => None,
            })
            .filter_map(|option| match option.operands.split_first() {
                Some((MetadataOperand::String(name), rest)) => Some((name.clone(), rest.to_vec())),
                _ => None,
            })
            .collect();
        Self(options)
    }

    pub(crate) fn find(
        &self,
        name: &str,
    ) -> Option<&Vec<MetadataOperand>> {
        self.0.iter().find(|(one, _)| one == name).map(|(_, rest)| rest)
    }

    /// `hasAnyUnrollPragma(L, Prefix)`: an option whose name starts with
    /// `prefix`.
    pub(crate) fn has_any(
        &self,
        prefix: &str,
    ) -> bool {
        self.0.iter().any(|(name, _)| name.starts_with(prefix))
    }

    /// `getBooleanLoopAttribute`.
    pub(crate) fn boolean(
        &self,
        run: &Run,
        name: &str,
    ) -> bool {
        match self.find(name) {
            None => false,
            Some(rest) => rest.first().is_none_or(|one| int_of(run, one).is_none_or(|value| value != 0)),
        }
    }

    /// `getOptionalIntLoopAttribute`.
    pub(crate) fn int(
        &self,
        run: &Run,
        name: &str,
    ) -> Option<u64> {
        self.find(name).and_then(|rest| rest.first()).and_then(|one| int_of(run, one))
    }

    /// `hasDisableAllTransformsHint`.
    fn disables_all(&self) -> bool {
        self.find("llvm.loop.disable_nonforced").is_some()
    }

    /// `hasUnrollTransformation`.
    fn unroll(
        &self,
        run: &Run,
    ) -> Mode {
        if self.boolean(run, "llvm.loop.unroll.disable") {
            return Mode::SuppressedByUser;
        }
        if let Some(count) = self.int(run, "llvm.loop.unroll.count") {
            return if count == 1 { Mode::SuppressedByUser } else { Mode::ForcedByUser };
        }
        if self.boolean(run, "llvm.loop.unroll.enable") || self.boolean(run, "llvm.loop.unroll.full") {
            return Mode::ForcedByUser;
        }
        if self.disables_all() {
            return Mode::Disable;
        }
        Mode::Unspecified
    }

    /// `hasUnrollAndJamTransformation`.
    pub(crate) fn unroll_and_jam(
        &self,
        run: &Run,
    ) -> Mode {
        if self.boolean(run, "llvm.loop.unroll_and_jam.disable") {
            return Mode::SuppressedByUser;
        }
        if let Some(count) = self.int(run, "llvm.loop.unroll_and_jam.count") {
            return if count == 1 { Mode::SuppressedByUser } else { Mode::ForcedByUser };
        }
        if self.boolean(run, "llvm.loop.unroll_and_jam.enable") {
            return Mode::ForcedByUser;
        }
        if self.disables_all() {
            return Mode::Disable;
        }
        Mode::Unspecified
    }
}

pub(crate) fn int_of(
    run: &Run,
    operand: &MetadataOperand,
) -> Option<u64> {
    let MetadataOperand::Constant(id) = operand else { return None };
    match run.context.get(*id).kind {
        ConstantKind::Int(bits) => Some(bits as u64),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `UnrollPragmaInfo`.
pub(crate) struct UnrollPragmaInfo {
    pub(crate) pragma_full_unroll: bool,
    pub(crate) pragma_count: u32,
    pub(crate) pragma_enable_unroll: bool,
    pub(crate) pragma_runtime_unroll_disable: bool,
    pub(crate) explicit_unroll: bool,
}

impl UnrollPragmaInfo {
    fn of(
        run: &Run,
        options: &LoopOptions,
    ) -> Self {
        let pragma_full_unroll = options.boolean(run, "llvm.loop.unroll.full");
        let pragma_count = options.int(run, "llvm.loop.unroll.count").unwrap_or(0) as u32;
        let pragma_enable_unroll = options.boolean(run, "llvm.loop.unroll.enable");
        Self {
            pragma_full_unroll,
            pragma_count,
            pragma_enable_unroll,
            pragma_runtime_unroll_disable: options.boolean(run, "llvm.loop.unroll.runtime.disable"),
            // There is no `-unroll-count` option here.
            explicit_unroll: pragma_count > 0 || pragma_full_unroll || pragma_enable_unroll,
        }
    }
}

/// `UnrollCostEstimator`.
pub(crate) struct UnrollCostEstimator {
    pub(crate) num_inline_candidates: u32,
    pub(crate) not_duplicatable: bool,
    pub(crate) loop_size: Cost,
}

impl UnrollCostEstimator {
    /// `getRolledLoopSize`.
    pub(crate) fn rolled_loop_size(&self) -> u32 {
        self.loop_size.value as u32
    }

    /// `getUnrolledLoopSize`.
    pub(crate) fn unrolled_loop_size(
        &self,
        up: &UnrollingPreferences,
        count: u32,
    ) -> u64 {
        let ls = self.rolled_loop_size();
        debug_assert!(ls >= up.be_insns, "LoopSize should not be less than BEInsns!");
        u64::from(ls - up.be_insns) * u64::from(count) + u64::from(up.be_insns)
    }

    /// `canUnroll`.
    pub(crate) fn can_unroll(&self) -> bool {
        // Convergence tokens are not in MIR (C1): none is `ExtendedLoop`.
        self.loop_size.valid && !self.not_duplicatable
    }
}

/// `EstimatedUnrollCost`.
struct EstimatedUnrollCost {
    unrolled_cost: u32,
    rolled_dynamic_cost: u32,
}

/// The analyses of one state of the function, made again after a change.
pub(crate) struct Analyzed {
    pub(crate) tree: DomTree,
    pub(crate) loops: LoopInfo,
    pub(crate) assumptions: AssumptionCache,
}

impl Analyzed {
    pub(crate) fn of(run: &Run) -> Self {
        let tree = DomTree::of(run.function);
        let loops = LoopInfo::analyze(run.function, || &tree);
        let assumptions = AssumptionCache::of(run.context, run.globals, run.metadata, run.function);
        Self { tree, loops, assumptions }
    }
}

/// `gatherUnrollingPreferences` for the full unroll pass (`UserThreshold`,
/// `AllowPartial`, `Runtime` and `UpperBound` provided: none, false, false,
/// false).
pub(crate) fn gather_unrolling_preferences(
    run: &Run,
    has_calls: bool,
) -> UnrollingPreferences {
    let mut up = gather_unrolling_preferences_of_target(run, has_calls);
    // Apply the values provided by the full unroll pass.
    up.partial = false;
    up.runtime = false;
    up.upper_bound = false;
    if UNROLL_MAX_UPPER_BOUND == 0 {
        up.upper_bound = false;
    }
    up
}

/// `gatherUnrollingPreferences` up to the values a caller provides: the
/// defaults, the target's, the size attributes'.
pub(crate) fn gather_unrolling_preferences_of_target(
    run: &Run,
    has_calls: bool,
) -> UnrollingPreferences {
    let mut up = UnrollingPreferences {
        threshold: if run.options.opt_level > 2 { UNROLL_THRESHOLD_AGGRESSIVE } else { UNROLL_THRESHOLD_DEFAULT },
        max_percent_threshold_boost: UNROLL_MAX_PERCENT_THRESHOLD_BOOST,
        opt_size_threshold: 0,
        partial_threshold: 150,
        partial_opt_size_threshold: 0,
        default_unroll_runtime_count: 8,
        max_count: u32::MAX,
        max_upper_bound: UNROLL_MAX_UPPER_BOUND,
        full_unroll_max_count: u32::MAX,
        be_insns: 2,
        partial: false,
        runtime: false,
        allow_remainder: true,
        unroll_remainder: false,
        allow_expensive_trip_count: false,
        force: false,
        upper_bound: false,
        max_iterations_count_to_analyze: UNROLL_MAX_ITERATIONS_COUNT_TO_ANALYZE,
        unroll_and_jam: false,
        unroll_and_jam_inner_loop_threshold: 60,
    };
    // Override with any target specific settings.
    run.tti.get_unrolling_preferences(has_calls, &mut up);
    // The target's tuning table.
    let (threshold, boost) = run.tti.unroll_threshold_overrides();
    up.threshold = threshold.unwrap_or(up.threshold);
    up.max_percent_threshold_boost = boost.unwrap_or(up.max_percent_threshold_boost);
    // Apply size attributes.
    if run.options.opt_size {
        up.threshold = up.opt_size_threshold;
        up.partial_threshold = up.partial_opt_size_threshold;
        up.max_percent_threshold_boost = 100;
    }
    up
}

/// `getFullUnrollBoostingFactor`.
fn full_unroll_boosting_factor(
    cost: &EstimatedUnrollCost,
    max_percent_threshold_boost: u32,
) -> u32 {
    if cost.rolled_dynamic_cost >= u32::MAX / 100 {
        100
    } else if cost.unrolled_cost != 0 {
        // The boosting factor is RolledDynamicCost / UnrolledCost.
        ((100 * u64::from(cost.rolled_dynamic_cost) / u64::from(cost.unrolled_cost)) as u32)
            .min(max_percent_threshold_boost)
    } else {
        max_percent_threshold_boost
    }
}

/// `UnrolledInstState`, keyed by the instruction and the iteration.
#[derive(Clone, Copy)]
struct InstState {
    is_free: bool,
    is_counted: bool,
}

/// `analyzeLoopUnrollCost`: the cost of the unrolled loop and the dynamic cost
/// of the rolled one, or none where no benefit is expected or the loop is too
/// big to analyze.
#[allow(clippy::too_many_arguments)]
fn analyze_loop_unroll_cost(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    trip_count: u32,
    eph: &IdSet<InstId>,
    max_unrolled_loop_size: u32,
    max_iterations_count_to_analyze: u32,
) -> Option<EstimatedUnrollCost> {
    // Only analyze inner loops: the cost of nested loops cannot be estimated
    // and inner loops are not visited again.
    if !lp.is_innermost() {
        return None;
    }
    // Do not simulate loops with a big or unknown trip count.
    if trip_count == 0 || trip_count > max_iterations_count_to_analyze {
        return None;
    }

    let header = lp.get_header();
    let latch = lp.get_loop_latch(run.function).expect("a latch");
    let preheader = lp.get_loop_preheader(run.function).expect("a preheader");
    let kind = if run.options.min_size { CostKind::CodeSize } else { CostKind::SizeAndLatency };
    let mut se = ScalarEvolution::new();

    let mut simplified_values = IdMap::<llrm_mir::module::ValueId, Operand>::new();
    let mut unrolled_cost = Cost::FREE;
    let mut rolled_dynamic_cost = Cost::FREE;
    let mut inst_cost_map = llrm_support::hash::HashMap::<(InstId, u32), InstState>::default();
    let mut exit_worklist: Vec<(BlockId, BlockId)> = Vec::new();

    // Simulate execution of each iteration of the loop counting instructions
    // which would be simplified.
    for iteration in 0..trip_count {
        // Prepare for the iteration by collecting any simplified entry or
        // backedge inputs.
        let mut simplified_input_values: Vec<(llrm_mir::module::ValueId, Operand)> = Vec::new();
        for inst in run.function.iter_block(header) {
            let operation = run.function.instruction(inst);
            if operation.opcode != Opcode::Phi {
                break;
            }
            // The loop header phis have exactly two inputs: one from the
            // preheader and one from the latch.
            let from = if iteration == 0 { preheader } else { latch };
            let mut value = operation
                .operands
                .chunks(2)
                .find(|pair| pair[1] == Operand::Block(from))
                .map(|pair| pair[0])
                .expect("a header phi with an input from the preheader and the latch");
            if iteration != 0
                && let Operand::Value(one) = value
                && let Some(&simple) = simplified_values.get(&one)
            {
                value = simple;
            }
            simplified_input_values.push((operation.result.expect("a phi's value"), value));
        }

        // Clear and populate the map for the next iteration.
        simplified_values = IdMap::new();
        while let Some((phi, value)) = simplified_input_values.pop() {
            simplified_values.insert(phi, value);
        }

        let mut analyzer = UnrolledInstAnalyzer::new(&mut se, iteration);

        let mut bb_worklist: Vec<BlockId> = vec![header];
        let mut in_worklist = IdSet::<BlockId>::new();
        in_worklist.insert(header);
        // The worklist grows as it is walked.
        let mut at = 0;
        while at < bb_worklist.len() {
            let bb = bb_worklist[at];
            at += 1;

            // Visit all instructions of the block and try to simplify them. The
            // IR is not changed, only optimization opportunities are counted.
            for inst in run.function.iter_block(bb).collect::<Vec<_>>() {
                // These will not get into the final code: do not even try
                // calculating their cost.
                if eph.contains(&inst) {
                    continue;
                }

                // Track this instruction's expected baseline cost when
                // executing the rolled loop form.
                rolled_dynamic_cost = rolled_dynamic_cost
                    + instruction_cost(run.tti, run.context, run.layout, run.globals, run.function, inst, kind);

                // Visit the instruction to analyze its loop cost after
                // unrolling; if the visitor returns true, it is free.
                let is_free = {
                    let mut env = Env {
                        context: &mut *run.context,
                        layout: run.layout,
                        globals: run.globals,
                        metadata: run.metadata,
                        function: &*run.function,
                        loops: &analyzed.loops,
                        tree: &analyzed.tree,
                        assumptions: &analyzed.assumptions,
                        se: &mut se,
                        header,
                    };
                    analyzer.visit(&mut env, &mut simplified_values, inst)
                };
                inst_cost_map.insert((inst, iteration), InstState { is_free, is_counted: false });

                if is_free {
                    continue;
                }

                // Cannot properly model the cost of a call.
                if let Some(callee) = call_target(run, inst) {
                    let lowered = match callee {
                        Some(global) if (global.0 as usize) < run.globals.len() => {
                            let value = &run.globals[global.0 as usize];
                            let name = value.name.as_deref();
                            !name.is_some_and(|name| name.starts_with("llvm."))
                                && run.tti.is_lowered_to_call(
                                    name,
                                    matches!(
                                        value.linkage,
                                        llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                                    ),
                                )
                        }
                        _ => true,
                    };
                    if lowered {
                        why(run, lp, "cannot analyze: a call");
                        return None;
                    }
                }

                // If the instruction might have a side effect, recursively
                // account for the cost of it and all the instructions leading
                // up to it.
                if has_side_effects(run, inst) {
                    add_cost_recursively(
                        run,
                        lp,
                        &simplified_values,
                        &mut inst_cost_map,
                        &mut unrolled_cost,
                        kind,
                        inst,
                        iteration,
                    );
                }

                // If the unrolled body turns out to be too big, bail out.
                if unrolled_cost.value > i64::from(max_unrolled_loop_size) {
                    why(
                        run,
                        lp,
                        &format!("cannot analyze: unrolled cost {} over {max_unrolled_loop_size}", unrolled_cost.value),
                    );
                    return None;
                }
            }

            let terminator = run.function.terminator(bb).expect("a terminated block");

            let simplified_constant = |value: Operand| -> Option<Operand> {
                let value = match value {
                    Operand::Value(one) => simplified_values.get(&one).copied().unwrap_or(value),
                    other => other,
                };
                matches!(value, Operand::Constant(_)).then_some(value)
            };

            // Add the live successors, by first checking whether the terminator
            // may be simplified from the values simplified by this iteration.
            let mut known_succ: Option<BlockId> = None;
            let end = run.function.instruction(terminator);
            if end.opcode == Opcode::Br && end.operands.len() == 3 {
                if let Some(Operand::Constant(condition)) = simplified_constant(end.operands[0]) {
                    // Take the first successor if the condition is undef.
                    let (Operand::Block(first), Operand::Block(second)) = (end.operands[1], end.operands[2]) else {
                        unreachable!("a branch's targets")
                    };
                    known_succ = match run.context.get(condition).kind {
                        ConstantKind::Poison => Some(first),
                        ConstantKind::Int(bits) => Some(if bits == 0 { second } else { first }),
                        ConstantKind::Zero => Some(second),
                        _ => None,
                    };
                }
            } else if end.opcode == Opcode::Switch
                && let Some(Operand::Constant(condition)) = simplified_constant(end.operands[0])
            {
                let Operand::Block(default) = end.operands[1] else { unreachable!("a default target") };
                known_succ = match run.context.get(condition).kind {
                    ConstantKind::Poison => Some(default),
                    ConstantKind::Int(bits) => {
                        let case = end
                            .operands[2..]
                            .chunks(2)
                            .find(
                                |pair| match pair[0] {
                                    Operand::Constant(id) => run.context.get(id).kind == ConstantKind::Int(bits),
                                    _ => false,
                                },
                            );
                        match case.map(|pair| pair[1]) {
                            Some(Operand::Block(target)) => Some(target),
                            _ => Some(default),
                        }
                    }
                    _ => None,
                };
            }
            if let Some(succ) = known_succ {
                if lp.contains(succ) {
                    if in_worklist.insert(succ) {
                        bb_worklist.push(succ);
                    }
                } else if !exit_worklist.contains(&(bb, succ)) {
                    exit_worklist.push((bb, succ));
                }
                continue;
            }

            // Add the block's successors to the worklist.
            for succ in run.function.successors(bb) {
                if lp.contains(succ) {
                    if in_worklist.insert(succ) {
                        bb_worklist.push(succ);
                    }
                } else if !exit_worklist.contains(&(bb, succ)) {
                    exit_worklist.push((bb, succ));
                }
            }
            add_cost_recursively(
                run,
                lp,
                &simplified_values,
                &mut inst_cost_map,
                &mut unrolled_cost,
                kind,
                terminator,
                iteration,
            );
        }

        // If no optimization opportunities were found on the first iteration,
        // none will be found on later ones.
        if unrolled_cost == rolled_dynamic_cost {
            why(run, lp, "cannot analyze: no opportunities");
            return None;
        }
    }

    while let Some((exiting_bb, exit_bb)) = exit_worklist.pop() {
        for &inst in run.function.iter_block(exit_bb).collect::<Vec<_>>().iter() {
            let operation = run.function.instruction(inst);
            if operation.opcode != Opcode::Phi {
                break;
            }
            let value =
                operation.operands.chunks(2).find(|pair| pair[1] == Operand::Block(exiting_bb)).map(|pair| pair[0]);
            if let Some(Operand::Value(one)) = value
                && let llrm_mir::module::ValueDef::Instruction(def) = run.function.value(one).def
                && lp.contains_instruction(run.function, def)
            {
                add_cost_recursively(
                    run,
                    lp,
                    &simplified_values,
                    &mut inst_cost_map,
                    &mut unrolled_cost,
                    kind,
                    def,
                    trip_count - 1,
                );
            }
        }
    }

    debug_assert!(unrolled_cost.valid && rolled_dynamic_cost.valid, "All instructions must have a valid cost");
    why(run, lp, &format!("analysis: unrolled {} rolled {}", unrolled_cost.value, rolled_dynamic_cost.value));
    Some(EstimatedUnrollCost {
        unrolled_cost: unrolled_cost.value as u32,
        rolled_dynamic_cost: rolled_dynamic_cost.value as u32,
    })
}

/// The callee of a call (`Some(None)` where it is not a named function), or
/// none where `inst` is not a call to price.
fn call_target(
    run: &Run,
    inst: InstId,
) -> Option<Option<GlobalId>> {
    match run.function.instruction(inst).opcode {
        Opcode::Call(_) => Some(llrm_mir::memory::callee(run.context, run.function, inst)),
        _ => None,
    }
}

/// `mayHaveSideEffects`.
pub(crate) fn has_side_effects(
    run: &Run,
    inst: InstId,
) -> bool {
    // A branch, switch, return or `unreachable` neither writes nor throws: one
    // whose way is known is not counted, as it was when called a side effect.
    if matches!(
        run.function.instruction(inst).opcode,
        Opcode::Br | Opcode::Switch | Opcode::Ret | Opcode::Unreachable
    )
    {
        return false;
    }
    !llrm_mir::memory::only_value(run.context, run.callees, run.function, inst)
        || llrm_mir::memory::of(run.context, run.callees, run.function, inst).writes
}

/// `AddCostRecursively`: the cost of `root` at `iteration` and of every
/// instruction that leads to it that is not free, going back through the loop
/// phis to earlier iterations.
#[allow(clippy::too_many_arguments)]
fn add_cost_recursively(
    run: &Run,
    lp: Loop,
    simplified_values: &IdMap<llrm_mir::module::ValueId, Operand>,
    inst_cost_map: &mut llrm_support::hash::HashMap<(InstId, u32), InstState>,
    unrolled_cost: &mut Cost,
    kind: CostKind,
    root: InstId,
    iteration: u32,
) {
    let header = lp.get_header();
    let latch = lp.get_loop_latch(run.function).expect("a latch");
    let mut cost_worklist: Vec<InstId> = vec![root];
    let mut phi_used_list: Vec<InstId> = Vec::new();
    let mut iteration = i64::from(iteration);
    loop {
        loop {
            let Some(inst) = cost_worklist.pop() else { break };
            // If an input to a phi comes from a dead path through the loop
            // there may be no cost data for it here: that means it
            // is free.
            let Some(state) = inst_cost_map.get_mut(&(inst, iteration as u32)) else { continue };
            if state.is_counted {
                // Already counted this instruction.
                continue;
            }
            // Mark that the cost of this instruction is being counted now.
            state.is_counted = true;
            let is_free = state.is_free;

            // A phi of the loop header is only added to the phi set.
            let operation = run.function.instruction(inst);
            if operation.opcode == Opcode::Phi && run.function.parent(inst) == Some(header) {
                debug_assert!(is_free, "Loop phis should not be evaluated as they inherently simplify");
                if iteration == 0 {
                    continue;
                }
                // Push the incoming value from the backedge into the phi used
                // list if it is an in-loop instruction; it populates the cost
                // worklist for the previous iteration (counting backwards).
                let from_latch =
                    operation.operands.chunks(2).find(|pair| pair[1] == Operand::Block(latch)).map(|pair| pair[0]);
                if let Some(Operand::Value(one)) = from_latch
                    && let llrm_mir::module::ValueDef::Instruction(def) = run.function.value(one).def
                    && lp.contains_instruction(run.function, def)
                {
                    phi_used_list.push(def);
                }
                continue;
            }

            // First accumulate the cost of this instruction.
            if !is_free {
                // Operands are considered as simplified.
                let operands: Vec<Operand> = operation
                    .operands
                    .iter()
                    .map(|&operand| match operand {
                        Operand::Value(value) => simplified_values.get(&value).copied().unwrap_or(operand),
                        other => other,
                    })
                    .collect();
                *unrolled_cost = *unrolled_cost
                    + instruction_cost_with(
                        run.tti,
                        run.context,
                        run.layout,
                        run.globals,
                        run.function,
                        inst,
                        &operands,
                        kind,
                    );
            }

            // The cost of every operand that is not free must be counted,
            // recursively: a constant or a value defined outside the loop is
            // free.
            for &operand in &operation.operands {
                let Operand::Value(value) = operand else { continue };
                let llrm_mir::module::ValueDef::Instruction(def) = run.function.value(value).def else { continue };
                if !lp.contains_instruction(run.function, def) {
                    continue;
                }
                cost_worklist.push(def);
            }
        }

        if phi_used_list.is_empty() {
            // The search is exhausted.
            break;
        }
        debug_assert!(iteration > 0, "Cannot track phi-used values past the first iteration!");
        cost_worklist.append(&mut phi_used_list);
        iteration -= 1;
    }
}

/// DEVIATE X1 (owner w10, exit 2026-11-06: price it in the CPU model): a full
/// unroll of a loop that reads floats from memory makes SROA promote the cells
/// to values, and where floats live on a stack (x87) more of them than the
/// stack holds spill or `fxch` around the nest (nbody -O2: 3808 dynamic
/// `fxch` against gcc's 1200, which keeps the arrays). The pin has no such
/// check; its x86 `getUnrollingPreferences` is absent. Not taken where the
/// target has registers for the type (`Tti::float_stack`).
fn float_cells_exceed_stack(
    run: &Run,
    lp: Loop,
    trip_count: u32,
) -> bool {
    use llrm_mir::opcode::Opcode;
    use llrm_mir::tti::Ty;
    // A cell the loop both reads and writes is a value carried through
    // memory: what SROA turns into a phi once the loop is gone.
    let mut loads: Vec<llrm_mir::module::Operand> = Vec::new();
    let mut stores: Vec<llrm_mir::module::Operand> = Vec::new();
    let mut stack: Option<u32> = None;
    for &block in lp.get_blocks() {
        for inst in run.function.iter_block(block) {
            let one = run.function.instruction(inst);
            let (pointer, ty) = match one.opcode {
                Opcode::Load { .. } => (one.operands[0], one.ty),
                Opcode::Store { .. } => {
                    let Some(ty) = run.function.operand_type(run.context, one.operands[0]) else { continue };
                    (one.operands[1], ty)
                }
                _ => continue,
            };
            let Some(ty) = Ty::of(&run.context.types, ty).filter(|ty| ty.is_float()) else { continue };
            let Some(size) = run.tti.float_stack(ty) else { continue };
            stack = Some(stack.map_or(size, |seen| seen.min(size)));
            if matches!(one.opcode, Opcode::Load { .. }) { loads.push(pointer) } else { stores.push(pointer) }
        }
    }
    let cells = loads.iter().filter(|pointer| stores.contains(pointer)).count() as u32;
    stack.is_some_and(|size| u64::from(cells) * u64::from(trip_count) > u64::from(size))
}

/// `shouldFullUnroll`.
fn should_full_unroll(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    eph: &IdSet<InstId>,
    full_unroll_trip_count: u32,
    uce: &UnrollCostEstimator,
    up: &UnrollingPreferences,
) -> Option<u32> {
    debug_assert!(full_unroll_trip_count != 0, "should be non-zero!");
    if full_unroll_trip_count > up.full_unroll_max_count {
        return None;
    }

    if float_cells_exceed_stack(run, lp, full_unroll_trip_count) {
        return None;
    }

    // When computing the unrolled size, note that BEInsns are not replicated
    // like the rest of the loop body.
    let unrolled_size = uce.unrolled_loop_size(up, full_unroll_trip_count);
    // DEVIATE U1: a loop of few trips is held to the target's `optsize` bar
    // (the pin's is `OptSizeThreshold`, 0 on x86).
    let few = if run.options.opt_size {
        run.tti
            .unroll_size_few_trips()
            .filter(|&(trips, _)| full_unroll_trip_count <= trips)
            .map_or(up.threshold, |(_, bar)| up.threshold.max(bar))
    } else {
        up.threshold
    };
    if unrolled_size < u64::from(few) {
        return Some(full_unroll_trip_count);
    }

    // The loop is not that small, but it can still be fully unrolled if that
    // removes a significant number of instructions. To check, run an
    // additional analysis on the loop.
    let max_unrolled_loop_size =
        (u64::from(up.threshold) * u64::from(up.max_percent_threshold_boost) / 100).min(u64::from(u32::MAX)) as u32;
    if let Some(cost) = analyze_loop_unroll_cost(
        run,
        analyzed,
        lp,
        full_unroll_trip_count,
        eph,
        max_unrolled_loop_size,
        up.max_iterations_count_to_analyze,
    ) {
        let boost = full_unroll_boosting_factor(&cost, up.max_percent_threshold_boost);
        let boosted_threshold = (u64::from(up.threshold) * u64::from(boost) / 100) as u32;
        if cost.unrolled_cost < boosted_threshold {
            return Some(full_unroll_trip_count);
        }
    }
    None
}

/// `shouldPragmaUnroll`.
fn should_pragma_unroll(
    pinfo: &UnrollPragmaInfo,
    trip_multiple: u32,
    trip_count: u32,
    max_trip_count: u32,
    up: &UnrollingPreferences,
) -> Option<u32> {
    // 2nd priority is the unroll count set by pragma (the first, the option, is
    // not here).
    if pinfo.pragma_count > 0 && (up.allow_remainder || trip_multiple % pinfo.pragma_count == 0) {
        return Some(pinfo.pragma_count);
    }

    if pinfo.pragma_full_unroll && trip_count != 0 {
        // Certain cases with UBSAN can cause the trip count to be calculated as
        // INT_MAX: block full unrolling at a reasonable limit.
        if trip_count > PRAGMA_UNROLL_FULL_MAX_ITERATIONS {
            return None;
        }
        return Some(trip_count);
    }

    if pinfo.pragma_enable_unroll && trip_count == 0 && max_trip_count != 0 && max_trip_count <= up.max_upper_bound {
        return Some(max_trip_count);
    }

    None
}

/// `shouldPartialUnroll`: the count, or none where there is no trip count.
fn should_partial_unroll(
    loop_size: u32,
    trip_count: u32,
    uce: &UnrollCostEstimator,
    up: &UnrollingPreferences,
) -> Option<u32> {
    if trip_count == 0 {
        return None;
    }
    if !up.partial {
        return Some(0);
    }
    let mut count = trip_count;
    if up.partial_threshold != NO_THRESHOLD {
        // Reduce the unroll count to be modulo the trip count for partial
        // unrolling.
        if uce.unrolled_loop_size(up, count) > u64::from(up.partial_threshold) {
            let new_count = (up.partial_threshold.max(up.be_insns + 1) - up.be_insns) / (loop_size - up.be_insns);
            count = new_count;
        }
        if count > up.max_count {
            count = up.max_count;
        }
        while count != 0 && trip_count % count != 0 {
            count -= 1;
        }
        if up.allow_remainder && count <= 1 {
            // No count divides the trip count: use the largest power-of-two
            // factor that satisfies the threshold limit, with a fixup loop.
            count = up.default_unroll_runtime_count;
            while count != 0 && uce.unrolled_loop_size(up, count) > u64::from(up.partial_threshold) {
                count >>= 1;
            }
        }
        if count < 2 {
            count = 0;
        }
    } else {
        count = trip_count;
    }
    if count > up.max_count {
        count = up.max_count;
    }
    Some(count)
}

/// `computeUnrollCount`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_unroll_count(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
    eph: &IdSet<InstId>,
    pinfo: &UnrollPragmaInfo,
    trip_count: u32,
    max_trip_count: u32,
    max_or_zero: bool,
    trip_multiple: u32,
    uce: &UnrollCostEstimator,
    up: &mut UnrollingPreferences,
    se: &mut ScalarEvolution,
    pp: &mut PeelingPreferences,
) -> u32 {
    let loop_size = uce.rolled_loop_size();

    // If a user provided an explicit unroll pragma (with or without count),
    // enable runtime unrolling and override expensive trip count checks.
    if pinfo.pragma_enable_unroll || pinfo.pragma_count > 0 {
        up.allow_expensive_trip_count = true;
        up.runtime = true;
    }

    // Check for an explicit unroll count: the pragma's.
    if let Some(factor) = should_pragma_unroll(pinfo, trip_multiple, trip_count, max_trip_count, up) {
        if pinfo.pragma_count > 0 {
            up.allow_expensive_trip_count = true;
            up.force = true;
        }
        return factor;
    } else if pinfo.explicit_unroll && trip_count != 0 {
        // A loop with an unrolling pragma gets more aggressive limits.
        up.threshold = up.threshold.max(PRAGMA_UNROLL_THRESHOLD);
        up.partial_threshold = up.partial_threshold.max(PRAGMA_UNROLL_THRESHOLD);
    }

    // 3rd priority is exact full unrolling: it eliminates all copies of some
    // exit test.
    if trip_count != 0
        && let Some(factor) = should_full_unroll(run, analyzed, lp, eph, trip_count, uce, up)
    {
        return factor;
    }

    // 4th priority is bounded unrolling: by the upper bound if generally
    // allowed or if the loop is executed either the upper bound or zero times.
    if trip_count == 0
        && max_trip_count != 0
        && (up.upper_bound || max_or_zero)
        && max_trip_count <= up.max_upper_bound
        && let Some(factor) = should_full_unroll(run, analyzed, lp, eph, max_trip_count, uce, up)
    {
        return factor;
    }

    // 5th priority is loop peeling.
    {
        let cx = llrm_analysis::scev::Cx {
            context: &*run.context,
            globals: run.globals,
            metadata: run.metadata,
            function: &*run.function,
            layout: run.layout,
            loops: &analyzed.loops,
            tree: &analyzed.tree,
            assumptions: &analyzed.assumptions,
        };
        let already = run.peeled.get(&lp.get_header()).copied().unwrap_or(0);
        crate::looppeel::compute_peel_count(se, &cx, run.callees, lp, loop_size, pp, up.threshold, already);
    }
    if pp.peel_count != 0 {
        up.runtime = false;
        return 1;
    }

    // Before starting partial unrolling, set `up.partial` if the user
    // explicitly asked for unrolling.
    if trip_count != 0 {
        up.partial |= pinfo.explicit_unroll;
    }

    // 6th priority is partial unrolling, when the trip count is known.
    if let Some(factor) = should_partial_unroll(loop_size, trip_count, uce, up) {
        return factor;
    }
    debug_assert!(trip_count == 0, "All cases when the trip count is constant should be covered here.");

    // 7th priority is runtime unrolling.
    // Do not unroll a runtime trip count loop when it is disabled.
    if pinfo.pragma_runtime_unroll_disable {
        return 0;
    }
    // Do not unroll a small upper bound loop unless the user or the target
    // asked to.
    if max_trip_count != 0 && !up.force && max_trip_count <= up.max_upper_bound {
        return 0;
    }
    if !up.runtime {
        return 0;
    }

    let mut count = up.default_unroll_runtime_count;
    // Reduce the count to the largest power-of-two factor of the original count
    // that satisfies the threshold limit.
    while count != 0 && uce.unrolled_loop_size(up, count) > u64::from(up.partial_threshold) {
        count >>= 1;
    }
    if !up.allow_remainder && count != 0 && trip_multiple % count != 0 {
        while count != 0 && trip_multiple % count != 0 {
            count >>= 1;
        }
    }
    if count > up.max_count {
        count = up.max_count;
    }
    if max_trip_count != 0 && count > max_trip_count {
        count = max_trip_count;
    }
    if count < 2 { 0 } else { count }
}

/// `Function::hasOneLiveUse` of `callee`, by scanning the module for its uses
/// (U5).
fn has_one_live_use(
    run: &Run,
    callee: GlobalId,
) -> bool {
    let mut uses = 0usize;
    let mentions = |function: &Function, context: &Context| {
        function
            .walk()
            .filter(|&(_, inst)| {
                function
                    .instruction(inst)
                    .operands
                    .iter()
                    .any(
                        |operand| match operand {
                            Operand::Constant(id) => {
                                matches!(
                                    context.get(*id).kind,
                                    ConstantKind::Global(one) if one == callee
                                )
                            }
                            _ => false,
                        },
                    )
            })
            .count()
    };
    for global in run.globals {
        if let GlobalKind::Function(function) = &global.kind {
            uses += mentions(function, run.context);
        }
    }
    // The function being changed is in `run.globals` as the module held it
    // before the pass; its uses are the same at the start of a loop's turn.
    uses == 1
}

/// The reason a loop is not unrolled, on `LLRM_DEBUG=unroll` (D3:
/// `LLVM_DEBUG`).
fn why(
    run: &Run,
    lp: Loop,
    reason: &str,
) {
    if llrm_support::debug::enabled("unroll") {
        let name = run.function.block(lp.get_header()).name.clone().unwrap_or_default();
        eprintln!("unroll: %{name}: {reason}");
    }
}

/// `tryToUnrollLoop`, for the full unroll pass.
fn try_to_unroll_loop(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> Unrolled {
    let options = LoopOptions::of(run, lp);
    let tm = options.unroll(run);
    // `TM & TM_Disable`: disabled, or suppressed by the user.
    if matches!(tm, Mode::Disable | Mode::SuppressedByUser) {
        why(run, lp, "disabled by metadata");
        return Unrolled::Unmodified;
    }

    // Do not unroll a loop that is not forced when its parent has an explicit
    // unroll-and-jam pragma, or when it has one itself.
    if let Some(parent) = lp.get_parent_loop() {
        let parent_options = LoopOptions::of(run, parent);
        if parent_options.unroll_and_jam(run).forced() && !tm.forced() {
            return Unrolled::Unmodified;
        }
    }
    if options.unroll_and_jam(run).forced() && !tm.forced() {
        return Unrolled::Unmodified;
    }

    // The loop must be in loop-simplify form.
    let (Some(_), Some(_)) = (lp.get_loop_preheader(run.function), lp.get_loop_latch(run.function)) else {
        return Unrolled::Unmodified;
    };
    if !lp.has_dedicated_exits(run.function) {
        why(run, lp, "not in loop-simplify form");
        return Unrolled::Unmodified;
    }

    // DEVIATE: a loop with a landing pad is not cloned. `ehprepare` takes one
    // pad and one module handler per function, which a copy of the pad breaks
    // (as in `jumpthreading` J10 and `dfajumpthreading` DF3).
    if lp.get_blocks().iter().any(|&block| crate::edges::starts_with_landing_pad(run.function, block)) {
        why(run, lp, "contains a landing pad");
        return Unrolled::Unmodified;
    }

    // When automatic unrolling is disabled, do not unroll unless overridden for
    // this loop.
    if run.options.only_when_forced && tm != Mode::ForcedByUser {
        return Unrolled::Unmodified;
    }

    let opt_for_size = run.options.opt_size;
    let has_calls = loop_has_calls(run, lp);
    let mut up = gather_unrolling_preferences(run, has_calls);

    // Exit early if unrolling is disabled. For size, the loop size is the
    // threshold later.
    if up.threshold == 0 && (!up.partial || up.partial_threshold == 0) && !opt_for_size {
        why(run, lp, "all thresholds are zero");
        return Unrolled::Unmodified;
    }

    let eph = ephemeral_values(run, analyzed, lp);
    let uce = cost_estimator(run, lp, &eph, up.be_insns);
    if !uce.can_unroll() {
        if llrm_support::debug::enabled("unroll") {
            for &block in lp.get_blocks() {
                for inst in run.function.iter_block(block) {
                    let cost = instruction_cost(
                        run.tti,
                        run.context,
                        run.layout,
                        run.globals,
                        run.function,
                        inst,
                        CostKind::CodeSize,
                    );
                    if !cost.valid {
                        eprintln!("unroll: no cost for {:?}", run.function.instruction(inst).opcode.mnemonic());
                    }
                }
            }
        }
        why(run, lp, "cannot unroll: loop size not computed or not duplicatable");
        return Unrolled::Unmodified;
    }
    let loop_size = uce.rolled_loop_size();

    // When optimizing for size, use LoopSize + 1 as the threshold (a size below
    // the threshold is wanted), to unroll loops if that does not increase the
    // code size.
    if opt_for_size {
        up.threshold = up.threshold.max(loop_size + 1);
    }

    if uce.num_inline_candidates != 0 {
        why(run, lp, "loop with inlinable calls");
        return Unrolled::Unmodified;
    }

    // Find the smallest exact trip count for any exit. This is an upper bound
    // on the loop trip count, but an exit at an earlier iteration is still
    // possible.
    let header = lp.get_header();
    let mut se = std::mem::replace(&mut run.scev, ScalarEvolution::new());
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut trip_count = 0u32;
    let mut trip_multiple = 1u32;
    let exiting_blocks = lp.get_exiting_blocks(run.function);
    let mut exits = Vec::new();
    for &exiting in &exiting_blocks {
        let tc = se.get_small_constant_trip_count_for_exit(&cx, header, exiting);
        exits.push(Exit { block: exiting, trip_count: tc });
        if tc != 0 && (trip_count == 0 || tc < trip_count) {
            trip_count = tc;
            trip_multiple = tc;
        }
    }
    if trip_count == 0 {
        // If no exact trip count is known, determine the trip multiple of
        // either the loop latch or the single exiting block.
        let latch = lp.get_loop_latch(run.function);
        let exiting = match latch {
            Some(latch) if lp.is_loop_exiting(run.function, latch) => Some(latch),
            _ => lp.get_exiting_block(run.function),
        };
        if let Some(exiting) = exiting {
            trip_multiple = se.get_small_constant_trip_multiple_for_exit(&cx, header, exiting);
        }
    }

    // Try to find the trip count upper bound if the exact one is not known.
    let (mut max_trip_count, mut max_or_zero) = (0, false);
    if trip_count == 0 {
        max_trip_count = se.get_small_constant_max_trip_count(&cx, header);
        max_or_zero = se.is_backedge_taken_count_max_or_zero(&cx, header);
    }

    let pinfo = UnrollPragmaInfo::of(run, &options);
    let mut pp = PeelingPreferences::default();
    let count = compute_unroll_count(
        run,
        analyzed,
        lp,
        &eph,
        &pinfo,
        trip_count,
        max_trip_count,
        max_or_zero,
        trip_multiple,
        &uce,
        &mut up,
        &mut se,
        &mut pp,
    );
    // Nothing has changed the function yet.
    run.scev = se;
    if count == 0 {
        why(
            run,
            lp,
            &format!(
                "no viable strategy found (trips {trip_count}, max {max_trip_count}, size {loop_size}, threshold {})",
                up.threshold
            ),
        );
        return Unrolled::Unmodified;
    }

    if pp.peel_count != 0 {
        why(run, lp, &format!("peeling {} iterations", pp.peel_count));
        let loop_ = peelable(run.function, lp);
        return match crate::loopclone::peeled_any_latch(run.function, &loop_, i64::from(pp.peel_count)) {
            Ok(Some(_)) => {
                let so_far = run.peeled.get(&header).copied().unwrap_or(0);
                run.peeled.insert(header, so_far + pp.peel_count);
                Unrolled::Peeled
            }
            Ok(None) => Unrolled::Unmodified,
            Err(error) => panic!("loop peel: {error}"),
        };
    }

    // Do not attempt partial/runtime unrolling in the full unroller.
    if (trip_count == 0 && max_trip_count == 0) || count < trip_count || count < max_trip_count {
        why(run, lp, "partial/runtime unroll in FullLoopUnroll");
        return Unrolled::Unmodified;
    }

    why(
        run,
        lp,
        &format!(
            "unrolling by {count} (trips {trip_count}, max {max_trip_count}, size {loop_size}, threshold {})",
            up.threshold
        ),
    );
    let body = lp.get_blocks().to_vec();
    let latches = lp.get_loop_latches(run.function);
    let unroll_options = UnrollOptions {
        count,
        max_trip_count: count,
        max_or_zero,
        exits: exits.into_iter().filter(|exit| exit.trip_count != 0).collect(),
    };
    match unroll_loop_blocks(run.function, header, &body, &latches, &unroll_options, &mut run.emptied) {
        Ok(result) => result,
        Err(error) => panic!("loop unroll: {error}"),
    }
}

/// Whether the loop calls something the target lowers to a call.
pub(crate) fn loop_has_calls(
    run: &Run,
    lp: Loop,
) -> bool {
    lp.get_blocks()
        .iter()
        .any(
            |&block| run.function
                .iter_block(block)
                .any(
                    |inst| match call_target(run, inst) {
                        Some(Some(callee)) if (callee.0 as usize) < run.globals.len() => {
                            let value = &run.globals[callee.0 as usize];
                            !value.name.as_deref().is_some_and(|name| name.starts_with("llvm."))
                                && run.tti.is_lowered_to_call(
                                    value.name.as_deref(),
                                    matches!(
                                        value.linkage,
                                        llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                                    ),
                                )
                        }
                        Some(_) => true,
                        None => false,
                    },
                ),
        )
}

/// `CodeMetrics::collectEphemeralValues(L, &AC, EphValues)`.
pub(crate) fn ephemeral_values(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> IdSet<InstId> {
    let assumes: Vec<InstId> = analyzed
        .assumptions
        .assumes
        .iter()
        .copied()
        .filter(|&assume| run.function.parent(assume).is_some_and(|block| lp.contains(block)))
        .collect();
    let function = &*run.function;
    let effects_free = |inst: InstId| llrm_mir::memory::only_value(run.context, run.callees, function, inst);
    collect_ephemeral_values(function, &effects_free, &assumes, &|block| lp.contains(block))
}

/// `UnrollCostEstimator(L, TTI, EphValues, BEInsns)`.
pub(crate) fn cost_estimator(
    run: &Run,
    lp: Loop,
    eph: &IdSet<InstId>,
    be_insns: u32,
) -> UnrollCostEstimator {
    let mut metrics = CodeMetrics::default();
    {
        let one_live_use = |callee: GlobalId| has_one_live_use(run, callee);
        let query = Query {
            tti: run.tti,
            context: run.context,
            layout: run.layout,
            globals: run.globals,
            this: run.this,
            has_one_live_use: &one_live_use,
        };
        for &block in lp.get_blocks() {
            metrics.analyze_basic_block(&query, run.function, block, eph, run.options.prepare_for_lto);
        }
    }
    let mut loop_size = metrics.num_insts;
    // Do not allow an estimate of size zero: the code using this size may
    // assume that each loop has at least three instructions.
    if loop_size.valid && loop_size.value < i64::from(be_insns) + 1 {
        loop_size = Cost::of(i64::from(be_insns) + 1);
    }
    UnrollCostEstimator {
        num_inline_candidates: metrics.num_inline_candidates,
        not_duplicatable: metrics.not_duplicatable,
        loop_size,
    }
}

/// The loop `lp` as the cloner reads it.
fn peelable(
    function: &Function,
    lp: Loop,
) -> llrm_analysis::graph::loops::Loop {
    use llrm_analysis::cfg;
    llrm_analysis::graph::loops::Loop {
        header: cfg::id(lp.get_header()),
        latches: lp.get_loop_latches(function).into_iter().map(cfg::id).collect(),
        body: llrm_mir::forest::Body::from_blocks(lp.get_blocks().iter().map(|&block| cfg::id(block))),
    }
}

/// `isInstructionTriviallyDead`: unused and without side effects.
fn is_dead(
    run: &Run,
    inst: InstId,
) -> bool {
    let operation = run.function.instruction(inst);
    !operation.opcode.is_terminator()
        && !matches!(operation.opcode, Opcode::LandingPad { .. })
        && operation.result.is_none_or(|result| run.function.users(result).is_empty())
        && !has_side_effects(run, inst)
}

/// `simplifyLoopAfterUnroll`
/// ([LoopUnroll.cpp:350](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L350))
/// of a complete unroll (`SimplifyIVs` is off): in each of `blocks`, what
/// simplifies is replaced where LCSSA form allows, `(x + c1) + c2` becomes
/// `x + (c1 + c2)`, and what is dead is deleted. `info` is None where no loop
/// holds the blocks, which makes every replacement keep LCSSA form.
pub(crate) fn simplify_loop_after_unroll(
    run: &mut Run,
    info: Option<&LoopInfo>,
    blocks: &[BlockId],
) {
    for &block in blocks {
        let mut dead: Vec<InstId> = Vec::new();
        for inst in run.function.iter_block(block).collect::<Vec<_>>() {
            if run.function.is_erased(inst) {
                continue;
            }
            if let Some(value) = llrm_mir::simplify::simplify_instruction_in(
                run.context,
                run.globals,
                run.layout,
                None,
                run.function,
                inst,
            ) && info
                .is_none_or(|info| crate::looprotate::replacement_preserves_lcssa_form(info, run.function, inst, value))
                && let Some(result) = run.function.instruction(inst).result
            {
                run.function.replace_all_uses_with(result, value);
            }
            if is_dead(run, inst) {
                dead.push(inst);
            }
            // Fold ((add X, C1), C2) to (add X, C1 + C2): common in unrolled
            // loops, and it spares the induction variable a long chain of adds.
            let operation = run.function.instruction(inst);
            if operation.opcode != Opcode::Binary(BinaryOp::Add) {
                continue;
            }
            let [Operand::Value(inner), Operand::Constant(c2)] = operation.operands[..] else { continue };
            let ValueDef::Instruction(inner_inst) = run.function.value(inner).def else { continue };
            let inner_op = run.function.instruction(inner_inst);
            let [x, Operand::Constant(c1)] = inner_op.operands[..] else { continue };
            if inner_op.opcode != Opcode::Binary(BinaryOp::Add) {
                continue;
            }
            let (ConstantKind::Int(a), ConstantKind::Int(b)) = (&run.context.get(c1).kind, &run.context.get(c2).kind)
            else {
                continue;
            };
            let (a, b) = (*a, *b);
            let ty = operation.ty;
            let width = run.context.types.int_bits(ty).expect("an integer add");
            let wide = llrm_mir::context::signed(a, width) + llrm_mir::context::signed(b, width);
            let sum = wide as u128 & llrm_mir::context::mask(width);
            let signed_overflow = wide != llrm_mir::context::signed(sum, width);
            let mut flags = operation.flags;
            let inner_flags = inner_op.flags;
            let mut kept = Flags::default();
            if flags.contains(Flags::NUW) && inner_flags.contains(Flags::NUW) {
                kept.insert(Flags::NUW);
            }
            if flags.contains(Flags::NSW) && inner_flags.contains(Flags::NSW) && !signed_overflow {
                kept.insert(Flags::NSW);
            }
            flags = Flags::default();
            flags.insert(kept);
            let constant = run.context.int(ty, sum as i128);
            run.function.set_operands(inst, vec![x, Operand::Constant(constant)]);
            run.function.set_flags(inst, flags);
            if is_dead(run, inner_inst) {
                dead.push(inner_inst);
            }
        }
        // Deleted once the block is done: a phi may use a later instruction.
        while let Some(inst) = dead.pop() {
            if run.function.is_erased(inst) || !is_dead(run, inst) {
                continue;
            }
            let operands = run.function.instruction(inst).operands.clone();
            run.function.erase(inst).expect("a dead instruction");
            for operand in operands {
                if let Operand::Value(value) = operand
                    && let ValueDef::Instruction(def) = run.function.value(value).def
                {
                    dead.push(def);
                }
            }
        }
    }
}

/// The blocks `UnrollLoop` hands `simplifyLoopAfterUnroll`: the loop's own that
/// remain (`body`) and those made since the function had `capacity` blocks.
pub(crate) fn unrolled_blocks(
    run: &Run,
    capacity: usize,
    body: &[BlockId],
) -> Vec<BlockId> {
    let own: IdSet<BlockId> = body.iter().copied().collect();
    run.function.layout().iter().copied().filter(|&at| at.0 as usize >= capacity || own.contains(&at)).collect()
}

/// `LoopFullUnrollPass` over a function: every loop, innermost first; the loops
/// a complete unroll makes of the inner loops are visited next. Whether any
/// loop was unrolled.
pub fn unroll_loops(run: &mut Run) -> bool {
    // `LoopUnrollPass::run` (L1762): every loop is put in simplified form and
    // LCSSA first, whether or not anything is unrolled.
    let simplified = crate::loopsimplify::simplified(run.function);
    let closed = crate::lcssa::closed(run.function).unwrap_or(false);
    let mut analyzed = Analyzed::of(run);
    let at_first: IdSet<BlockId> = analyzed.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
    // The worklist is popped from the back (`appendReversedLoopsToWorklist`):
    // the loops are visited in the reverse of
    // `getLoopsInReverseSiblingPreorder`, each loop after the loops it
    // holds.
    let mut work: std::collections::VecDeque<BlockId> =
        analyzed.loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    // `LoopFullUnrollPass` hands `addSiblingLoops` every sibling the unroll
    // left that was not there before, the unrolled loop's own first iteration
    // among them: they are new to the loop passes after it.
    let mut revisited: Vec<BlockId> = Vec::new();
    let mut changed = simplified || closed;
    if changed {
        run.scev = ScalarEvolution::new();
    }
    while let Some(header) = work.pop_front() {
        if !analyzed.loops.is_loop_header(header) {
            continue;
        }
        let lp = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
        // The loops beside this one before the unroll: those not in this set
        // afterwards are the new siblings.
        let parent_header = lp.get_parent_loop().map(|one| one.get_header());
        let siblings_of = |loops: &LoopInfo| -> Vec<BlockId> {
            match parent_header {
                Some(parent) => loops
                    .get_loop_for(parent)
                    .map(|one| one.get_sub_loops().map(|sub| sub.get_header()).collect())
                    .unwrap_or_default(),
                None => loops.top_level_loops().map(|one| one.get_header()).collect(),
            }
        };
        let old: IdSet<BlockId> = siblings_of(&analyzed.loops).into_iter().collect();
        let capacity = run.function.block_capacity();
        let body = lp.get_blocks().to_vec();
        let holds_loops = lp.get_sub_loops().next().is_some();
        let result = try_to_unroll_loop(run, &analyzed, lp);
        if result == Unrolled::Unmodified {
            continue;
        }
        let peeled = result == Unrolled::Peeled;
        changed = true;
        run.scev = ScalarEvolution::new();
        if peeled {
            // `PartiallyUnrolled`: its analyses and its nest are made again
            // (the cloned iterations are loops' blocks of its parent).
            // `peelLoop` ends in `simplifyLoop`: the exits the copies share
            // are dedicated again (here for the whole function, U6).
            crate::loopsimplify::simplified(run.function);
            let _ = crate::lcssa::closed(run.function);
            analyzed = Analyzed::of(run);
            // `simplifyLoopAfterUnroll(L, SimplifyIVs = true)`: the IVs first
            // (PL5).
            let _ = crate::simplifyindvar::simplified_loop_ivs(
                run.context,
                run.layout,
                run.function,
                run.declared,
                run.outer,
                header,
            );
            let after = unrolled_blocks(run, capacity, &body);
            simplify_loop_after_unroll(run, Some(&analyzed.loops), &after);
            analyzed = Analyzed::of(run);
            // `LoopFullUnrollPass` does not revisit the loop it peeled (only
            // `LoopUnrollPass` does); the loops the peeled copy holds are
            // new siblings, visited next.
            let mut fresh: Vec<BlockId> =
                siblings_of(&analyzed.loops).into_iter().filter(|one| !old.contains(one)).collect();
            fresh.reverse();
            for one in fresh {
                work.push_front(one);
            }
            continue;
        }
        // A top-level loop with no loops inside leaves the analyses of the
        // other loops as they were: they are made again only otherwise (made
        // after every loop they were `n^2` over `n` loops in a function).
        if parent_header.is_none() && !holds_loops {
            let after = unrolled_blocks(run, capacity, &body);
            simplify_loop_after_unroll(run, None, &after);
            continue;
        }
        // `UnrollLoop` closes the outer loop's SSA form
        // (`formLCSSARecursively`).
        let _ = crate::lcssa::closed(run.function);
        analyzed = Analyzed::of(run);
        // The blocks `UnrollLoop` hands `simplifyLoopAfterUnroll`: the loop's
        // own that remain and the ones it made.
        let after = unrolled_blocks(run, capacity, &body);
        simplify_loop_after_unroll(run, Some(&analyzed.loops), &after);
        analyzed = Analyzed::of(run);
        // `UnrollLoop` puts the loops it cloned back in loop-simplify form
        // (`simplifyLoop` of the outer loop); here the whole function is, when
        // the unrolled loop had an outer loop or held loops (U6).
        if parent_header.is_some() || siblings_of(&analyzed.loops).iter().any(|one| !old.contains(one)) {
            crate::loopsimplify::simplified(run.function);
            let _ = crate::lcssa::closed(run.function);
            analyzed = Analyzed::of(run);
        }
        let mut fresh: Vec<BlockId> =
            siblings_of(&analyzed.loops).into_iter().filter(|one| !old.contains(one)).collect();
        fresh.reverse();
        for &one in &fresh {
            work.push_front(one);
        }
        revisited.extend(fresh);
    }
    let emptied = std::mem::take(&mut run.emptied);
    run.function.erase_blocks(&emptied).expect("blocks merged away are empty and unnamed");
    if changed {
        analyzed = Analyzed::of(run);
    }
    let mut created: Vec<BlockId> = analyzed
        .loops
        .get_loops_in_preorder()
        .iter()
        .map(|one| one.get_header())
        .filter(|header| !at_first.contains(header))
        .collect();
    for header in revisited {
        let Some(found) = analyzed.loops.get_loop_for(header).filter(|one| one.get_header() == header) else {
            continue;
        };
        let mut nest = vec![found];
        while let Some(one) = nest.pop() {
            if !created.contains(&one.get_header()) {
                created.push(one.get_header());
            }
            nest.extend(one.get_sub_loops());
        }
    }
    run.created = created;
    changed
}

/// `LoopFullUnrollPass` as a slot of the pipeline.
pub struct LoopFullUnroll {
    pub opt_level: u32,
    pub opt_size: bool,
    /// `OnlyWhenForced`: only a loop that asks to be unrolled is
    /// (`!PTO.LoopUnrolling`, clang's `-fno-unroll-loops`, the default at -O1).
    pub only_when_forced: bool,
    /// The headers of the loops the last run made, for the slot after it: the
    /// loop pass manager's worklist.
    pub created: std::rc::Rc<std::cell::RefCell<Vec<BlockId>>>,
}

impl FunctionPass for LoopFullUnroll {
    fn name(&self) -> &'static str {
        "loopfullunroll"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-unroll-full")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let tti = crate::looprotate::target_tti(outer.target());
        let held = analyses.get::<llrm_analysis::scev::analysis::Scevs>(unit.context, unit.layout, unit.function);
        let mut run = Run {
            context: unit.context,
            layout: unit.layout,
            globals: &outer.globals,
            metadata: unit.metadata,
            callees: outer.callees(),
            this: unit.id,
            tti: &*tti,
            function: unit.function,
            outer: &outer,
            declared: unit.declared,
            emptied: Vec::new(),
            peeled: Default::default(),
            created: Vec::new(),
            scev: held.take(),
            options: Options {
                opt_level: self.opt_level,
                opt_size: self.opt_size,
                min_size: false,
                only_when_forced: self.only_when_forced,
                prepare_for_lto: false,
            },
        };
        let changed = unroll_loops(&mut run);
        *self.created.borrow_mut() = std::mem::take(&mut run.created);
        if changed {
            PreservedAnalyses::none()
        } else {
            held.put(std::mem::replace(&mut run.scev, ScalarEvolution::new()));
            PreservedAnalyses::all()
        }
    }
}

/// `unroll_loops` over the function `name` of `module`; whether any loop was
/// unrolled.
pub fn unroll_function(
    module: &mut llrm_mir::module::Module,
    name: &str,
    tti: &dyn Tti,
    options: Options,
) -> Result<bool, String> {
    let layout =
        DataLayout::parse(module.datalayout.as_deref().unwrap_or("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"))?;
    let callees = llrm_mir::memory::callees(module);
    let outer = llrm_mir::passes::Outer::of(module, None);
    let mut declared = llrm_mir::passes::Declared::default();
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let id = module
        .functions()
        .find(|(_, global, _)| global.name.as_deref() == Some(name))
        .map(|(id, _, _)| id)
        .ok_or_else(|| format!("no @{name}"))?;
    let context = &mut module.context;
    let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else {
        return Err(format!("@{name} is not a function"));
    };
    let mut run = Run {
        context,
        layout: &layout,
        globals: &globals,
        metadata: &metadata,
        callees: &callees,
        this: Some(id),
        tti,
        function,
        outer: &outer,
        declared: &mut declared,
        options,
        emptied: Vec::new(),
        peeled: Default::default(),
        created: Vec::new(),
        scev: ScalarEvolution::new(),
    };
    Ok(unroll_loops(&mut run))
}

#[cfg(test)]
#[path = "loopunrollpass_tests.rs"]
mod tests;
