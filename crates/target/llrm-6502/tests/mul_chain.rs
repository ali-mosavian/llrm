//! A chain of `h * 257 + x` kept one multiply as a `__mulsi3_fast` call: a
//! demanded-bits simplification of the third multiply's operand returned the
//! node itself without queueing it again, so the target never decomposed it
//! (wide_shift_by_one -O2: 215,000 of 406,000 clocks, 2026-11-12).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const CHAIN: &str = "@v = global i32 0\n\
    define i32 @f(i32 %a, i32 %h) {\n\
      %t3 = zext i32 %a to i64\n  %t4 = shl nuw i64 %t3, 32\n  %t5 = load volatile i32, ptr @v\n  %t6 = zext i32 %t5 to i64\n\
      %t7 = or disjoint i64 %t4, %t6\n  %t8 = shl i64 %t7, 1\n  %t9 = lshr i64 %t7, 1\n  %t10 = trunc i64 %t8 to i32\n\
      %t11 = mul i32 %h, 257\n  %t12 = add i32 %t11, %t10\n  %t13 = lshr i64 %t8, 32\n  %t14 = trunc nuw i64 %t13 to i32\n\
      %t15 = mul i32 %t12, 257\n  %t16 = add i32 %t15, %t14\n  %t17 = trunc i64 %t9 to i32\n\
      %t18 = mul i32 %t16, 257\n  %t19 = add i32 %t18, %t17\n  ret i32 %t19\n}\n";

#[test]
fn every_multiply_of_a_chain_by_257_is_a_shift_and_add() {
    let ir = format!("target datalayout = \"{}\"\n{CHAIN}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(!body.contains("__mulsi3"), "{body}");
}
