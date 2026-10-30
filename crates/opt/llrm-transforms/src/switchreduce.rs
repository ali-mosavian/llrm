//! A sparse `switch` made dense: LLVM's
//! [`reduceSwitchRange`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7925),
//! a step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
//! with `isSwitchDense`
//! ([L7367](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7367))
//! and `getDenseSwitchRangeReductionShift`
//! ([L7390](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7390)).
//!
//! # How it works
//! With four or more cases on a legal integer of at most 64 bits, and the
//! sorted case values not already dense (`isSwitchDense`: 10% of the range
//! used, 40% at `optsize`), find a base and a shift that make them dense:
//! base 0 if dense at the optsize threshold, else the smallest case.
//!
//! # Example
//! ```text
//!   switch i32 %x [ 5, 9, 13, 17 ]    %s = sub i32 %x, 5
//!                                 ==> %r = call i32 @llvm.fshl.i32(i32 %s, i32 %s, i32 30)
//!                                     switch i32 %r [ 0, 1, 2, 3 ]
//! ```
//!
//! A rotate right by the shift puts the bits shifted off in the top, so a
//! value that did not divide cleanly is large and takes the default.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights (the pin keeps the switch's own). |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_mir::context::{Constant, ConstantKind, Context, signed};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{Attribute, Flags, Opcode};
use llrm_mir::passes::Declared;

use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// `isSwitchDense(NumCases, CaseRange, OptSize)`.
pub(crate) fn is_dense(
    cases: u64,
    range: u64,
    opt_size: bool,
) -> bool {
    // The density of a jump table: 40% for size, 10% otherwise.
    let min_density: u64 = if opt_size { 40 } else { 10 };
    if range >= u64::MAX / 100 {
        return false;
    }
    cases * 100 >= range * min_density
}

/// `isSwitchDense(Values, OptSize)` of sorted `values`.
fn values_dense(
    values: &[i64],
    opt_size: bool,
) -> bool {
    let diff = (values[values.len() - 1] as u64).wrapping_sub(values[0] as u64);
    let Some(range) = diff.checked_add(1) else { return false };
    is_dense(values.len() as u64, range, opt_size)
}

/// `getDenseSwitchRangeReductionShift`: the shift that makes the sorted
/// `values`, less `base`, dense.
fn reduction_shift(
    values: &[i64],
    base: i64,
    opt_size: bool,
) -> Option<u32> {
    if values.iter().any(|&v| v < base) {
        return None;
    }
    let reduced: Vec<u64> = values.iter().map(|&v| (v as u64).wrapping_sub(base as u64)).collect();
    let shift = reduced.iter().fold(0, |or, &v| or | v).trailing_zeros();
    // The values are distinct and more than one, so some bit is set.
    debug_assert!(shift < 64);
    let shifted: Vec<i64> = reduced.iter().map(|&v| (v >> shift) as i64).collect();
    values_dense(&shifted, opt_size).then_some(shift)
}

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let operands = function.instruction(last).operands.clone();
    let cond = operands[0];
    let ty = function.operand_type(context, cond).expect("a switch value");
    let width = context.types.int_bits(ty).expect("a switch on an integer");
    // `fitsInLegalInteger`.
    if width > 64 || width > spec.layout.largest_legal_integer() {
        return false;
    }
    // SDAG only makes jump tables of 4 or more cases.
    let cases = (operands.len() - 2) / 2;
    if cases < 4 {
        return false;
    }

    // Case values as signed, so a run crossing zero is dense.
    let mut values: Vec<i64> = operands[2..]
        .chunks_exact(2)
        .map(|case| {
            let Operand::Constant(id) = case[0] else { unreachable!("a case value is constant") };
            let ConstantKind::Int(bits) = context.get(id).kind else { unreachable!("an integer case") };
            signed(bits, width) as i64
        })
        .collect();
    values.sort_unstable();

    let opt_size = function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "optsize"));
    if values_dense(&values, opt_size) {
        return false;
    }

    // Prefer Base=0 when it is dense even at the optsize threshold: it saves
    // the subtract.
    let (base, shift) = match reduction_shift(&values, 0, true) {
        Some(shift) => (0, shift),
        None if values[0] != 0 => match reduction_shift(&values, values[0], opt_size) {
            Some(shift) => (values[0], shift),
            None => return false,
        },
        None => return false,
    };

    let int = |value: u128| Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }));
    let mask = llrm_mir::context::mask(width);
    let at = |function: &mut Function, opcode, operands| {
        let new = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(new, Position::Before(last)).expect("a placed terminator");
        Operand::Value(function.instruction(new).result.expect("a value"))
    };
    let mut sub = cond;
    if base != 0 {
        let amount = int(base as u128 & mask);
        sub = at(function, Opcode::Binary(llrm_mir::opcode::BinaryOp::Sub), vec![cond, amount]);
    }
    let rotate = int(u128::from(width - shift));
    let rot = crate::switchopt::intrinsic_call(context, declared, function, last, "fshl", ty, vec![sub, sub, rotate]);

    let mut now = operands;
    now[0] = rot;
    for case in now[2..].chunks_exact_mut(2) {
        let Operand::Constant(id) = case[0] else { unreachable!("a case value is constant") };
        let ConstantKind::Int(bits) = context.get(id).kind else { unreachable!("an integer case") };
        let reduced = bits.wrapping_sub(base as u128 & mask) & mask;
        case[0] = int(reduced >> shift);
    }
    function.set_operands(last, now);
    true
}

#[cfg(test)]
#[path = "switchreduce_tests.rs"]
mod tests;
