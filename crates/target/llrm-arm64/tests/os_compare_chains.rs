//! -Os compare chains the pin makes with ccmp/csinv (R1, R2; owner arm64,
//! exit 2026-11-20).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn asm(source: &str) -> String {
    driver::assembly(source, Platform::Linux, Level::Os).expect("lowers")
}

/// R1: `(a ^ b) | (c ^ d) == 0` is cmp/ccmp/cset in the pin
/// (`performOrXorChainCombine` feeding `emitConjunction`), not eor/eor/orr/cmp.
#[test]
#[ignore = "until the combine lands"]
fn an_or_of_xors_compared_with_zero_is_a_ccmp_chain() {
    let text = asm(
        "define i1 @f(i64 %a, i64 %b, i64 %c, i64 %d) {\n\
         %x = xor i64 %a, %b\n %y = xor i64 %c, %d\n %o = or i64 %x, %y\n\
         %r = icmp eq i64 %o, 0\n ret i1 %r\n}\n",
    );
    assert!(text.contains("ccmp"), "{text}");
    assert!(!text.contains("eor"), "{text}");
}

/// R2: a three-way compare (`a < b ? -1 : a != b`) is cmp/cset/csinv.
#[test]
#[ignore = "until the combine lands"]
fn a_three_way_compare_ends_in_csinv() {
    let text = asm(
        "define i32 @f(i32 %a, i32 %b) {\n\
         %lt = icmp slt i32 %a, %b\n %ne = icmp ne i32 %a, %b\n\
         %z = zext i1 %ne to i32\n %r = select i1 %lt, i32 -1, i32 %z\n ret i32 %r\n}\n",
    );
    assert!(text.contains("csinv"), "{text}");
}
