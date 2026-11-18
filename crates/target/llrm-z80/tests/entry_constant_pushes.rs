//! A constant stored to a whole word of the frame at entry is a push of it.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LAYOUT: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n";

fn compiled(
    ir: &str,
    level: Level,
) -> String {
    text(&format!("{LAYOUT}{ir}"), "run", 0, &Request::new(level)).expect("compiles")
}

/// inlined_tail_call_keeps_frame_stores `run` stored its local's second word by `ld -2(ix), #2; ld -1(ix), #0`
/// (8 B, 38 T) after `push af; push af`: 60 B against SDCC's 59. The word is the push: `ld hl, #2; push hl` (4 B).
#[test]
fn a_constant_stored_to_a_frame_word_at_entry_is_a_push() {
    let ir = "declare void @set(ptr)
define i16 @run() {
  %q = alloca [2 x i16]
  %p1 = getelementptr i16, ptr %q, i16 1
  store i16 2, ptr %p1
  call void @set(ptr %q)
  %v = load i16, ptr %q
  ret i16 %v
}
";
    for level in [Level::O1, Level::Os, Level::Oz] {
        let out = compiled(ir, level);
        assert!(out.contains("ld hl, #2") && out.contains("push hl"), "{level:?}:\n{out}");
        assert!(!out.contains("), #2") && !out.contains("(ix), #0"), "{level:?}:\n{out}");
    }
}

/// With an argument held in every pair at entry no pair is free to carry the constant: the store stays two bytes
/// of the frame (a push through `hl` would have clobbered the first argument).
#[test]
fn no_pair_is_loaded_while_every_pair_holds_an_argument() {
    let ir = "define i16 @run(i16 %a, i16 %b, i16 %c) {
  %q = alloca [2 x i16]
  %p1 = getelementptr i16, ptr %q, i16 1
  store volatile i16 7, ptr %p1
  %x = add i16 %a, %b
  %y = add i16 %x, %c
  store volatile i16 %y, ptr %q
  ret i16 %y
}
";
    let out = compiled(ir, Level::Os);
    assert!(!out.contains("ld hl, #7") && !out.contains("ld de, #7") && !out.contains("ld bc, #7"), "{out}");
}
