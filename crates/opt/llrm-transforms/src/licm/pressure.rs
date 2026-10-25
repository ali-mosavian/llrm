//! DEVIATE LP1 (no number): a hoist that would hold one more value across the
//! loop than the target has registers is refused. Fact: LLVM's LICM prices no
//! registers (`LICM.cpp`) and leaves the cost to the allocator and MachineLICM;
//! on m16's six word registers `qb/hoist_pressure` keeps 34 stores to the frame
//! (-Os, #2449), and seven of the eleven objects #2449 changed at m16 -Os grew
//! (x_transpose +385, queens +133, x_bubble +111). Exit: none while a spilled
//! invariant is not made again where the allocator wants it.
//!
//! # How it works
//! The loop's pressure is the invariant values its instructions read that
//! stand outside it (the hoisted ones included), plus the phis of its header
//! (carried across the trips), plus one register to work in. A hoist adds its
//! result to the invariants if the loop reads it, and takes away each operand
//! no other instruction of the loop reads. It is refused only when it adds more
//! than it takes away and the sum then passes the register count the target
//! states (`Tti::number_of_gpr_registers`). A hoist that holds as many values
//! or fewer is never refused. Floating values are not counted: they live in the
//! target's float file.

use std::collections::{HashMap, HashSet};

use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Outer};
use llrm_mir::types::Type;

use super::LoopCtx;

/// One loop's held values, kept as hoists are decided.
pub struct Pressure {
    limit: usize,
    phis: usize,
    /// How many operands of the loop's instructions name each integer or
    /// pointer value.
    reads: HashMap<ValueId, u32>,
    /// The values defined outside the loop that it reads.
    held: HashSet<ValueId>,
}

impl Pressure {
    /// The loop's pressure; none where the target states no register count.
    pub fn of(
        unit: &passes::Unit,
        outer: &Outer,
        ctx: &LoopCtx,
    ) -> Option<Self> {
        let limit = outer.target().tti()?.number_of_gpr_registers() as usize;
        if limit == 0 {
            return None;
        }
        let mut reads: HashMap<ValueId, u32> = HashMap::new();
        let mut phis = 0;
        let header = llrm_analysis::cfg::block(ctx.loop_.header);
        let mut count = |inst: InstId| {
            for operand in &unit.function.instruction(inst).operands {
                if let Operand::Value(value) = *operand
                    && counted(unit, value)
                {
                    *reads.entry(value).or_default() += 1;
                }
            }
        };
        for &at in ctx.loop_.body.iter() {
            let block = llrm_analysis::cfg::block(at);
            for inst in unit.function.iter_block(block) {
                if ctx.state.gone.borrow().contains(&inst) || ctx.state.moved.borrow().contains_key(&inst) {
                    continue;
                }
                let data = unit.function.instruction(inst);
                if block == header && data.opcode == Opcode::Phi && data.result.is_some_and(|r| counted(unit, r)) {
                    phis += 1;
                }
                count(inst);
            }
        }
        // What earlier loops put in a block of this one: their preheaders.
        let moved: Vec<(InstId, _)> = ctx.state.moved.borrow().iter().map(|(inst, to)| (inst, *to)).collect();
        for (inst, to) in moved {
            if ctx.contains_block(to) && !ctx.state.gone.borrow().contains(&inst) {
                count(inst);
            }
        }
        let held: HashSet<ValueId> =
            reads.keys().copied().filter(|&value| ctx.is_loop_invariant(Operand::Value(value))).collect();
        Some(Self { limit, phis, reads, held })
    }

    /// Whether hoisting `inst` keeps the loop within the registers, or holds no
    /// more than before.
    pub fn admits(
        &self,
        unit: &passes::Unit,
        inst: InstId,
    ) -> bool {
        let (adds, takes) = self.change(unit, inst);
        adds <= takes || self.held.len() + self.phis + 1 + adds - takes <= self.limit
    }

    /// `inst` is hoisted: its result is held (if the loop reads it) and the
    /// operands no one else reads are not.
    pub fn took(
        &mut self,
        unit: &passes::Unit,
        inst: InstId,
    ) {
        let data = unit.function.instruction(inst);
        for operand in &data.operands {
            let Operand::Value(value) = *operand else { continue };
            if let Some(left) = self.reads.get_mut(&value) {
                *left -= 1;
                if *left == 0 {
                    self.reads.remove(&value);
                    self.held.remove(&value);
                }
            }
        }
        if let Some(result) = data.result
            && self.reads.contains_key(&result)
        {
            self.held.insert(result);
        }
    }

    /// What hoisting `inst` adds to the held values and what it takes away.
    fn change(
        &self,
        unit: &passes::Unit,
        inst: InstId,
    ) -> (usize, usize) {
        let data = unit.function.instruction(inst);
        let adds = usize::from(data.result.is_some_and(|result| self.reads.contains_key(&result)));
        let mut seen: Vec<ValueId> = Vec::new();
        let mut takes = 0;
        for operand in &data.operands {
            let Operand::Value(value) = *operand else { continue };
            if seen.contains(&value) {
                continue;
            }
            seen.push(value);
            let named = data.operands.iter().filter(|other| **other == *operand).count() as u32;
            if self.held.contains(&value) && self.reads.get(&value) == Some(&named) {
                takes += 1;
            }
        }
        (adds, takes)
    }
}

/// Whether `value` takes a general register: an integer wider than a flag, or
/// a pointer.
fn counted(
    unit: &passes::Unit,
    value: ValueId,
) -> bool {
    match unit.context.types.get(unit.function.value(value).ty) {
        Type::Int(bits) => *bits > 1,
        Type::Pointer(_) => true,
        _ => false,
    }
}
