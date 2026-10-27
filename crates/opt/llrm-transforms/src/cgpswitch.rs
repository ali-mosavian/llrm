//! CodeGenPrepare's `optimizeSwitchType`: a switch on an integer narrower than
//! the target's preferred width is a switch on the extended condition
//! ([CodeGenPrepare.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp)).
//!
//! # How it works
//! `switch i16 %x` with cases 1, 2, 3 becomes `%w = zext i16 %x to i32` and
//! `switch i32 %w` with cases 1, 2, 3 as `i32`, so the comparisons or the jump
//! table's range check are made on a full register and not on a 16-bit one
//! beside an extension of the index. The extension is `zext`, `sext` when the
//! condition is a `signext` argument; a `zeroext` argument is `zext`.
//!
//! MIRROR: the whole function.
//!
//! DEVIATE SW1: the target's preferred width is
//! `Lowering::preferred_switch_condition_bits`. Exit: none.
//!
//! # Example
//! `test_a_switch_on_an_i16_is_a_switch_on_its_zext_to_i32` in
//! `cgpswitch_tests.rs`.
//!
//! # Runs
//! In `cgpaddr`'s run.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

#[cfg(test)]
#[path = "cgpswitch_tests.rs"]
mod tests;

/// `optimizeSwitchType(SI)`: whether the condition was widened.
fn widened(
    context: &Context,
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
    switch: InstId,
) -> bool {
    let condition = function.instruction(switch).operands[0];
    if matches!(condition, Operand::Constant(_)) {
        return false;
    }
    let Some(old_ty) = function.operand_type(context, condition) else { return false };
    let Some(old_bits) = context.types.int_bits(old_ty) else { return false };
    let wide_bits = lowering.preferred_switch_condition_bits(old_bits);
    // DEVIATE SW2: a switch is not widened past the pointer: the 16-bit target
    // keeps its switch narrow (a 32-bit compare there costs an operand-size
    // prefix per case), as `is_desirable_to_promote_op` reads it. Exit: none.
    if wide_bits <= old_bits || wide_bits > layout.pointer(0).bits {
        return false;
    }
    let mut kind = CastOp::ZExt;
    if let Operand::Value(value) = condition
        && let ValueDef::Argument(at) = function.value(value).def
        && let Some(attrs) = function.parameter_attrs.get(at as usize)
    {
        if llrm_mir::memory::has(attrs, "signext") {
            kind = CastOp::SExt;
        }
        if llrm_mir::memory::has(attrs, "zeroext") {
            kind = CastOp::ZExt;
        }
    }
    let wide_ty = context.types.int(wide_bits);
    let cast = function.create_instruction(Opcode::Cast(kind), wide_ty, vec![condition], Flags::default(), None);
    function.insert(cast, Position::Before(switch)).expect("a position before the switch");
    let extended = Operand::Value(function.instruction(cast).result.expect("a cast has a value"));
    function.set_operand(switch, 0, extended);
    let narrow_mask = if old_bits >= 128 { u128::MAX } else { (1u128 << old_bits) - 1 };
    let wide_mask = if wide_bits >= 128 { u128::MAX } else { (1u128 << wide_bits) - 1 };
    for at in (2..function.instruction(switch).operands.len()).step_by(2) {
        let Operand::Constant(case) = function.instruction(switch).operands[at] else { continue };
        let ConstantKind::Int(bits) = context.get(case).kind else { continue };
        let negative = kind == CastOp::SExt && bits >> (old_bits - 1) & 1 == 1;
        let bits = (bits & narrow_mask) | if negative { wide_mask & !narrow_mask } else { 0 };
        let made = context.constant(Constant { ty: wide_ty, kind: ConstantKind::Int(bits) });
        function.set_operand(switch, at, Operand::Constant(made));
    }
    true
}

/// `optimizeSwitchType` for each switch of `function`; whether any was widened.
pub fn switch_types_widened(
    context: &Context,
    layout: &DataLayout,
    lowering: &dyn Lowering,
    function: &mut Function,
) -> bool {
    let switches: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Switch)
        .collect();
    let mut changed = false;
    for switch in switches {
        changed |= widened(context, layout, lowering, function, switch);
    }
    changed
}
