use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Module, Operand};

use super::combine;
use crate::testing::{parsed, printed};

/// `@f` combined; whether anything changed.
fn combined(module: &mut Module) -> bool {
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len());
    combine(context, &globals, &Default::default(), &metadata, &DataLayout::default(), function, &mut declared)
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
        &Default::default(),
        &metadata,
        &DataLayout::default(),
        function,
        super::visit,
        false,
        &mut llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len()),
    );
    assert!(!changed);
    assert_eq!(printed(&module), printed(&parsed(SINKABLE)));
}

/// `isInstructionTriviallyDead` named opcodes only: a dead call to a readnone
/// nounwind willreturn function stayed (the pin deletes it).
#[test]
fn a_dead_pure_call_is_deleted() {
    let mut module = parsed(
        "declare i32 @pure(i32) nounwind willreturn memory(none)\n\ndefine i32 @f(i32 %a) {\nb0:\n  %x = call i32 @pure(i32 %a)\n  ret i32 %a\n}\n",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len());
    combine(context, &globals, &callees, &metadata, &DataLayout::default(), function, &mut declared);
    assert!(!printed(&module).contains("call i32 @pure"), "{}", printed(&module));
}

/// x_hash at -m16 (UB: a shift by 24 of a 16-bit value): `br i1 undef` left
/// both arms live and their code in place. A branch on undef is UB, so no
/// successor is live: the phi input over each edge is poison and the dead
/// blocks hold no instruction but their terminator
/// (`prepareWorklist`, InstructionCombining.cpp#L6105).
#[test]
fn a_branch_on_undef_has_no_live_successor() {
    check(
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 undef, label %a, label %b\na:\n  %p = add i16 %x, 1\n  br label %j\nb:\n  %q = add i16 %y, 2\n  br label %j\nj:\n  %r = phi i16 [ %p, %a ], [ %q, %b ]\n  ret i16 %r\n}\n",
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 undef, label %a, label %b\n\na:\n  br label %j\n\nb:\n  br label %j\n\nj:\n  ret i16 poison\n}\n",
    );
}

/// The same for a `switch` on poison.
#[test]
fn a_switch_on_poison_has_no_live_successor() {
    check(
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  switch i16 poison, label %d [\n    i16 1, label %c\n  ]\nc:\n  store i16 %x, ptr @g\n  br label %j\nd:\n  store i16 7, ptr @g\n  br label %j\nj:\n  %r = load i16, ptr @g\n  ret i16 %r\n}\n",
        "@g = global i16 0\n\ndefine i16 @f(i16 %x) {\nb0:\n  switch i16 poison, label %d [\n    i16 1, label %c\n  ]\n\nc:\n  br label %j\n\nd:\n  br label %j\n\nj:\n  ret i16 poison\n}\n",
    );
}

/// `visit` asked the generic simplifier about every instruction with a value,
/// and the visitor of an add, a shift or a compare asked again as its first
/// step: the same question twice (d_faces -O2: 9,931 generic asks, 9.6 M Ir of
/// them on compares). Those opcodes are asked once; a cast, whose visitor has
/// tests before its own, is still asked by `visit`.
#[test]
fn an_instruction_whose_visitor_simplifies_is_not_simplified_before_it() {
    let asks = |text: &str| {
        let mut module = parsed(text);
        super::GENERIC_SIMPLIFIES.with(|one| one.set(0));
        combined(&mut module);
        super::GENERIC_SIMPLIFIES.with(std::cell::Cell::get)
    };
    let own = "define i1 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %x, %y
  %s = shl i32 %a, 3
  %c = icmp slt i32 %s, %y
  ret i1 %c
}
";
    let cast = "define i64 @f(i32 %x) {
b0:
  %e = zext i32 %x to i64
  ret i64 %e
}
";
    assert_eq!(asks(own), 0, "an add, a shift and a compare ask for themselves");
    assert_eq!(asks(cast), 1, "a cast is asked by `visit`");
}

/// scroll.nib's descending copy loop tests `j - 1 > -1` at the bottom after
/// `j <u 1920` (the bounds check) dominates it. `foldICmpWithDominatingICmp`
/// makes that `j != 0` (the compares' ranges leave one value out); the pin
/// then canonicalizes it to `j == 0` with the arms swapped
/// (`canonicalizeICmpPredicate`).
#[test]
fn a_signed_bound_on_a_value_a_dominating_compare_keeps_below_a_limit_is_an_equality() {
    let mut module = parsed(
        "define void @f(ptr %a) {
entry:
  br label %body

body:
  %j = phi i16 [ 1919, %entry ], [ %jm, %do ]
  %ok = icmp samesign ult i16 %j, 1920
  br i1 %ok, label %do, label %trap

do:
  %p = getelementptr inbounds nuw [2 x i8], ptr %a, i16 %j
  %v = load i16, ptr %p, align 2
  store i16 %v, ptr %a, align 2
  %jm = add nsw i16 %j, -1
  %c = icmp sgt i16 %jm, -1
  br i1 %c, label %body, label %exit

trap:
  unreachable

exit:
  ret void
}
",
    );
    combined(&mut module);
    let after = printed(&module);
    assert!(after.contains("icmp eq i16 %j, 0"), "{after}");
    assert!(after.contains("label %exit, label %body"), "{after}");
}

/// A loop that tests `i != n` at its latch leaves the pin as `i == n` with the
/// arms swapped (`canonicalizeICmpPredicate`: `ne` is not canonical, and a
/// branch takes the inversion for free). It stayed `ne`, so JumpThreading read
/// x_list's list-sum loop another way round and kept two pointer phis.
#[test]
fn a_branch_on_a_non_canonical_compare_is_the_inverse_compare_with_the_arms_swapped() {
    let mut module = parsed(
        "define void @f(i16 %n) {
entry:
  br label %body

body:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %next = add i16 %i, 1
  %more = icmp ne i16 %next, %n
  br i1 %more, label %body, label %exit

exit:
  ret void
}
",
    );
    combined(&mut module);
    let after = printed(&module);
    assert!(after.contains("icmp eq i16 %next, %n"), "{after}");
    assert!(after.contains("label %exit, label %body"), "{after}");
}

/// `canonicalizeICmpBool`: `icmp eq i1 %x, false` is `not %x`, and a branch on
/// a `not` takes the arms swapped. With `canonicalizeICmpPredicate` the nib
/// zip loop's `icmp ne i1 %c, false` became `icmp eq` with swapped arms and
/// stayed so: ConstraintElimination then found no `%c` to read and three
/// bounds checks of the loop stayed.
#[test]
fn a_branch_on_an_i1_equal_to_false_is_a_branch_on_it_with_the_arms_swapped() {
    let mut module = parsed(
        "define void @f(i1 %x) {
entry:
  %c = icmp eq i1 %x, false
  br i1 %c, label %a, label %b

a:
  ret void

b:
  ret void
}
",
    );
    combined(&mut module);
    let after = printed(&module);
    assert!(after.contains("br i1 %x, label %b, label %a"), "{after}");
    assert!(!after.contains("icmp"), "{after}");
}
