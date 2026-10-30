//! Stores to one address on both arms of a diamond become one store below it,
//! as LLVM's `MergedLoadStoreMotion` (`mldst-motion<no-split-footer-bb>`).
//!
//! # How it works
//! A translation of LLVM
//! [`MergedLoadStoreMotion.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/MergedLoadStoreMotion.cpp)
//! at d1106deb71cc. A diamond head is a conditional branch whose two targets
//! each have the head as their only predecessor and the same single successor,
//! the tail. Walking the first arm from its end, a simple store with a store
//! on the second arm that must alias it, with nothing after either that may
//! throw, read or write the bytes, is paired with it. The first arm's store
//! moves to the top of the tail, taking a phi of the two values where they
//! differ; the second arm's goes. When the addresses are two identical
//! `getelementptr`s, each used only by its store and in its arm's block, the
//! address moves with the store. The walk then starts again from the end of the
//! first arm. At most 250 store-times-second-arm-size steps are tried
//! (`MagicCompileTimeControl`).
//!
//! # Example
//! ```text
//!  before                       after
//!  then: store i32 1, %p        then: ...
//!  else: store i32 2, %p        else: ...
//!  tail: ...                    tail: %v = phi [1, then], [2, else]
//!                                     store i32 %v, %p
//! ```
//!
//! The pipeline runs it without the footer split (`no-split-footer-bb`): a
//! tail with three predecessors or more is left alone.
//!
//! DEVIATE 1: `CastInst::isBitOrNoopPointerCastable` stands for "the two stored
//! types are the same type, or both pointers" (MIR has no `bitcast` to insert
//! for the other pairs).
//! DEVIATE 2: debug locations and `DIAssignID` are not carried; MIR keeps none.
//!
//! # Parity slot
//! `mldst-motion@783` in the simplify list between `sroa@776` and `gvn@787`
//! (PassBuilderPipelines.cpp#L783).
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.

use std::rc::Rc;

use llrm_analysis::memory::{MemRef, Unit as View};
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::alias::{AliasQuery, AliasResult};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};

/// `MagicCompileTimeControl`.
const COMPILE_TIME_CONTROL: usize = 250;

pub struct MergedLoadStoreMotion;

/// A store that is not volatile: `StoreInst::isSimple`.
fn is_simple_store(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Store { volatile: false, .. }
    )
}

/// `isDiamondHead`, and its tail: the arms in operand order.
fn diamond(
    function: &Function,
    head: BlockId,
) -> Option<(BlockId, BlockId, BlockId)> {
    let last = function.terminator(head)?;
    let instruction = function.instruction(last);
    let (Opcode::Br, [_, Operand::Block(first), Operand::Block(second)]) =
        (&instruction.opcode, &instruction.operands[..])
    else {
        return None;
    };
    let (first, second) = (*first, *second);
    if function.predecessors(first).len() != 1 || function.predecessors(second).len() != 1 {
        return None;
    }
    let single = |block: BlockId| {
        let successors = function.successors(block);
        (successors.len() == 1).then(|| successors[0])
    };
    let (tail, other) = (single(first)?, single(second)?);
    (tail == other).then_some((first, second, tail))
}

/// `Instruction::getNextNode`: the instruction after `inst` in its block.
fn after(
    function: &Function,
    inst: InstId,
) -> InstId {
    function.next_in_block(inst).expect("a store is followed by its block's terminator")
}

/// The store `inst` writes: its pointer and the bytes it stores.
fn written(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    inst: InstId,
) -> (Operand, u32) {
    let operands = &function.instruction(inst).operands;
    let ty = function.operand_type(context, operands[0]).expect("a typed stored value");
    (operands[1], layout.store_size(&context.types, ty) as u32)
}

/// What a run needs of the function as it stands; built again after each edit.
struct Query<'a> {
    context: &'a Context,
    layout: &'a DataLayout,
    function: &'a Function,
    outer: &'a Outer,
    aa: AliasQuery<'a>,
}

impl Query<'_> {
    /// `isStoreSinkBarrierInRange(Start, End, Loc)`: an instruction from
    /// `start` up to but not including `end` that may throw, or one from
    /// `start` through `end` that may read or write the bytes the store
    /// `of` writes.
    fn is_barrier(
        &self,
        start: InstId,
        end: InstId,
        of: InstId,
    ) -> bool {
        let mut walk = Some(start);
        while let Some(inst) = walk.filter(|&inst| inst != end) {
            if crate::licm::calls::may_throw(self.outer, self.context, self.function, inst) {
                return true;
            }
            walk = self.function.next_in_block(inst);
        }
        let (pointer, width) = written(self.context, self.layout, self.function, of);
        let location = self.aa.location_for(&MemRef::at(self.aa.unit(), pointer, width), false);
        let mut walk = Some(start);
        while let Some(inst) = walk {
            let found = self.aa.get_mod_ref_info(inst, &location);
            if found.reads || found.writes {
                return true;
            }
            if inst == end {
                break;
            }
            walk = self.function.next_in_block(inst);
        }
        false
    }

    /// `canSinkFromBlock(BB1, Store0)`.
    fn can_sink_from_block(
        &self,
        block: BlockId,
        store0: InstId,
    ) -> Option<InstId> {
        let function = self.function;
        let layout = self.layout;
        let block0 = function.parent(store0).expect("a placed store");
        let last0 = function.terminator(block0).expect("a terminated block");
        let last1 = function.terminator(block).expect("a terminated block");
        let (pointer0, width0) = written(self.context, layout, function, store0);
        let location0 = self.aa.location_for(&MemRef::at(self.aa.unit(), pointer0, width0), false);
        for store1 in function.iter_block_rev(block) {
            let Opcode::Store { .. } = function.instruction(store1).opcode else { continue };
            let (pointer1, width1) = written(self.context, layout, function, store1);
            let location1 = self.aa.location_for(&MemRef::at(self.aa.unit(), pointer1, width1), false);
            if self.aa.alias(&location0, &location1) == AliasResult::Must
                && !self.is_barrier(after(function, store1), last1, store1)
                && !self.is_barrier(after(function, store0), last0, store0)
                && function.instruction(store0).opcode == function.instruction(store1).opcode
                && self.castable(store0, store1)
            {
                return Some(store1);
            }
        }
        None
    }

    /// DEVIATE 1.
    fn castable(
        &self,
        store0: InstId,
        store1: InstId,
    ) -> bool {
        let value_type =
            |store: InstId| self.function.operand_type(self.context, self.function.instruction(store).operands[0]);
        let (a, b) = (value_type(store0), value_type(store1));
        a == b
    }
}

/// `canSinkStoresAndGEPs`.
fn can_sink_stores_and_geps(
    function: &Function,
    s0: InstId,
    s1: InstId,
) -> bool {
    let (p0, p1) = (function.instruction(s0).operands[1], function.instruction(s1).operands[1]);
    if p0 == p1 {
        return true;
    }
    let definition = |pointer: Operand, store: InstId| -> Option<InstId> {
        let Operand::Value(value) = pointer else { return None };
        let ValueDef::Instruction(gep) = function.value(value).def else { return None };
        let instruction = function.instruction(gep);
        (matches!(instruction.opcode, Opcode::GetElementPtr { .. })
            && function.users(value).has_one_use()
            && function.parent(gep) == function.parent(store))
        .then_some(gep)
    };
    let (Some(g0), Some(g1)) = (definition(p0, s0), definition(p1, s1)) else { return false };
    function.instruction(g0).is_identical_to_when_defined(function.instruction(g1))
}

/// The first instruction of `block` that is neither a phi nor a landing pad.
fn first_insertion_point(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .iter_block(block)
        .find(|&inst| !matches!(
            function.instruction(inst).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ))
        .expect("a terminated block")
}

/// `sinkStoresAndGEPs(BB, S0, S1)`: `s0` goes to the top of `tail`, with a phi
/// of the two values where they differ, and `s1` goes.
fn sink_stores_and_geps(
    unit: &mut Unit,
    tail: BlockId,
    s0: InstId,
    s1: InstId,
) {
    let function = &mut *unit.function;
    let (value0, value1) = (function.instruction(s0).operands[0], function.instruction(s1).operands[0]);
    let (gep0, gep1) = {
        let (p0, p1) = (function.instruction(s0).operands[1], function.instruction(s1).operands[1]);
        let owner = |pointer: Operand| match pointer {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(gep) => Some(gep),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        };
        if p0 == p1 { (None, None) } else { (owner(p0), owner(p1)) }
    };
    let (block0, block1) = (function.parent(s0).expect("a placed store"), function.parent(s1).expect("a placed store"));
    crate::local::and_ir_flags(unit.context, function, s0, s1);
    crate::local::combine_metadata_for_cse(unit.metadata, function, s0, s1, true);
    if value0 != value1 {
        let ty = function.operand_type(unit.context, value0).expect("a typed stored value");
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("sink"));
        let first = function.iter_block(tail).next().expect("a terminated block");
        function.insert(phi, Position::Before(first)).expect("a new phi");
        function.set_operands(phi, vec![value0, Operand::Block(block0), value1, Operand::Block(block1)]);
        let merged = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        function.set_operand(s0, 0, merged);
    }
    let at = first_insertion_point(function, tail);
    function.move_to(s0, Position::Before(at)).expect("a placed store");
    function.erase(s1).expect("a store nothing reads");
    if let (Some(g0), Some(g1)) = (gep0, gep1) {
        function.move_to(g0, Position::Before(s0)).expect("a placed address");
        function.erase(g1).expect("an address nothing reads");
    }
}

impl MergedLoadStoreMotion {
    /// `mergeStores(HeadBB)`.
    fn merge_stores(
        unit: &mut Unit,
        outer: &Rc<Outer>,
        head: BlockId,
    ) -> bool {
        let Some((pred0, pred1, tail)) = diamond(unit.function, head) else { return false };
        if pred0 == pred1 || unit.function.predecessors(tail).len() >= 3 {
            return false;
        }
        let size1 = unit.function.iter_block(pred1).count();
        let mut stores = 0usize;
        let mut merged = false;
        'search: loop {
            let candidates: Vec<InstId> =
                unit.function.iter_block_rev(pred0).filter(|&inst| is_simple_store(unit.function, inst)).collect();
            for s0 in candidates {
                stores += 1;
                if stores * size1 >= COMPILE_TIME_CONTROL {
                    break 'search;
                }
                let found = {
                    let accesses = Accesses::lazy();
                    let view = View::within(&*unit.context, unit.layout, &*unit.function, outer);
                    let query = Query {
                        context: &*unit.context,
                        layout: unit.layout,
                        function: &*unit.function,
                        outer,
                        aa: AliasQuery::new(&view, &accesses),
                    };
                    query.can_sink_from_block(pred1, s0)
                };
                let Some(s1) = found else { continue };
                if !can_sink_stores_and_geps(unit.function, s0, s1) {
                    break 'search;
                }
                merged = true;
                sink_stores_and_geps(unit, tail, s0, s1);
                continue 'search;
            }
            break;
        }
        merged
    }
}

impl FunctionPass for MergedLoadStoreMotion {
    fn name(&self) -> &'static str {
        "mldst-motion"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("mldst-motion")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = Rc::clone(analyses.outer());
        let mut changed = false;
        for head in unit.function.layout().to_vec() {
            changed |= Self::merge_stores(unit, &outer, head);
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "mldstmotion_tests.rs"]
mod tests;
