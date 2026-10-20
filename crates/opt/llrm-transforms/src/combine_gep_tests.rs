use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitGetElementPtrInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(
        c.function.instruction(inst).opcode,
        Opcode::GetElementPtr { .. }
    ) {
        c.visit_get_element_ptr(inst)
    } else {
        Visit::None
    }
}

/// The body of `@f(ptr %b, i64 %i, i64 %j, i1 %c)` after
/// `visitGetElementPtrInst` alone, through the driver.
fn combined(body: &str) -> String {
    let text = format!("define ptr @f(ptr %b, i64 %i, i64 %j, i1 %c) {{\nb0:\n{body}  ret ptr %p\n}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    let out = printed(&module);
    out.split_once("b0:\n").expect("a block").1.split("  ret ptr").next().expect("a body").to_owned()
}

/// A GEP of constants is a byte offset from its base.
#[test]
fn constant_indices_are_a_byte_offset() {
    assert_eq!(
        combined("  %p = getelementptr [4 x i16], ptr %b, i64 0, i64 3\n"),
        "  %0 = getelementptr i8, ptr %b, i64 6\n"
    );
}

/// A struct field is its layout offset.
#[test]
fn a_struct_field_is_its_offset() {
    assert_eq!(
        combined("  %p = getelementptr { i16, i16 }, ptr %b, i64 0, i32 1\n"),
        "  %0 = getelementptr i8, ptr %b, i64 2\n"
    );
}

/// A zero last index changes nothing and goes; the one that is left scales by
/// the whole array.
#[test]
fn a_trailing_zero_index_is_dropped() {
    assert_eq!(
        combined("  %p = getelementptr [4 x i16], ptr %b, i64 %i, i64 0\n"),
        "  %0 = getelementptr [8 x i8], ptr %b, i64 %i\n"
    );
}

/// A leading zero index steps into the array: the element type is the source.
#[test]
fn a_leading_zero_index_is_dropped() {
    assert_eq!(
        combined("  %p = getelementptr [4 x i16], ptr %b, i64 0, i64 %i\n"),
        "  %0 = getelementptr [2 x i8], ptr %b, i64 %i\n"
    );
}

/// A narrower index is sign-extended to the layout's index type.
#[test]
fn a_narrow_index_is_extended_to_the_index_type() {
    assert_eq!(
        combined("  %n = trunc i64 %i to i16\n  %p = getelementptr i8, ptr %b, i16 %n\n"),
        "  %n = trunc i64 %i to i16\n  %0 = sext i16 %n to i64\n  %p = getelementptr i8, ptr %b, i64 %0\n"
    );
}

/// A wider index is truncated to it; a truncated zero-extension is the source.
#[test]
fn a_wide_index_is_truncated_to_the_index_type() {
    let out = combined("  %w = zext i64 %i to i128\n  %p = getelementptr i8, ptr %b, i128 %w\n");
    assert!(out.contains("getelementptr i8, ptr %b, i64 %i"), "{out}");
}

/// A multiply of the single index by a constant is folded into the byte
/// offset, so the two multiplies meet.
#[test]
fn a_scaled_index_becomes_a_byte_offset() {
    assert_eq!(
        combined("  %m = mul i64 %i, 3\n  %p = getelementptr i16, ptr %b, i64 %m\n"),
        "  %m = mul i64 %i, 3\n  %0 = mul i64 %m, 2\n  %1 = getelementptr i8, ptr %b, i64 %0\n"
    );
}

/// Two non-zero indices: the first goes to its own GEP, the struct field to
/// a byte offset, and the array index to the last.
#[test]
fn two_non_zero_indices_are_split() {
    assert_eq!(
        combined("  %p = getelementptr { [2 x i16], [2 x i16] }, ptr %b, i64 %i, i32 1, i64 %j\n"),
        "  %0 = getelementptr [8 x i8], ptr %b, i64 %i\n  %1 = getelementptr i8, ptr %0, i64 4\n  %2 = getelementptr [2 x i8], ptr %1, i64 %j\n"
    );
}

/// Two GEPs of constants are one offset.
#[test]
fn a_gep_of_a_constant_gep_is_one_offset() {
    assert_eq!(
        combined("  %q = getelementptr i32, ptr %b, i64 2\n  %p = getelementptr i32, ptr %q, i64 3\n"),
        "  %0 = getelementptr i8, ptr %b, i64 20\n"
    );
}

/// Two variable indices whose sum does not simplify stay two GEPs.
#[test]
fn a_gep_of_a_gep_with_a_variable_sum_is_left() {
    assert_eq!(
        combined("  %q = getelementptr i32, ptr %b, i64 %i\n  %p = getelementptr i32, ptr %q, i64 %j\n"),
        "  %q = getelementptr [4 x i8], ptr %b, i64 %i\n  %p = getelementptr [4 x i8], ptr %q, i64 %j\n"
    );
}

/// `gep T, (gep i8, b, C1), (x + C2)` moves `C2 * sizeof(T)` into the byte
/// offset.
#[test]
fn a_constant_added_to_the_index_joins_the_byte_offset() {
    assert_eq!(
        combined(
            "  %q = getelementptr i8, ptr %b, i64 16\n  %x = add i64 %i, 3\n  %p = getelementptr i32, ptr %q, i64 %x\n"
        ),
        "  %0 = getelementptr i8, ptr %b, i64 28\n  %1 = getelementptr [4 x i8], ptr %0, i64 %i\n"
    );
}

/// A constant offset through a select index is a select of offsets.
#[test]
fn a_select_index_after_a_constant_offset_is_a_select_of_offsets() {
    assert_eq!(
        combined(
            "  %q = getelementptr i8, ptr %b, i64 4\n  %s = select i1 %c, i64 1, i64 2\n  %p = getelementptr i8, ptr %q, i64 %s\n"
        ),
        "  %0 = select i1 %c, i64 5, i64 6\n  %1 = getelementptr i8, ptr %b, i64 %0\n"
    );
}

/// Constant offsets on both sides of a variable one are added.
#[test]
fn constant_offsets_across_a_variable_one_are_added() {
    assert_eq!(
        combined(
            "  %a = getelementptr i8, ptr %b, i64 4\n  %x = getelementptr i8, ptr %a, i64 %i\n  %p = getelementptr i8, ptr %x, i64 8\n"
        ),
        "  %x = getelementptr i8, ptr %b, i64 %i\n  %0 = getelementptr i8, ptr %x, i64 12\n"
    );
}

/// `@f` of `text` after `visitGetElementPtrInst` alone.
fn combined_function(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

const GEPS_IN_A_PHI: &str = "define ptr @f(ptr %base, i1 %c, i64 %i, i64 %jj, i64 %k) {
b0:
  br i1 %c, label %a, label %b

a:
  %x = getelementptr i32, ptr %base, i64 %i
  br label %m

b:
  %y = getelementptr i32, ptr %base, i64 %jj
  br label %m

m:
  %p = phi ptr [ %x, %a ], [ %y, %b ]
  %r = getelementptr i32, ptr %p, i64 %k
  ret ptr %r
}
";

/// A GEP of a phi of GEPs that differ in one index: one GEP of a phi of
/// that index.
#[test]
fn geps_in_a_phi_differing_in_one_index_become_a_phi_of_the_index() {
    let out = combined_function(GEPS_IN_A_PHI);
    assert!(out.contains("phi i64 [ %i, %a ], [ %jj, %b ]"), "{out}");
    assert!(out.contains("getelementptr [4 x i8], ptr %base, i64 %"), "{out}");
    assert!(!out.contains("phi ptr"), "{out}");
}

/// The phi has another user: the new phi would not replace it, so nothing.
#[test]
fn a_phi_of_geps_with_another_user_is_left() {
    let text = GEPS_IN_A_PHI.replace("  ret ptr %r", "  store ptr %p, ptr %base\n  ret ptr %r");
    let out = combined_function(&text);
    assert!(out.contains("phi ptr [ %x, %a ], [ %y, %b ]"), "{out}");
}

/// GEPs that differ in two operands are not folded.
#[test]
fn geps_in_a_phi_differing_in_two_operands_are_left() {
    let text = GEPS_IN_A_PHI
        .replace("%y = getelementptr i32, ptr %base, i64 %jj", "%y = getelementptr i32, ptr %k2, i64 %jj")
        .replace("ptr %base, i1 %c", "ptr %base, ptr %k2, i1 %c");
    let out = combined_function(&text);
    assert!(out.contains("phi ptr [ %x, %a ], [ %y, %b ]"), "{out}");
}

fn function_of(
    params: &str,
    ret: &str,
    body: &str,
) -> String {
    combined_function(&format!(
        "target datalayout = \"e-p:64:64-i64:64\"\n@g1 = global [8 x i8] zeroinitializer\n@g2 = global [8 x i8] zeroinitializer\ndefine {ret} @f({params}) {{\nb0:\n{body}}}\n"
    ))
}

/// An exact division by the element size is the byte index.
#[test]
fn an_exact_division_by_the_element_size_is_a_byte_index() {
    for divide in ["udiv exact i64 %i, 4", "sdiv exact i64 %i, 4", "lshr exact i64 %i, 2", "ashr exact i64 %i, 2"] {
        let out = function_of(
            "ptr %b, i64 %i",
            "ptr",
            &format!("  %d = {divide}\n  %g = getelementptr i32, ptr %b, i64 %d\n  ret ptr %g\n"),
        );
        assert!(out.contains("getelementptr i8, ptr %b, i64 %i") && !out.contains("exact"), "{divide}: {out}");
    }
}

/// A division that is not exact is not the index.
#[test]
fn an_inexact_division_is_left() {
    let out = function_of(
        "ptr %b, i64 %i",
        "ptr",
        "  %d = udiv i64 %i, 4\n  %g = getelementptr i32, ptr %b, i64 %d\n  ret ptr %g\n",
    );
    assert!(out.contains("udiv i64 %i, 4"), "{out}");
}

/// A pointer difference added back to its subtrahend is the minuend, for a
/// compare of it.
#[test]
fn a_pointer_difference_added_back_is_the_pointer_for_a_compare() {
    let out = function_of(
        "ptr %p, ptr %q",
        "i1",
        "  %a = ptrtoint ptr %p to i64\n  %b = ptrtoint ptr %q to i64\n  %d = sub i64 %a, %b\n  %g = getelementptr i8, ptr %q, i64 %d\n  %c = icmp eq ptr %g, null\n  ret i1 %c\n",
    );
    assert!(out.contains("icmp eq ptr %p, null"), "{out}");
}

/// `gep p, x + y` is two GEPs.
#[test]
fn an_added_index_is_two_geps() {
    let out = function_of(
        "ptr %b, i64 %x, i64 %y",
        "ptr",
        "  %s = add i64 %x, %y\n  %g = getelementptr i32, ptr %b, i64 %s\n  ret ptr %g\n",
    );
    assert!(out.contains("ptr %b, i64 %x") && out.contains(", i64 %y"), "{out}");
    assert!(!out.contains("add i64"), "{out}");
}

/// A signed remainder by a power of two, under `inbounds nuw`, is unsigned.
#[test]
fn a_signed_remainder_index_under_inbounds_nuw_is_unsigned() {
    let out = function_of(
        "ptr %b, i64 %i",
        "ptr",
        "  %r = srem i64 %i, 8\n  %g = getelementptr inbounds nuw i8, ptr %b, i64 %r\n  ret ptr %g\n",
    );
    assert!(out.contains("urem i64 %i, 8") && !out.contains("srem"), "{out}");
}

/// A constant offset through a select of constants is a select of constants.
#[test]
fn a_gep_of_a_select_of_globals_is_a_select_of_geps() {
    let out = function_of(
        "i1 %c",
        "ptr",
        "  %s = select i1 %c, ptr @g1, ptr @g2\n  %g = getelementptr i8, ptr %s, i64 4\n  ret ptr %g\n",
    );
    assert!(out.contains("select i1 %c,") && !out.contains("%s = select i1 %c, ptr @g1, ptr @g2"), "{out}");
}

/// `nusw` over indices known non-negative is `nuw` too.
#[test]
fn nusw_with_a_non_negative_index_gains_nuw() {
    let out = function_of(
        "ptr %b, i64 %i",
        "ptr",
        "  %n = and i64 %i, 255\n  %g = getelementptr nusw i8, ptr %b, i64 %n\n  ret ptr %g\n",
    );
    assert!(out.contains("getelementptr nusw nuw i8"), "{out}");
}

/// A constant offset inside the object a pointer is dereferenceable for is
/// in bounds.
#[test]
fn an_offset_inside_a_dereferenceable_object_is_inbounds() {
    let out =
        function_of("ptr dereferenceable(16) %b", "ptr", "  %g = getelementptr i8, ptr %b, i64 4\n  ret ptr %g\n");
    assert!(out.contains("getelementptr inbounds"), "{out}");
    let past =
        function_of("ptr dereferenceable(16) %b", "ptr", "  %g = getelementptr i8, ptr %b, i64 17\n  ret ptr %g\n");
    assert!(!past.contains("inbounds"), "{past}");
}
