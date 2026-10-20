//! A `switch` over powers of two made dense: LLVM's
//! [`simplifySwitchOfPowersOfTwo`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8129),
//! a step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).
//!
//! # How it works
//! With four or more cases, all powers of two, dense once reduced to their
//! exponents (`isSwitchDense`), and a `cttz` the target prices at 2 or less
//! (`TCC_Basic * 2`, size and latency), the switch is on `cttz(x)` and each
//! case is its exponent. A reachable default is protected by a branch on
//! `ctpop(x) == 1` to the default, so a value that is not a power of two
//! never reaches the `cttz`.
//!
//! # Example
//! ```text
//!   switch i32 %x [ 1, 2, 64, 128 ]    %n = call i32 @llvm.cttz.i32(i32 %x, i1 true)
//!                                  ==> switch i32 %n [ 0, 1, 6, 7 ]
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | No branch weights, no `DomTreeUpdater`. |
//! | D2 | The block split off for a reachable default is placed last, not after the switch's block. |
//!
//! # Used by
//! `switchopt::simplified`.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{Attribute, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::tti::{CostKind, Ty};

use crate::simplifycfg::Options;
use crate::switchopt::intrinsic_call;
use crate::switchreduce::is_dense;
use crate::twoentryphi::Speculation;

/// `TCC_Basic * 2`: the most a `cttz` may cost.
const MAX_CTTZ_COST: i64 = 2;

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    _options: &Options,
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(switch) = function.terminator(block) else { return false };
    let operands = function.instruction(switch).operands.clone();
    let cond = operands[0];
    let ty = function.operand_type(context, cond).expect("a switch value");
    let width = context.types.int_bits(ty).expect("an integer switch");
    if width > 64 || width > spec.layout.largest_legal_integer() {
        return false;
    }

    // The trailing zeros count must not be too expensive.
    let cost = spec
        .tti
        .intrinsic_cost(
            &format!("llvm.cttz.i{width}"),
            Some(Ty::Int(width)),
            &[Ty::Int(width), Ty::Int(1)],
            CostKind::SizeAndLatency,
        );
    if !cost.valid || cost.value > MAX_CTTZ_COST {
        return false;
    }

    // SDAG starts emitting jump tables for 4 or more cases.
    let cases = (operands.len() - 2) / 2;
    if cases < 4 {
        return false;
    }

    // The cases are powers of two.
    let mut values: Vec<u64> = Vec::with_capacity(cases);
    for pair in operands[2..].chunks_exact(2) {
        let Operand::Constant(id) = pair[0] else { unreachable!("a case value is constant") };
        let ConstantKind::Int(bits) = context.get(id).kind else { return false };
        if !(bits as u64).is_power_of_two() {
            return false;
        }
        values.push(bits as u64);
    }
    values.sort_unstable();
    let opt_size = function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "optsize"));
    let exponents = u64::from(values[values.len() - 1].trailing_zeros() - values[0].trailing_zeros()) + 1;
    if !is_dense(cases as u64, exponents, opt_size) {
        return false;
    }

    let Operand::Block(default) = operands[1] else { unreachable!("a default") };
    if !crate::simplifycfg::only_unreachable(function, default) {
        // A value that is not a power of two goes to the default.
        let i1 = context.types.int(1);
        let count = intrinsic_call(context, declared, function, switch, "ctpop", ty, vec![cond]);
        let one = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(1) }));
        let compare =
            function.create_instruction(Opcode::ICmp(IntPredicate::Eq), i1, vec![count, one], Flags::default(), None);
        function.insert(compare, Position::Before(switch)).expect("a placed terminator");
        let is_pow2 = Operand::Value(function.instruction(compare).result.expect("a compare's value"));

        let split = crate::edges::split_block_at(function, block, switch);
        let jump = function.terminator(block).expect("the jump the split made");
        let void = function.instruction(jump).ty;
        let branch = function.create_instruction(
            Opcode::Br,
            void,
            vec![is_pow2, Operand::Block(split), Operand::Block(default)],
            Flags::default(),
            None,
        );
        function.insert(branch, Position::Before(jump)).expect("a placed terminator");
        function.erase(jump).expect("a terminator defines nothing");
        crate::edges::predecessor_added(function, default, block, split);
    }

    // Each case is its exponent, the condition its trailing zeros count.
    let mut now = operands;
    for pair in now[2..].chunks_exact_mut(2) {
        let Operand::Constant(id) = pair[0] else { unreachable!("a case value is constant") };
        let ConstantKind::Int(bits) = context.get(id).kind else { unreachable!("an integer case") };
        let exponent = u128::from(bits.trailing_zeros());
        pair[0] = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(exponent) }));
    }
    let i1 = context.types.int(1);
    let yes = Operand::Constant(context.constant(Constant { ty: i1, kind: ConstantKind::Int(1) }));
    now[0] = intrinsic_call(context, declared, function, switch, "cttz", ty, vec![cond, yes]);
    function.set_operands(switch, now);
    true
}

#[cfg(test)]
#[path = "switchpow2_tests.rs"]
mod tests;
