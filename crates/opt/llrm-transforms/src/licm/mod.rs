//! Loop-invariant code motion, translated from LLVM's `LICM.cpp`
//! ([`d1106deb71cc`](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266)).
//! L1: the hoisting decisions only; nothing is moved. The pipeline does not
//! call it.
//!
//! # See also
//! `docs/architecture/licm-port.md`; the MemorySSA it asks is
//! `llrm_analysis::mssa`.

pub(crate) mod apply;
pub(crate) mod calls;
pub mod hoist;
mod pressure;
pub(crate) mod promote;
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
use llrm_analysis::mssa::updater::MemorySsaUpdater;
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
    /// `LicmAllowSpeculation` of this run: false for LPM1's first LICM.
    pub allow_speculation: bool,
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
        self.innermost.get(&block).is_some_and(|&at| self.shape.loops[at].header != self.loop_.header)
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
    /// `NumBOAssociationsHoisted`.
    pub reassociated: Vec<Hoisted>,
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
    !found.hoisted.is_empty()
        || !found.reassociated.is_empty()
        || !found.sunk.is_empty()
        || !found.erased.is_empty()
        || !found.promoted.is_empty()
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
/// over every loop, inner loops first, as decisions: the function is not
/// changed, what a loop decided stands in for the edits to the loops after it.
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
    let exposed = promote::Exposed::default();
    for one in shape.loops.iter() {
        let ctx = LoopCtx {
            function: unit.function,
            shape: &shape,
            loop_: one,
            innermost: &innermost,
            state: &state,
            strict_sink: false,
            allow_speculation: true,
        };
        let header = cfg::block(one.header);
        let (sunk, hoisted, reassociated) = sink_and_hoist(unit, &outer, &ctx, &mut mssa, &aa, &accesses);
        record_edits(unit, &mut decisions, header, &sunk, &hoisted, &reassociated);
        // Promoting one set of accesses may make the pointers for another set
        // loop invariant, so run this in a loop.
        let mut room = None;
        loop {
            let (candidates, promoted) =
                promotion_round(unit, &outer, &ctx, &mssa, &aa, &accesses, &exposed, &mut room);
            decisions.promotion_candidates += candidates;
            let more = !promoted.is_empty();
            for one in promoted {
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
            if !more {
                break;
            }
        }
    }
    decisions
}

/// What the loop's decisions are, counted as LLVM's statistics are.
fn record_edits(
    unit: &passes::Unit,
    decisions: &mut Decisions,
    header: BlockId,
    sunk: &[(InstId, sink::Sinking)],
    hoisted: &[hoist::Hoist],
    reassociated: &[(usize, hoist::Reassociation)],
) {
    let count = |inst: InstId, decisions: &mut Decisions| match unit.function.instruction(inst).opcode {
        llrm_mir::opcode::Opcode::Load { .. } => decisions.moved_loads += 1,
        llrm_mir::opcode::Opcode::Call(_) => decisions.moved_calls += 1,
        _ => {}
    };
    for &(inst, how) in sunk {
        match how {
            sink::Sinking::Erased => decisions.erased.push(Hoisted { function: unit.id, header, inst }),
            sink::Sinking::Sunk | sink::Sinking::Cloned => {
                decisions.sunk.push(Hoisted { function: unit.id, header, inst });
                count(inst, decisions);
            }
        }
    }
    for &hoist::Hoist { inst, .. } in hoisted {
        decisions.hoisted.push(Hoisted { function: unit.id, header, inst });
        count(inst, decisions);
    }
    for (_, one) in reassociated {
        decisions.reassociated.push(Hoisted { function: unit.id, header, inst: one.inst() });
    }
}

/// `runOnLoop` over every loop, inner loops first, the edits made as they are
/// decided: what is dead erased, what sinks sunk, what hoists hoisted; then the
/// accesses promoted, a round at a time. The memory graph is made again
/// whenever a loop changed, which `MemorySSAUpdater` will replace (stage 3).
pub fn run(
    unit: &mut passes::Unit,
    analyses: &mut Analyses,
    allow_speculation: bool,
) -> Decisions {
    let shape = analyses.get::<Shape>(unit.context, unit.layout, unit.function);
    let mut decisions = Decisions::default();
    if shape.loops.is_empty() {
        return decisions;
    }
    let outer = std::rc::Rc::clone(analyses.outer());
    let registers = analyses.get::<llrm_analysis::manager::Registers>(unit.context, unit.layout, unit.function);
    let innermost = innermost_loops(&shape);
    // Found on the first ask, once.
    let exposed = promote::Exposed::default();
    // `LLRM_CHECK_LICM_GRAPH`: the graph the updater kept says what a graph
    // made afresh of the body says. (Phis left non-minimal in cycles of
    // irreducible control flow, as LLVM's updater leaves them, are not
    // compared.)
    let check_graph = |unit: &passes::Unit, built: &Built, what: &str| {
        if !llrm_support::env_flag!("LLRM_CHECK_LICM_GRAPH") || !shape.dominance.irreducible(unit.function).is_empty() {
            return;
        }
        let memory = llrm_analysis::memory::Unit::within(unit.context, unit.layout, unit.function, &outer)
            .with_registers(&registers)
            .with_shape(&shape);
        // The accesses the manager would make of the body now, as the kept
        // graph's were: a call's footprint from its callee's summary.
        let mut clean = Analyses::new(std::rc::Rc::clone(&outer));
        let fresh_accesses =
            llrm_analysis::memoryssa::Accesses::managed(unit.context, unit.layout, unit.function, &mut clean)
                .unwrap_or_else(|error| panic!("LLRM_CHECK_LICM_GRAPH after {what}: {error}"));
        let mut fresh = llrm_analysis::mssa::Mssa::build(
            unit.function,
            &shape.dominance,
            &llrm_analysis::mssa::shadow::effect_of(&fresh_accesses, unit.function),
        );
        // What the walker answers is what is compared: the kept graph's uses
        // read what the walker found, a fresh graph's what is nearest.
        let mut kept = built.mssa.clone();
        if let Err(why) = kept.verify_memory_ssa(unit.function, llrm_analysis::mssa::verify::Level::Full) {
            panic!("LLRM_CHECK_LICM_GRAPH after {what}: {why}");
        }
        let aa = AliasQuery::new(&memory, &fresh_accesses);
        let left = llrm_analysis::mssa::shadow::walked(&mut kept, &aa, unit.function);
        let right = llrm_analysis::mssa::shadow::walked(&mut fresh, &aa, unit.function);
        let differ: Vec<String> = left
            .iter()
            .filter(|(inst, name)| {
                right.get(inst).is_none_or(|now| !llrm_analysis::mssa::shadow::answer_is_safe(&fresh, name, now))
            })
            .map(|(inst, name)| format!("{inst:?}: kept {name:?}, fresh {:?}", right.get(&inst)))
            .collect();
        if !differ.is_empty() {
            panic!("LLRM_CHECK_LICM_GRAPH after {what}: the walker answers differ: {differ:?}");
        }
    };
    let mut graph: Option<Built> = None;
    // One look at the function as it stands: the loop's context, the graph
    // (made once, kept right by the updater), and what `body` decides.
    macro_rules! looking {
        ($one:expr, $strict:expr, |$view:ident, $ctx:ident, $mssa:ident, $aa:ident, $accesses:ident| $body:expr) => {{
            let $view: &passes::Unit = &*unit;
            let memory = llrm_analysis::memory::Unit::within($view.context, $view.layout, $view.function, &outer)
                .with_registers(&registers)
                .with_shape(&shape);
            if graph.is_none() {
                // The manager's graph of this body, cloned: the updater edits
                // the clone, and the manager drops its own with the pass's
                // change.
                let held =
                    analyses.get::<llrm_analysis::manager::MemorySsa>($view.context, $view.layout, $view.function);
                let made = match &*held {
                    Ok(held) => held
                        .accesses($view.context, $view.layout, $view.function, analyses)
                        .ok()
                        .map(|accesses| (held.mssa.clone(), (*accesses).clone())),
                    Err(_) => None,
                };
                let (mssa, accesses) = match made {
                    Some(made) => made,
                    None => {
                        let (mssa, accesses) = llrm_analysis::mssa::shadow::translated(&memory);
                        decisions.rebuilds += 1;
                        decisions.rebuilt_instructions += $view.function.walk().count();
                        (mssa, accesses)
                    }
                };
                graph = Some(Built { mssa, accesses, updater: MemorySsaUpdater::new() });
            }
            let built = graph.as_mut().expect("a graph");
            let ($mssa, $accesses) = (&mut built.mssa, &built.accesses);
            let $aa = AliasQuery::new(&memory, $accesses);
            // The function stands as the edits so far left it: nothing is
            // simulated.
            let state = State::default();
            let $ctx = LoopCtx {
                function: $view.function,
                shape: &shape,
                loop_: $one,
                innermost: &innermost,
                state: &state,
                strict_sink: $strict,
                allow_speculation,
            };
            $body
        }};
    }
    for one in shape.loops.iter() {
        let header = cfg::block(one.header);
        // No instruction to start from: nothing is asked of the graph.
        if !seeded(unit.function, &shape, &innermost, one) {
            continue;
        }
        let preheader = {
            use llrm_analysis::cfg::Around;
            one.preheader(unit.function).map(cfg::block)
        };
        let (sunk, hoisted, reassociated) = looking!(one, true, |view, ctx, mssa, aa, accesses| {
            let found = sink_and_hoist(view, &outer, &ctx, mssa, &aa, accesses);
            record_edits(view, &mut decisions, header, &found.0, &found.1, &found.2);
            found
        });
        {
            let built = graph.as_mut().expect("a graph");
            let mut kept = apply::Graph {
                mssa: &mut built.mssa,
                accesses: &mut built.accesses,
                updater: &mut built.updater,
                outer: &outer,
                layout: unit.layout,
                last_store: IdMap::new(),
            };
            apply::apply_loop(
                &mut *unit.function,
                &*unit.context,
                &mut kept,
                one,
                preheader,
                &sunk,
                &hoisted,
                &reassociated,
            )
            .unwrap_or_else(|why| panic!("licm: {why}"));
        }
        check_graph(
            unit,
            graph.as_ref().expect("a graph"),
            &format!(
                "the sinking and hoisting of the loop at {header:?}: {} sunk, {} hoisted, {} reassociated",
                sunk.len(),
                hoisted.len(),
                reassociated.len()
            ),
        );
        let Some(preheader) = preheader else { continue };
        // The stores of a promotion go before the first instruction of each
        // exit block that is not a phi, taken before any promotion: the
        // next ones follow them.
        let exits: Vec<(BlockId, InstId)> = {
            let view: &passes::Unit = &*unit;
            let ctx = LoopCtx {
                function: view.function,
                shape: &shape,
                loop_: one,
                innermost: &innermost,
                state: &State::default(),
                strict_sink: true,
                allow_speculation: true,
            };
            promote::unique_exit_blocks(&ctx)
                .into_iter()
                .filter_map(|exit| {
                    let first = view
                        .function
                        .iter_block(exit)
                        .find(|&one| !matches!(
                            view.function.instruction(one).opcode,
                            llrm_mir::opcode::Opcode::Phi
                        ))?;
                    Some((exit, first))
                })
                .collect()
        };
        let mut last_store: IdMap<BlockId, llrm_analysis::mssa::AccessId> = IdMap::new();
        let mut room = None;
        loop {
            let (candidates, promoted) = looking!(one, true, |view, ctx, mssa, aa, accesses| {
                promotion_round(view, &outer, &ctx, mssa, &aa, accesses, &exposed, &mut room)
            });
            decisions.promotion_candidates += candidates;
            if promoted.is_empty() {
                break;
            }
            let loop_of = |block: BlockId| innermost.get(&block).map(|&at| &shape.loops[at]);
            let site = apply::PromotionSite { preheader, exits: &exits, loop_of: &loop_of };
            for found in &promoted {
                let built = graph.as_mut().expect("a graph");
                let mut kept = apply::Graph {
                    mssa: &mut built.mssa,
                    accesses: &mut built.accesses,
                    updater: &mut built.updater,
                    outer: &outer,
                    layout: unit.layout,
                    last_store: std::mem::take(&mut last_store),
                };
                apply::promote(&mut *unit.function, &mut *unit.context, &mut kept, &site, found)
                    .unwrap_or_else(|why| panic!("licm: {why}"));
                last_store = kept.last_store;
                check_graph(
                    unit,
                    graph.as_ref().expect("a graph"),
                    &format!("promoting {:?} out of the loop at {header:?}", found.pointers),
                );
                decisions
                    .promoted
                    .push(
                        Promoted {
                            function: unit.id,
                            header,
                            pointers: found.pointers.clone(),
                            uses: found.uses.clone(),
                            store_safe: found.store_safe,
                        },
                    );
            }
        }
    }
    decisions
}

/// Whether the loop has an instruction any of the three could start from: one
/// that may be hoisted (every operand outside the loop), one read only outside
/// it or by no one (sunk or erased), a GEP also read outside it, an operation
/// on one of its own kind (reassociated), or a load or store through a pointer
/// the loop does not change (promoted). The cheap look LLVM's passes have in
/// the instructions they walk, ahead of the graph they would ask.
fn seeded(
    function: &Function,
    shape: &Shape,
    innermost: &IdMap<BlockId, usize>,
    loop_: &Loop,
) -> bool {
    let state = State::default();
    let ctx = LoopCtx { function, shape, loop_, innermost, state: &state, strict_sink: true, allow_speculation: true };
    ctx.loop_.body.iter().any(|&at| {
        let block = cfg::block(at);
        !ctx.in_sub_loop(block)
            && function.iter_block(block).any(|inst| {
                let instruction = function.instruction(inst);
                if instruction.opcode.is_terminator() || matches!(instruction.opcode, llrm_mir::opcode::Opcode::Phi) {
                    return false;
                }
                let invariant = ctx.has_loop_invariant_operands(inst);
                let outside_only = instruction
                    .result
                    .is_some_and(|result| function.users(result).iter().all(|one| !ctx.contains(one.user)));
                // A GEP read outside the loop may be copied there
                // (`FoldableInLoop`).
                let foldable = matches!(
                    instruction.opcode,
                    llrm_mir::opcode::Opcode::GetElementPtr { .. }
                )
                    && instruction
                        .result
                        .is_some_and(|result| function.users(result).iter().any(|one| !ctx.contains(one.user)));
                let reassociable = hoist::may_reassociate(function, inst);
                let access = match instruction.opcode {
                    llrm_mir::opcode::Opcode::Load { .. } => Some(instruction.operands[0]),
                    llrm_mir::opcode::Opcode::Store { .. } => Some(instruction.operands[1]),
                    _ => None,
                };
                invariant
                    || outside_only
                    || foldable
                    || reassociable
                    || access.is_some_and(|pointer| ctx.is_loop_invariant(pointer))
            })
    })
}

/// `runOnLoop` up to the promotion: the sinking, then the hoisting, as
/// decisions.
fn sink_and_hoist(
    unit: &passes::Unit,
    outer: &llrm_mir::passes::Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    accesses: &Accesses,
) -> (Vec<(InstId, sink::Sinking)>, Vec<hoist::Hoist>, Vec<(usize, hoist::Reassociation)>) {
    use llrm_analysis::cfg::Around;
    let preheader = ctx.loop_.preheader(unit.function).map(cfg::block);
    let mut flags = Flags::new(
        LICM_MSSA_OPT_CAP,
        LICM_MSSA_NO_ACC_FOR_PROMOTION_CAP,
        true,
        ctx.loop_.body.iter().map(|&at| cfg::block(at)),
        mssa,
    );
    let safety = LoopSafety::new(unit.context, unit.layout, unit.function, accesses, outer, ctx.loop_, ctx.shape)
        .allowing_speculation(ctx.allow_speculation);
    let sunk = if has_dedicated_exits(ctx) {
        sink::sink_region(unit, outer, ctx, mssa, aa, accesses, &mut flags, &safety)
    } else {
        Vec::new()
    };
    flags.set_is_sink(false);
    let (hoisted, reassociated) = match preheader {
        Some(preheader) => hoist::hoist_region(unit, outer, ctx, preheader, mssa, aa, &mut flags, &safety),
        None => (Vec::new(), Vec::new()),
    };
    (sunk, hoisted, reassociated)
}

/// One pass of the do-while of `runOnLoop`'s promotion: the candidates counted,
/// and the sets promoted, each taken out of the loop as the later decisions see
/// it.
fn promotion_round(
    unit: &passes::Unit,
    outer: &llrm_mir::passes::Outer,
    ctx: &LoopCtx,
    mssa: &Mssa,
    aa: &AliasQuery,
    accesses: &Accesses,
    exposed: &promote::Exposed,
    room: &mut Option<Option<usize>>,
) -> (usize, Vec<promote::Promotion>) {
    use llrm_analysis::cfg::Around;
    let Some(preheader) = ctx.loop_.preheader(unit.function).map(cfg::block) else { return (0, Vec::new()) };
    let flags = Flags::new(
        LICM_MSSA_OPT_CAP,
        LICM_MSSA_NO_ACC_FOR_PROMOTION_CAP,
        false,
        ctx.loop_.body.iter().map(|&at| cfg::block(at)),
        mssa,
    );
    // Don't sink stores from loops without dedicated block exits, or when the
    // loop has too many memory accesses.
    if !has_dedicated_exits(ctx) || flags.too_many_memory_accesses() {
        return (0, Vec::new());
    }
    let safety = LoopSafety::new(unit.context, unit.layout, unit.function, accesses, outer, ctx.loop_, ctx.shape)
        .allowing_speculation(ctx.allow_speculation);
    let exits = promote::unique_exit_blocks(ctx);
    let mut candidates = 0;
    let mut promoted = Vec::new();
    // Taken once for the loop: the rounds after a promotion see the same room.
    let room = room.get_or_insert_with(|| pressure::Pressure::of(unit, outer, ctx).map(|held| held.room()));
    let mut found_candidates = promote::collect_promotion_candidates(unit, ctx, mssa, aa, &safety);
    if room.is_some() {
        found_candidates
            .sort_by_key(|candidate| std::cmp::Reverse(promote::accesses_of(unit, ctx, &candidate.pointers)));
    }
    for candidate in found_candidates {
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
            if let Some(room) = room.as_mut()
                && pressure::counts_type(unit, found.access_ty)
            {
                if !promote::frees_addresses(unit, ctx, &candidate.pointers) {
                    if *room == 0 {
                        continue;
                    }
                    *room -= 1;
                }
            }
            promote::take_out_of_loop(unit, ctx, preheader, &found);
            promoted.push(found);
        }
    }
    (candidates, promoted)
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
/// The graph and what keeps it right, for the length of a function's run.
struct Built {
    mssa: Mssa,
    accesses: Accesses,
    updater: MemorySsaUpdater,
}

pub struct Licm {
    pub apply: bool,
    /// `LicmAllowSpeculation`: `licm<no-allowspeculation>` hoists only what
    /// the loop certainly runs.
    pub speculate: bool,
}

thread_local! {
    /// What the last runs decided, for the shadow driver.
    pub static DECIDED: std::cell::RefCell<Decisions> = std::cell::RefCell::new(Decisions::default());
}

impl FunctionPass for Licm {
    fn name(&self) -> &'static str {
        "licm"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("licm")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let found = if self.apply { run(unit, analyses, self.speculate) } else { decide(unit, analyses) };
        let changed = found_changed(&found);
        let only_moved = found.reassociated.is_empty()
            && found.sunk.is_empty()
            && found.erased.is_empty()
            && found.promoted.is_empty();
        if self.apply && llrm_support::env_flag!("LLRM_DEBUG_LICM") {
            eprintln!(
                "licm: {} rebuilds of {} instructions, {} hoisted, {} sunk, {} erased, {} promoted",
                found.rebuilds,
                found.rebuilt_instructions,
                found.hoisted.len(),
                found.sunk.len(),
                found.erased.len(),
                found.promoted.len()
            );
        }
        DECIDED.with(|all| {
            let mut all = all.borrow_mut();
            all.hoisted.extend(found.hoisted);
            all.reassociated.extend(found.reassociated);
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
            let kept = PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>();
            if only_moved {
                // Instructions moved between blocks and none made or erased:
                // what the memory accesses are annotated with and what each
                // call writes stand (rebuilding them was 81% of the cost of a
                // MemorySSA miss after a LICM edit).
                kept.preserve::<llrm_analysis::manager::Annotated>()
                    .preserve::<llrm_analysis::manager::Writes>()
                    .preserve::<llrm_analysis::manager::PlainAccesses>()
            } else {
                kept
            }
        } else {
            PreservedAnalyses::all()
        }
    }
}
