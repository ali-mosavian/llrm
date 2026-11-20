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

/// `not (icmp ..)` with other users is the inverse compare when every user can
/// take the inversion (`canFreelyInvertAllUsersOf`): here a branch swaps its
/// successors. Before, only a one-use compare was inverted, so a loop exit
/// `select (not c), true, d` kept its `xor` and its `select` where the pin has
/// `or` (memory_expansion -O2: 217 of each).
#[test]
fn a_not_of_a_compare_with_invertible_users_is_the_inverse_compare() {
    let mut module = parsed(
        "define i32 @f(i64 noundef %p, i64 noundef %q) {\nb0:\n  %c = icmp eq i64 %p, %q\n  %n = xor i1 %c, true\n  %d = icmp eq i64 %p, 1199\n  %r = select i1 %n, i1 true, i1 %d\n  br i1 %r, label %x, label %y\nx:\n  br i1 %c, label %z, label %y\ny:\n  ret i32 1\nz:\n  ret i32 0\n}\n",
    );
    combined(&mut module);
    let out = printed(&module);
    assert!(!out.contains("xor i1"), "{out}");
    assert!(out.contains("icmp ne i64 %p, %q"), "{out}");
}

/// The profile of InstCombine's work had no counters: "visits at parity with
/// the pin" was read from callgrind call counts. A function with nothing to
/// fold is drained once and each instruction visited exactly once; a fold
/// re-adds its users, so visits exceed the initial list.
#[test]
fn drains_visits_and_inserts_count_the_worklist() {
    use llrm_mir::counts::{Event, get};
    let counts = || [Event::CombineDrains, Event::CombineVisits, Event::CombineInitial, Event::CombineInserts].map(get);
    let delta = |before: [u64; 4]| {
        let now = counts();
        [0, 1, 2, 3].map(|i| now[i] - before[i])
    };
    let before = counts();
    let mut module = parsed("define i16 @f(i16 %x, i16 %y) {\nb0:\n  %a = add i16 %x, %y\n  ret i16 %a\n}\n");
    assert!(!combined(&mut module));
    let [drains, visits, initial, inserts] = delta(before);
    assert_eq!((drains, visits, initial), (1, 2, 2));
    assert_eq!(inserts, initial);

    let before = counts();
    let mut module =
        parsed("define i16 @f(i16 %x) {\nb0:\n  %a = add i16 %x, 0\n  %b = sub i16 %a, %x\n  ret i16 %b\n}\n");
    assert!(combined(&mut module));
    let [_, visits, initial, inserts] = delta(before);
    assert!(visits > initial - 1 && inserts >= initial, "{visits} {initial} {inserts}");
}

/// The loop vectorizer's `add <8 x i16>` reached `fold_box_multiply`, which
/// reads an integer width, and panicked ("an integer") the arm64 compile of a
/// widened add. The vector operations are left as they are.
#[test]
fn a_vector_operation_is_not_visited_by_the_scalar_folds() {
    for op in ["shl", "lshr", "ashr", "and", "or", "xor"] {
        let text = format!(
            "define <8 x i16> @f(<8 x i16> %x, <8 x i16> %y) {{\nb0:\n  %a = {op} <8 x i16> %x, %y\n  ret <8 x i16> %a\n}}\n"
        );
        check(&text, &text);
    }
}

/// A vector `zext`/`sext`/`trunc` (a widened sum of bytes) reached `too_wide`,
/// which reads an integer width, and panicked ("an integer") the compile of an
/// extending reduction. The vector casts are left as they are.
#[test]
fn a_vector_cast_is_not_visited_by_the_scalar_folds() {
    for (op, from, to) in [("zext", "i8", "i64"), ("sext", "i16", "i32"), ("trunc", "i64", "i16")] {
        let text = format!(
            "define <8 x {to}> @f(<8 x {from}> %x) {{\nb0:\n  %a = {op} <8 x {from}> %x to <8 x {to}>\n  ret <8 x {to}> %a\n}}\n"
        );
        check(&text, &text);
    }
}

/// x_ld_horner -O2 m32: a `fadd` of two loop results was sunk to the block that
/// returns, two floats alive across every later loop where one sum was
/// (a spill and a reload of the multiplier each trip, +50 clk against gcc).
/// A float stays where it is on a target with an x87 stack; the pin sinks it.
const FLOAT_SUM: &str = "define double @f(double %a, double %b, i1 %c) {
b0:
  %s = fadd double %a, %b
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  ret double %s
}
";

fn float_sum_combined(keep: bool) -> String {
    let mut module = parsed(FLOAT_SUM);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    super::combine_on_target(
        context,
        &globals,
        &Default::default(),
        &metadata,
        &DataLayout::default(),
        function,
        &mut declared,
        super::TargetHooks { keeps_float_off_returning_sink: keep, ..Default::default() },
    );
    printed(&module)
}

#[test]
fn test_a_float_is_not_sunk_to_a_returning_block_on_an_x87_stack() {
    let text = float_sum_combined(true);
    assert!(text.find("fadd").unwrap() < text.find("br i1 %c").unwrap(), "{text}");
}

/// Control: the pin's sink, and an integer on the x87 target, are unchanged.
#[test]
fn test_without_an_x87_stack_the_float_is_sunk_as_the_pin_does() {
    let text = float_sum_combined(false);
    assert!(text.find("fadd").unwrap() > text.find("br i1 %c").unwrap(), "{text}");
}

/// A widened reduction whose vector loop ran once adds its first part to a zero
/// start: `add <4 x i32> zeroinitializer, %x` stayed (the vector operations
/// were not visited at all), an extra `saddw` and register copy per part in
/// tile's `-O2` rows (1,431 instructions against the pin's 980). `x + 0` and
/// `x * 1` are `x` for vectors too.
#[test]
fn a_vector_identity_folds_as_the_scalar_one_does() {
    check(
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = add <4 x i32> zeroinitializer, %x\n  %b = mul <4 x i32> %a, <i32 1, i32 1, i32 1, i32 1>\n  ret <4 x i32> %b\n}\n",
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  ret <4 x i32> %x\n}\n",
    );
    // The reduction's first part, as tile's widened sum has it.
    check(
        "define <8 x i64> @f(<8 x i64> %x) {\nb0:\n  %a = add nsw <8 x i64> zeroinitializer, %x\n  ret <8 x i64> %a\n}\n",
        "define <8 x i64> @f(<8 x i64> %x) {\nb0:\n  ret <8 x i64> %x\n}\n",
    );
}

#[test]
fn a_vector_multiply_by_one_and_an_and_with_all_ones_fold() {
    check(
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = mul <4 x i32> %x, <i32 1, i32 1, i32 1, i32 1>\n  %b = and <4 x i32> %a, <i32 -1, i32 -1, i32 -1, i32 -1>\n  ret <4 x i32> %b\n}\n",
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  ret <4 x i32> %x\n}\n",
    );
}

/// `urem (and x, 255), 251` on vectors was folded to its dividend: the value
/// analyses read a vector's width as 0, so its range came out as one below 251
/// (a debug build: "attempt to subtract with overflow"). `huge` -O2 summed
/// 319,345,180, not 5,020,780. A vector is what every lane holds, a splat
/// constant's number, else nothing.
#[test]
fn a_vector_remainder_is_not_folded_by_a_range_it_does_not_have() {
    let text = "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = and <4 x i32> %x, <i32 255, i32 255, i32 255, i32 255>\n  %r = urem <4 x i32> %a, <i32 251, i32 251, i32 251, i32 251>\n  ret <4 x i32> %r\n}\n";
    check(text, text);
}

/// A vector `urem` by a splat and an `and` with zero are not identities: the
/// first attempt read the splat as a scalar and dropped the `urem` (huge O2
/// returned a wrong sum).
#[test]
fn a_vector_operation_that_is_no_identity_stays() {
    let urem = "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = urem <4 x i32> %x, <i32 251, i32 251, i32 251, i32 251>\n  ret <4 x i32> %a\n}\n";
    check(urem, urem);
    let and = "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = and <4 x i32> %x, <i32 1, i32 1, i32 1, i32 1>\n  ret <4 x i32> %a\n}\n";
    check(and, and);
}

/// recsum's inner loop on arm64 added two constants to a vector one after the
/// other (`(x + 3) + 4`): the visitors read integer widths and skipped
/// vectors, so the pair stayed (two `add`s a trip, 8,776 instructions against
/// clang's 7,780). The sum of the constants is one lane-wise constant.
#[test]
fn a_vector_add_of_two_constants_is_one_add() {
    check(
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = add <4 x i32> %x, <i32 3, i32 3, i32 3, i32 3>\n  %b = add <4 x i32> %a, <i32 4, i32 4, i32 4, i32 4>\n  ret <4 x i32> %b\n}\n",
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %b = add <4 x i32> %x, <i32 7, i32 7, i32 7, i32 7>\n  ret <4 x i32> %b\n}\n",
    );
}

#[test]
fn a_vector_add_of_a_value_to_itself_is_a_shift() {
    check(
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %a = add <4 x i32> %x, %x\n  ret <4 x i32> %a\n}\n",
        "define <4 x i32> @f(<4 x i32> %x) {\nb0:\n  %0 = shl <4 x i32> %x, <i32 1, i32 1, i32 1, i32 1>\n  ret <4 x i32> %0\n}\n",
    );
}

/// The scalar text of `template` and its vector twin: `{T}` is the type,
/// `{C:n}` the constant `n` (a splat in the vector).
fn twins(template: &str) -> (String, String) {
    let (mut scalar, mut vector) = (String::new(), String::new());
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        let (before, after) = rest.split_at(at);
        scalar.push_str(before);
        vector.push_str(before);
        let end = after[1..].find('$').expect("a closing $") + 1;
        let key = &after[1..end];
        if key == "T" {
            scalar.push_str("i32");
            vector.push_str("<4 x i32>");
        } else {
            let number = key.strip_prefix("C:").expect("$T$ or $C:n$");
            scalar.push_str(number);
            vector.push_str(&format!("<i32 {number}, i32 {number}, i32 {number}, i32 {number}>"));
        }
        rest = &after[end + 1..];
    }
    scalar.push_str(rest);
    vector.push_str(rest);
    (scalar, vector)
}

/// The vector twin of what a scalar program combines to, as the scalar one.
fn as_scalar(text: &str) -> String {
    let mut text = text.replace("<4 x i32>", "i32").replace("zeroinitializer", "0");
    for number in -9..=99 {
        let splat = format!("<i32 {number}, i32 {number}, i32 {number}, i32 {number}>");
        text = text.replace(&splat, &number.to_string());
    }
    text
}

/// A vector operation is what the scalar one is, lane by lane: the visitors
/// the pin's InstCombine runs on a splat give the result they give on the
/// scalar. Each template is a scalar shape the add visitors know.
#[test]
fn a_vector_add_folds_as_the_scalar_add_does() {
    let templates = [
        "define $T$ @f($T$ %x) {\nb0:\n  %a = add $T$ %x, $C:3$\n  %b = add $T$ %a, $C:4$\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = add $T$ %x, %x\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %n = sub $T$ $C:0$, %y\n  %a = add $T$ %x, %n\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %s = shl $T$ %y, $C:2$\n  %a = add $T$ %x, %s\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %n = xor $T$ %x, $C:-1$\n  %a = add $T$ %n, $C:1$\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %a = and $T$ %x, %y\n  %o = or $T$ %x, %y\n  %s = add $T$ %a, %o\n  ret $T$ %s\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = add nsw $T$ %x, $C:5$\n  %b = add nuw $T$ %a, $C:6$\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %a = sub $T$ %x, %y\n  %b = add $T$ %a, %y\n  ret $T$ %b\n}\n",
    ];
    for template in templates {
        let (scalar, vector) = twins(template);
        let mut one = parsed(&scalar);
        combined(&mut one);
        let mut other = parsed(&vector);
        combined(&mut other);
        assert_eq!(as_scalar(&printed(&other)), printed(&one), "{template}");
    }
}

/// As [`a_vector_add_folds_as_the_scalar_add_does`], for `sub` and `mul`:
/// recsum's loop multiplies `(x + 3)` by a constant three times a trip, which
/// the pin's InstCombine makes `x * 3 + 9` and these visitors left alone on
/// vectors.
#[test]
fn a_vector_sub_and_mul_fold_as_the_scalar_ones_do() {
    let templates = [
        "define $T$ @f($T$ %x) {\nb0:\n  %a = sub $T$ %x, $C:5$\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = sub $T$ %x, %x\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %a = add $T$ %x, %y\n  %b = sub $T$ %a, %y\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %n = sub $T$ $C:0$, %x\n  %b = sub $T$ $C:0$, %n\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = mul $T$ %x, $C:8$\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = mul $T$ %x, $C:-1$\n  ret $T$ %a\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = mul $T$ %x, $C:3$\n  %b = mul $T$ %a, $C:5$\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x) {\nb0:\n  %a = add $T$ %x, $C:3$\n  %b = mul $T$ %a, $C:5$\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %a = mul $T$ %x, $C:3$\n  %b = add $T$ %a, %x\n  ret $T$ %b\n}\n",
        "define $T$ @f($T$ %x, $T$ %y) {\nb0:\n  %a = sub $T$ $C:0$, %x\n  %b = sub $T$ $C:0$, %y\n  %c = mul $T$ %a, %b\n  ret $T$ %c\n}\n",
    ];
    for template in templates {
        let (scalar, vector) = twins(template);
        let mut one = parsed(&scalar);
        combined(&mut one);
        let mut other = parsed(&vector);
        combined(&mut other);
        assert_eq!(as_scalar(&printed(&other)), printed(&one), "{template}");
    }
}

/// `sinkNotIntoOtherHandOfLogicalOp`: `(x == 0) | ~(a & b)` is `~((x != 0) & (a
/// & b))` and the select on it swaps its arms. The `or`, the `not` and the
/// inverted compare stayed (the `round_whole` of QuickrBasic `double.c`: six
/// more arm64 instructions than the pin's `ands`, `ccmp`, `cset`).
#[test]
fn a_not_beside_a_free_to_invert_compare_sinks_into_the_logic_and_swaps_the_select() {
    let mut module = parsed(
        "define i64 @f(i64 %x, i1 %a, i64 %y) {\nb0:\n  %b = icmp ne i64 %y, 0\n  %c = and i1 %a, %b\n  %n = xor i1 %c, true\n  %m = and i64 %x, 9223372036854775807\n  %z = icmp eq i64 %m, 0\n  %o = or i1 %z, %n\n  %r = select i1 %o, i64 %y, i64 -4616189618054758400\n  ret i64 %r\n}\n",
    );
    combined(&mut module);
    let text = printed(&module);
    assert!(!text.contains(" or i1") && !text.contains("xor i1"), "{text}");
    assert!(text.contains("%z = icmp ne i64 %m, 0"), "{text}");
    assert!(text.contains("%0 = and i1 %z, %c"), "{text}");
    assert!(text.contains("select i1 %0, i64 -4616189618054758400, i64 %y"), "{text}");
}
