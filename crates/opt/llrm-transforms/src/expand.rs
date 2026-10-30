//! Values placed from their sums (LLVM's `SCEVExpander`): a `Scev` is built
//! from its terms, each scaled and added once, before one instruction.
//!
//! # How it works
//! An `Expander` places everything before the instruction it was made for,
//! normally the branch ending a loop's preheader, and remembers what it made,
//! by sum and type, so the same sum or monomial is built once.
//!
//! `int` builds a sum of terms `factor * (v1 * v2 * ...)` plus a constant. Each
//! monomial's values are multiplied (a value wider than the sum is truncated
//! first, one narrower is zero-extended: only an extended recurrence's start
//! names one, after its proof); the product is scaled by the factor's magnitude
//! (`scaled`: nothing for 1, `shl` for a power of two, else `mul`); positive
//! terms are added, negative ones subtracted afterwards, then the constant is
//! added. Where no term is variable the result is the constant.
//!
//! ```text
//!  sum = 4*a*b + 3*c - 7
//!  %ab  = mul a, b
//!  %t1  = shl %ab, 2
//!  %t2  = mul c, 3
//!  %s   = add %t1, %t2
//!  %r   = sub %s, 7        (one build; a repeat of the sum reads %r)
//! ```
//!
//! `value` is `pointer + sum` as an `i8` GEP (or the integer sum where there is
//! no pointer). `least` is the least of several sums as unsigned numbers, an
//! `icmp ult` and a `select` per further sum.
//!
//! # Example
//! None: `expand` is infrastructure and has no pass-level before and after of
//! its own. Its `test_generated_recurrences_expand_to_what_the_loop_computes`
//! in `expand_tests.rs` checks expanded values against the interpreter; the
//! passes that use it show it at work, see [`crate::lsr_pass`].
//!
//! # Used by
//! [`crate::lsr_pass`], while it rewrites a loop, and
//! [`crate::counting`], which builds its seeds with it. Not a pass: it runs
//! only inside those.
//!
//! # See also
//! [`crate::counting`].

use llrm_analysis::induction::{Monomial, Scev};
use llrm_analysis::num::Num as BigInt;
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;
use llrm_support::hash::HashMap;

use crate::counting;

/// `value`, a `width`-bit pattern, as a signed number.
pub fn signed(
    value: &BigInt,
    width: u32,
) -> BigInt {
    let modulus = BigInt::from(1) << width;
    if value >= &(&modulus >> 1) { value - modulus } else { value.clone() }
}

/// Builds invariants before the preheader's branch, each once.
pub struct Expander {
    at: InstId,
    made: HashMap<(Option<Operand>, Scev, TypeId), Operand>,
}

impl Expander {
    /// Placing each value before `at`.
    pub fn new(at: InstId) -> Self {
        Self { at, made: HashMap::default() }
    }

    /// `sum` as an integer of its width.
    pub fn int(
        &mut self,
        context: &Context,
        function: &mut Function,
        sum: &Scev,
    ) -> Operand {
        sum_of(context, function, sum, Position::Before(self.at), &mut self.made)
    }

    /// `pointer + sum`, of type `ty`, or the integer sum where no pointer.
    pub fn value(
        &mut self,
        context: &Context,
        function: &mut Function,
        pointer: Option<Operand>,
        sum: &Scev,
        ty: TypeId,
    ) -> Operand {
        let Some(pointer) = pointer else { return self.int(context, function, sum) };
        if sum.is_zero() {
            return pointer;
        }
        let key = (Some(pointer), sum.clone(), ty);
        if let Some(&made) = self.made.get(&key) {
            return made;
        }
        let offset = self.int(context, function, sum);
        let i8 = context.types.int(8);
        let made = placed(
            context,
            function,
            Opcode::GetElementPtr { source: i8 },
            ty,
            vec![pointer, offset],
            Position::Before(self.at),
        );
        self.made.insert(key, made);
        made
    }

    /// The least of `sums` as unsigned numbers, each of their one width.
    pub fn least(
        &mut self,
        context: &Context,
        function: &mut Function,
        sums: &[Scev],
    ) -> Operand {
        let at = Position::Before(self.at);
        let mut least = self.int(context, function, &sums[0]);
        for sum in &sums[1..] {
            let other = self.int(context, function, sum);
            let ty = context.types.int(sum.width);
            let bit = context.types.int(1);
            let below = placed(context, function, Opcode::ICmp(IntPredicate::Ult), bit, vec![least, other], at);
            least = placed(context, function, Opcode::Select, ty, vec![below, least, other], at);
        }
        least
    }
}

/// `sum` placed at `at`, reusing what `made` holds.
fn sum_of(
    context: &Context,
    function: &mut Function,
    sum: &Scev,
    at: Position,
    made: &mut HashMap<(Option<Operand>, Scev, TypeId), Operand>,
) -> Operand {
    let ty = context.types.int(sum.width);
    if let Some(&one) = made.get(&(None, sum.clone(), ty)) {
        return one;
    }
    let mut total: Option<Operand> = None;
    let mut negative = Vec::new();
    for (product, factor) in &sum.terms {
        let factor = signed(factor, sum.width);
        let magnitude = BigInt::from(factor.magnitude().clone());
        let alone = (None, Scev::monomial(product.clone(), magnitude.clone(), sum.width), ty);
        let scaled = match made.get(&alone) {
            Some(&one) => one,
            None => {
                let one = product_of(context, function, product, sum.width, at, made);
                let one = scaled(context, function, one, &magnitude, sum.width, at);
                made.insert(alone, one);
                one
            }
        };
        if factor < BigInt::from(0) {
            negative.push(scaled);
        } else {
            total = Some(match total {
                Some(total) => placed(context, function, Opcode::Binary(BinaryOp::Add), ty, vec![total, scaled], at),
                None => scaled,
            });
        }
    }
    for one in negative {
        let from = total.unwrap_or_else(|| counting::constant(context, &BigInt::from(0), sum.width));
        total = Some(placed(context, function, Opcode::Binary(BinaryOp::Sub), ty, vec![from, one], at));
    }
    let constant = signed(&sum.constant, sum.width);
    let result = match total {
        None => counting::constant(context, &constant, sum.width),
        Some(total) if constant == BigInt::from(0) => total,
        Some(total) => {
            let constant = counting::constant(context, &constant, sum.width);
            placed(context, function, Opcode::Binary(BinaryOp::Add), ty, vec![total, constant], at)
        }
    };
    made.insert((None, sum.clone(), ty), result);
    result
}

/// The unknowns of `product` multiplied once, a product of invariants in
/// the block `at` is in: each factor is its value's low bits, or its value
/// zero-extended where the value is narrower than the product.
fn product_of(
    context: &Context,
    function: &mut Function,
    product: &Monomial,
    width: u32,
    at: Position,
    made: &mut HashMap<(Option<Operand>, Scev, TypeId), Operand>,
) -> Operand {
    let ty = context.types.int(width);
    let key = (None, Scev::monomial(product.clone(), BigInt::from(1), width), ty);
    if let Some(&one) = made.get(&key) {
        return one;
    }
    let mut made_product = None;
    for &value in product.values() {
        let bits = context.types.int_bits(function.value(value).ty).unwrap_or(width);
        let factor = if bits > width {
            placed(context, function, Opcode::Cast(CastOp::Trunc), ty, vec![Operand::Value(value)], at)
        } else if bits < width {
            // A narrower factor is read zero-extended: only `extended` builds
            // one, after proving its value never leaves `[0, 2^bits)`.
            placed(context, function, Opcode::Cast(CastOp::ZExt), ty, vec![Operand::Value(value)], at)
        } else {
            Operand::Value(value)
        };
        made_product = Some(match made_product {
            Some(so_far) => placed(context, function, Opcode::Binary(BinaryOp::Mul), ty, vec![so_far, factor], at),
            None => factor,
        });
    }
    let made_product = made_product.expect("a product has a factor");
    made.insert(key, made_product);
    made_product
}

/// `value * k`, `k` positive: itself, a shift or a multiply.
pub fn scaled(
    context: &Context,
    function: &mut Function,
    value: Operand,
    k: &BigInt,
    width: u32,
    at: Position,
) -> Operand {
    let ty = context.types.int(width);
    if *k == BigInt::from(1) {
        return value;
    }
    if (k & (k - 1)) == BigInt::from(0) {
        let shift = counting::constant(context, &BigInt::from(k.bits() - 1), width);
        return placed(context, function, Opcode::Binary(BinaryOp::Shl), ty, vec![value, shift], at);
    }
    let by = counting::constant(context, k, width);
    placed(context, function, Opcode::Binary(BinaryOp::Mul), ty, vec![value, by], at)
}

pub fn placed(
    context: &Context,
    function: &mut Function,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    at: Position,
) -> Operand {
    let _ = context;
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, at).expect("a placed position");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

#[cfg(test)]
#[path = "expand_tests.rs"]
mod tests;
