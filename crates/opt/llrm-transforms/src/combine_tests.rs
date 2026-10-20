use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Module, Operand};

use super::combine;
use crate::testing::{parsed, printed};

/// `@f` combined; whether anything changed.
fn combined(module: &mut Module) -> bool {
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine(context, &globals, &metadata, &DataLayout::default(), function)
}

fn check(
    text: &str,
    expected: &str,
) {
    let mut module = parsed(text);
    let changed = combined(&mut module);
    assert_eq!(printed(&module), expected);
    assert_eq!(changed, text != expected);
}

/// What simplifies away takes its users with it, in one visit each: `x + 0` is
/// `x`, so `(x + 0) - x` is `x - x`, which is `0`, so the return reads a
/// number.
#[test]
fn a_chain_of_identities_folds_in_one_drain() {
    check(
        "define i16 @f(i16 %x) {\nb0:\n  %a = add i16 %x, 0\n  %b = sub i16 %a, %x\n  %c = add i16 %b, 5\n  ret i16 %c\n}\n",
        "define i16 @f(i16 %x) {\nb0:\n  ret i16 5\n}\n",
    );
}

/// A branch on a number leaves one live successor: the phi input over the dead
/// edge is poison, so the phi has one value.
#[test]
fn a_constant_branch_makes_the_dead_phi_input_poison_and_the_phi_its_other() {
    check(
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 true, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %r = phi i16 [ %x, %a ], [ %y, %b ]\n  ret i16 %r\n}\n",
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 true, label %a, label %b\n\na:\n  br label %j\n\nb:\n  br label %j\n\nj:\n  ret i16 %x\n}\n",
    );
}

/// The dead are deleted as chains: nothing reads the last, so each before it
/// goes.
#[test]
fn what_only_a_dead_instruction_read_goes_with_it() {
    check(
        "define i16 @f(i16 %x) {\nb0:\n  %a = mul i16 %x, 3\n  %b = add i16 %a, 7\n  ret i16 %x\n}\n",
        "define i16 @f(i16 %x) {\nb0:\n  ret i16 %x\n}\n",
    );
}

/// A body with nothing to do is left as it is.
#[test]
fn a_body_with_nothing_to_simplify_is_unchanged() {
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %a = add i16 %x, %y\n  ret i16 %a\n}\n";
    check(text, text);
}

/// `mul x, 2` becomes `add x, x` through `insert_before` (the new instruction
/// is queued) and `Visit::Replace`; its users are visited again, so the
/// `sub` of it with itself folds to 0 in the same drain.
#[test]
fn a_visit_that_builds_a_replacement_has_it_and_the_users_revisited() {
    use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

    use super::visit as simplified;
    use super::{Combiner, Visit, combine_with};

    fn visit(
        c: &mut Combiner,
        inst: llrm_mir::module::InstId,
    ) -> Visit {
        let instruction = c.function.instruction(inst).clone();
        if instruction.opcode != Opcode::Binary(BinaryOp::Mul)
            || instruction.operands[1] != Operand::Constant(c.context.int(instruction.ty, 2))
        {
            return simplified(c, inst);
        }
        let x = instruction.operands[0];
        let new = c.insert_before(inst, Opcode::Binary(BinaryOp::Add), instruction.ty, vec![x, x], Flags::default());
        Visit::Replace(Operand::Value(c.function.instruction(new).result.expect("a value")))
    }

    let mut module =
        parsed("define i16 @f(i16 %x) {\nb0:\n  %a = mul i16 %x, 2\n  %c = sub i16 %a, %a\n  ret i16 %c\n}\n");
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit));
    assert_eq!(printed(&module), "define i16 @f(i16 %x) {\nb0:\n  ret i16 0\n}\n");
}

/// A visit that erases the instruction itself and returns `Erased` was a
/// change: the driver reported none while the store was gone.
#[test]
fn a_visit_that_erases_its_instruction_is_a_change() {
    use llrm_mir::opcode::Opcode;

    use super::{Combiner, Visit, combine_with};

    fn visit(
        c: &mut Combiner,
        inst: llrm_mir::module::InstId,
    ) -> Visit {
        if matches!(c.function.instruction(inst).opcode, Opcode::Store { .. }) {
            c.erase(inst);
            return Visit::Erased;
        }
        Visit::None
    }

    let mut module = parsed("define void @f(ptr %p) {\nb0:\n  store i16 1, ptr %p\n  ret void\n}\n");
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit));
    assert_eq!(printed(&module), "define void @f(ptr %p) {\nb0:\n  ret void\n}\n");
}

/// `(x + -5) * 3 + 7` (`sub x, 5` is this once the Sub arm canonicalizes it):
/// the multiply is visited before its user and distributes (`x * 3 - 15`), so
/// the add folds the constants: `opt` gives `mul x, 3; add -8`. With no
/// multiply visit the add saw `mul(x + -5, 3) + 7` and kept it. (By eight the
/// multiply is a shift first, and the shift distributes: the shift family's.)
#[test]
fn a_multiply_is_visited_before_the_add_that_reads_it() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {\nb0:\n  %s = add i16 %x, -5\n  %m = mul i16 %s, 3\n  %r = add i16 %m, 7\n  ret i16 %r\n}\n",
    );
    combined(&mut module);
    let out = printed(&module);
    assert!(out.contains("mul i16 %x, 3") && out.contains("add i16 %") && out.contains(", -8"), "{out}");
    assert!(!out.contains("%s = add"), "{out}");
}

const SINKABLE: &str = "define i16 @f(i16 %a, i1 %c) {
b0:
  %x = mul i16 %a, %a
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
";

/// `tryToSinkInstruction`: a value read only in the one successor that only
/// `b0` enters is computed there, so the path to `b2` does not run it.
#[test]
fn the_driver_sinks_an_instruction_to_its_only_users_block() {
    check(
        SINKABLE,
        "define i16 @f(i16 %a, i1 %c) {\nb0:\n  br i1 %c, label %b1, label %b2\n\nb1:\n  %x = mul i16 %a, %a\n  ret i16 %x\n\nb2:\n  ret i16 0\n}\n",
    );
}

/// With sinking off (the tests of one family) it stays where it is.
#[test]
fn without_sinking_it_stays() {
    let mut module = parsed(SINKABLE);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::combine_with_sinking(
        context,
        &globals,
        &metadata,
        &DataLayout::default(),
        function,
        super::visit,
        false,
    );
    assert!(!changed);
    assert_eq!(printed(&module), printed(&parsed(SINKABLE)));
}
