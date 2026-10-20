use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `@f` with `visit` run on every `sub`.
fn combined(
    text: &str,
    visit: fn(&mut Combiner, InstId) -> Visit,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// `Builder.CreateAdd(x, 0)` is `x`, found by `simplifyAddInst`: no
/// instruction is made.
#[test]
fn the_builder_returns_the_value_simplify_finds() {
    fn visit(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Sub) {
            return Visit::None;
        }
        let x = c.function.instruction(inst).operands[0];
        let zero = c.int(c.type_of(x), 0);
        Visit::Replace(c.create_add(inst, x, zero))
    }
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %s = sub i16 %x, %y\n  ret i16 %s\n}\n";
    assert_eq!(combined(text, visit), "define i16 @f(i16 %x, i16 %y) {\nb0:\n  ret i16 %x\n}\n");
}

/// `Builder.CreateAdd(x, y)` makes an `add` before the visited instruction and
/// queues it: the `sub` then reads it.
#[test]
fn the_builder_inserts_before_the_visited_instruction() {
    fn visit(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Sub) {
            return Visit::None;
        }
        let [x, y] = c.function.instruction(inst).operands[..] else { return Visit::None };
        let sum = c.create_add(inst, x, y);
        Visit::Replace(sum)
    }
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %s = sub i16 %x, %y\n  ret i16 %s\n}\n";
    assert_eq!(combined(text, visit), "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %0 = add i16 %x, %y\n  ret i16 %0\n}\n");
}

/// `ConstantInt::get` wraps its value to the width, and `getAllOnesValue` is
/// all ones at the width.
#[test]
fn constants_wrap_to_the_width() {
    fn visit(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Sub) {
            return Visit::None;
        }
        let ty = c.type_of(Operand::Value(c.function.instruction(inst).result.unwrap()));
        let wrapped = c.int(ty, 0x1_0005);
        let ones = c.all_ones(ty);
        let sum = c.create_add(inst, wrapped, ones);
        Visit::Replace(sum)
    }
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %s = sub i16 %x, %y\n  ret i16 %s\n}\n";
    assert_eq!(combined(text, visit), "define i16 @f(i16 %x, i16 %y) {\nb0:\n  ret i16 4\n}\n");
}
