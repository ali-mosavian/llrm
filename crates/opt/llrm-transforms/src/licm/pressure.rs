//! DEVIATE LP1 (no number): a promotion that would hold one more value across
//! the loop than the target has registers is refused. Fact: LLVM's LICM
//! promotes every location it can (`promoteLoopAccessesToScalars`) and prices
//! no registers; on m16's six word registers nbody.bas promotes 17 globals out
//! of its loop nest, each a header phi the allocator then spills to the frame
//! (m16 -Os 1983 B on main, 2568 B with the promotions, 2096 B with this).
//! Exit: none while a spilled promoted value is not stored back where the
//! allocator wants it. A location the loop only stores to takes no room: its
//! value crosses no back edge, so the second pass that sinks the stores of
//! locations the first pass loaded is not charged for them again.
//! A target with no registers to spill into (wasm) states no limit
//! (`Tti::licm_register_limit`).
//!
//! # How it works
//! The loop's pressure is the invariant integer or pointer values its
//! instructions read that stand outside it, plus the phis of its header, plus
//! one register to work in. The room left under the register count the target
//! states (`Tti::number_of_gpr_registers`) is what promotions may take, one
//! each, the most-accessed locations first. Floating values are not counted
//! there: they live in the target's float file.
//!
//! DEVIATE LP1-FP (owner x86-m32, exit 2026-11-30): the same count over the
//! float file prices the hoisting of a float load
//! (`Tti::licm_float_register_limit`: the values a loop may hold across its
//! trips, the x87 stack's eight slots less the three an operation's two
//! operands and result take, as `MachineLICM`'s `CARRY_RESERVE`). LLVM's LICM
//! hoists every invariant load; GCC's `loop-invariant.cc` with
//! `-fira-loop-pressure` (-O3) moves an invariant only while its class has
//! registers left. x_ld_matmul -O3: twelve `A[i][k]` hoisted across the `j`
//! loop on an eight-slot stack cost ten `fstp` and ten `fxch` per iteration,
//! where reading the element in the loop is one `fld`.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Outer};
use llrm_mir::types::Type;

use super::LoopCtx;

#[cfg(test)]
thread_local! {
    pub static COUNTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The register file a held value takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Class {
    General,
    Float,
}

/// One loop's held values.
pub struct Pressure {
    limit: usize,
    /// What the loop needs besides what it holds: one register to work in on
    /// the general file; none on the float file, whose limit already leaves
    /// the operands' slots.
    work: usize,
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
        Self::of_class(unit, outer, ctx, Class::General)
    }

    /// [`Pressure::of`] over the values of one register file.
    pub fn of_class(
        unit: &passes::Unit,
        outer: &Outer,
        ctx: &LoopCtx,
        class: Class,
    ) -> Option<Self> {
        #[cfg(test)]
        COUNTED.with(|count| count.set(count.get() + 1));
        let tti = outer.target().tti()?;
        let limit = match class {
            Class::General => tti.licm_register_limit()?,
            Class::Float => tti.licm_float_register_limit()?,
        } as usize;
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
                    && counted(unit, value, class)
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
                if block == header && data.opcode == Opcode::Phi && data.result.is_some_and(|r| counted(unit, r, class))
                {
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
        Some(Self { limit, work: usize::from(class == Class::General), phis, held })
    }

    /// How many more values the loop can hold in registers: what the target
    /// has, less the invariants, the header phis and the one to work in.
    pub fn room(&self) -> usize {
        self.limit.saturating_sub(self.held.len() + self.phis + self.work)
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

/// Whether a value of type `ty` is one word or less: an integer of 16 bits at
/// most (and wider than a flag) or a pointer, which the target holds in one
/// register where a wider one is several and dear to compute.
pub fn is_one_register_value(
    unit: &passes::Unit,
    ty: llrm_mir::types::TypeId,
) -> bool {
    match unit.context.types.get(ty) {
        Type::Int(bits) => (2..=16).contains(bits),
        Type::Pointer(_) => true,
        _ => false,
    }
}

/// Whether a value of type `ty` takes a register of the float file.
pub fn counts_float_type(
    unit: &passes::Unit,
    ty: llrm_mir::types::TypeId,
) -> bool {
    matches!(unit.context.types.get(ty), Type::Float(_))
}

/// Whether `value` takes a register of `class`: a general one for an integer
/// wider than a flag or a pointer, a float one for a floating value.
fn counted(
    unit: &passes::Unit,
    value: ValueId,
    class: Class,
) -> bool {
    let ty = unit.function.value(value).ty;
    match class {
        Class::General => counts_type(unit, ty),
        Class::Float => counts_float_type(unit, ty),
    }
}
