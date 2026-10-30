use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with, visit};
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\ndeclare void @use(i1)\n";

/// `@f(i16 %x, i16 %a, i16 %b)` after the whole InstCombine visit.
fn run(body: &str) -> String {
    let mut module = parsed(&format!("{HEAD}define i16 @f(i16 %x, i16 %a, i16 %b) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// crc16 on the 6502 grew 90 -> 92 bytes and 6081 -> 6497 cycles: the select
/// on `x > -1` stayed, where the pin tests `x < 0` and swaps the arms.
#[test]
fn a_select_on_greater_than_minus_one_tests_the_sign_with_swapped_arms() {
    let out = run(
        "  %c = icmp sgt i16 %x, -1\n  %s = shl i16 %x, 1\n  %k = xor i16 %s, 4129\n  %r = select i1 %c, i16 %s, i16 %k\n  ret i16 %r\n",
    );
    assert!(out.contains("icmp slt i16 %x, 0") && !out.contains("icmp sgt"), "{out}");
    assert!(out.contains("select i1 %") && out.contains(", i16 %k, i16 %s"), "{out}");
}

/// Control: a constant arm stays (the pin's own exclusion).
#[test]
fn a_constant_arm_keeps_the_compare() {
    let out = run("  %c = icmp sgt i16 %x, -1\n  %r = select i1 %c, i16 %a, i16 7\n  ret i16 %r\n");
    assert!(out.contains("icmp sgt i16 %x, -1") && out.contains("select i1 %c, i16 %a, i16 7"), "{out}");
}

/// Control: a compare with another user stays.
#[test]
fn a_compare_with_two_users_stays() {
    let out =
        run("  %c = icmp sgt i16 %x, -1\n  call void @use(i1 %c)\n  %r = select i1 %c, i16 %a, i16 %b\n  ret i16 %r\n");
    assert!(out.contains("icmp sgt i16 %x, -1") && out.contains("select i1 %c, i16 %a, i16 %b"), "{out}");
}
