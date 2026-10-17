//! Loop-invariant code motion, translated from LLVM's `LICM.cpp`
//! ([`d1106deb71cc`](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266)).
//! L1: the hoisting decisions only; nothing is moved. The pipeline does not
//! call it.
//!
//! # See also
//! `docs/architecture/licm-port.md`; the MemorySSA it asks is
//! `llrm_analysis::mssa`.

mod calls;
mod hoist;
mod safety;
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
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
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

/// One loop, as `hoistRegion` sees it: its blocks, the loops in it, and where
/// the instructions the pass has hoisted stand.
pub struct LoopCtx<'a> {
    pub function: &'a Function,
    pub shape: &'a Shape,
    pub loop_: &'a Loop,
    /// Each block's innermost loop, an index in `shape.loops`.
    pub innermost: &'a IdMap<BlockId, usize>,
    /// Where each hoisted instruction stands now: the preheader it went to.
    pub moved: &'a IdMap<InstId, BlockId>,
    /// What this loop has hoisted so far.
    pub local: std::cell::RefCell<IdMap<InstId, BlockId>>,
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
        let block = self
            .local
            .borrow()
            .get(&inst)
            .copied()
            .or_else(|| self.moved.get(&inst).copied())
            .or_else(|| self.function.parent(inst));
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

/// What the pass decided over a function.
#[derive(Clone, Debug, Default)]
pub struct Decisions {
    pub hoisted: Vec<Hoisted>,
    /// `NumMovedLoads` and `NumMovedCalls`.
    pub moved_loads: usize,
    pub moved_calls: usize,
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
    let mut innermost: IdMap<BlockId, usize> = IdMap::new();
    // `shape.loops` is innermost first where loops nest: claim from the
    // outermost in, so the last to claim a block is its innermost.
    for (at, one) in shape.loops.iter().enumerate().rev() {
        for &block in &one.body {
            innermost.insert(cfg::block(block), at);
        }
    }
    let mut moved: IdMap<InstId, BlockId> = IdMap::new();
    for (at, one) in shape.loops.iter().enumerate() {
        let ctx = LoopCtx {
            function: unit.function,
            shape: &shape,
            loop_: one,
            innermost: &innermost,
            moved: &moved,
            local: Default::default(),
        };
        let found = run_on_loop(unit, &outer, &ctx, &mut mssa, &aa, &accesses);
        let _ = at;
        for (inst, preheader) in &found {
            decisions.hoisted.push(Hoisted { function: unit.id, header: cfg::block(one.header), inst: *inst });
            match unit.function.instruction(*inst).opcode {
                llrm_mir::opcode::Opcode::Load { .. } => decisions.moved_loads += 1,
                llrm_mir::opcode::Opcode::Call(_) => decisions.moved_calls += 1,
                _ => {}
            }
            let _ = preheader;
        }
        for (inst, preheader) in found {
            moved.insert(inst, preheader);
        }
    }
    decisions
}

/// `runOnLoop` for the hoisting: the instructions hoisted, each with the
/// preheader it goes to.
fn run_on_loop(
    unit: &passes::Unit,
    outer: &llrm_mir::passes::Outer,
    ctx: &LoopCtx,
    mssa: &mut Mssa,
    aa: &AliasQuery,
    accesses: &Accesses,
) -> Vec<(InstId, BlockId)> {
    use llrm_analysis::cfg::Around;
    let Some(preheader) = ctx.loop_.preheader(unit.function).map(cfg::block) else { return Vec::new() };
    let mut flags = Flags::new(
        LICM_MSSA_OPT_CAP,
        LICM_MSSA_NO_ACC_FOR_PROMOTION_CAP,
        false,
        ctx.loop_.body.iter().map(|&at| cfg::block(at)),
        mssa,
    );
    let safety = LoopSafety::new(unit.context, unit.function, accesses, outer, ctx.loop_, ctx.shape);
    hoist::hoist_region(unit, outer, ctx, preheader, mssa, aa, &mut flags, &safety)
}

/// The hoisting decisions as a pass that moves nothing: it counts what it
/// would hoist.
pub struct Licm;

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
        let found = decide(unit, analyses);
        DECIDED.with(|all| {
            let mut all = all.borrow_mut();
            all.hoisted.extend(found.hoisted);
            all.moved_loads += found.moved_loads;
            all.moved_calls += found.moved_calls;
        });
        PreservedAnalyses::all()
    }
}
