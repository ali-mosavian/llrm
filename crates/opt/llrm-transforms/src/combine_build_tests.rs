use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::combine::{Combiner, Visit, combine_with, combine_with_declared};
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

/// `Builder.CreateIntrinsic(i16, umax, {x, y})` is a call of `llvm.umax.i16`
/// with its declaration added to the module; a second one reads the same
/// declaration. Intrinsics could not be built before (the funnel-shift fold,
/// `abs` and the min/max folds waited on it).
#[test]
fn the_builder_declares_the_intrinsic_it_calls() {
    fn visit(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Sub) {
            return Visit::None;
        }
        let [x, y] = c.function.instruction(inst).operands[..] else { return Visit::None };
        let first = c.create_intrinsic(inst, "umax", c.type_of(x), &[x, y]);
        Visit::Replace(c.create_intrinsic(inst, "umax", c.type_of(x), &[first, y]))
    }
    let mut module = parsed("define i16 @f(i16 %x, i16 %y) {\nb0:\n  %s = sub i16 %x, %y\n  ret i16 %s\n}\n");
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    let out = printed(&module);
    assert!(out.contains("declare i16 @llvm.umax.i16(i16, i16)"), "{out}");
    assert_eq!(out.matches("call i16 @llvm.umax.i16").count(), 2, "{out}");
    assert_eq!(out.matches("declare").count(), 1, "{out}");
}

/// `Builder.SetInsertPoint(&I)` takes `I`'s debug location: the `shl` made for
/// a `mul x, 2` had none, so inlining gave it the call's line (nib's
/// `scale(counter)`, line 11, where the `return factor * 2` is line 6).
#[test]
fn an_instruction_the_builder_makes_has_the_debug_location_of_the_visited_one() {
    fn visit(
        c: &mut Combiner,
        inst: InstId,
    ) -> Visit {
        if c.function.instruction(inst).opcode != Opcode::Binary(BinaryOp::Sub) {
            return Visit::None;
        }
        let (x, y) = (c.function.instruction(inst).operands[0], c.function.instruction(inst).operands[1]);
        Visit::Replace(c.create_add(inst, x, y))
    }
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %s = sub i16 %x, %y, !dbg !5\n  ret i16 %s\n}\n!5 = !{i32 6}\n";
    let made = combined(text, visit);
    assert!(made.lines().any(|line| line.contains("= add i16 %x, %y") && line.contains(", !dbg !")), "{made}");
}
