use crate::valuetracking::sign_bits;
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
