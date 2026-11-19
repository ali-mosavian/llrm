//! `performOrXorChainCombine`: `(a ^ b) | (c ^ d) == 0` is `cmp; ccmp; cset` in
//! the pin, three instructions where the xors and the `orr` made five
//! (the -Os gap to the pin, 28 B in the corpus).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const SOURCE: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i1 @f(i64 %a, i64 %b, i64 %c, i64 %d) {\n\
  %x = xor i64 %a, %b\n  %y = xor i64 %c, %d\n  %o = or i64 %x, %y\n\
  %r = icmp eq i64 %o, 0\n  ret i1 %r\n}\n";

fn asm(level: Level) -> String {
    driver::assembly(SOURCE, Platform::Linux, level).expect("lowers")
}

#[test]
fn an_or_of_xors_compared_with_zero_is_a_ccmp_chain_at_os() {
    let text = asm(Level::Os);
    assert!(text.contains("ccmp"), "{text}");
    assert!(!text.contains("eor"), "{text}");
}

#[test]
fn an_or_of_xors_compared_with_zero_is_a_ccmp_chain_at_o2() {
    let text = asm(Level::O2);
    assert!(text.contains("ccmp"), "{text}");
    assert!(!text.contains("eor"), "{text}");
}
