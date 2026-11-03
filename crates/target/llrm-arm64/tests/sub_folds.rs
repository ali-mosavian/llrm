//! `visitSUB`'s algebraic folds.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

/// `x % 7` was `lsl; sub; sub` after the quotient (the product `q * 7` as
/// `8q - q`, then subtracted); the pin folds `x - (8q - q)` to `x + (q - 8q)`,
/// one `sub` with a shifted operand and an `add` (QB `recmany`: 1 instruction
/// a trip, 39034 against the pin's 37534).
#[test]
fn a_remainder_by_seven_subtracts_the_product_with_a_shifted_operand() {
    let ir = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i64 @f(i64 %x) {
  %r = srem i64 %x, 7
  ret i64 %r
}
"#;
    let asm = driver::assembly(ir, Platform::Linux, true).expect("lowers");
    assert!(asm.lines().any(|line| line.trim_start().starts_with("sub ") && line.contains("lsl #3")), "{asm}");
}

fn lower(body: &str) -> String {
    let ir = format!(
        r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
{body}
"#
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// `a - (a & b)` was `and; sub`: the pin folds it to `a & ~b`, one `bic`.
#[test]
fn a_value_minus_itself_masked_is_a_bic() {
    let asm = lower("define i64 @f(i64 %a, i64 %b) {\n %n = and i64 %a, %b\n %r = sub i64 %a, %n\n ret i64 %r\n}");
    assert!(asm.contains("bic"), "{asm}");
}
