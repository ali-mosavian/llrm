//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=sccp` makes of the same body (checked
//! with `tools/difftest`).

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::run_sccp;
use crate::testing::{parsed, printed, results};

/// @f after SCCP; whether anything changed.
fn sccped(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = llrm_analysis::testing::layout(module);
    let id = module
        .globals
        .iter()
        .position(|one| one.name.as_deref() == Some("f"))
        .map(|at| llrm_mir::context::GlobalId(at as u32));
    let metadata = outer.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let dt = llrm_mir::domtree::DomTree::of(function);
    run_sccp(context, &layout, &outer, &metadata, id, &dt, function).anything
}

/// `text` after SCCP is `expected` and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(sccped(&mut module), text != expected);
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A branch on a compare of constants goes one way: the other block is gone,
/// and the compare with it.
#[test]
fn test_a_branch_on_a_constant_compare_goes_one_way() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 1, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i32 7

b2:
  ret i32 8
}
",
        "define i32 @f(i32 %x) {
b0:
  br label %b1

b1:
  ret i32 7
}
",
        &[&[0], &[5]],
    );
}

/// A phi whose feasible inputs agree is that constant, and so is what reads it.
#[test]
fn test_a_phi_of_equal_constants_and_its_user_fold() {
    check(
        "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i32 [ 5, %b1 ], [ 5, %b2 ]
  %r = add i32 %p, 1
  ret i32 %r
}
",
        "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  ret i32 6
}
",
        &[&[0], &[1]],
    );
}

/// A phi input from an edge that is never taken does not count: the loop
/// counter's `%i` cannot reach `%b3` with a value other than the constant.
#[test]
fn test_a_phi_input_from_an_infeasible_edge_is_ignored() {
    check(
        "define i32 @f(i32 %x) {
b0:
  br i1 false, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i32 [ %x, %b1 ], [ 9, %b2 ]
  ret i32 %p
}
",
        "define i32 @f(i32 %x) {
b0:
  br label %b2

b2:
  br label %b3

b3:
  ret i32 9
}
",
        &[&[0], &[3]],
    );
}

/// A sign extension of a value the range says is not negative is a zero
/// extension marked `nneg`, and keeps the name.
#[test]
fn test_a_sign_extension_of_a_non_negative_range_is_a_nneg_zero_extension() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %a = and i8 %x, 127
  %s = sext i8 %a to i32
  ret i32 %s
}
",
        "define range(i32 0, 128) i32 @f(i8 %x) {
b0:
  %a = and i8 %x, 127
  %s = zext nneg i8 %a to i32
  ret i32 %s
}
",
        &[&[0], &[200], &[127]],
    );
}

/// An add whose operand ranges cannot wrap gets `nuw nsw`.
#[test]
fn test_an_add_that_cannot_wrap_gets_the_flags() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %a = add i32 %z, 1
  ret i32 %a
}
",
        "define range(i32 1, 257) i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %a = add nuw nsw i32 %z, 1
  ret i32 %a
}
",
        &[&[0], &[255]],
    );
}

/// A switch on a value whose range holds two of its cases drops the third and
/// sends the default to a block of `unreachable`.
#[test]
fn test_a_switch_loses_the_cases_the_range_excludes() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %m = and i32 %x, 1
  switch i32 %m, label %b4 [
    i32 0, label %b1
    i32 1, label %b2
    i32 2, label %b3
  ]

b1:
  ret i32 10

b2:
  ret i32 20

b3:
  ret i32 30

b4:
  ret i32 40
}
",
        "define range(i32 10, 21) i32 @f(i32 %x) {
b0:
  %m = and i32 %x, 1
  switch i32 %m, label %default.unreachable [
    i32 0, label %b1
    i32 1, label %b2
  ]

b1:
  ret i32 10

b2:
  ret i32 20

default.unreachable:
  unreachable
}
",
        &[&[0], &[3]],
    );
}

/// The function returns 3 or 5, so its return value is in `[3, 6)`.
#[test]
fn test_the_return_range_becomes_an_attribute() {
    check(
        "define i32 @f(i1 %c) {
b0:
  %s = select i1 %c, i32 3, i32 5
  ret i32 %s
}
",
        "define range(i32 3, 6) i32 @f(i1 %c) {
b0:
  %s = select i1 %c, i32 3, i32 5
  ret i32 %s
}
",
        &[&[0], &[1]],
    );
}

/// A mask that clears no bit the value can have is the value.
#[test]
fn test_a_mask_the_range_already_fits_is_removed() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %m = and i32 %z, 255
  ret i32 %m
}
",
        "define range(i32 0, 256) i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  ret i32 %z
}
",
        &[&[0], &[255]],
    );
}

/// `x u< 10` where `x` is known to be in `[0, 4)` is `true`: a branch on it
/// goes one way, through a block whose argument range comes from a `range`
/// attribute.
#[test]
fn test_an_argument_range_decides_a_compare() {
    check(
        "define i32 @f(i32 range(i32 0, 4) %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  ret i32 1

b2:
  ret i32 2
}
",
        "define i32 @f(i32 range(i32 0, 4) %x) {
b0:
  br label %b1

b1:
  ret i32 1
}
",
        &[&[0], &[3]],
    );
}

/// A compare a branch has already decided is a constant where the branch holds
/// (`PredicateInfo`'s copy of `%x` carries `[0, 10)` into `%b1`).
#[test]
fn test_a_branch_condition_decides_a_later_compare() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  %d = icmp ult i32 %x, 20
  br i1 %d, label %b3, label %b4

b3:
  ret i32 1

b4:
  ret i32 2

b2:
  ret i32 3
}
",
        "define range(i32 1, 4) i32 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b3:
  ret i32 1

b2:
  ret i32 3
}
",
        &[&[0], &[9], &[10], &[50]],
    );
}

/// Where `%x == 5` holds, `%x` is 5: what reads it there folds.
#[test]
fn test_an_equality_branch_makes_the_value_a_constant_where_it_holds() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %r = add i32 %x, 1
  ret i32 %r

b2:
  ret i32 0
}
",
        "define range(i32 0, 7) i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  ret i32 6

b2:
  ret i32 0
}
",
        &[&[5], &[6]],
    );
}
