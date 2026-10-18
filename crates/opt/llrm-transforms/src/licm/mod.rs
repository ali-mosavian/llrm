//! Loop-invariant code motion, translated from LLVM's `LICM.cpp`
//! ([`d1106deb71cc`](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266)).
//! L1: the hoisting decisions only; nothing is moved. The pipeline does not
//! call it.
//!
//! # See also
//! `docs/architecture/licm-port.md`; the MemorySSA it asks is
//! `llrm_analysis::mssa`.

mod apply;
mod calls;
mod hoist;
mod promote;
mod safety;
mod sink;
#[cfg(test)]
mod tests;

pub use hoist::{can_hoist_load, can_sink_or_hoist_inst};
use llrm_analysis::cfg::{self, Shape};
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::Mssa;
use llrm_analysis::mssa::alias::AliasQuery;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
pub use safety::LoopSafety;

/// `SetLicmMssaOptCap`: how many clobbering-access walks a loop may ask before
/// the defining access stands in
/// ([`LICM.cpp#L111`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L111)).
pub const LICM_MSSA_OPT_CAP: u32 = 100;

/// `SetLicmMssaNoAccForPromotionCap`: how many memory accesses a loop may hold
/// before no store is hoisted or sunk and nothing is promoted
/// ([`LICM.cpp#L118`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L118)).
pub const LICM_MSSA_NO_ACC_FOR_PROMOTION_CAP: u32 = 250;

/// [`SinkAndHoistLICMFlags`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L388).
pub struct Flags {
    no_of_mem_acc_too_large: bool,
    licm_mssa_opt_counter: u32,
    licm_mssa_opt_cap: u32,
    is_sink: bool,
}

impl Flags {
    pub fn new(
        licm_mssa_opt_cap: u32,
        licm_mssa_no_acc_for_promotion_cap: u32,
        is_sink: bool,
        blocks: impl IntoIterator<Item = BlockId>,
        mssa: &Mssa,
    ) -> Self {
        let mut access_cap_count = 0u32;
        let mut too_large = false;
        'count: for block in blocks {
            if let Some(accesses) = mssa.get_block_accesses(block) {
                for _ in accesses {
                    access_cap_count += 1;
                    if access_cap_count > licm_mssa_no_acc_for_promotion_cap {
                        too_large = true;
                        break 'count;
                    }
                }
            }
        }
        Self { no_of_mem_acc_too_large: too_large, licm_mssa_opt_counter: 0, licm_mssa_opt_cap, is_sink }
    }

    pub fn set_is_sink(
        &mut self,
        is_sink: bool,
    ) {
        self.is_sink = is_sink;
    }

    pub fn is_sink(&self) -> bool {
        self.is_sink
    }

    pub fn too_many_memory_accesses(&self) -> bool {
        self.no_of_mem_acc_too_large
    }

    pub fn too_many_clobbering_calls(&self) -> bool {
        self.licm_mssa_opt_counter >= self.licm_mssa_opt_cap
    }

    pub fn increment_clobbering_calls(&mut self) {
        self.licm_mssa_opt_counter += 1;
    }
}

/// What the decisions so far have done to the function: where the
/// instructions moved now stand, and which are gone from the loops they were
/// in (erased as dead, or sunk).
#[derive(Default)]
pub struct State {
    pub moved: std::cell::RefCell<IdMap<InstId, BlockId>>,
    pub gone: std::cell::RefCell<llrm_mir::dense::IdSet<InstId>>,
    /// Those of `gone` that were sunk, not erased: a clone of each stands in an
    /// exit block, so what it reads has a user outside the loop.
    pub sunk: std::cell::RefCell<llrm_mir::dense::IdSet<InstId>>,
}

/// One loop, as `hoistRegion` sees it: its blocks, the loops in it, and where
/// the instructions the pass has moved stand.
pub struct LoopCtx<'a> {
    pub function: &'a Function,
    pub shape: &'a Shape,
    pub loop_: &'a Loop,
    /// Each block's innermost loop, an index in `shape.loops`.
    pub innermost: &'a IdMap<BlockId, usize>,
    pub state: &'a State,
    /// Only sink through exit phis that read nothing else
    /// (`isTriviallyReplaceablePHI`): the pass that moves instructions does
    /// not split exit blocks.
    pub strict_sink: bool,
}

impl LoopCtx<'_> {
    /// `Loop::contains(BB)`.
    pub fn contains_block(
        &self,
        block: BlockId,
    ) -> bool {
        self.loop_.body.contains(&cfg::id(block))
    }

    /// `Loop::contains(const Instruction *)`.
    pub fn contains(
        &self,
        inst: InstId,
    ) -> bool {
        let block = self.state.moved.borrow().get(&inst).copied().or_else(|| self.function.parent(inst));
        block.is_some_and(|block| self.contains_block(block))
    }

    /// `inSubLoop`: the block belongs to a loop inside this one.
    pub fn in_sub_loop(
        &self,
        block: BlockId,
    ) -> bool {
        self.innermost.get(&block).is_some_and(|&at| self.shape.loops[at] != *self.loop_)
    }

    /// `Loop::isLoopInvariant(Value)`.
    pub fn is_loop_invariant(
        &self,
        operand: Operand,
    ) -> bool {
        match operand {
            Operand::Value(value) => match self.function.value(value).def {
                ValueDef::Instruction(def) => !self.contains(def),
                ValueDef::Argument(_) => true,
            },
            Operand::Constant(_) | Operand::Block(_) => true,
        }
    }

    /// `Loop::hasLoopInvariantOperands`.
    pub fn has_loop_invariant_operands(
        &self,
        inst: InstId,
    ) -> bool {
        self.function.instruction(inst).operands.iter().all(|&operand| self.is_loop_invariant(operand))
    }
}

/// An instruction `hoistRegion` hoists out of the loop headed by `header`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hoisted {
    /// The function, where its module names it.
    pub function: Option<llrm_mir::GlobalId>,
    pub header: BlockId,
    pub inst: InstId,
}

/// A set of pointers whose loop accesses `promoteLoopAccessesToScalars` takes
/// out of the loop headed by `header`.
#[derive(Clone, Debug)]
pub struct Promoted {
    pub function: Option<llrm_mir::GlobalId>,
    pub header: BlockId,
    pub pointers: Vec<Operand>,
    pub uses: Vec<InstId>,
    pub store_safe: bool,
}

/// What the pass decided over a function.
#[derive(Clone, Debug, Default)]
pub struct Decisions {
    pub hoisted: Vec<Hoisted>,
    /// `NumPromotionCandidates`, and the sets promoted.
    pub promotion_candidates: usize,
    pub promoted: Vec<Promoted>,
    /// `NumSunk`, and what `isInstructionTriviallyDead` erases.
    pub sunk: Vec<Hoisted>,
    pub erased: Vec<Hoisted>,
    /// `NumMovedLoads` and `NumMovedCalls`.
    pub moved_loads: usize,
    pub moved_calls: usize,
    /// The memory graph made again after a loop changed: how many times in a
    /// function (the most, over the functions run), and the instructions of the
    /// function each time, added up.
    pub rebuilds: usize,
    pub max_rebuilds: usize,
    pub rebuilt_instructions: usize,
}

fn found_changed(found: &Decisions) -> bool {
    !found.hoisted.is_empty() || !found.sunk.is_empty() || !found.erased.is_empty()
}

/// Each block's innermost loop, an index in `shape.loops`.
fn innermost_loops(shape: &Shape) -> IdMap<BlockId, usize> {
    let mut innermost: IdMap<BlockId, usize> = IdMap::new();
    // `shape.loops` is innermost first where loops nest: claim from the
    // outermost in, so the last to claim a block is its innermost.
    for (at, one) in shape.loops.iter().enumerate().rev() {
        for &block in &one.body {
            innermost.insert(cfg::block(block), at);
        }
    }
    innermost
}

/// [`LoopInvariantCodeMotion::runOnLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L410)
/// over every loop, inner loops first: the hoisting, as decisions.
pub fn decide(
    unit: &passes::Unit,
    analyses: &mut Analyses,
) -> Decisions {
    let shape = analyses.get::<Shape>(unit.context, unit.layout, unit.function);
    let mut decisions = Decisions::default();
    if shape.loops.is_empty() {
        return decisions;
    }
    let outer = std::rc::Rc::clone(analyses.outer());
    let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let memory = llrm_analysis::memory::Unit::within(unit.context, unit.layout, unit.function, &outer)
        .with_registers(&registers)
        .with_shape(&shape);
    let (mut mssa, accesses) = llrm_analysis::mssa::shadow::translated(&memory);
    let aa = AliasQuery::new(&memory, &accesses);
    let innermost = innermost_loops(&shape);
    let state = State::default();
    let exposed: llrm_mir::dense::IdSet<llrm_mir::module::ValueId> =
        llrm_analysis::memory::exposed_frames(&memory).into_iter().collect();
    for (at, one) in shape.loops.iter().enumerate() {
        let _ = at;
        let ctx = LoopCtx {
            function: unit.function,
            shape: &shape,
            loop_: one,
            innermost: &innermost,
            state: &state,
            strict_sink: false,
        };
        let found = run_on_loop(unit, &outer, &ctx, &mut mssa, &aa, &accesses, &exposed, true);
        let (sunk, hoisted) = (found.sunk, found.hoisted);
        let header = cfg::block(one.header);
        decisions.promotion_candidates += found.candidates;
        for one in found.promoted {
            decisions
                .promoted
                .push(
                    Promoted {
                        function: unit.id,
                        header,
                        pointers: one.pointers,
                        uses: one.uses,
                        store_safe: one.store_safe,
                    },
                );
        }
        let count = |inst: InstId, decisions: &mut Decisions| match unit.function.instruction(inst).opcode {
            llrm_mir::opcode::Opcode::Load { .. } => decisions.moved_loads += 1,
            llrm_mir::opcode::Opcode::Call(_) => decisions.moved_calls += 1,
            _ => {}
        };
        for (inst, how) in sunk {
            match how {
                sink::Sinking::Erased => decisions.erased.push(Hoisted { function: unit.id, header, inst }),
                sink::Sinking::Sunk => {
                    decisions.sunk.push(Hoisted { function: unit.id, header, inst });
                    count(inst, &mut decisions);
                }
            }
        }
        for (inst, _) in hoisted {
            decisions.hoisted.push(Hoisted { function: unit.id, header, inst });
            count(inst, &mut decisions);
        }
    }
    decisions
}

/// `runOnLoop` over every loop, inner loops first, the edits made as they are
/// decided. The memory graph is made again before the next loop whenever a
/// loop changed, which `MemorySSAUpdater` will replace (stage 3).
pub fn run(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
) -> Decisions {
    let shape = analyses.get::<Shape>(unit.context, unit.layout, unit.function);
    let mut decisions = Decisions::default();
    if shape.loops.is_empty() {
        return decisions;
    }
    let outer = std::rc::Rc::clone(analyses.outer());
    let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let innermost = innermost_loops(&shape);
    let no_exposed = llrm_mir::dense::IdSet::new();
    // Exposure of an alloca does not change as instructions move between
    // blocks: found once, for the alias answers.
    let exposed = {
        let memory = llrm_analysis::memory::Unit::within(unit.context, unit.layout, unit.function, &outer)
            .with_registers(&registers)
            .with_shape(&shape);
        std::rc::Rc::new(llrm_analysis::memory::exposed_frames(&memory).into_iter().collect())
    };
    let mut graph: Option<(Mssa, Accesses)> = None;
    for one in shape.loops.iter() {
        let header = cfg::block(one.header);
        let (found, preheader) = {
            let view: &passes::Unit = &*unit;
            let memory = llrm_analysis::memory::Unit::within(view.context, view.layout, view.function, &outer)
                .with_registers(&registers)
                .with_shape(&shape);
            if graph.is_none() {
                graph = Some(llrm_analysis::mssa::shadow::translated(&memory));
                decisions.rebuilds += 1;
                decisions.rebuilt_instructions += view.function.walk().count();
            }
            let (mssa, accesses) = graph.as_mut().expect("a graph");
            let aa = AliasQuery::new(&memory, accesses).with_exposed(std::rc::Rc::clone(&exposed));
            // The function stands as the earlier loops left it: nothing is
            // simulated.
            let state = State::default();
            let ctx = LoopCtx {
                function: view.function,
                shape: &shape,
                loop_: one,
                innermost: &innermost,
                state: &state,
                strict_sink: true,
            };
            let found = run_on_loop(view, &outer, &ctx, mssa, &aa, accesses, &no_exposed, false);
            let preheader = {
                use llrm_analysis::cfg::Around;
                one.preheader(view.function).map(cfg::block)
            };
            (found, preheader)
        };
        let function = &mut *unit.function;
        let changed = apply::apply_loop(function, one, preheader, &found.sunk, &found.hoisted)
            .unwrap_or_else(|why| panic!("licm: {why}"));
        for &(inst, how) in &found.sunk {
            match how {
                sink::Sinking::Erased => decisions.erased.push(Hoisted { function: unit.id, header, inst }),
                sink::Sinking::Sunk => decisions.sunk.push(Hoisted { function: unit.id, header, inst }),
            }
        }
        for &(inst, _) in &found.hoisted {
            decisions.hoisted.push(Hoisted { function: unit.id, header, inst });
        }
        if changed {
            graph = None;
        }
    }
    decisions
}

/// What `runOnLoop` decided for one loop.
struct LoopDecisions {
    sunk: Vec<(InstId, sink::Sinking)>,
    hoisted: Vec<(InstId, BlockId)>,
    candidates: usize,
    promoted: Vec<promote::Promotion>,
}

/// `runOnLoop`: the sinking, then the hoisting, then the promotion, as
/// decisions.
fn run_on_loop(
    unit: &passes::Unit,
    outer: &llrm_mir::passes::Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    accesses: &Accesses,
    exposed: &llrm_mir::dense::IdSet<llrm_mir::module::ValueId>,
    promote: bool,
) -> LoopDecisions {
    use llrm_analysis::cfg::Around;
    let preheader = ctx.loop_.preheader(unit.function).map(cfg::block);
    let mut flags = Flags::new(
        LICM_MSSA_OPT_CAP,
        LICM_MSSA_NO_ACC_FOR_PROMOTION_CAP,
        true,
        ctx.loop_.body.iter().map(|&at| cfg::block(at)),
        mssa,
    );
    let safety = LoopSafety::new(unit.context, unit.function, accesses, outer, ctx.loop_, ctx.shape);
    let dedicated = has_dedicated_exits(ctx);
    let sunk = if dedicated {
        sink::sink_region(unit, outer, ctx, mssa, aa, accesses, &mut flags, &safety)
    } else {
        Vec::new()
    };
    flags.set_is_sink(false);
    let hoisted = match preheader {
        Some(preheader) => hoist::hoist_region(unit, outer, ctx, preheader, mssa, aa, &mut flags, &safety),
        None => Vec::new(),
    };
    // Now that all loop invariants have been removed from the loop, promote any
    // memory references to scalars that we can.
    let mut candidates = 0;
    let mut promoted = Vec::new();
    if promote
        && let Some(preheader) = preheader
        && dedicated
        && !flags.too_many_memory_accesses()
    {
        let exits = promote::unique_exit_blocks(ctx);
        // Promoting one set of accesses may make the pointers for another set
        // loop invariant, so run this in a loop.
        loop {
            let mut local_promoted = false;
            for candidate in promote::collect_promotion_candidates(unit, ctx, mssa, aa, &safety) {
                candidates += 1;
                if let Some(found) = promote::promote_loop_accesses_to_scalars(
                    unit,
                    outer,
                    ctx,
                    &safety,
                    exposed,
                    &exits,
                    &candidate.pointers,
                    candidate.has_reads_outside_set,
                ) {
                    promote::take_out_of_loop(unit, ctx, preheader, &found);
                    promoted.push(found);
                    local_promoted = true;
                }
            }
            if !local_promoted {
                break;
            }
        }
    }
    LoopDecisions { sunk, hoisted, candidates, promoted }
}

/// `Loop::hasDedicatedExits`: every exit block has only predecessors in the
/// loop.
fn has_dedicated_exits(ctx: &LoopCtx) -> bool {
    ctx.loop_.body.iter().all(|&at| {
        ctx.function.successors(cfg::block(at)).into_iter().all(|exit| {
            ctx.contains_block(exit) || ctx.function.predecessors(exit).into_iter().all(|pred| ctx.contains_block(pred))
        })
    })
}

/// LICM. With `apply` false it moves nothing and counts what it would
/// (`DECIDED`); with it true it hoists, sinks and erases, rebuilding the memory
/// graph after each loop it changed until `MemorySSAUpdater` lands. Promotion
/// is decided in either, not yet applied.
pub struct Licm {
    pub apply: bool,
}

thread_local! {
    /// What the last runs decided, for the shadow driver.
    pub static DECIDED: std::cell::RefCell<Decisions> = std::cell::RefCell::new(Decisions::default());
}

impl FunctionPass for Licm {
    fn name(&self) -> &'static str {
        "licm"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let found = if self.apply { run(unit, analyses) } else { decide(unit, analyses) };
        let changed = found_changed(&found);
        DECIDED.with(|all| {
            let mut all = all.borrow_mut();
            all.hoisted.extend(found.hoisted);
            all.sunk.extend(found.sunk);
            all.erased.extend(found.erased);
            all.promotion_candidates += found.promotion_candidates;
            all.promoted.extend(found.promoted);
            all.moved_loads += found.moved_loads;
            all.moved_calls += found.moved_calls;
            all.rebuilds += found.rebuilds;
            all.rebuilt_instructions += found.rebuilt_instructions;
            all.max_rebuilds = all.max_rebuilds.max(found.rebuilds);
        });
        if self.apply && changed {
            // Instructions move between blocks; blocks and edges stand.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
