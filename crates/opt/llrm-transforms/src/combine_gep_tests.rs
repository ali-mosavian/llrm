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

/// The body of `@f(ptr %b, i64 %i, i64 %j)` after `visitGetElementPtrInst`
/// alone, through the driver.
fn combined(body: &str) -> String {
    let text = format!("define ptr @f(ptr %b, i64 %i, i64 %j) {{\nb0:\n{body}  ret ptr %p\n}}\n");
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
