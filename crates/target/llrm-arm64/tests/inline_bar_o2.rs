//! The -O2 inline bar is the target's own, not the pin's 225.

use llrm_arm64::tti::AArch64;
use llrm_mir::tti::Tti;

/// QB text at -O2 was 30,104 B over the best of gcc and clang with the pin's
/// bar: the generic profile states a lower one for -O2 only.
#[test]
fn the_generic_profile_states_an_inline_bar_at_o2_only() {
    let generic = AArch64::of_cpu("generic").unwrap();
    assert_eq!([1, 2, 3].map(|level| generic.inline_threshold_at_level(level)), [None, Some(160), None]);
    let m1 = AArch64::of_cpu("apple-m1").unwrap();
    assert_eq!([1, 2, 3].map(|level| m1.inline_threshold_at_level(level)), [None, None, None]);
}

const CALLER: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
define i32 @callee(i32 %a, i32 %b) {
  %x = mul i32 %a, %b
  %y = add i32 %x, %a
  ret i32 %y
}
define i32 @caller(i32 %a, i32 %b) {
  %r = call i32 @callee(i32 %a, i32 %b)
  %s = add i32 %r, %b
  ret i32 %s
}
";

/// The decision reads it: a call is priced against a lower threshold at -O2
/// than at -O1 (equal while the pin's bar serves both). The threshold printed
/// carries the pin's site bonuses, so the two are compared.
#[test]
fn the_inliner_judges_a_call_at_o2_against_the_stated_bar() {
    let bar = |level: &str| {
        let dir = tempfile::tempdir().unwrap();
        let ir = dir.path().join("c.ll");
        std::fs::write(&ir, CALLER).unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
            .env("LLRM_INLINE_REMARKS", "1")
            .arg(level)
            .arg("-o")
            .arg(dir.path().join("o.o"))
            .arg(&ir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    let threshold = |log: &str| -> i64 {
        let tail = log.split("threshold=").nth(1).unwrap_or_else(|| panic!("no threshold: {log}"));
        tail.split(')').next().unwrap().parse().unwrap()
    };
    let (o1, o2) = (threshold(&bar("-O1")), threshold(&bar("-O2")));
    assert!(o2 < o1, "-O1 {o1}, -O2 {o2}");
}
