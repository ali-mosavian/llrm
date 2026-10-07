//! Adapted from llrm-core's `optimize/transform_tests.rs` (`folded_tests`)
//! and the tests of `analysis/consts_tests.rs` that waited for this port,
//! each body now MIR text run by llrm-mir's interpreter.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::{Fold, folded};
use crate::testing::{ACROSS_READONLY_CALL, managed, parsed, printed, results};

/// @f folded; whether anything changed.
fn fold(module: &mut Module) -> bool {
    let (layout, outer) = (layout(module), Outer::of(module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    folded(context, &layout, function, &outer, &llrm_analysis::consts::Calls::default())
}

/// `text` folded is `expected`, and computes what it did on `inputs`.
fn check(text: &str, expected: &str, inputs: &[&[i128]]) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(fold(&mut module), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// LNGMXX retained invariant division by 7 because its constant divisor
/// stayed opaque to LICM.
#[test]
fn divisor_constants_propagate_without_reordering() {
    for divisor in [7, -1] {
        check(
            &format!(
                "define i32 @f(i32 %x) {{
b0:
  %k = add i32 {divisor}, 0
  %q = sdiv i32 %x, %k
  ret i32 %q
}}
"
            ),
            &format!(
                "define i32 @f(i32 %x) {{
b0:
  %k = add i32 {divisor}, 0
  %q = sdiv i32 %x, {divisor}
  ret i32 %q
}}
"
            ),
            &[&[0], &[100], &[-50]],
        );
    }
}

#[test]
fn test_signed_widening_produces_a_whole_long_constant() {
    for number in [0_i64, 1, 32767, 32768, 65535] {
        let expected = (number ^ 0x8000) - 0x8000;
        check(
            &format!(
                "define i32 @f() {{
b0:
  %w = sext i16 {number} to i32
  ret i32 %w
}}
"
            ),
            &format!(
                "define i32 @f() {{
b0:
  %w = sext i16 {expected} to i32
  ret i32 {expected}
}}
"
            ),
            &[&[]],
        );
    }
}

/// NDMAX's zero displacement should fold without taking its base for an
/// integer offset.
#[test]
fn test_pointer_displacement_constants_preserve_order_and_width() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = alloca [4 x i16]
  %d = sub i16 %x, %x
  %q = getelementptr i8, ptr %a, i16 %d
  store i16 %x, ptr %q
  %v = load i16, ptr %a
  ret i16 %v
}
",
        "define i16 @f(i16 %x) {
b0:
  %a = alloca [4 x i16]
  %d = sub i16 %x, %x
  %q = getelementptr i8, ptr %a, i16 0
  store i16 %x, ptr %q
  %v = load i16, ptr %a
  ret i16 %v
}
",
        &[&[5], &[-9]],
    );
}

/// NESTED kept constant loop bounds in registers; propagating them must
/// not reverse a subtraction or a compare.
#[test]
fn test_constant_subtraction_preserves_operand_order() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %b = add i16 2, 3
  %r = sub i16 %b, %x
  %s = sub i16 %x, %b
  %t = sub i16 %b, %b
  %c = icmp slt i16 %b, %x
  %z = zext i1 %c to i16
  %u = add i16 %r, %s
  %v = add i16 %u, %t
  %w = add i16 %v, %z
  ret i16 %w
}
",
        "define i16 @f(i16 %x) {
b0:
  %b = add i16 2, 3
  %r = sub i16 5, %x
  %s = sub i16 %x, 5
  %t = sub i16 5, 5
  %c = icmp slt i16 5, %x
  %z = zext i1 %c to i16
  %u = add i16 %r, %s
  %v = add i16 %u, 0
  %w = add i16 %v, %z
  ret i16 %w
}
",
        &[&[0], &[5], &[6], &[-3]],
    );
}

/// A known factor is the multiply's constant operand, on the right.
#[test]
fn test_a_known_factor_becomes_a_multiply_operand() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %f = add i16 10, 10
  %m = mul i16 %f, %x
  %n = mul i16 %x, %f
  %s = add i16 %m, %n
  ret i16 %s
}
",
        "define i16 @f(i16 %x) {
b0:
  %f = add i16 10, 10
  %m = mul i16 %x, 20
  %n = mul i16 %x, 20
  %s = add i16 %m, %n
  ret i16 %s
}
",
        &[&[0], &[3], &[-7]],
    );
}

/// A division of two known numbers is its quotient or remainder, and what
/// is computed from them is known in the same run.
#[test]
fn test_a_division_of_numbers_is_its_answer() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = sub i16 0, 7
  %q = sdiv i16 %a, 2
  %r = srem i16 %a, 2
  %u = udiv i16 %a, 2
  %s = add i16 %q, %r
  %t = add i16 %s, %u
  ret i16 %t
}
",
        "define i16 @f(i16 %x) {
b0:
  %a = sub i16 0, 7
  %q = sdiv i16 -7, 2
  %r = srem i16 -7, 2
  %u = udiv i16 -7, 2
  %s = add i16 -3, -1
  %t = add i16 -4, 32764
  ret i16 32760
}
",
        &[&[0]],
    );
}

/// A division that may trap stays: by a value that may be zero, by zero,
/// and the one quotient that overflows.
#[test]
fn test_a_division_that_may_trap_stays() {
    let text = "define i16 @f(i16 %x) {
b0:
  %zero = sub i16 %x, %x
  %a = udiv i16 100, %x
  %b = udiv i16 100, %zero
  %c = sdiv i16 -32768, -1
  %s = add i16 %a, %b
  %t = add i16 %s, %c
  ret i16 %t
}
";
    let mut module = parsed(text);
    fold(&mut module);
    let printed = printed(&module);
    for kept in ["udiv i16 100, %x", "udiv i16 100, 0", "sdiv i16 -32768, -1", "add i16 %a, %b", "add i16 %s, %c"] {
        assert!(printed.contains(kept), "{kept}\n{printed}");
    }
}

/// What a store put in a cell is what a plain load after it reads; a
/// volatile load, and a load after a call that may write, stay.
#[test]
fn test_a_load_of_a_known_cell_is_its_number() {
    check(
        &format!(
            "{DOS}@g = global i16 0

define void @clobber() {{
b0:
  store i16 1, ptr @g
  ret void
}}

define i16 @f(i16 %x) {{
b0:
  store i16 9, ptr @g
  %a = load i16, ptr @g
  %v = load volatile i16, ptr @g
  call void @clobber()
  %b = load i16, ptr @g
  %s = add i16 %a, %v
  %t = add i16 %s, %b
  ret i16 %t
}}
"
        ),
        &format!(
            "{DOS}@g = global i16 0

define void @clobber() {{
b0:
  store i16 1, ptr @g
  ret void
}}

define i16 @f(i16 %x) {{
b0:
  store i16 9, ptr @g
  %a = load i16, ptr @g
  %v = load volatile i16, ptr @g
  call void @clobber()
  %b = load i16, ptr @g
  %s = add i16 %v, 9
  %t = add i16 %s, %b
  ret i16 %t
}}
"
        ),
        &[&[0]],
    );
}

/// A pure operation a join feeds, known on each edge, becomes a join of
/// its numbers; one reading an unknown value too stays.
#[test]
fn test_an_operation_on_a_join_is_folded_on_its_edges() {
    check(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %y = add i16 %p, %x
  %s = add i16 %p, 10
  %t = add i16 %s, %y
  ret i16 %t
}
",
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %s1 = phi i16 [ 11, %b1 ], [ 12, %b2 ]
  %y = add i16 %p, %x
  %t = add i16 %s1, %y
  ret i16 %t
}
",
        &[&[0, 5], &[1, 5], &[1, -20]],
    );
}

/// Nothing known, nothing changes.
#[test]
fn test_a_body_of_unknowns_is_unchanged() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %y = mul i16 %x, %x
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  ret i16 %r
}
";
    check(text, text, &[&[3, 0], &[3, 1]]);
}

/// The pass puts a compare folding left constant-first back in order.
#[test]
fn test_fold_leaves_compares_canonical() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  %b = add i16 2, 3
  %c = icmp slt i16 %b, %x
  %z = zext i1 %c to i16
  ret i16 %z
}
",
    );
    let before = results(&module, &[&[4], &[5], &[6]]);
    assert!(managed(&mut module, Fold).contains("icmp sgt i16 %x, 5"));
    assert_eq!(results(&module, &[&[4], &[5], &[6]]), before);
}

/// Under the pass manager Fold still reads @peek's `readonly`: without the
/// module's globals it took the call for a writer and kept the load.
#[test]
fn a_cell_kept_across_a_readonly_call_is_folded() {
    let mut module = parsed(&format!("{DOS}{ACROSS_READONLY_CALL}"));
    let after = managed(&mut module, Fold);
    assert!(after.contains("  ret i16 7\n"), "{after}");
}

/// The old Fold ended in floatfold: an exact float is its constant.
#[test]
fn test_an_exact_float_folds() {
    check(
        "define i16 @f() {
b0:
  %x = fadd double 1.000000e+00, 2.000000e+00
  %y = fmul double %x, 5.000000e-01
  %i = fptosi double %y to i16
  ret i16 %i
}
",
        "define i16 @f() {
b0:
  %x = fadd double 1.000000e+00, 2.000000e+00
  %y = fmul double 3.000000e+00, 5.000000e-01
  %i = fptosi double 1.500000e+00 to i16
  ret i16 1
}
",
        &[&[]],
    );
}

/// A counted loop's exact float stores are known past its exit, as
/// floatfacts' exit cells say.
#[test]
fn test_a_float_loop_exit_is_known_after_it() {
    let text = "@m = global [4 x i8] zeroinitializer

define i32 @f() {
b0:
  store float 0.0, ptr @m
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b2, label %b3

b2:
  %v = load float, ptr @m
  %w = fadd float %v, 48.75
  store float %w, ptr @m
  %n = add i16 %i, 1
  br label %b1

b3:
  %after = load i32, ptr @m
  ret i32 %after
}
";
    check(text, &text.replace("ret i32 %after", "ret i32 1140047872").replace("store float 0.0", "store float 0.000000e+00").replace("48.75", "4.875000e+01"), &[&[]]);
}

#[test]
fn test_a_float_cell_is_kept_across_a_call_that_cannot_write_it() {
    use crate::floatfold::tests::{ACROSS_A_CALL, summarized};
    let mut module = parsed(&format!("{DOS}{ACROSS_A_CALL}"));
    let before = results(&module, &[&[]]);
    assert!(summarized(&mut module, Fold).contains("ret float 3.000000e+00"));
    assert_eq!(results(&module, &[&[]]), before);
}

/// FPEMU's `X& \ 1024` of a known X&: algebraic expands the division to
/// shifts, and Fold folds them, `ashr` among them. It stayed: consts had
/// no `ashr`.
#[test]
fn a_constant_division_expanded_to_shifts_folds() {
    for (dividend, quotient) in [(1073741831, 1048576), (-1073741831, -1048576)] {
        let mut module = parsed(&format!("define i32 @f(i32 %x) {{\nb0:\n  %q = sdiv i32 {dividend}, 1024\n  ret i32 %q\n}}\n"));
        let before = results(&module, &[&[0]]);
        managed(&mut module, crate::algebraic::Algebraic);
        let expanded = managed(&mut module, Fold);
        assert!(expanded.contains(&format!("ret i32 {quotient}")), "{expanded}");
        assert_eq!(results(&module, &[&[0]]), before);
    }
}

/// `icmp ne ptr null, null` survived the folder (only integers were known),
/// so a drop of a null vec kept its guard and the dead code it guarded.
#[test]
fn comparing_constant_pointers_folds() {
    let text = "define i16 @f() {\nb0:\n  %a = icmp ne ptr null, null\n  %b = icmp eq ptr inttoptr (i16 1132 to ptr), inttoptr (i16 1132 to ptr)\n  %c = icmp ne ptr inttoptr (i16 1132 to ptr), null\n  %x = zext i1 %a to i16\n  %y = zext i1 %b to i16\n  %z = zext i1 %c to i16\n  %s = add i16 %x, %y\n  %t = add i16 %s, %z\n  ret i16 %t\n}\n";
    assert!(text.contains("icmp ne ptr null, null"), "the shape that survived");
    let mut module = parsed(text);
    assert!(fold(&mut module));
    assert_eq!(results(&module, &[&[]]), results(&parsed(text), &[&[]]));
    let after = printed(&module);
    for folded in ["%x = zext i1 false to i16", "%y = zext i1 true to i16", "%z = zext i1 true to i16"] {
        assert!(after.contains(folded), "{after}");
    }
}

/// `fold` asked what is known of the body through memory for its integers and again, in `floatfold`, for the float solve
/// under it: two derivations of one fact (#560). One serves both where nothing was changed between them.
#[test]
fn what_is_known_through_memory_is_derived_once_for_the_integers_and_the_floats() {
    let mut module = parsed(&format!("{DOS}define float @f() {{\nb0:\n  %p = alloca float\n  store float 1.500000e+00, ptr %p\n  %v = load float, ptr %p\n  %w = fadd float %v, 2.000000e+00\n  ret float %w\n}}\n"));
    let before = llrm_analysis::consts::memory_derivations();
    assert!(fold(&mut module), "the floats are folded");
    assert!(llrm_analysis::consts::memory_derivations() - before <= 1, "{} derivations", llrm_analysis::consts::memory_derivations() - before);
}
