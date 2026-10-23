//! A transaction takes its edits back, last first.

use llrm_mir::module::Operand;
use llrm_mir::opcode::CastOp;

use super::Transaction;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-n8:16:32\"\n\n";

const TEXT: &str = "define i32 @f(i16 %a, i16 %b) {
b0:
  %s = add i16 %a, %b
  %e = sext i16 %s to i32
  %t = add i32 %e, 1
  ret i32 %t
}
";

fn module() -> llrm_mir::module::Module {
    parsed(&format!("{HEAD}{TEXT}"))
}

/// A `sext` made in a speculative promotion is erased again by the rollback,
/// and the function prints as it did.
#[test]
fn test_a_created_extension_is_erased_by_a_rollback() {
    let mut module = module();
    let before = printed(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let ret = function.walk().map(|(_, inst)| inst).last().expect("a return");
    let operand = function.instruction(ret).operands[0];
    let i16 = context.types.int(16);
    let mut tpt = Transaction::new();
    let point = tpt.restoration_point();
    let count = function.walk().count();
    let made = tpt.create_cast(function, CastOp::Trunc, operand, i16, ret);
    assert!(matches!(made, Operand::Value(_)));
    assert_eq!(function.walk().count(), count + 1, "the cast is there");
    tpt.rollback(function, point);
    assert_eq!(printed(&module), before);
}

/// An instruction removed with a replacement gives its uses to the replacement
/// and is erased at commit; a rollback before the commit puts the uses back.
#[test]
fn test_a_removed_instruction_returns_to_its_uses_on_a_rollback_and_goes_on_a_commit() {
    let mut module = module();
    let before = printed(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let ext = function.walk().map(|(_, inst)| inst).nth(1).expect("the sext");
    let source = function.instruction(ext).operands[0];
    let mut tpt = Transaction::new();
    let point = tpt.restoration_point();
    tpt.erase_instruction(function, context, ext, Some(source));
    let user = function.walk().map(|(_, inst)| inst).nth(2).expect("the add of the extension");
    assert_eq!(function.instruction(user).operands[0], source, "the uses read the replacement");
    tpt.rollback(function, point);
    assert_eq!(printed(&module), before, "rolled back");

    let (context, function) = module.function_mut("f").expect("@f");
    let ext = function.walk().map(|(_, inst)| inst).nth(1).expect("the sext");
    let source = function.instruction(ext).operands[0];
    let mut tpt = Transaction::new();
    tpt.erase_instruction(function, context, ext, Some(source));
    tpt.commit(function);
    assert_eq!(function.walk().count(), 3, "the extension is gone");
}

/// Types and operands mutate and come back.
#[test]
fn test_a_mutated_type_and_a_set_operand_come_back() {
    let mut module = module();
    let before = printed(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let add = function.walk().map(|(_, inst)| inst).next().expect("the add");
    let i32 = context.types.int(32);
    let mut tpt = Transaction::new();
    let point = tpt.restoration_point();
    tpt.mutate_type(function, add, i32);
    let a = function.instruction(add).operands[1];
    tpt.set_operand(function, add, 0, a);
    let (_, function) = module.function_mut("f").expect("@f");
    tpt.rollback(function, point);
    assert_eq!(printed(&module), before);
}
