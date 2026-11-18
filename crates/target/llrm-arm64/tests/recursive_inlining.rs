//! gcc's recursive inlining (REC-1) of a recursion with a loop body: the calls
//! were less than 3/5 of the body's clocks and the body past 40 `gcc_insns`, so
//! `chop` (recchop) was never copied into itself: 3,654 instructions at -O3
//! against gcc's 2,768. Copied, it runs 2,867.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn chop_calls(level: Level) -> usize {
    let asm = driver::assembly(include_str!("data/recursive_loop_body.ll"), Platform::Linux, level).expect("lowers");
    let mut lines = asm.lines().skip_while(|line| *line != "chop:").skip(1);
    let body = lines.by_ref().take_while(|line| line.starts_with('\t') || line.starts_with(".L"));
    body.filter(|line| line.trim_start().starts_with("bl ") && line.contains("chop")).count()
}

#[test]
fn a_recursion_with_a_loop_body_is_copied_into_itself_at_speed() {
    for level in [Level::O2, Level::O3] {
        let calls = chop_calls(level);
        assert!(calls >= 4, "{level:?}: `chop` makes {calls} calls of itself, so it was not copied into itself");
    }
}

#[test]
fn a_recursion_is_not_copied_at_os() {
    let calls = chop_calls(Level::Os);
    assert!(calls <= 2, "-Os: `chop` makes {calls} calls of itself");
}
