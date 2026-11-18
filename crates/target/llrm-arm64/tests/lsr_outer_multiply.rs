//! DEVIATE LSR-OUTER on arm64: the pin's LSR skips a loop with a subloop, gcc's ivopts does not.

use llrm_arm64::tti::AArch64;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

/// An outer loop that feeds `i * MULTIPLIER + 3` to an inner loop: recgcd's `gcd(i * 7 + 3, i * 5 + 2)`.
fn outer_loop(multiplier: u32) -> String {
    format!(
        "target datalayout = \"e-m:e-i64:64-i128:128-n32:64-S128\"\n\
define i32 @k(i32 %n) {{\n\
entry:\n  br label %outer\n\
outer:\n  %i = phi i32 [ 1, %entry ], [ %i1, %latch ]\n  %acc = phi i32 [ 0, %entry ], [ %acc1, %latch ]\n\
  %m = mul nuw nsw i32 %i, {multiplier}\n  %z0 = add nuw nsw i32 %m, 3\n  br label %inner\n\
inner:\n  %k = phi i32 [ 0, %outer ], [ %k1, %inner ]\n  %z = phi i32 [ %z0, %outer ], [ %z1, %inner ]\n\
  %t = xor i32 %z, %k\n  %z1 = add i32 %t, 7\n  %k1 = add nuw nsw i32 %k, 1\n  %c = icmp eq i32 %k1, 4\n  br i1 %c, label %latch, label %inner\n\
latch:\n  %acc1 = add i32 %acc, %z1\n  %i1 = add nuw nsw i32 %i, 1\n  %e = icmp eq i32 %i1, %n\n  br i1 %e, label %exit, label %outer\n\
exit:\n  ret i32 %acc1\n}}\n"
    )
}

fn reduced(ir: &str) -> String {
    let mut module = llrm_mir::parse::module(ir).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("datalayout");
    let tti = AArch64::of_cpu("generic").unwrap();
    let outer = Outer::of(&module, None);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("k").expect("a function");
    llrm_transforms::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    llrm_mir::print::module(&module)
}

/// The outer loop's text, from its header to the inner loop's.
fn outer_part(text: &str) -> &str {
    let start = text.find("outer:").expect("the outer loop");
    &text[start..text[start..].find("inner:").map_or(text.len(), |at| start + at)]
}

/// recgcd -O2/-O3 kept `7 * i + 3` (a `lsl` and a `sub`, then an `add`) in its outer loop, where gcc
/// keeps counters: 17611 instructions against gcc's 17314. A multiply that is two instructions (`2^N -
/// 1`) is a counter stepped by the multiplier. One that is a single instruction (`i * 5`, a shifted add;
/// `i * 24`, a `madd`) is not worth a register held across the inner loop: mandel O2/O3 took one extra
/// `add` in its innermost loop (134739 -> 142026 instructions, over gcc's 134956) when `24 * px - 128`
/// became a counter.
#[test]
fn an_outer_loops_multiply_of_two_instructions_or_more_is_a_counter() {
    for multiplier in [7u32, 15, 31] {
        let text = reduced(&outer_loop(multiplier));
        assert!(!outer_part(&text).contains(" mul "), "x{multiplier} stays a multiply:\n{text}");
    }
    for multiplier in [5u32, 8, 9, 24, 300] {
        let text = reduced(&outer_loop(multiplier));
        assert!(outer_part(&text).contains(" mul "), "x{multiplier} stays one instruction:\n{text}");
    }
}
