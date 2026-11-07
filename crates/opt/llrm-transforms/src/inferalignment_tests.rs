use llrm_mir::datalayout::DataLayout;

use super::infer_alignment;
use crate::testing::{parsed, printed};

const DATA: &str = "@a = global [100 x i8] zeroinitializer, align 4\n@b = global [100 x i8] zeroinitializer, align 4\n\
                    declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n";

/// @f of `DATA` and `body` after InferAlignment, and whether anything changed.
fn inferred(body: &str) -> (String, bool) {
    let mut module = parsed(&format!("{DATA}define i32 @f(i32 %n, ptr %p) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let layout = DataLayout::parse("e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32").unwrap();
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = infer_alignment(context, &globals, &metadata, &layout, function);
    (printed(&module), changed)
}

/// `memcpy` of two `align 4` globals kept align 1 on both arguments, so the
/// backend copied them in bytes (`opt` gives `align 4` on each).
#[test]
fn a_memcpy_of_aligned_globals_gets_their_alignment() {
    let (text, changed) =
        inferred("  call void @llvm.memcpy.p0.p0.i32(ptr @b, ptr @a, i32 100, i1 false)\n  ret i32 0\n");
    assert!(changed);
    assert!(text.contains("ptr align 4 @b, ptr align 4 @a"), "{text}");
}

/// An argument nothing is known about keeps no attribute.
#[test]
fn a_memcpy_of_unknown_pointers_is_left_alone() {
    let (text, changed) =
        inferred("  call void @llvm.memcpy.p0.p0.i32(ptr %p, ptr @a, i32 100, i1 false)\n  ret i32 0\n");
    assert!(changed);
    assert!(text.contains("ptr %p, ptr align 4 @a"), "{text}");
}

/// A load of `@a+8` through align 1 is a dword load (`opt` agrees).
#[test]
fn a_load_at_an_offset_of_an_aligned_global_gets_the_common_alignment() {
    let (text, _) = inferred(
        "  %q = getelementptr [100 x i8], ptr @a, i32 0, i32 8\n  %v = load i32, ptr %q, align 1\n  ret i32 %v\n",
    );
    assert!(text.contains("load i32, ptr %q, align 4"), "{text}");
    let (text, _) = inferred(
        "  %q = getelementptr [100 x i8], ptr @a, i32 0, i32 6\n  %v = load i32, ptr %q, align 1\n  ret i32 %v\n",
    );
    assert!(text.contains("load i32, ptr %q, align 2"), "{text}");
}

/// A pointer an access of align 8 proved aligned lends that to the second
/// access to it, and to one at a multiple-of-eight offset (`opt` agrees).
#[test]
fn an_access_lends_its_alignment_to_later_ones_on_the_same_base() {
    let (text, _) = inferred(
        "  %a = load i32, ptr %p, align 8\n  %q = getelementptr i8, ptr %p, i32 16\n  %v = load i32, ptr %q, align 1\n  \
         %w = load i32, ptr %p, align 1\n  %s = add i32 %a, %v\n  %t = add i32 %s, %w\n  ret i32 %t\n",
    );
    assert!(text.contains("%v = load i32, ptr %q, align 8"), "{text}");
    assert!(text.contains("%w = load i32, ptr %p, align 8"), "{text}");
}

/// `ptrtoint @a & 3` is 0 and `ptrtoint @a & -4` is the address (`opt`
/// agrees).
#[test]
fn a_mask_of_an_aligned_address_is_decided() {
    let (text, changed) = inferred(
        "  %i = ptrtoint ptr @a to i32\n  %m = and i32 %i, 3\n  %k = and i32 %i, -4\n  %r = add i32 %m, %k\n  ret i32 %r\n",
    );
    assert!(changed);
    assert!(text.contains("add i32 0, %i"), "{text}");
}

/// Compile time: every instruction was copied whole to be looked at for a
/// mask, a truncation or a call (56,940 copies, 48 M Ir on memexp -O2). Plain
/// arithmetic and returns are not copied.
#[test]
fn an_instruction_that_is_not_a_mask_a_trunc_or_a_call_is_not_copied() {
    super::COPIES.with(|copies| copies.set(0));
    inferred("  %a = add i32 %n, 1\n  %b = mul i32 %a, 3\n  ret i32 %b\n");
    assert_eq!(super::COPIES.with(std::cell::Cell::get), 0);
    inferred("  %a = and i32 %n, 7\n  ret i32 %a\n");
    assert_eq!(super::COPIES.with(std::cell::Cell::get), 2, "an `and` is copied once a walk");
}
