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
//! A count that is not the whole trip count is partial unrolling, which the
//! pass does only as `LoopUnrollPass` (`Options::target_unrolling`: the
//! target's `AllowPartial` and `UpperBound`) and only where the count divides
//! the trip count; runtime unrolling (the remainder loop) is not here yet. The
//! loop is then unrolled by [`crate::loopunroll::unroll_loop_blocks`].
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

mod price;
mod throughput;
mod recurrence;
pub use crate::trial;

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
    /// `LoopUnrollPass` rather than `LoopFullUnrollPass`: `AllowPartial` and
    /// `UpperBound` are the target's, not forced off.
    pub target_unrolling: bool,
    /// The inliner still runs after this pass and may take an internal function (inlining is on and the
    /// function is not `noinline`): its loops are then left to be priced in the context of each site.
    pub inliner_follows: bool,
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
    /// The headers of loops this run unrolled at run time, and of their
    /// remainders (`setLoopAlreadyUnrolled`, D4: a set, not metadata).
    pub finished: IdSet<BlockId>,
    /// The headers of the loops this run made: those that were no loop's header
    /// when it began (`LPMUpdater::addSiblingLoops`, for the passes after it).
    pub created: Vec<BlockId>,
    /// The headers of loops earlier runs made by unrolling a loop around them:
    /// part of a nest already decided, though no longer in `created` (the loop
    /// pass manager runs the pass again for each loop).
    pub decided: IdSet<BlockId>,
    /// The headers of the loops the run began with: a loop not among them is a
    /// copy an unroll made.
    pub first: IdSet<BlockId>,
    /// The expressions of the passes before (`Scevs`), current while the
    /// function is as they left it: dropped at the first change.
    pub scev: ScalarEvolution,
    /// The manager's tree and nest of the function as it came in, used when
    /// loop-simplify and LCSSA change nothing (the pin's pass gets both from
    /// the analysis manager).
    pub cached: Option<(std::rc::Rc<DomTree>, std::rc::Rc<LoopInfo>)>,
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
    /// PL6: what the first iteration's instructions cost once counted (those a
    /// later iteration or a side effect needs), zero where not priced.
    first_iteration: u32,
    /// The bytes of the instructions the copies keep (the analysis folds the
    /// rest): the unit costs above are SizeAndLatency, and a call is about 8 of
    /// them and 8 bytes.
    unrolled_bytes: i64,
    /// A call was priced through (`through_calls`): the pin's analysis would
    /// have given up.
    through_call: bool,
    /// The multiplies and divides of one rolled trip, and of the first trip
    /// once counted: what the entry values make cheaper there is the peel's
    /// gain.
    heavy_rolled: u32,
    heavy_first: u32,
    /// PL6: the clocks the first trip's multiplies and divides lose to its
    /// entry values (folded away, or priced by a constant operand).
    heavy_folded: u32,
}

/// The analyses of one state of the function, made again after a change.
pub(crate) struct Analyzed {
    pub(crate) tree: std::rc::Rc<DomTree>,
    pub(crate) loops: std::rc::Rc<LoopInfo>,
    pub(crate) assumptions: AssumptionCache,
}

impl Analyzed {
    pub(crate) fn of(run: &Run) -> Self {
        llrm_mir::counts::bump(llrm_mir::counts::Event::LoopAnalyses);
        let tree = DomTree::of(run.function);
        let loops = LoopInfo::analyze(run.function, || &tree);
        let assumptions = AssumptionCache::of_declared(
            run.context,
            run.globals,
            run.metadata,
            run.function,
            run.outer.assume_declared(),
        );
        Self { tree: tree.into(), loops: loops.into(), assumptions }
    }

    fn entry(run: &mut Run, unchanged: bool) -> Self {
        match run.cached.take() {
            Some((tree, loops)) if unchanged => {
                let assumptions = AssumptionCache::of_declared(
                    run.context,
                    run.globals,
                    run.metadata,
                    run.function,
                    run.outer.assume_declared(),
                );
                Self { tree, loops, assumptions }
            }
            _ => Self::of(run),
        }
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
    if !run.options.target_unrolling {
        up.partial = false;
        up.runtime = false;
        up.upper_bound = false;
    }
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
    up.partial_threshold = run.tti.unroll_partial_threshold_override().unwrap_or(up.partial_threshold);
    // Apply size attributes.
    if run.options.opt_size {
        up.threshold = up.opt_size_threshold;
        up.partial_threshold = up.partial_opt_size_threshold;
        up.max_percent_threshold_boost = 100;
    }
    // DEVIATE UNROLL-O1: where -O1 never grows the code (`unroll_never_grows_at_o1`), a partial or run-time
    // unroll, which always copies the body, is left to the trial compile that keeps it only if the object
    // is no larger.
    if run.options.opt_level == 1 && !run.options.opt_size && run.tti.unroll_never_grows_at_o1() && trial::active().is_none() {
        up.partial = false;
        up.runtime = false;
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
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    trip_count: u32,
    eph: &IdSet<InstId>,
    max_unrolled_loop_size: u32,
    max_iterations_count_to_analyze: u32,
    clocks: Option<&llrm_mir::target::OperationCosts>,
    through_calls: bool,
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
    let mut simplified_values = IdMap::<llrm_mir::module::ValueId, Operand>::new();
    let mut unrolled_cost = Cost::FREE;
    let mut first_iteration = Cost::FREE;
    let mut unrolled_bytes = 0_i64;
    let mut through_call = false;
    let mut heavy_rolled = Cost::FREE;
    let mut heavy_first = Cost::FREE;
    let mut heavy_folded = 0_i64;
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

        let mut analyzer = UnrolledInstAnalyzer::new(se, iteration);

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
                if is_heavy(&run.function.instruction(inst).opcode) {
                    heavy_rolled = heavy_rolled
                        + clocks
                            .and_then(|costs| {
                                simplified_clocks(run, inst, &run.function.instruction(inst).operands, costs)
                            })
                            .map_or(Cost::FREE, Cost::of);
                }
                rolled_dynamic_cost = rolled_dynamic_cost
                    + clocks
                        .and_then(|costs| simplified_clocks(run, inst, &run.function.instruction(inst).operands, costs))
                        .map_or_else(
                            || {
                                instruction_cost(
                                    run.tti,
                                    run.context,
                                    run.layout,
                                    run.globals,
                                    run.function,
                                    inst,
                                    kind,
                                )
                            },
                            Cost::of,
                        );

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
                        se: &mut *se,
                        header,
                    };
                    analyzer.visit(&mut env, &mut simplified_values, inst)
                };
                inst_cost_map.insert((inst, iteration), InstState { is_free, is_counted: false });
                if iteration == 0 && is_heavy(&run.function.instruction(inst).opcode) {
                    let operation = run.function.instruction(inst);
                    let simplified: Vec<Operand> = operation
                        .operands
                        .iter()
                        .map(|&operand| match operand {
                            Operand::Value(value) => simplified_values.get(&value).copied().unwrap_or(operand),
                            other => other,
                        })
                        .collect();
                    if let Some(costs) = clocks {
                        let before = simplified_clocks(run, inst, &operation.operands, costs).unwrap_or(0);
                        let after = if is_free {
                            0
                        } else {
                            simplified_clocks(run, inst, &simplified, costs).unwrap_or(before)
                        };
                        heavy_folded += (before - after).max(0);
                    }
                }

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
                    // DEVIATE UP1-CALL (owner x87, exit 2026-11-20: the pin's
                    // `getInstructionCost` of a call or gcc's estimate prices
                    // it): the pin gives up on a call, and the priced unroll
                    // then takes the copies as the whole body (static_pointers:
                    // 7 x `push imm; call` priced 305 B, was 6 B smaller). A
                    // call here is one instruction the copies keep, with the
                    // operands that feed it.
                    if lowered && !through_calls {
                        why(run, lp, "cannot analyze: a call");
                        return None;
                    }
                    through_call |= lowered;
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
                        &mut first_iteration,
                        &mut unrolled_bytes,
                        &mut heavy_first,
                        kind,
                        clocks,
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
                &mut first_iteration,
                &mut unrolled_bytes,
                &mut heavy_first,
                kind,
                clocks,
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
                    &mut first_iteration,
                    &mut unrolled_bytes,
                    &mut heavy_first,
                    kind,
                    clocks,
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
        first_iteration: first_iteration.value as u32,
        unrolled_bytes,
        through_call,
        heavy_rolled: heavy_rolled.value as u32,
        heavy_first: heavy_first.value as u32,
        heavy_folded: heavy_folded as u32,
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
    first_iteration: &mut Cost,
    unrolled_bytes: &mut i64,
    heavy_first: &mut Cost,
    kind: CostKind,
    clocks: Option<&llrm_mir::target::OperationCosts>,
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
                let price = clocks
                    .and_then(|costs| simplified_clocks(run, inst, &operands, costs))
                    .map_or_else(
                        || {
                            instruction_cost_with(
                                run.tti,
                                run.context,
                                run.layout,
                                run.globals,
                                run.function,
                                inst,
                                &operands,
                                kind,
                            )
                        },
                        Cost::of,
                    );
                *unrolled_cost = *unrolled_cost + price;
                if !matches!(
                    operation.opcode,
                    Opcode::Phi | Opcode::GetElementPtr { .. } | Opcode::ICmp(_)
                ) {
                    let size = run.outer.target().byte_costs();
                    *unrolled_bytes +=
                        crate::profit::operation(run.context, run.layout, run.function, run.callees, inst, &size)
                            .unwrap_or(size.add);
                }
                if iteration == 0 {
                    *first_iteration = *first_iteration + price;
                    if is_heavy(&operation.opcode) {
                        *heavy_first = *heavy_first + price;
                    }
                }
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

/// `shouldFullUnroll`.
fn should_full_unroll(
    run: &mut Run,
    se: &mut ScalarEvolution,
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

    // When computing the unrolled size, note that BEInsns are not replicated
    // like the rest of the loop body.
    let unrolled_size = uce.unrolled_loop_size(up, full_unroll_trip_count);
    // DEVIATE UNROLL-O1: at -O1 on a target that says so, the copies must not
    // cost more than the rolled loop: by size, or by the cost the analysis
    // finds once the trip's constants fold (gcc's size-neutral complete unroll).
    // A loop this refuses goes to the trial (DEVIATE UNROLL-O1-TRIAL, `note_for_trial`): the
    // driver compiles again with it unrolled and keeps that only if bytes and work do not grow.
    if run.options.opt_level == 1 && !run.options.opt_size && run.tti.unroll_never_grows_at_o1() && trial::active().is_none() {
        let rolled = u64::from(uce.rolled_loop_size());
        let fits = unrolled_size <= rolled
            || analyze_loop_unroll_cost(
                run,
                se,
                analyzed,
                lp,
                full_unroll_trip_count,
                eph,
                rolled.min(u64::from(u32::MAX)) as u32,
                run.tti.unroll_o1_analyzed_trips().map_or(up.max_iterations_count_to_analyze, |trips| trips.max(up.max_iterations_count_to_analyze)),
                None,
                run.tti.unroll_o1_prices_calls(),
            )
            .is_some_and(|cost| u64::from(cost.unrolled_cost) <= rolled);
        let unroll = fits && price::full_unroll_pays(run, se, analyzed, lp, eph, full_unroll_trip_count, uce, up);
        // Only a step of a nest of known trips that the pin's threshold unrolls whole is noted: a loop
        // that goes with its parent takes the counters and registers of both away, which is what an
        // unroll can pay in bytes (x_life); a lone loop refused by one instruction did not (1 of 46
        // programs with such a loop paid), nor did a nest too large (matmul +1601 B), and the trial
        // compiles the module again.
        if !unroll
            && run.tti.unroll_trial_threshold().is_some_and(|limit| {
                !run.tti.unroll_trial_nests_only() || price::nest_unroll_fits(run, se, analyzed, lp, up, limit)
            })
        {
            note_for_trial(run, unrolled_size);
        }
        return unroll.then_some(full_unroll_trip_count);
    }
    // DEVIATE UNROLL-TRIAL: while a trial runs, every loop under the trial
    // size is unrolled; the function is priced whole afterwards.
    if let Some(limit) = trial::active() {
        if unrolled_size < u64::from(limit) {
            why(run, lp, "trial unroll");
            return Some(full_unroll_trip_count);
        }
    }
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
        return price::full_unroll_pays(run, se, analyzed, lp, eph, full_unroll_trip_count, uce, up)
            .then_some(full_unroll_trip_count);
    }

    // The loop is not that small, but it can still be fully unrolled if that
    // removes a significant number of instructions. To check, run an
    // additional analysis on the loop.
    let max_unrolled_loop_size =
        (u64::from(up.threshold) * u64::from(up.max_percent_threshold_boost) / 100).min(u64::from(u32::MAX)) as u32;
    // DEVIATE UNROLL-ANALYZE (owner m68k float_in_memory_loop, exit
    // 2026-11-20): the pin unrolls a loop under its own bar without analysis,
    // so its 10-trip cap never meets a loop that bar admits. Under a lower
    // target bar the same loop reaches the analysis, which now covers every
    // trip count the pin's bar would have unrolled. The same as the pin where
    // the target keeps the bar.
    let pin_threshold = if run.options.opt_size {
        0
    } else if run.options.opt_level > 2 {
        UNROLL_THRESHOLD_AGGRESSIVE
    } else {
        UNROLL_THRESHOLD_DEFAULT
    };
    let trips_to_analyze = if unrolled_size < u64::from(pin_threshold) {
        up.max_iterations_count_to_analyze.max(full_unroll_trip_count)
    } else {
        up.max_iterations_count_to_analyze
    };
    if let Some(cost) = analyze_loop_unroll_cost(
        run,
        se,
        analyzed,
        lp,
        full_unroll_trip_count,
        eph,
        max_unrolled_loop_size,
        trips_to_analyze,
        None,
        false,
    ) {
        let boost = full_unroll_boosting_factor(&cost, up.max_percent_threshold_boost);
        let boosted_threshold = (u64::from(up.threshold) * u64::from(boost) / 100) as u32;
        if cost.unrolled_cost < boosted_threshold {
            return price::full_unroll_pays(run, se, analyzed, lp, eph, full_unroll_trip_count, uce, up)
                .then_some(full_unroll_trip_count);
        }
    }
    note_for_trial(run, unrolled_size);
    None
}

/// DEVIATE UNROLL-TRIAL: a loop the walk refused is noted for the trial that
/// follows it.
fn note_for_trial(
    run: &Run,
    unrolled_size: u64,
) {
    // At -Os the size trial's bar (UNROLL-OS-TRIAL) where the target states one, else the -O1 trial's.
    let threshold = if run.options.opt_size {
        run.tti.unroll_trial_at_size().or_else(|| run.tti.unroll_trial_threshold())
    } else {
        run.tti.unroll_trial_threshold()
    };
    if trial::active().is_none()
        && let (Some(limit), Some(this)) = (threshold, run.this)
        && unrolled_size < u64::from(limit)
        && let Some(name) = run.globals[this.0 as usize].name.as_deref()
    {
        trial::note(name, unrolled_size);
    }
}

/// DEVIATE PEEL-OS-TRIAL (owner m68k, exit 2026-11-24): `computePeelCount`
/// reads `UP.Threshold`, which is 0 at optsize, so the pin never peels at -Os.
/// A target that states `Tti::peels_on_trial_at_size` has the peels the
/// `unroll_trial_at_size` bar would admit noted at -Os, and taken in the trial
/// compile; the driver keeps them where the finished functions are smaller
/// (what a peel frees lands in later passes, so only the code decides).
#[allow(clippy::too_many_arguments)]
fn peel_on_trial(
    run: &Run,
    cx: &llrm_analysis::scev::Cx,
    se: &mut ScalarEvolution,
    lp: Loop,
    loop_size: u32,
    already: u32,
    before: crate::looppeel::PeelingPreferences,
    pp: &mut crate::looppeel::PeelingPreferences,
) {
    if !run.options.opt_size || pp.peel_count != 0 || !run.tti.peels_on_trial_at_size() {
        return;
    }
    let Some(limit) = trial::active().or(run.tti.unroll_trial_at_size()) else { return };
    let mut wide = before;
    crate::looppeel::compute_peel_count(se, cx, run.callees, lp, loop_size, &mut wide, limit, already);
    if wide.peel_count == 0 {
        return;
    }
    if trial::active().is_some() {
        *pp = wide;
    } else if let Some(this) = run.this
        && let Some(name) = run.globals[this.0 as usize].name.as_deref()
    {
        trial::note(name, u64::from(loop_size) * u64::from(wide.peel_count + 1));
    }
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

/// The multiplies and divides: the operations whose clocks an entry constant
/// removes (the rest of a trip's folding is a clock or two each).
fn is_heavy(opcode: &Opcode) -> bool {
    use llrm_mir::opcode::BinaryOp;
    matches!(
        opcode,
        Opcode::Binary(BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem)
    )
}

/// What `inst` costs in clocks once `operands` are what its operands simplified
/// to: a divide or remainder by a constant is the `and` or the multiply-high
/// sequence the backend makes of it, not a `div` (x_primes: `i % d` at `d =
/// 2`).
fn simplified_clocks(
    run: &Run,
    inst: InstId,
    operands: &[Operand],
    costs: &llrm_mir::target::OperationCosts,
) -> Option<i64> {
    use llrm_mir::opcode::BinaryOp;
    let operation = run.function.instruction(inst);
    if let (
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem),
        Some(&Operand::Constant(divisor)),
    ) = (&operation.opcode, operands.get(1))
        && let ConstantKind::Int(bits) = run.context.get(divisor).kind
    {
        return Some(if bits.is_power_of_two() { costs.add } else { costs.multiply + costs.shift + costs.add });
    }
    // A multiply by a constant is the shifts and adds the backend makes of it.
    if matches!(operation.opcode, Opcode::Binary(BinaryOp::Mul))
        && operands.iter().take(2).any(|operand| matches!(operand, Operand::Constant(_)))
    {
        return Some(costs.shift + costs.add);
    }
    crate::profit::operation(run.context, run.layout, run.function, run.callees, inst, costs)
}

/// DEVIATE PL6 (owner x86-m32; exit 2026-12-10: the pin's `computePeelCount`
/// grows a count for folding phis, or gcc's header threading is translated):
/// the first iteration of an innermost loop is peeled where its entry
/// constants fold away cost that, at the speed rate, repays the copy. gcc
/// threads the entry edge of a loop whose phis start at constants and runs
/// that iteration folded (mandel -O2: x = y = 0, two multiplies and a
/// compare, 25k of 660k clocks). The clocks saved are `analyzeLoopUnrollCost`
/// over one iteration priced by the target's operation clocks (its folded
/// share is `rolled - unrolled`);
/// the copy is the loop's size at about `BYTES_PER_COST` bytes a unit.
fn peels_folded_entry(
    run: &mut Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    eph: &IdSet<InstId>,
    loop_size: u32,
    up: &UnrollingPreferences,
) -> bool {
    // The late pass only: the pin counts peels in loop metadata
    // (`llvm.loop.peeled.count`), which this port keeps for one run (PL1),
    // and the entry constants of the residual loop fold again (x_primes: d
    // = 2, then d = 3).
    if !run.tti.peels_folded_entry()
        || !run.options.target_unrolling
        || run.options.opt_size
        || run.options.min_size
        || run.options.opt_level < 2
        || loop_size.saturating_mul(2) > up.threshold
    {
        return false;
    }
    let rate = run.tti.speed_milliclocks_per_byte().max(0) as u64;
    if rate == 0 {
        return false;
    }
    let costs = crate::profit::costs(run.outer);
    // One trip, read for what the entry values change in its multiplies and
    // divides (what only feeds a next trip is not in that count).
    let Some(cost) = analyze_loop_unroll_cost(run, se, analyzed, lp, 1, eph, u32::MAX, 1, Some(&costs), false) else {
        return false;
    };
    // The multiplies and divides the first trip's entry values remove or
    // cheapen.
    let saved = u64::from(cost.heavy_folded);
    why(run, lp, &format!("peel the first iteration: folds {saved} of {loop_size}, rate {rate}"));
    folded_entry_pays(saved, loop_size, rate)
}

/// Bytes of code one unit of the loop's size costs (a typical encoded length).
const BYTES_PER_COST: u64 = 3;

/// Whether the `saved` clocks of a peeled first iteration repay the copy of a
/// loop of `loop_size` units at `rate` thousandths of a clock a byte.
fn folded_entry_pays(
    saved: u64,
    loop_size: u32,
    rate: u64,
) -> bool {
    saved * 1000 >= u64::from(loop_size) * BYTES_PER_COST * rate
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
        && let Some(factor) = should_full_unroll(run, se, analyzed, lp, eph, trip_count, uce, up)
    {
        return factor;
    }

    // 4th priority is bounded unrolling: by the upper bound if generally
    // allowed or if the loop is executed either the upper bound or zero times.
    if trip_count == 0
        && max_trip_count != 0
        && (up.upper_bound || max_or_zero)
        && max_trip_count <= up.max_upper_bound
        && let Some(factor) = should_full_unroll(run, se, analyzed, lp, eph, max_trip_count, uce, up)
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
        let before = *pp;
        crate::looppeel::compute_peel_count(se, &cx, run.callees, lp, loop_size, pp, up.threshold, already);
        peel_on_trial(run, &cx, se, lp, loop_size, already, before, pp);
        // DEVIATE UNROLL-O1: a target that cannot pay for the copy does not peel at -O1.
        if run.options.opt_level == 1 && !run.options.opt_size && !run.tti.peels_at_o1() {
            pp.peel_count = 0;
            pp.allow_peeling = false;
        }
        if pp.peel_count == 0
            && already == 0
            && pp.allow_peeling
            && crate::looppeel::can_peel(&cx, lp)
            && peels_folded_entry(run, se, analyzed, lp, eph, loop_size, up)
        {
            pp.peel_count = 1;
        }
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

    // DEVIATE RU-REC: the same veto as runtime unrolling, for a known trip
    // count, unless the user asked for the unroll.
    if trip_count != 0 && !up.force && !pinfo.explicit_unroll && recurrence::recurrence_bound(run, lp) {
        why(run, lp, "partial unroll: the loop-carried chain bounds a trip (RU-REC)");
        return 0;
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
    if recurrence::recurrence_bound(run, lp) {
        why(run, lp, "runtime unroll: the loop-carried chain bounds a trip (RU-REC)");
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
        let owner = run.this.and_then(|id| (*run.globals[id.0 as usize].name).clone()).unwrap_or_default();
        eprintln!("unroll: {}: {reason}", loop_label(run.function, lp.get_header(), &owner));
    }
}

/// `@function %label` of a loop's header: the label the IR dumps print, so a
/// debug line finds its loop there (an unnamed block prints as its slot).
fn loop_label(
    function: &Function,
    header: BlockId,
    owner: &str,
) -> String {
    let labels = llrm_mir::print::block_labels(function);
    format!("@{owner} %{}", labels[&header])
}

/// `tryToUnrollLoop`, for the full unroll pass.
/// The instructions of `blocks`.
fn body_instructions(
    function: &Function,
    blocks: impl IntoIterator<Item = BlockId>,
) -> Vec<llrm_mir::module::InstId> {
    blocks.into_iter().flat_map(|block| function.iter_block(block).collect::<Vec<_>>()).collect()
}

fn try_to_unroll_loop(
    run: &mut Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> Unrolled {
    if run.finished.contains(&lp.get_header()) {
        why(run, lp, "already unrolled");
        return Unrolled::Unmodified;
    }
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
    let mut se = std::mem::replace(&mut run.scev, ScalarEvolution::placeholder());
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
    let mut object_bounded: Option<u32> = None;
    if trip_count == 0 {
        max_trip_count = se.get_small_constant_max_trip_count(&cx, header);
        max_or_zero = se.is_backedge_taken_count_max_or_zero(&cx, header);
        // DEVIATE OB1: where the pin's bound is none or too large to unroll by, the object the loop's
        // accesses run through gives one (its range-derived 2^31-1 for `i < n`).
        if let Some(cap) = run.tti.unroll_object_bound()
            && !opt_for_size
            && run.options.opt_level >= 2
            && let Some(bound) = crate::unrollbound::object_max_trip_count(&mut se, &cx, header)
            && bound <= cap
            // The pin's bound is none, or too large to unroll by.
            && (max_trip_count == 0 || max_trip_count > cap)
            // Not under a loop that unrolls whole: its constant counter makes the trips of this loop exact in
            // the copies, where a bounded unroll leaves exit tests (nbody -O2/-O3: 20,604 more instructions).
            && !lp.get_parent_loop().is_some_and(|parent| {
                let trips = se.get_small_constant_trip_count(&cx, parent.get_header());
                trips != 0 && trips <= up.full_unroll_max_count
            })
        {
            max_trip_count = bound;
            object_bounded = Some(cap);
        }
    }

    // DEVIATE U3: on a float stack, copies of a body whose floats do not fit
    // twice are interleaved past the stack and spill, where the unroll keeps
    // more floats live across its trips: the elements of an array it both
    // reads and writes are forwarded from copy to copy, one value each
    // (nbody_single and fpbench -O3: 57 stores and 90 reloads against 30 and
    // 30 rolled, -O2 within the default bar the unroll won: -2.5%, -4.0%).
    // Such a body is held to the default bar only then: independent trips
    // keep nothing across them (x_jpeg_single -O3: three 8-trip loops of 9
    // floats, held to 150 and 2.3M clocks slower than the pin's unroll).
    // Owner llrm-target-x86-m32, exit 2026-11-20.
    // The bar only matters where it is above the default: the liveness is not
    // solved for a loop it cannot change.
    if !opt_for_size
        && up.threshold > UNROLL_THRESHOLD_DEFAULT
        && let Some(peak) = float_stack_overflow(run, lp)
    {
        let kept = if trip_count == 0 { usize::MAX } else { price::forwarded_floats(run, analyzed, lp, trip_count) };
        let depth = run.tti.x87_stack_slots().map_or(0, |slots| slots as usize);
        if peak.saturating_add(kept) > peak.max(depth) {
            why(run, lp, &format!("{peak} floats live in a body, {kept} kept across its trips: the default bar"));
            up.threshold = UNROLL_THRESHOLD_DEFAULT;
        }
    }

    // OB1: a bound the object gives is unrolled by in full, to the target's cap.
    if let Some(cap) = object_bounded {
        up.upper_bound = true;
        up.max_upper_bound = up.max_upper_bound.max(cap);
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
        let scopes = crate::noaliasscopes::declared_scopes(
            run.function,
            body_instructions(run.function, lp.get_blocks().iter().copied()),
        );
        return match crate::loopclone::peeled_any_latch(run.function, &loop_, i64::from(pp.peel_count)) {
            Ok(Some(labels)) => {
                // `cloneAndAdaptNoAliasScopes` for each peeled iteration
                // (LoopPeel.cpp:956).
                for (iteration, blocks) in labels.iter().enumerate() {
                    let copies = body_instructions(run.function, blocks.values().copied());
                    crate::noaliasscopes::cloned_and_adapted(
                        run.function,
                        &scopes,
                        &copies,
                        &format!("Peel{}", iteration + 1),
                        run.metadata,
                        run.declared,
                    );
                }
                let so_far = run.peeled.get(&header).copied().unwrap_or(0);
                run.peeled.insert(header, so_far + pp.peel_count);
                Unrolled::Peeled
            }
            Ok(None) => Unrolled::Unmodified,
            Err(error) => panic!("loop peel: {error}"),
        };
    }

    // `LoopFullUnrollPass` does not do partial or runtime unrolling.
    // A known trip count is unrolled in place, the exit kept in the copy it
    // falls in (`BreakoutTrip`): `UP.Runtime &= TripCount == 0 && ...`
    // ([LoopUnrollPass.cpp:1439](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1439)).
    let partial = trip_count != 0 && count < trip_count;
    if !run.options.target_unrolling
        && ((trip_count == 0 && max_trip_count == 0) || count < trip_count || count < max_trip_count)
    {
        why(run, lp, "partial/runtime unroll in FullLoopUnroll");
        return Unrolled::Unmodified;
    }
    // Runtime unrolling: the epilog remainder first (`UnrollLoop`), then the
    // loop, made again, is copied out `count` times.
    let mut remade = None;
    // `UnrollLoop`: a count that reaches the max trip count unrolls the loop
    // whole, and "there's no point in performing runtime unrolling" then
    // ([LoopUnroll.cpp#L1019-L1026](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L1019-L1026)).
    let completely = max_trip_count != 0 && count >= max_trip_count;
    if run.options.target_unrolling && trip_count == 0 && !completely {
        if trip_multiple % count != 0 {
            // `EpilogProfitability` (LoopUnroll.cpp#L1058): the epilog only
            // where a header phi starts at an integer constant.
            let use_epilog = lp.get_loop_preheader(run.function).is_some_and(|pre| {
                crate::loopunrollruntime::is_epilog_profitable(run.context, run.function, header, pre)
            });
            let expansion =
                llrm_analysis::scev::expand_cost::MachineCosts { tti: run.outer.target().tti(), minimum_size: false };
            let remainder_mark = run.function.mark();
            let mut session = llrm_analysis::scev::analysis::ScevSession::of(run.context, run.function, run.outer);
            // The pin hands `UnrollLoop` the one `ScalarEvolution` the count
            // was worked out in: an earlier loop's remainder guard, read while
            // counting this loop, is a value its expansion reuses.
            session.adopt(std::mem::replace(&mut run.scev, ScalarEvolution::placeholder()));
            let mut env = crate::loopunrollruntime::Env {
                context: &mut *run.context,
                layout: run.layout,
                function: &mut *run.function,
                declared: &mut *run.declared,
                outer: run.outer,
                tti: &expansion,
            };
            let done = crate::loopunrollruntime::unroll_runtime_loop_remainder(
                &mut env,
                &mut session,
                header,
                count,
                false,
                use_epilog,
            );
            let Some(epilog) = done else {
                run.scev = session.into_scev();
                why(run, lp, "the remainder loop could not be generated");
                return Unrolled::Unmodified;
            };
            // `forgetTopmostLoop`: the one `ScalarEvolution` lives on, so the
            // next loop does not work out every earlier loop's count again.
            run.scev = session.into_scev();
            forget_since(run, analyzed, remainder_mark);
            run.finished.insert(header);
            if let Some(epilog) = epilog {
                run.finished.insert(epilog);
            }
            if up.unroll_remainder
                && let Some(epilog) = epilog
            {
                crate::loopunrollandjam::unroll_remainder_loop(run, epilog, count - 1);
            }
        }
        remade = Some(Analyzed::of(run));
    }
    let runtime = remade.is_some();
    let (analyzed, lp) = match &remade {
        Some(analyzed) => {
            (analyzed, analyzed.loops.get_loop_for(header).expect("the loop is still headed by its block"))
        }
        None => (analyzed, lp),
    };

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
        // A run-time unroll keeps the back edge: the remainder made the latch's
        // trip multiple `count`, so only its last copy tests the exit.
        max_trip_count: if runtime {
            u32::MAX
        } else if partial {
            trip_count
        } else {
            count
        },
        max_or_zero,
        exits: if runtime {
            lp.get_loop_latch(run.function).map(|block| Exit { block, trip_count: count }).into_iter().collect()
        } else {
            exits.into_iter().filter(|exit| exit.trip_count != 0).collect()
        },
    };
    let scopes =
        crate::noaliasscopes::declared_scopes(run.function, body_instructions(run.function, body.iter().copied()));
    let (metadata, declared) = (run.metadata, &mut *run.declared);
    let mut rescoped = |function: &mut Function, copies: &[llrm_mir::module::InstId], iteration: usize| {
        crate::noaliasscopes::cloned_and_adapted(
            function,
            &scopes,
            copies,
            &format!("It{iteration}"),
            metadata,
            declared,
        );
    };
    match unroll_loop_blocks(run.function, header, &body, &latches, &unroll_options, &mut run.emptied, &mut rescoped) {
        Ok(result) => result,
        Err(error) => panic!("loop unroll: {error}"),
    }
}

/// Whether the loop calls something the target lowers to a call.
/// The most floats live at once in the body of `lp`, if the target computes
/// them on a stack and twice that many do not fit it.
fn float_stack_overflow(
    run: &Run,
    lp: Loop,
) -> Option<usize> {
    let depth = run.tti.x87_stack_slots()? as usize;
    let blocks = lp.get_blocks();
    let is_float = |value: llrm_mir::module::ValueId| {
        llrm_mir::tti::Ty::of(&run.context.types, run.function.value(value).ty).is_some_and(|ty| ty.is_float())
    };
    if !blocks.iter().any(|&block| {
        run.function.iter_block(block).any(|inst| run.function.instruction(inst).result.is_some_and(&is_float))
    }) {
        return None;
    }
    let peak = llrm_analysis::liveness::pressure_in(run.function, blocks, &is_float);
    (2 * peak > depth).then_some(peak)
}

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

/// `!CompletelyUnroll`: the loop headed by `header` is a loop after the unroll,
/// its header reached from one of `after` (the preheader is not).
fn is_still_a_loop(
    run: &Run,
    header: BlockId,
    after: &[BlockId],
) -> bool {
    run.function.predecessors(header).into_iter().any(|one| after.contains(&one))
}

/// The edges out of the blocks an unroll of the loop `body` can change: its
/// own, its preheaders' and its exits'.
fn unroll_edges(
    function: &Function,
    header: BlockId,
    body: &[BlockId],
) -> Vec<(BlockId, BlockId)> {
    let inside: IdSet<BlockId> = body.iter().copied().collect();
    let mut from: Vec<BlockId> = body.to_vec();
    for pred in function.predecessors(header) {
        if !inside.contains(&pred) {
            from.push(pred);
        }
    }
    for &one in body {
        for next in function.successors(one) {
            if !inside.contains(&next) && !from.contains(&next) {
                from.push(next);
            }
        }
    }
    edges_from(function, &from)
}

fn edges_from(
    function: &Function,
    from: &[BlockId],
) -> Vec<(BlockId, BlockId)> {
    let mut edges = Vec::new();
    for &one in from {
        for next in function.successors(one) {
            edges.push((one, next));
        }
    }
    edges
}

/// `UnrollLoop`'s `DTU` updates: the edges the unroll made and removed, as
/// `applyUpdates` takes them, from the edges before it of `before`'s blocks
/// and the blocks made since the function had `capacity`.
fn unroll_updates(
    function: &Function,
    capacity: usize,
    before: &[(BlockId, BlockId)],
) -> Vec<llrm_mir::domtree::Update> {
    use llrm_mir::domtree::Update;
    let mut from: Vec<BlockId> = before.iter().map(|&(one, _)| one).collect();
    from.dedup();
    from.extend(function.layout().iter().copied().filter(|one| one.0 as usize >= capacity));
    let after = edges_from(function, &from);
    let kept: std::collections::HashSet<(BlockId, BlockId)> = after.iter().copied().collect();
    let was: std::collections::HashSet<(BlockId, BlockId)> = before.iter().copied().collect();
    let mut updates = Vec::new();
    for &(a, b) in before {
        if !kept.contains(&(a, b)) {
            updates.push(Update::delete(a, b));
        }
    }
    for &(a, b) in &after {
        if !was.contains(&(a, b)) {
            updates.push(Update::insert(a, b));
        }
    }
    updates
}

/// The blocks of the natural loop of `header` in the tree: those that reach a
/// back edge without passing the header.
fn natural_loop(
    function: &Function,
    tree: &DomTree,
    header: BlockId,
) -> IdSet<BlockId> {
    let mut found: IdSet<BlockId> = IdSet::new();
    found.insert(header);
    let mut work: Vec<BlockId> = function.predecessors(header).into_iter().filter(|&p| tree.dominates(header, p)).collect();
    while let Some(one) = work.pop() {
        if found.insert(one) {
            work.extend(function.predecessors(one));
        }
    }
    found
}

/// The nest after an unroll that kept the loop headed by `header` and made no
/// other loop (`UnrollLoop` updates `LI` in place): the blocks the unroll made
/// join the loop and those it merged away leave it. `None` where the loop's
/// blocks are not those, and the nest is made again.
fn followed_nest(
    run: &Run,
    analyzed: &mut Analyzed,
    header: BlockId,
    body: &[BlockId],
    capacity: usize,
    merged: usize,
) -> bool {
    let gone: Vec<BlockId> = run.emptied[merged..].iter().copied().filter(|one| body.contains(one)).collect();
    let made: Vec<BlockId> = run
        .function
        .layout()
        .iter()
        .copied()
        .filter(|one| one.0 as usize >= capacity && !run.emptied.contains(one))
        .collect();
    let mut expected: IdSet<BlockId> = body.iter().copied().filter(|one| !gone.contains(one)).collect();
    expected.extend(made.iter().copied());
    if natural_loop(run.function, &analyzed.tree, header) != expected {
        return false;
    }
    let loops = std::rc::Rc::make_mut(&mut analyzed.loops);
    let Some(id) = loops.get_loop_for(header).map(|one| one.id()) else { return false };
    loops.remove_blocks_from_loop(id, &gone);
    loops.add_basic_blocks_to_loop(id, &made);
    true
}

/// The `SimplifyIVs` half of `simplifyLoopAfterUnroll`
/// ([LoopUnroll.cpp:350](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L350)),
/// run when the unroll is partial (`!CompletelyUnroll && Count > 1`):
/// `simplifyLoopIVs`, then `loadCSE`. `analyzed` describes the function before
/// the IVs are simplified, whose edits leave its blocks and edges as they are.
fn simplify_ivs_and_load_cse(
    run: &mut Run,
    analyzed: &Analyzed,
    header: BlockId,
) {
    let env = llrm_analysis::scev::expander::Env {
        globals: run.globals,
        metadata: run.metadata,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mark = run.function.mark();
    let _ = crate::simplifyindvar::simplified_loop_ivs(
        run.context,
        run.layout,
        run.function,
        run.declared,
        run.outer,
        &mut run.scev,
        env,
        header,
    );
    crate::loopunrollloadcse::load_cse(run, analyzed, header);
    forget_since(run, analyzed, mark);
}

/// `forgetTopmostLoop` and the deleted values (`SCEVCallbackVH`): the
/// expressions the edits since `mark` made stale are dropped and the rest
/// kept, where the pin's one `ScalarEvolution` lives on through the unroll.
/// `before` are the loops of the function as it was at `mark`; a block made
/// since has none, a block erased or rewritten drops the counts of its loops.
/// The expressions are dropped whole where the log from `mark` is gone.
fn forget_since(
    run: &mut Run,
    before: &Analyzed,
    mark: llrm_mir::module::Mark,
) {
    let Some(touched) = run.function.touched_since(mark) else {
        run.scev = ScalarEvolution::new();
        return;
    };
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &before.loops,
        tree: &before.tree,
        assumptions: &before.assumptions,
    };
    run.scev.forget_touched(&cx, &touched);
}

/// `forgetTopmostLoop(L)` of `UnrollLoop`, for a loop fully unrolled that held
/// loops: what the expressions of its blocks and its loops' counts said of the
/// nest is stale once the nest is straight-line code and copies.
fn forget_nest(
    run: &mut Run,
    before: &Analyzed,
    lp: Loop,
) {
    let mut top = lp;
    while let Some(parent) = top.get_parent_loop() {
        top = parent;
    }
    let headers: Vec<BlockId> =
        before.loops.get_loops_in_preorder().iter().filter(|one| top.contains_loop(Some(**one))).map(|one| one.get_header()).collect();
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &before.loops,
        tree: &before.tree,
        assumptions: &before.assumptions,
    };
    run.scev.forget_loop_nest(&cx, top.get_blocks(), &headers);
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
    // LCSSA first, whether or not anything is unrolled. `LoopFullUnrollPass`
    // runs on one loop of a loop pass manager whose loops the slots before it
    // left in both forms and does neither: a run narrowed to one loop
    // (`loopfilter`) skips them.
    let (simplified, closed) = if run.options.target_unrolling || !crate::loopfilter::scoped() {
        // The manager's tree and loops stand for the function as it is: the
        // first look at its loops asks them, not a tree made again.
        let held = run.cached.as_ref().map(|(tree, loops)| llrm_analysis::cfg::Shape {
            dominance: llrm_analysis::cfg::Dominance::new(std::rc::Rc::clone(tree)),
            loops: llrm_analysis::cfg::natural(run.function, loops),
        });
        let simplified = crate::loopsimplify::simplified_given(run.function, held);
        // Nothing was reshaped: the manager's tree and loops are the function's.
        let closed = match &run.cached {
            Some((tree, loops)) if !simplified => {
                crate::lcssa::form_on_all_loops(run.context, run.layout, run.function, tree, loops)
            }
            _ => crate::lcssa::closed(run.context, run.layout, run.function),
        }
        .unwrap_or(false);
        (simplified, closed)
    } else {
        (false, false)
    };
    let mut analyzed = Analyzed::entry(run, !simplified && !closed);
    let at_first: IdSet<BlockId> = analyzed.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
    run.first = at_first.clone();
    let mut kept = IdSet::new();
    for header in run.decided.iter() {
        if analyzed.loops.is_loop_header(header) {
            kept.insert(header);
        }
    }
    run.decided = kept;
    // The worklist is popped from the back (`appendReversedLoopsToWorklist`):
    // the loops are visited in the reverse of
    // `getLoopsInReverseSiblingPreorder`, each loop after the loops it
    // holds.
    let mut work: Vec<BlockId> =
        analyzed.loops.get_loops_in_reverse_sibling_preorder().iter().rev().map(|one| one.get_header()).collect();
    crate::loopfilter::retain(&mut work);
    let mut work: std::collections::VecDeque<BlockId> = work.into();
    // `LoopFullUnrollPass` hands `addSiblingLoops` every sibling the unroll
    // left that was not there before, the unrolled loop's own first iteration
    // among them: they are new to the loop passes after it.
    let mut revisited: Vec<BlockId> = Vec::new();
    let mut parents: Vec<BlockId> = Vec::new();
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
        let mark = run.function.mark();
        let inside: Vec<BlockId> = if holds_loops {
            analyzed
                .loops
                .get_loops_in_preorder()
                .iter()
                .filter(|one| one.get_header() != header && lp.contains_loop(Some(**one)))
                .map(|one| one.get_header())
                .collect()
        } else {
            Vec::new()
        };
        let edges_before = (parent_header.is_none() && !holds_loops).then(|| unroll_edges(run.function, header, &body));
        let merged = run.emptied.len();
        let result = try_to_unroll_loop(run, &analyzed, lp);
        if result == Unrolled::Unmodified {
            continue;
        }
        // The loops of a nest whose outer loop is unrolled are its copies'
        // loops, whichever copy keeps the first ids: they are priced as the
        // copies are (`price.rs`, loops this run made).
        if result == Unrolled::Fully {
            for one in &inside {
                run.first.remove(one);
            }
        }
        let peeled = result == Unrolled::Peeled;
        changed = true;
        forget_since(run, &analyzed, mark);
        if result == Unrolled::Fully && holds_loops {
            forget_nest(run, &analyzed, lp);
        }
        // The pin's loop pass manager runs LPM2's whole list on a loop,
        // children before parents ([`L270`][pop]): a parent's IndVars
        // sees its child already unrolled. Here each pass runs over
        // every loop in turn, so the parent is queued for IndVars
        // again.
        //
        // [pop]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopPassManager.cpp#L270
        if let Some(parent) = parent_header {
            parents.push(parent);
        }
        if peeled {
            // `PartiallyUnrolled`: its analyses and its nest are made again
            // (the cloned iterations are loops' blocks of its parent).
            // `peelLoop` ends in `simplifyLoop`: the exits the copies share
            // are dedicated again (here for the whole function, U6).
            crate::loopsimplify::simplified(run.function);
            let _ = crate::lcssa::closed(run.context, run.layout, run.function);
            analyzed = Analyzed::of(run);
            // `simplifyLoopAfterUnroll(L, SimplifyIVs = true)`: the IVs first
            // (PL5).
            let env = llrm_analysis::scev::expander::Env {
                globals: run.globals,
                metadata: run.metadata,
                layout: run.layout,
                loops: &analyzed.loops,
                tree: &analyzed.tree,
                assumptions: &analyzed.assumptions,
            };
            let _ = crate::simplifyindvar::simplified_loop_ivs(
                run.context,
                run.layout,
                run.function,
                run.declared,
                run.outer,
                &mut run.scev,
                env,
                header,
            );
            // Only the loop that remains (`L->getBlocks()`,
            // LoopUnrollPass.cpp:1417): the peeled copies keep what they were
            // cloned as.
            let after = unrolled_blocks(run, usize::MAX, &body);
            simplify_loop_after_unroll(run, Some(&analyzed.loops), &after);
            analyzed = Analyzed::of(run);
            // `LoopFullUnrollPass` does not revisit the loop it peeled; the
            // loops the peeled copy holds are new siblings, visited next
            // (`addSiblingLoops`). `LoopUnrollPass` walks the loops it began
            // with: it adds none.
            if !run.options.target_unrolling {
                let mut fresh: Vec<BlockId> =
                    siblings_of(&analyzed.loops).into_iter().filter(|one| !old.contains(one)).collect();
                fresh.reverse();
                for one in fresh {
                    work.push_front(one);
                }
            }
            continue;
        }
        // A top-level loop with no loops inside leaves the analyses of the
        // other loops as they were: they are made again only otherwise (made
        // after every loop they were `n^2` over `n` loops in a function).
        if let Some(edges) = edges_before {
            let after = unrolled_blocks(run, capacity, &body);
            // The tree follows the unroll's edges (`DTU`), where the nest is
            // made again.
            let updates = unroll_updates(run.function, capacity, &edges);
            std::rc::Rc::make_mut(&mut analyzed.tree).apply_updates(run.function, &updates);
            if llrm_support::env_flag!("LLRM_CHECK_DOMTREE_MEMO") {
                assert!(*analyzed.tree == DomTree::of(run.function), "the unroll's updates left the tree different");
            }
            if is_still_a_loop(run, header, &after) {
                if !followed_nest(run, &mut analyzed, header, &body, capacity, merged) {
                    analyzed.loops = LoopInfo::analyze(run.function, || &*analyzed.tree).into();
                    llrm_mir::counts::bump(llrm_mir::counts::Event::LoopAnalyses);
                }
                if llrm_support::env_flag!("LLRM_CHECK_DOMTREE_MEMO") {
                    assert!(
                        analyzed.loops.equivalent(&LoopInfo::analyze(run.function, || &*analyzed.tree), run.function),
                        "the unroll's updates left the nest different"
                    );
                }
                let assumptions = AssumptionCache::of_declared(
                    run.context,
                    run.globals,
                    run.metadata,
                    run.function,
                    run.outer.assume_declared(),
                );
                let fresh = Analyzed {
                    tree: std::rc::Rc::clone(&analyzed.tree),
                    loops: std::rc::Rc::clone(&analyzed.loops),
                    assumptions,
                };
                simplify_ivs_and_load_cse(run, &fresh, header);
                simplify_loop_after_unroll(run, Some(&fresh.loops), &after);
            } else {
                simplify_loop_after_unroll(run, None, &after);
            }
            continue;
        }
        // `UnrollLoop` closes the outer loop's SSA form
        // (`formLCSSARecursively`).
        let _ = crate::lcssa::closed(run.context, run.layout, run.function);
        analyzed = Analyzed::of(run);
        // The blocks `UnrollLoop` hands `simplifyLoopAfterUnroll`: the loop's
        // own that remain and the ones it made.
        let after = unrolled_blocks(run, capacity, &body);
        if is_still_a_loop(run, header, &after) {
            simplify_ivs_and_load_cse(run, &analyzed, header);
        }
        simplify_loop_after_unroll(run, Some(&analyzed.loops), &after);
        // `UnrollLoop` puts the loops it cloned back in loop-simplify form
        // (`simplifyLoop` of the outer loop); here the whole function is, when
        // the unrolled loop had an outer loop or held loops (U6).
        if parent_header.is_some() || siblings_of(&analyzed.loops).iter().any(|one| !old.contains(one)) {
            // The blocks change only when loop-simplify changed them (LCSSA
            // edits phis alone).
            let reshaped = crate::loopsimplify::simplified(run.function);
            let _ = crate::lcssa::closed(run.context, run.layout, run.function);
            if reshaped {
                analyzed = Analyzed::of(run);
            }
        }
        let mut fresh: Vec<BlockId> =
            siblings_of(&analyzed.loops).into_iter().filter(|one| !old.contains(one)).collect();
        fresh.reverse();
        if !run.options.target_unrolling {
            for &one in &fresh {
                work.push_front(one);
            }
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
    for header in parents {
        if analyzed.loops.get_loop_for(header).is_some_and(|one| one.get_header() == header)
            && !created.contains(&header)
        {
            created.push(header);
        }
    }
    for header in &created {
        run.decided.insert(*header);
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
    /// The pin's `LoopUnrollPass` (`AllowPartial`, `Runtime` and `UpperBound`
    /// from the target) where false is `LoopFullUnrollPass`.
    pub target_unrolling: bool,
    /// `Options::inliner_follows`.
    pub inliner_follows: bool,
    /// The headers of the loops the last run made, for the slot after it: the
    /// loop pass manager's worklist.
    pub created: std::rc::Rc<std::cell::RefCell<Vec<BlockId>>>,
    /// Every loop an earlier run made, kept (unlike `created`) for the runs after it.
    pub decided: std::rc::Rc<std::cell::RefCell<Vec<BlockId>>>,
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
        let cached = (
            analyses.get::<llrm_mir::passes::Dominators>(unit.context, unit.layout, unit.function),
            analyses.get::<llrm_mir::passes::Loops>(unit.context, unit.layout, unit.function),
        );
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
            finished: IdSet::new(),
            created: Vec::new(),
            decided: self.decided.borrow().iter().copied().collect(),
            first: Default::default(),
            scev: held.take(),
            cached: Some(cached),
            options: Options {
                opt_level: self.opt_level,
                opt_size: self.opt_size,
                min_size: false,
                only_when_forced: self.only_when_forced,
                prepare_for_lto: false,
                target_unrolling: self.target_unrolling,
                inliner_follows: self.inliner_follows,
            },
        };
        let changed = unroll_loops(&mut run);
        *self.created.borrow_mut() = std::mem::take(&mut run.created);
        *self.decided.borrow_mut() = run.decided.iter().collect();
        if changed {
            PreservedAnalyses::none()
        } else {
            held.put(std::mem::replace(&mut run.scev, ScalarEvolution::placeholder()));
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
    unroll_function_on(module, name, tti, options, None)
}

/// `unroll_function` for a program of its own for `target`: the machine's costs and throughput model.
pub fn unroll_function_on(
    module: &mut llrm_mir::module::Module,
    name: &str,
    tti: &dyn Tti,
    options: Options,
    target: Option<std::rc::Rc<dyn llrm_mir::target::Machine>>,
) -> Result<bool, String> {
    unroll_function_hiding(module, name, tti, options, target, 0)
}

/// `unroll_function_on` with the last `hidden` globals left out of the pass's view of the module: the callees a
/// pipeline run declares after its snapshot (`Declared`), which a call in the function names by an id past it.
pub(crate) fn unroll_function_hiding(
    module: &mut llrm_mir::module::Module,
    name: &str,
    tti: &dyn Tti,
    options: Options,
    target: Option<std::rc::Rc<dyn llrm_mir::target::Machine>>,
    hidden: usize,
) -> Result<bool, String> {
    let layout =
        DataLayout::parse(module.datalayout.as_deref().unwrap_or("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"))?;
    let callees = llrm_mir::memory::callees(module);
    let outer = llrm_mir::passes::Outer::of(module, target);
    let mut declared = llrm_mir::passes::Declared::of(module);
    let mut globals = module.globals.clone();
    globals.truncate(globals.len() - hidden);
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
        finished: IdSet::new(),
        created: Vec::new(),
        decided: IdSet::new(),
        first: Default::default(),
        scev: ScalarEvolution::new(),
        cached: None,
    };
    let unrolled = unroll_loops(&mut run);
    declared.place(module)?;
    Ok(unrolled)
}

#[cfg(test)]
#[path = "loopunrollpass_tests.rs"]
mod tests;
