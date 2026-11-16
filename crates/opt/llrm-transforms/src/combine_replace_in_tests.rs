use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

/// `@f` after `visitSelectInst` alone.
fn run(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Opt: the compare in the true arm reads a select on the same condition, which
/// there is its true value. Without it the select stayed under the compare:
/// known_loads_walks -O2 ran 76,228 instructions against the pin's 75,696,
/// the loop's phis never became one `i1` phi for the peel.
#[test]
fn the_true_arm_reads_the_condition_as_true() {
    let out = run(
        "define i1 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %b = select i1 %c, i8 %q, i8 2\n  %i = icmp ugt i8 %p, %b\n  %r = select i1 %c, i1 %i, i1 false\n  ret i1 %r\n}\n",
    );
    assert!(out.contains("select i1 true, i8 %q, i8 2"), "{out}");
}

/// The false arm reads it as false.
#[test]
fn the_false_arm_reads_the_condition_as_false() {
    let out = run(
        "define i1 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %b = select i1 %c, i8 %q, i8 2\n  %i = icmp ugt i8 %p, %b\n  %r = select i1 %c, i1 true, i1 %i\n  ret i1 %r\n}\n",
    );
    assert!(out.contains("select i1 false, i8 %q, i8 2"), "{out}");
}

/// Two instructions up and no further: the select under the add is the third.
#[test]
fn a_use_three_instructions_up_is_left() {
    let text = "define i1 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %b = select i1 %c, i8 %q, i8 2\n  %a = add i8 %b, 1\n  %i = icmp ugt i8 %p, %a\n  %r = select i1 %c, i1 %i, i1 false\n  ret i1 %r\n}\n";
    assert_eq!(run(text), printed(&parsed(text)));
}

/// A value with another user keeps its operands: the other user still reads the
/// select on its own condition.
#[test]
fn a_value_with_two_users_is_left() {
    let text = "define i1 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %b = select i1 %c, i8 %q, i8 2\n  %i = icmp ugt i8 %p, %b\n  %j = icmp ult i8 %p, %b\n  %k = and i1 %i, %j\n  %r = select i1 %c, i1 %i, i1 %k\n  ret i1 %r\n}\n";
    let out = run(text);
    assert!(out.contains("%b = select i1 %c, i8 %q, i8 2"), "{out}");
}

/// `foldSelectValueEquivalence`: `x == 5 ? x + y : z` reads 5 for `x` even when
/// the arm does not simplify.
#[test]
fn an_arm_that_does_not_simplify_reads_the_constant_for_the_compared_value() {
    let out = run(
        "define i8 @f(i8 %x, i8 %y, i8 %z) {\nb0:\n  %c = icmp eq i8 %x, 5\n  %a = add i8 %x, %y\n  %r = select i1 %c, i8 %a, i8 %z\n  ret i8 %r\n}\n",
    );
    assert!(out.contains("add i8 5, %y"), "{out}");
}

/// Opt, the reduced known_loads_walks loop through the whole combine: the two
/// recurrences `p`, `q` that agree after one trip become an `i1` phi, which
/// is what the unroll's peel needs. Without `replaceInInstruction` the selects
/// on `%c` stayed in the loop (known_loads_walks -O2, +532 instructions over
/// the pin).
#[test]
fn a_loop_of_selects_on_an_invariant_condition_gets_an_i1_recurrence() {
    use crate::combine_pass::Combine;
    use crate::testing::managed;
    let mut module = parsed(
        "define i8 @f(i1 %c, i8 %x0, i8 %y0, i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [ 1, %entry ], [ %i1, %loop ]\n  %p = phi i8 [ %x0, %entry ], [ %r, %loop ]\n  %q = phi i8 [ %y0, %entry ], [ %r, %loop ]\n  %a = select i1 %c, i8 %p, i8 0\n  %b = select i1 %c, i8 %q, i8 2\n  %cmp = icmp ult i8 %b, %a\n  %r = select i1 %cmp, i8 0, i8 %a\n  %i1 = add nuw i8 %i, 1\n  %e = icmp ult i8 %i1, %n\n  br i1 %e, label %loop, label %exit\nexit:\n  ret i8 %r\n}\n",
    );
    let out = managed(&mut module, Combine { enabled: true, phi_constant_users: false });
    assert!(out.contains("phi i1") && !out.contains("i8 2"), "{out}");
}
