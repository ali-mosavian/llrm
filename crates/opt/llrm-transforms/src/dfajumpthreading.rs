//! DFA jump threading: a loop whose `switch` runs on a state variable that
//! each path of the loop sets to a constant, as the body of a state machine
//! does, is threaded: every such path gets its own copy of the blocks from
//! where the state is set to the `switch`, and the copy of the `switch` is a
//! jump to the case the constant picks; as LLVM's `dfa-jump-threading`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `DFAJumpThreading.cpp`, one Rust
//! function per C++ function, each linking its line; the plan and the stages
//! are `docs/architecture/dfajumpthreading-translation.md`. This is stages S1
//! and S2. [`mainswitch`] decides whether a `switch` is a candidate
//! (`MainSwitch`): its condition is a `phi` in a loop and what flows into it is
//! constants, `select`s that can be unfolded or values nothing predicts.
//! [`unfold`] turns those `select`s into control flow (a work list), so each
//! value of the state is set on a path of its own; the pin counts that as a
//! change even when nothing is threaded after it. [`paths`] enumerates the
//! paths that set the state to a constant (`AllSwitchPaths`), bounded by the
//! pin's three caps. [`transform`] decides whether copying is legal and
//! profitable and does it (`TransformDFA`): the blocks of each path from the
//! determinator on are cloned per exit value, the predecessors are redirected
//! to the clones, and SSA is restored.
//!
//! ```text
//!   p1: s = 1 --\                       p1: s = 1 --> sw.jt1: br c1
//!   p2: s = 2 ---+--> det: phi s        p2: s = 2 --> sw.jt2: br c2
//!                     sw: switch s      det, sw stay for the paths nobody decided
//! ```
//!
//! DEVIATE (DF2, DF3; DF1, a `select` in the chain rejecting the switch, was
//! closed by S2, and DF4, the loop info rebuilt after the unfolding, by S3; DF5
//! is a parity note):
//! - DF2. No `DomTreeUpdater`: the pass returns `none()` when it threaded
//!   anything and the manager rebuilds the dominator tree and the loop info
//!   (the pin keeps the dominator tree, and the loop info unless it cloned);
//!   `updateSSA` reads a dominator tree built after the threading. No noalias
//!   scopes to clone (J8 of `jumpthreading.rs`), no optimization remarks, and
//!   no `!prof` or debug location is carried to the new branches.
//! - DF3. A path through a block that starts with a `landingpad` is not
//!   threaded: the copy would be a second pad, and the one module handler
//!   (`ehprepare`) cannot choose between two (J10 of `jumpthreading.rs`).
//! - DF5 (parity at the IR, a row in the lowering). On the kernel written for
//!   32 bits (`i32` is `long`, #3535) `x_switch` m16 -O2/-O3 is 709 B with the
//!   pass and 629 B without (+80 B); -O1 and -Os are identical (size gate). The
//!   pass's decision is the pin's: `bench_x_switch` is 86 IR instructions with
//!   it and 50 without, and `opt -passes='default<O2>'` (m16 target,
//!   `-enable-dfa-jump-thread` true/false) gives 93 and 48. The loss is in
//!   lowering the threaded state machine: 198 asm instructions with the pass
//!   and 178 without, where the pin's `llc` has 108 and 131 (the pin gains, we
//!   lose). Owner w-4, exit 2026-11-09. The earlier +17 B was UB code.
//!
//! # Example
//! The state is set to 1 on one path and to 2 on the other, and the `switch`
//! after the join picks the case from it:
//! ```mir
//! entry:
//!   br i1 %c, label %p1, label %p2
//! p1:
//!   br label %det
//! p2:
//!   br label %det
//! det:
//!   %s = phi i16 [ 1, %p1 ], [ 2, %p2 ]
//!   switch i16 %s, label %d [ i16 1, label %c1 i16 2, label %c2 ]
//! ```
//!
//! # Used by
//! `pipeline` (`dfajumpthreading@806`), a `Partial` slot of the parity
//! registry; `llrm-pass dfajumpthreading`.
//!
//! # See also
//! [`DFAJumpThreadingPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1561).

mod mainswitch;
mod paths;
mod transform;
mod unfold;

use std::rc::Rc;

use llrm_mir::codemetrics::Query as CostQuery;
use llrm_mir::context::ConstantKind;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;
use llrm_mir::valuetracking::AssumptionCache;

pub use self::paths::path_steps;

/// What `DFAJumpThreading` is asked for: the `cl::opt`s of the file.
#[derive(Clone, Debug)]
pub struct DfaJumpThreadingOptions {
    /// `dfa-early-exit-heuristic`: give up on a switch whose state has an
    /// unpredictable value coming from its own loop.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L101>
    pub early_exit_heuristic: bool,
    /// `dfa-max-path-length`: the blocks searched to find a path.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L106>
    pub max_path_length: u32,
    /// `dfa-max-num-visited-paths`: the blocks visited enumerating paths around
    /// a switch.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L111>
    pub max_num_visited_paths: u32,
    /// `dfa-max-num-paths`: the paths enumerated around a switch.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L117>
    pub max_num_paths: u32,
    /// `dfa-cost-threshold`: the most the copying may cost.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L122>
    pub cost_threshold: u32,
    /// `dfa-max-cloned-rate`: the most instructions cloned per instruction
    /// duplicated.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L127>
    pub max_cloned_rate: f64,
    /// `dfa-max-out-use-blocks`: the unduplicated blocks with outer uses
    /// accepted.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L133>
    pub max_outer_use_blocks: u32,
}

impl Default for DfaJumpThreadingOptions {
    fn default() -> Self {
        Self {
            early_exit_heuristic: true,
            max_path_length: 20,
            max_num_visited_paths: 2500,
            max_num_paths: 200,
            cost_threshold: 50,
            max_cloned_rate: 7.5,
            max_outer_use_blocks: 40,
        }
    }
}

/// `DFAJumpThreadingPass`. On by default; `LLRM_NO_DFAJUMPTHREADING` (set)
/// turns it off.
pub struct DfaJumpThreading {
    pub enabled: bool,
    /// The build is for size (-Os, -Oz): clang gives its functions `optsize`
    /// there, and the pin skips them (`hasOptSize`). llrm's functions do not
    /// carry the attribute, so the pipeline says it.
    pub for_size: bool,
    pub options: DfaJumpThreadingOptions,
}

impl DfaJumpThreading {
    pub fn from_env() -> Self {
        Self {
            enabled: !llrm_support::env_flag!("LLRM_NO_DFAJUMPTHREADING"),
            for_size: false,
            options: DfaJumpThreadingOptions::default(),
        }
    }
}

impl DfaJumpThreading {
    /// For a build that prefers size: nothing is threaded.
    pub fn for_size(
        mut self,
        for_size: bool,
    ) -> Self {
        self.for_size = for_size;
        self
    }
}

impl FunctionPass for DfaJumpThreading {
    fn name(&self) -> &'static str {
        "dfajumpthreading"
    }

    /// `DFAJumpThreadingPass::run`
    /// ([L1561](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1561)).
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if !self.enabled {
            return PreservedAnalyses::all();
        }
        let outer = Rc::clone(analyses.outer());
        let loops = analyses.get::<Loops>(&*unit.context, unit.layout, &*unit.function);
        let tti = crate::looprotate::target_tti(outer.target());
        let mut run = Run {
            unit,
            globals: Rc::clone(&outer.globals),
            callees: outer.callees(),
            tti: &*tti,
            options: &self.options,
            for_size: self.for_size,
        };
        if run.run_impl(loops) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// What the pass reads and changes: the function with its unit, the module's
/// globals and callees, the target's costs and the options.
struct Run<'a, 'u> {
    unit: &'a mut Unit<'u>,
    globals: Rc<Vec<GlobalValue>>,
    callees: &'a Callees,
    tti: &'a dyn Tti,
    options: &'a DfaJumpThreadingOptions,
    for_size: bool,
}

impl Run<'_, '_> {
    fn function(&self) -> &Function {
        &*self.unit.function
    }

    /// `DFAJumpThreading::run`
    /// ([L1478](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1478)):
    /// the first switch of the function with paths to thread, threaded; whether
    /// anything changed.
    fn run_impl(
        &mut self,
        mut loops: Rc<LoopInfo>,
    ) -> bool {
        // Skipping due to the 'minsize' attribute (`hasOptSize`).
        if self.for_size
            || self.function().attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
                )
            })
        {
            return false;
        }

        // For the time being limit this optimization to occurring once in a
        // function since it can change the CFG significantly: the first switch
        // with threading paths.
        let mut unfolded = false;
        let mut threadable = None;
        for bb in self.function().layout().to_vec() {
            let Some(term) = self.function().terminator(bb) else { continue };
            if self.function().instruction(term).opcode != Opcode::Switch {
                continue;
            }
            let Some(main) = mainswitch::MainSwitch::of(self.function(), &loops, self.options, term) else {
                continue;
            };
            if !main.selects.is_empty() {
                unfold::unfold_select_instrs(&mut self.unit.function, Rc::make_mut(&mut loops), &main.selects);
                unfolded = true;
                // The update of the loop info equals an analysis made afresh.
                debug_assert!(
                    loops.equivalent(
                        &LoopInfo::analyze(self.function(), || llrm_mir::domtree::DomTree::of(self.function())),
                        self.function()
                    ),
                    "the loop info after unfolding differs from a fresh analysis"
                );
            }
            let outermost = loops.get_loop_for(bb).expect("a candidate is in a loop").get_outermost_loop();
            let found = paths::find(self.function(), &*self.unit.context, &loops, self.options, main.switch, outermost);
            if !found.paths.is_empty() {
                threadable = Some(found);
                break;
            }
        }
        let Some(found) = threadable else { return unfolded };

        let assumes =
            AssumptionCache::of(&*self.unit.context, &self.globals, self.unit.metadata, self.function()).assumes;
        let function = self.function();
        let context = &*self.unit.context;
        let callees = self.callees;
        let effects_free = |inst: InstId| llrm_mir::memory::only_value(context, callees, function, inst);
        let eph = llrm_mir::codemetrics::collect_ephemeral_values(function, &effects_free, &assumes, &|_| true);
        // The transform may refuse: the unfolding stays.
        transform::TransformDfa { run: self, paths: found, eph }.run() || unfolded
    }

    /// What `CodeMetrics` reads besides the block.
    fn cost_query(&self) -> CostQuery<'_> {
        CostQuery {
            tti: self.tti,
            context: &*self.unit.context,
            layout: self.unit.layout,
            globals: &self.globals,
            this: self.unit.id,
            has_one_live_use: &|_| false,
        }
    }
}

/// `successors(BB)`: the blocks the terminator names, one per edge, in
/// operand order.
fn successor_edges(
    function: &Function,
    block: BlockId,
) -> Vec<BlockId> {
    let term = function.terminator(block).expect("a terminated block");
    function
        .instruction(term)
        .operands
        .iter()
        .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
        .collect()
}

/// `Phi->blocks()` with `getIncomingValueForBlock` of each: one entry per
/// input, and the first input's value for a block that appears twice.
fn incoming_per_block(
    function: &Function,
    phi: InstId,
) -> Vec<(BlockId, Operand)> {
    let arms = crate::lcssa::arms(function, phi);
    arms.iter()
        .map(|&(_, block)| (block, arms.iter().find(|one| one.1 == block).expect("an input from the block").0))
        .collect()
}

/// `getIncomingValueForBlock`: the value `phi` takes from `block`, if it has
/// an input from it.
fn incoming_value_for_block(
    function: &Function,
    phi: InstId,
    block: BlockId,
) -> Option<Operand> {
    crate::lcssa::arms(function, phi).into_iter().find(|&(_, from)| from == block).map(|(value, _)| value)
}

/// `dyn_cast<PHINode>(value)`.
fn phi_of(
    function: &Function,
    value: Operand,
) -> Option<InstId> {
    let Operand::Value(id) = value else { return None };
    match function.value(id).def {
        llrm_mir::module::ValueDef::Instruction(inst) if function.instruction(inst).opcode == Opcode::Phi => Some(inst),
        _ => None,
    }
}

/// `dyn_cast<ConstantInt>(value)`: the bits of an integer constant.
fn constant_int(
    context: &llrm_mir::context::Context,
    value: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = value else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// The cases of the `switch`: each case value's bits and successor, in order.
fn switch_cases(
    function: &Function,
    context: &llrm_mir::context::Context,
    switch: InstId,
) -> Vec<(u128, BlockId)> {
    function.instruction(switch).operands[2..]
        .chunks(2)
        .map(|pair| match (constant_int(context, pair[0]), pair[1]) {
            (Some(bits), Operand::Block(to)) => (bits, to),
            other => unreachable!("a switch case is an integer and a block, not {other:?}"),
        })
        .collect()
}

/// The `switch`'s default destination.
fn default_dest(
    function: &Function,
    switch: InstId,
) -> BlockId {
    let Operand::Block(default) = function.instruction(switch).operands[1] else {
        unreachable!("a switch names its default")
    };
    default
}
