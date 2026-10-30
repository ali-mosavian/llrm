//! DEVIATE LP1 (no number): a promotion that would hold one more value across
//! the loop than the target has registers is refused. Fact: LLVM's LICM
//! promotes every location it can (`promoteLoopAccessesToScalars`) and prices
//! no registers; on m16's six word registers nbody.bas promotes 17 globals out
//! of its loop nest, each a header phi the allocator then spills to the frame
//! (m16 -Os 1983 B on main, 2568 B with the promotions, 2096 B with this).
//! Exit: none while a spilled promoted value is not stored back where the
//! allocator wants it.
//!
//! # How it works
//! The loop's pressure is the invariant integer or pointer values its
//! instructions read that stand outside it, plus the phis of its header, plus
//! one register to work in. The room left under the register count the target
//! states (`Tti::number_of_gpr_registers`) is what promotions may take, one
//! each, the most-accessed locations first. Floating values are not counted:
//! they live in the target's float file.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Outer};
use llrm_mir::types::Type;

use super::LoopCtx;

/// One loop's held values.
pub struct Pressure {
    limit: usize,
    phis: usize,
    /// The values defined outside the loop that it reads.
    held: IdSet<ValueId>,
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
        let mut reads = IdSet::<ValueId>::new();
        let mut order: Vec<ValueId> = Vec::new();
        let mut phis = 0;
        let header = llrm_analysis::cfg::block(ctx.loop_.header);
        let mut count = |inst: InstId| {
            for operand in &unit.function.instruction(inst).operands {
                if let Operand::Value(value) = *operand
                    && counted(unit, value)
                {
                    if reads.insert(value) {
                        order.push(value);
                    }
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
        let mut held = IdSet::<ValueId>::new();
        for value in order.into_iter().filter(|&value| ctx.is_loop_invariant(Operand::Value(value))) {
            held.insert(value);
        }
        Some(Self { limit, phis, held })
    }

    /// How many more values the loop can hold in registers: what the target
    /// has, less the invariants, the header phis and the one to work in.
    pub fn room(&self) -> usize {
        self.limit.saturating_sub(self.held.len() + self.phis + 1)
    }
}

/// Whether a value of type `ty` takes a general register.
pub fn counts_type(
    unit: &passes::Unit,
    ty: llrm_mir::types::TypeId,
) -> bool {
    match unit.context.types.get(ty) {
        Type::Int(bits) => *bits > 1,
        Type::Pointer(_) => true,
        _ => false,
    }
}

/// Whether `value` takes a general register: an integer wider than a flag, or
/// a pointer.
fn counted(
    unit: &passes::Unit,
    value: ValueId,
) -> bool {
    counts_type(unit, unit.function.value(value).ty)
}
