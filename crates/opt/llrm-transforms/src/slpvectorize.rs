//! `SLPVectorizer`'s store chains: consecutive stores of one scalar type
//! become vector stores when the cost model says the vector is cheaper.
//!
//! # How it works
//! `vectorizeStores` (l.29748): the stores of a block are grouped by the
//! object they address and the type they store, each group sorted by offset,
//! and cut into runs of adjacent cells. Over a run the vectorization factor
//! falls from the widest power of two the vector register and the run hold to
//! the target's `getStoreMinimumVF`; at each factor the run is tried in
//! windows of that many stores, a window that vectorizes is not tried again.
//! A window is a tree of the stores and the vector of their values; its cost
//! is the vector store less the scalar ones, the values' own (a vector of
//! constants costs nothing), and it vectorizes when that is below the
//! `slp-threshold` of 0 (`getTreeCost` l.19795, rejection l.19079). The
//! vector store stands at the last store of the window, with the address and
//! alignment of the first.
//!
//! DEVIATE SLP1 (owner w12, exit 2026-11-24): the values of a window are
//! constants only. The tree of loads and arithmetic (`buildTreeRec`
//! l.12052, `getEntryCost` l.16168), gathers and scheduling beyond "nothing
//! between the first and last store touches memory" are the next steps of
//! the series.
//!
//! # Runs
//! At the pin's `slp-vectorizer` slot (`PassBuilderPipelines.cpp` l.1455).
//!
//! # Used by
//! `pipeline` (Late).

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::tti::{CostKind, MemoryOp, Tti, Ty, VectorTy};
use llrm_mir::types::{FloatKind, Type, TypeId};

/// `-slp-threshold`: a tree is taken when its cost is below minus this.
const COST_THRESHOLD: i64 = 0;

/// One store of a chain.
#[derive(Clone)]
struct Seed {
    store: InstId,
    /// The store's place in its block.
    order: usize,
    disp: i64,
    align: Option<u64>,
    value: Operand,
}

/// The scalar `ty` as the cost hooks name it.
fn lane_of(
    context: &Context,
    ty: TypeId,
) -> Option<(Ty, u64)> {
    match context.types.get(ty) {
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some((Ty::Int(*bits), u64::from(*bits / 8))),
        Type::Float(FloatKind::Float) => Some((Ty::Float, 4)),
        Type::Float(FloatKind::Double) => Some((Ty::Double, 8)),
        _ => None,
    }
}

/// Stores grouped by the object and type they write, each group in offset
/// order with the runs of adjacent cells cut apart (`collectSeedInstructions`
/// l.30007 and the chain sort of `vectorizeStoreChains` l.36033).
fn chains(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
) -> Vec<(TypeId, Vec<Seed>)> {
    let mut groups: Vec<(Operand, TypeId, Vec<Seed>)> = Vec::new();
    for (order, inst) in function.iter_block(block).enumerate() {
        let instruction = function.instruction(inst);
        let Opcode::Store { align, volatile: false } = instruction.opcode else { continue };
        let value = instruction.operands[0];
        let Some(ty) = function.operand_type(unit.context, value) else { continue };
        let Some((_, bytes)) = lane_of(unit.context, ty) else { continue };
        let reference = MemRef::at(unit, instruction.operands[1], bytes as u32);
        let Some(root) = reference.root.filter(|_| reference.base.is_none() && reference.segment.is_none()) else {
            continue;
        };
        let seed = Seed { store: inst, order, disp: reference.disp, align, value };
        match groups.iter_mut().find(|group| group.0 == root && group.1 == ty) {
            Some(group) => group.2.push(seed),
            None => groups.push((root, ty, vec![seed])),
        }
    }
    let mut runs = Vec::new();
    for (_, ty, mut seeds) in groups {
        let bytes = lane_of(unit.context, ty).expect("a lane type").1 as i64;
        seeds.sort_by_key(|seed| seed.disp);
        seeds.dedup_by_key(|seed| seed.disp);
        let mut run: Vec<Seed> = Vec::new();
        for seed in seeds {
            if run.last().is_some_and(|last| seed.disp != last.disp + bytes) {
                runs.push((ty, std::mem::take(&mut run)));
            }
            run.push(seed);
        }
        runs.push((ty, run));
    }
    runs
}

/// A window of `vf` stores is one vector store: its cost below the threshold.
fn worthwhile(
    tti: &dyn Tti,
    lane: Ty,
    align: u64,
    window: &[Seed],
    context: &Context,
) -> bool {
    let vf = window.len() as u32;
    let kind = CostKind::RecipThroughput;
    // `getEntryCost` of the store entry (l.17322): the vector store less the
    // scalar ones.
    let Some(vector) = tti.vector_memory_op_cost(MemoryOp::Store, VectorTy { lane, count: vf }, align, kind) else {
        return false;
    };
    let scalar = tti.memory_op_cost(MemoryOp::Store, lane, kind, true);
    if !vector.valid || !scalar.valid {
        return false;
    }
    // The values: a vector of constants is a constant (`getGatherCost`,
    // l.21526).
    let constants = window
        .iter()
        .all(
            |seed| matches!(
                seed.value,
                Operand::Constant(id) if is_scalar_constant(context, id)
            ),
        );
    constants && vector.value - scalar.value * i64::from(vf) < -COST_THRESHOLD
}

fn is_scalar_constant(
    context: &Context,
    id: llrm_mir::context::ConstantId,
) -> bool {
    matches!(
        context.get(id).kind,
        ConstantKind::Int(_) | ConstantKind::Float(_) | ConstantKind::Zero | ConstantKind::Null
    )
}

/// Nothing between the window's first and last store reads or writes memory
/// but the window's own stores: the vector store may stand at the last.
fn movable(
    function: &Function,
    block: BlockId,
    window: &[Seed],
) -> bool {
    let first = window.iter().map(|seed| seed.order).min().expect("a window");
    let last = window.iter().map(|seed| seed.order).max().expect("a window");
    function
        .iter_block(block)
        .enumerate()
        .skip(first)
        .take(last - first + 1)
        .all(
            |(order, inst)| {
                let own = window.iter().any(|seed| seed.order == order);
                own || !matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { .. }
                        | Opcode::Store { .. }
                        | Opcode::Call(_)
                        | Opcode::Invoke(_)
                        | Opcode::Atomic(_)
                        | Opcode::VaArg
                )
            },
        )
}

/// The vector store in place of `window`'s stores.
fn vectorize(
    context: &mut Context,
    function: &mut Function,
    ty: TypeId,
    window: &[Seed],
) {
    let vf = window.len();
    let vector_ty = context.types.intern(Type::Vector { element: ty, count: vf as u32 });
    let first = &window[0];
    let last = window.iter().max_by_key(|seed| seed.order).expect("a window");
    let pointer = function.instruction(first.store).operands[1];
    let Operand::Constant(lane) = first.value else { unreachable!("a window of constants") };
    let kind = match context.get(lane).kind.clone() {
        ConstantKind::Int(0) | ConstantKind::Float(0) | ConstantKind::Zero => ConstantKind::Zero,
        _ if window.iter().all(|seed| seed.value == first.value) => ConstantKind::Aggregate(vec![lane; vf]),
        _ => ConstantKind::Aggregate(
            window
                .iter()
                .map(|seed| match seed.value {
                    Operand::Constant(one) => one,
                    _ => unreachable!("a window of constants"),
                })
                .collect(),
        ),
    };
    let value = Operand::Constant(context.constant(Constant { ty: vector_ty, kind }));
    let void = context.types.void();
    let store = function.create_instruction(
        Opcode::Store { align: first.align, volatile: false },
        void,
        vec![value, pointer],
        Flags::default(),
        None,
    );
    function.insert(store, Position::After(last.store)).expect("the stores' block");
    for seed in window {
        function.erase(seed.store).expect("a store");
    }
}

/// Store chains to vector stores in `function`; whether any was made.
fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut plan: Vec<(TypeId, Vec<Seed>)> = Vec::new();
    {
        let unit = MemUnit::within(context, layout, function, outer);
        for block in function.layout().to_vec() {
            for (ty, run) in chains(&unit, function, block) {
                let (lane, bytes) = lane_of(unit.context, ty).expect("a lane type");
                let widest = (u64::from(tti.vector_register_bits()) / (bytes * 8)).max(1);
                let mut vf = (widest.min(run.len() as u64)).next_power_of_two();
                if vf > widest.min(run.len() as u64) {
                    vf /= 2;
                }
                let floor = tti.store_minimum_vf(2, lane, lane).max(2);
                let mut taken = vec![false; run.len()];
                while vf >= u64::from(floor).max(2) {
                    let mut at = 0;
                    while at + vf as usize <= run.len() {
                        let window = &run[at..at + vf as usize];
                        let free = !taken[at..at + vf as usize].iter().any(|&one| one);
                        let align = window[0].align.unwrap_or(bytes);
                        if free
                            && worthwhile(tti, lane, align, window, unit.context)
                            && movable(function, block, window)
                        {
                            taken[at..at + vf as usize].fill(true);
                            plan.push((ty, window.to_vec()));
                            at += vf as usize;
                        } else {
                            at += 1;
                        }
                    }
                    vf /= 2;
                }
            }
        }
    }
    for (ty, window) in &plan {
        vectorize(context, function, *ty, window);
    }
    !plan.is_empty()
}

/// Store chains to vector stores, on a target with vector registers.
pub struct SlpVectorize;

impl FunctionPass for SlpVectorize {
    fn name(&self) -> &'static str {
        "slpvectorize"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("slp-vectorizer")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        if tti.vector_register_bits() == 0 || !outer.target().has_vector_mir() {
            return PreservedAnalyses::all();
        }
        let chains = run_function(unit.context, unit.layout, unit.function, &outer, &*tti);
        // The pin's one pass also makes the wide compare of an `and` chain.
        let compares = crate::slpcompares::SlpCompares.run(unit, analyses);
        if chains { PreservedAnalyses::none() } else { compares }
    }
}

#[cfg(test)]
#[path = "slpvectorize_tests.rs"]
mod tests;
