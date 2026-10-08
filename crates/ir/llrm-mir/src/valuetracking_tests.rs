use crate::valuetracking::{alignment, sign_bits};
use crate::{parse, GlobalKind};

/// The sign bits of what `@f` returns.
fn returned(body: &str) -> u32 {
    let module = parse::module(&format!("define i64 @f(i32 %a, i32 %b, i64 %w) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    sign_bits(&module.context, function, function.instruction(ret).operands[0])
}

/// A 32x32 product in 64 bits, then its fixed-point forms: what isel asks
/// before it multiplies or divides in one 32-bit instruction.
#[test]
fn test_sign_bits_follow_extension_shifts_and_products() {
    assert_eq!(returned("  %x = sext i32 %a to i64\n  ret i64 %x"), 33);
    assert_eq!(returned("  %x = zext i32 %a to i64\n  ret i64 %x"), 32);
    assert_eq!(returned("  %x = sext i32 %a to i64\n  %y = shl i64 %x, 16\n  ret i64 %y"), 17);
    assert_eq!(returned("  %x = sext i32 %a to i64\n  %y = ashr i64 %x, 4\n  ret i64 %y"), 37);
    assert_eq!(returned("  %x = sext i32 %a to i64\n  %y = sext i32 %b to i64\n  %z = mul i64 %x, %y\n  ret i64 %z"), 1);
    assert_eq!(returned("  %x = ashr i64 %w, 48\n  %y = ashr i64 %w, 40\n  %z = mul i64 %x, %y\n  ret i64 %z"), 25);
    assert_eq!(returned("  ret i64 -1"), 64);
    assert_eq!(returned("  ret i64 65535"), 48);
    assert_eq!(returned("  ret i64 %w"), 1);
}

/// The alignment of what `@f` returns, in a module with a word-aligned
/// `@w` and an unaligned `@b`.
fn aligned(body: &str) -> u64 {
    let module = parse::module(&format!("@w = global [100 x i8] zeroinitializer, align 2\n@b = global [100 x i8] zeroinitializer\ndefine ptr @f(i16 %i) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    let layout = crate::datalayout::DataLayout::default();
    alignment(&module.context, &layout, &module.globals, function, function.instruction(ret).operands[0])
}

/// An element of a word-aligned word array is word aligned wherever its
/// index is: what lets hoist show a word read cannot cross offset FFFFh.
#[test]
fn test_alignment_is_the_object_s_less_what_each_index_may_add() {
    assert_eq!(aligned("  %p = getelementptr inbounds i16, ptr @w, i16 %i\n  ret ptr %p"), 2);
    assert_eq!(aligned("  %d = mul i16 %i, 2\n  %p = getelementptr i8, ptr @w, i16 %d\n  %q = getelementptr i8, ptr %p, i16 1280\n  ret ptr %q"), 2);
    assert_eq!(aligned("  %p = getelementptr i8, ptr @w, i16 %i\n  ret ptr %p"), 1);
    assert_eq!(aligned("  %p = getelementptr inbounds i16, ptr @w, i16 %i\n  %q = getelementptr i8, ptr %p, i16 1\n  ret ptr %q"), 1);
    assert_eq!(aligned("  %p = getelementptr inbounds i32, ptr @b, i16 %i\n  ret ptr %p"), 1);
}

/// The bits `@f` returns that are proven zero, as a 64-bit mask.
fn zeros(body: &str) -> u128 {
    let module = parse::module(&format!("define i64 @f(i32 %a, i32 %b, i64 %w) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    crate::valuetracking::known_zero(&module.context, function, function.instruction(ret).operands[0])
}

#[test]
fn test_known_zero_bits_follow_masks_extensions_and_shifts() {
    let high = |low: u32| u128::from(u64::MAX) & !((1_u128 << low) - 1);
    assert_eq!(zeros("  %m = and i32 %a, 255\n  %x = zext i32 %m to i64\n  ret i64 %x"), high(8));
    assert_eq!(zeros("  %m = and i64 %w, 255\n  %t = trunc i64 %m to i8\n  %x = zext i8 %t to i64\n  ret i64 %x"), high(8));
    assert_eq!(zeros("  %m = and i64 %w, 255\n  %x = lshr i64 %m, 4\n  ret i64 %x"), high(4));
    assert_eq!(zeros("  %x = shl i64 %w, 3\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = and i64 %w, 15\n  %n = and i64 %w, 3\n  %x = or i64 %m, %n\n  ret i64 %x"), high(4));
    assert_eq!(zeros("  ret i64 %w"), 0);
}

/// A scale by 2 or 4 clears the low bits, and a sum of two such multiples keeps the least: tile's
/// `(x + 7) * 2 & 126` kept its mask because `mul` and `add` proved no bit.
#[test]
fn test_known_zero_low_bits_follow_products_and_sums() {
    assert_eq!(zeros("  %x = mul i64 %w, 8\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %n = mul i64 %w, 2\n  %x = mul i64 %m, %n\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %n = mul i64 %w, 16\n  %x = add i64 %m, %n\n  ret i64 %x"), 3);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %x = sub i64 %m, 1\n  ret i64 %x"), 0);
    assert_eq!(zeros("  %x = mul i64 %w, 3\n  ret i64 %x"), 0);
}
