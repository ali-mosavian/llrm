//! A float converted to an unsigned 64-bit integer, as signed conversions and a select, before instruction selection:
//! the x87 stores only signed qwords. LLVM's legalizer (`expandFP_TO_UINT`) and GCC's `fixuns` expansion do the same: where
//! `x < 2^63` the conversion is `fptosi x`; else it is `fptosi (x - 2^63)` with the top bit set. The select is the target's
//! general one (`selects`: a branch on a 486, which has no conditional move), so there is one answer for select.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::module::{Function, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, Opcode};
use llrm_mir::types::{FloatKind, Type};
use llrm_mir::edit::Position;

/// Every `fptoui` to a 64-bit integer in `module` made signed conversions.
pub fn expanded(module: &mut Module) -> Result<(), String> {
    let context = &mut module.context;
    for global in &mut module.globals {
        let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind else { continue };
        let found: Vec<InstId> = function.walk().map(|(_, inst)| inst).filter(|&inst| unsigned_wide(context, function, inst)).collect();
        for inst in found {
            expand(context, function, inst)?;
        }
    }
    Ok(())
}

fn unsigned_wide(context: &Context, function: &Function, inst: InstId) -> bool {
    let instruction = function.instruction(inst);
    instruction.opcode == Opcode::Cast(CastOp::FPToUI) && context.types.int_bits(instruction.ty) == Some(64)
}

/// `inst`, `fptoui x to i64`, as `fptosi (x - (x < 2^63 ? 0 : 2^63))` xor `(x < 2^63 ? 0 : 1 << 63)`.
fn expand(context: &mut Context, function: &mut Function, inst: InstId) -> Result<(), String> {
    let instruction = function.instruction(inst).clone();
    let (x, wide, result) = (instruction.operands[0], instruction.ty, instruction.result.ok_or("a conversion's value")?);
    let float = function.operand_type(context, x).ok_or("a typed operand")?;
    let Type::Float(kind) = context.types.get(float).clone() else { return Err("a float operand".into()) };
    let bit = context.types.int(1);
    let bits = |value: f64| match kind {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        _ => value.to_bits(),
    };
    let mut constant = |value: f64| Operand::Constant(context.constant(Constant { ty: float, kind: ConstantKind::Float(bits(value)) }));
    let (zero, limit) = (constant(0.0), constant(9_223_372_036_854_775_808.0));
    let integer = |context: &mut Context, value: i128| Operand::Constant(context.int(wide, value));
    let (none, top) = (integer(context, 0), integer(context, i128::from(i64::MIN)));
    let mut before = |function: &mut Function, opcode, ty, operands: Vec<Operand>| -> Result<Operand, String> {
        let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(made, Position::Before(inst))?;
        Ok(Operand::Value(function.instruction(made).result.expect("a value")))
    };
    let below = before(function, Opcode::FCmp(FloatPredicate::Olt), bit, vec![x, limit])?;
    let adjust = before(function, Opcode::Select, float, vec![below, zero, limit])?;
    let rest = before(function, Opcode::Binary(BinaryOp::FSub), float, vec![x, adjust])?;
    let stored = before(function, Opcode::Cast(CastOp::FPToSI), wide, vec![rest])?;
    let high = before(function, Opcode::Select, wide, vec![below, none, top])?;
    let answer = before(function, Opcode::Binary(BinaryOp::Xor), wide, vec![stored, high])?;
    function.replace_all_uses_with(result, answer);
    function.erase(inst)
}

#[cfg(test)]
mod tests {
    use llrm_mir::interpret::{Val, run};

    /// A double to an unsigned qword below, at and past 2^63: the same answers as the program's, and no `fptoui` left.
    #[test]
    fn test_a_float_to_unsigned_qword_is_signed_conversions_and_a_select() {
        let text = "define i64 @f(double %d) {
entry:
  %u = fptoui double %d to i64
  ret i64 %u
}
";
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        super::expanded(&mut after).unwrap();
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        assert!(!llrm_mir::print::module(&after).contains("fptoui"));
        for value in [0.0, 1.5, 4294967296.0, 9.223372036854775e18, 9223372036854775808.0, 9223372036854777856.0, 18446744073709549568.0] {
            let argument = vec![Val::Float(llrm_mir::types::FloatKind::Double, f64::to_bits(value))];
            assert_eq!(run(&after, "f", argument.clone(), 100).unwrap(), run(&before, "f", argument, 100).unwrap(), "{value}");
        }
    }
}
