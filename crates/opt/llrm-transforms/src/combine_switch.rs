//! InstCombine's `switch`: `InstCombinerImpl::visitSwitchInst`
//! ([InstructionCombining.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L4470)).
//!
//! # How it works
//! A condition computed by an invertible operation on a value (`x + c`,
//! `c - x`, `x ^ c`, `x << c`, an extension) is replaced by the value, the
//! case values mapped through the inverse. A condition whose high bits are
//! known, and are the same in every case, is truncated to the bits that vary.
//!
//! # Example
//! ```text
//! switch i32 %n, ...  [i32 5, ...]   ==>   switch i32 %x, ...  [i32 3, ...]
//! ; %n = add i32 %x, 2
//! ```
//!
//! MIRROR: the order of the folds and each guard.
//!
//! DEVIATE 1: `simplifySwitchOnSelectUsingRanges` is not translated. Exit
//! 2026-11-12.
//!
//! DEVIATE 2: `handlePotentiallyDeadSuccessors` for a constant or undef
//! condition is not here; the CFG simplifiers fold a switch on a constant.
//!
//! DEVIATE 3: conditions wider than 128 bits are left alone (case values are
//! `u128`).
//!
//! # Used by
//! `combine` (its dispatch on `Opcode::Switch`).

use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::{known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{query, replace_operand, should_change_type};

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// Sign-extends the low `bits` of `value`.
fn signed(
    value: u128,
    bits: u32,
) -> i128 {
    let shift = 128 - bits;
    ((value << shift) as i128) >> shift
}

impl Combiner<'_> {
    /// `visitSwitchInst`.
    pub(crate) fn visit_switch(
        &mut self,
        switch: InstId,
    ) -> Visit {
        let cond = self.function.instruction(switch).operands[0];
        let ty = self.type_of(cond);
        let Some(width) = self.context.types.int_bits(ty).filter(|&width| width <= 128) else { return Visit::None };
        let q = query(self, Some(switch));
        let cases: Option<Vec<u128>> = self.function.instruction(switch).operands[2..]
            .chunks(2)
            .map(|pair| q.int_constant(pair[0]).map(|(value, _)| value))
            .collect();
        let Some(cases) = cases else { return Visit::None };
        let one_use = self.has_one_use(cond);

        // switch (X + C) case K: -> switch (X) case K - C; likewise C - X, X ^
        // C.
        let invertible: Option<(Operand, Box<dyn Fn(u128) -> u128>)> = if let Some((_, _, x, c)) =
            pattern::binary(&q, cond, BinaryOp::Add)
            && let Some((c, _)) = q.int_constant(c)
        {
            Some((x, Box::new(move |case| case.wrapping_sub(c) & mask(width))))
        } else if let Some((_, _, c, x)) = pattern::binary(&q, cond, BinaryOp::Sub)
            && let Some((c, _)) = q.int_constant(c)
        {
            Some((x, Box::new(move |case| c.wrapping_sub(case) & mask(width))))
        } else if let Some((_, _, x, c)) = pattern::binary(&q, cond, BinaryOp::Xor)
            && let Some((c, _)) = q.int_constant(c)
            && c != 1u128 << (width - 1)
            && c != mask(width) >> 1
        {
            Some((x, Box::new(move |case| case ^ c)))
        } else {
            None
        };
        if let (Some((x, invert)), true) = (invertible, one_use) {
            let mapped: Vec<u128> = cases.iter().map(|&case| invert(case)).collect();
            self.set_cases(switch, ty, &mapped);
            replace_operand(self, switch, 0, x);
            return Visit::Changed;
        }

        // switch (X << 2) case 4: -> switch (X) case 1:
        if let Some((_, shl, x, amount)) = pattern::binary(&q, cond, BinaryOp::Shl)
            && let Some((amount, _)) = q.int_constant(amount)
            && amount < u128::from(width)
            && cases.iter().all(|&case| u128::from(case.trailing_zeros().min(width)) >= amount)
        {
            let (nuw, nsw) = (shl.flags.contains(Flags::NUW), shl.flags.contains(Flags::NSW));
            if nuw || nsw || one_use {
                let amount = amount as u32;
                let new_cond = if nuw || nsw {
                    x
                } else {
                    // The shift may wrap: mask off the shifted bits.
                    let keep = self.int(ty, mask(width - amount));
                    self.create_binop(switch, BinaryOp::And, x, keep, Flags::default())
                };
                let shifted: Vec<u128> = cases
                    .iter()
                    .map(
                        |&case| {
                            if nsw { (signed(case, width) >> amount) as u128 & mask(width) } else { case >> amount }
                        },
                    )
                    .collect();
                self.set_cases(switch, ty, &shifted);
                replace_operand(self, switch, 0, new_cond);
                return Visit::Changed;
            }
        }

        // switch (zext/sext X) -> switch (X) if the cases fit.
        if let Some((_, extension)) = q.instruction_of(cond)
            && let Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) = extension.opcode
        {
            let x = extension.operands[0];
            let narrow_ty = self.type_of(x);
            let narrow = self.context.types.int_bits(narrow_ty).expect("an integer");
            let fits = |case: u128| {
                if kind == CastOp::ZExt {
                    case & !mask(narrow) == 0
                } else {
                    signed(case, width) == signed(case & mask(narrow), narrow)
                }
            };
            if cases.iter().all(|&case| fits(case)) {
                let truncated: Vec<u128> = cases.iter().map(|&case| case & mask(narrow)).collect();
                self.set_cases(switch, narrow_ty, &truncated);
                replace_operand(self, switch, 0, x);
                return Visit::Changed;
            }
        }

        // Shrink the condition to the bits that differ.
        let known = known::compute_known_bits_at(&q, cond, Some(switch), 0);
        let (mut zeros, mut ones) = (known.count_min_leading_zeros(), known.count_min_leading_ones());
        for &case in &cases {
            zeros = zeros.min((case << (128 - width)).leading_zeros().min(width));
            ones = ones.min((!(case << (128 - width))).leading_zeros().min(width));
        }
        let new_width = width - zeros.max(ones);
        if new_width > 0 && new_width < width && should_change_type(self.env.layout, width, new_width) {
            let narrow_ty = self.context.types.int(new_width);
            let new_cond = self.create_cast(switch, CastOp::Trunc, cond, narrow_ty, Flags::default());
            let truncated: Vec<u128> = cases.iter().map(|&case| case & mask(new_width)).collect();
            self.set_cases(switch, narrow_ty, &truncated);
            replace_operand(self, switch, 0, new_cond);
            return Visit::Changed;
        }
        Visit::None
    }

    /// `Case.setValue` for every case: `values` as constants of `ty`.
    fn set_cases(
        &mut self,
        switch: InstId,
        ty: llrm_mir::types::TypeId,
        values: &[u128],
    ) {
        for (at, &value) in values.iter().enumerate() {
            let constant = self.int(ty, value);
            self.function.set_operand(switch, 2 + 2 * at, constant);
        }
    }
}

#[cfg(test)]
#[path = "combine_switch_tests.rs"]
mod tests;
