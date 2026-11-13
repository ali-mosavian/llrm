//! A compare whose flags are read after a call is made again after it: a call
//! clobbers NZCV. The scheduler merged `icmp eq %n, 12` with `add %n, -12`
//! (one `subs`), put the call between that and the branch, and the branch read
//! the flags the callee left: fib -O2 (self-copies) printed 5148 for 6765, and
//! the same wrong value came from release/next's own llc.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const SOURCE: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
declare i32 @g(i32)\n\
    define i32 @f(i32 %n) {\n\
      %m = add i32 %n, -12\n\
      %r = call i32 @g(i32 %m)\n\
      %c = icmp eq i32 %n, 12\n\
      br i1 %c, label %a, label %b\n\
    a:\n  %x = add i32 %r, 1\n  ret i32 %x\n\
    b:\n  %y = add i32 %r, 2\n  ret i32 %y\n}\n";

/// The first line that reads the condition flags with none set since the last
/// call.
fn stale_read(level: Level) -> Option<String> {
    let asm = driver::assembly(SOURCE, Platform::Linux, level).expect("lowers");
    let sets = ["subs ", "cmp ", "cmn ", "ands ", "tst ", "adds ", "ccmp ", "ccmn "];
    let reads = ["b.", "csel ", "csinc ", "csinv ", "csneg ", "cset ", "cinc ", "cinv ", "cneg "];
    let mut live = false;
    for line in asm.lines().map(str::trim) {
        if line.starts_with("bl ") {
            live = false;
        } else if sets.iter().any(|one| line.starts_with(one)) {
            live = true;
        } else if reads.iter().any(|one| line.starts_with(one)) && !live {
            return Some(line.to_owned());
        }
    }
    None
}

#[test]
fn flags_are_not_read_after_a_call_that_clobbers_them_at_o0() {
    assert_eq!(stale_read(Level::O0), None);
}

#[test]
fn flags_are_not_read_after_a_call_that_clobbers_them_at_o2() {
    assert_eq!(stale_read(Level::O2), None);
}
