use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `foldOpIntoPhi` of an instruction whose first
/// operand is a phi.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if !matches!(
        c.function.instruction(inst).opcode,
        Opcode::Binary(_) | Opcode::ICmp(_)
    ) {
        return Visit::None;
    }
    let Some(Operand::Value(first)) = c.function.instruction(inst).operands.first().copied() else {
        return Visit::None;
    };
    match c.function.value(first).def {
        ValueDef::Instruction(phi) if c.function.instruction(phi).opcode == Opcode::Phi => {
            c.fold_op_into_phi(inst, phi, false)
        }
        _ => Visit::None,
    }
}

/// `@f` after `foldOpIntoPhi` alone, through the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

const DIAMOND: &str = "define i16 @f(i1 %c, i16 %y) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %p = phi i16 [ 1, %a ], [ 2, %b ]
  %r = add i16 %p, 5
  ret i16 %r
}
";

/// Every incoming value is a number: the add folds into each, so the phi holds
/// 6 and 7 and the add is gone.
#[test]
fn an_add_of_a_phi_of_numbers_is_a_phi_of_sums() {
    assert_eq!(
        combined(DIAMOND),
        "define i16 @f(i1 %c, i16 %y) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  br label %j

j:
  %0 = phi i16 [ 6, %a ], [ 7, %b ]
  ret i16 %0
}
"
    );
}

/// One incoming value is unknown and its block jumps straight to the join:
/// the add is copied into that block, the numbers fold.
#[test]
fn one_unknown_incoming_value_gets_a_copy_of_the_operation() {
    let text = DIAMOND.replace("[ 2, %b ]", "[ %y, %b ]");
    assert_eq!(
        combined(&text),
        "define i16 @f(i1 %c, i16 %y) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %j

b:
  %0 = add i16 %y, 5
  br label %j

j:
  %1 = phi i16 [ 6, %a ], [ %0, %b ]
  ret i16 %1
}
"
    );
}

/// Two incoming values are unknown: a copy in each would be more work than
/// the phi saves, so nothing changes.
#[test]
fn two_unknown_incoming_values_leave_the_add_where_it_is() {
    let text = DIAMOND.replace("[ 1, %a ]", "[ %y, %a ]").replace("[ 2, %b ]", "[ %c2, %b ]");
    let text = text.replace("i1 %c, i16 %y", "i1 %c, i16 %y, i16 %c2");
    assert_eq!(combined(&text), text);
}

/// The phi has two users that are not the same operation: folding one into
/// the phi would leave the other reading the old phi, so it stays.
#[test]
fn a_phi_with_another_kind_of_user_is_left() {
    let text = DIAMOND.replace("ret i16 %r", "%s = mul i16 %p, 3\n  %t = add i16 %r, %s\n  ret i16 %t");
    assert_eq!(combined(&text), text);
}

/// The copy would go on a loop's back edge: not done, or the combine would not
/// end.
#[test]
fn the_operation_is_not_copied_onto_a_back_edge() {
    let text = "define i16 @f(i16 %n, i16 %y) {
b0:
  br label %h

h:
  %p = phi i16 [ 1, %b0 ], [ %y, %h ]
  %r = add i16 %p, 5
  %c = icmp eq i16 %r, %n
  br i1 %c, label %x, label %h

x:
  ret i16 %r
}
";
    assert_eq!(combined(text), text);
}

/// `icmp eq (phi [zext b, ..], ...), 0` where the zext has no other user: the
/// compare is copied next to it, to fold into `!b`.
#[test]
fn a_compare_with_zero_of_a_zext_of_a_bool_is_moved_to_its_block() {
    let text = "define i1 @f(i1 %c, i1 %k) {
b0:
  br i1 %c, label %a, label %b

a:
  %z = zext i1 %k to i16
  br label %j

b:
  br label %j

j:
  %p = phi i16 [ %z, %a ], [ 7, %b ]
  %r = icmp eq i16 %p, 0
  ret i1 %r
}
";
    let out = combined(text);
    assert!(out.contains("phi i1 [ %0, %a ], [ false, %b ]"), "{out}");
    assert!(!out.contains("icmp eq i16 %p"), "{out}");
}

/// Two identical adds read the phi: both become the one new phi, as the pin
/// replaces every identical user, so their difference is 0.
#[test]
fn identical_users_of_a_phi_all_read_the_new_phi() {
    let text = DIAMOND.replace("ret i16 %r", "%s = add i16 %p, 5\n  %t = sub i16 %r, %s\n  ret i16 %t");
    let out = combined(&text);
    // `sub %n, %n` is 0 on each edge only if both users read the same phi.
    assert!(out.contains("phi i16 [ 0, %a ], [ 0, %b ]"), "{out}");
}

/// `visit` that does only `foldBinopWithPhiOperands`.
fn visit_binop(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    c.fold_binop_with_phi_operands(inst)
}

fn combined_binop(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit_binop);
    printed(&module)
}

fn joined(
    phis: &str,
    op: &str,
) -> String {
    format!(
        "define i16 @f(i1 %c, i16 %i, i16 %j) {{
b0:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
{phis}  %r = {op}
  ret i16 %r
}}
"
    )
}

/// `0` on one side of every pair: the add is a phi of the other sides, not a
/// binary operator of two phis (the pin's `%phi0 [0, %bb0], [%i, %bb1]`
/// example).
#[test]
fn an_identity_in_each_pair_leaves_a_phi_of_the_others() {
    let text = joined("  %p = phi i16 [ 0, %a ], [ %i, %b ]\n  %q = phi i16 [ %j, %a ], [ 0, %b ]\n", "add i16 %p, %q");
    assert_eq!(
        combined_binop(&text),
        "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
  %0 = phi i16 [ %j, %a ], [ %i, %b ]
  ret i16 %0
}
"
    );
}

/// A pair of constants folds in its predecessor; the add on the unknowns
/// moves to the other predecessor.
#[test]
fn a_constant_pair_folds_and_the_operation_moves_to_the_other_predecessor() {
    let text = joined("  %p = phi i16 [ 3, %a ], [ %i, %b ]\n  %q = phi i16 [ 4, %a ], [ %j, %b ]\n", "add i16 %p, %q");
    assert_eq!(
        combined_binop(&text),
        "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  %0 = add i16 %i, %j
  br label %m

m:
  %1 = phi i16 [ %0, %b ], [ 7, %a ]
  ret i16 %1
}
"
    );
}

/// The other predecessor ends in a conditional branch: the add would run on a
/// path that never reads it, so nothing moves.
#[test]
fn a_predecessor_that_can_go_elsewhere_gets_no_copy() {
    let text = "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  br i1 %c, label %m, label %a

a:
  br label %m

m:
  %p = phi i16 [ 3, %a ], [ %i, %b0 ]
  %q = phi i16 [ 4, %a ], [ %j, %b0 ]
  %r = add i16 %p, %q
  ret i16 %r
}
";
    assert_eq!(combined_binop(text), text);
}

/// Two counters stepped by constants and added: one counter stepped by the
/// sum, started at the sum.
#[test]
fn two_recurrences_become_one() {
    let text = "define i16 @f(i1 %c) {
b0:
  br label %l

l:
  %p = phi i16 [ 1, %b0 ], [ %p.next, %l ]
  %q = phi i16 [ 10, %b0 ], [ %q.next, %l ]
  %p.next = add i16 %p, 2
  %q.next = add i16 %q, 3
  %r = add i16 %p.next, %q.next
  br i1 %c, label %l, label %x

x:
  ret i16 %r
}
";
    assert_eq!(
        combined_binop(text),
        "define i16 @f(i1 %c) {
b0:
  br label %l

l:
  %0 = phi i16 [ 11, %b0 ], [ %r, %l ]
  %r = add i16 %0, 5
  br i1 %c, label %l, label %x

x:
  ret i16 %r
}
"
    );
}
