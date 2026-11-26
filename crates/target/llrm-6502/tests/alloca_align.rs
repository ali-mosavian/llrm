//! A local aligned past what the 6502 keeps realigns the stack. It was refused
//! by name; before that it got an object at a stack address with no such
//! alignment, and the mid-end folded `ptr & 63` to 0 from the alloca's `align
//! 64`.

use llrm_6502::driver::{Level, Request, assembly};

const OVER_ALIGNED: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8\"

declare void @use(ptr)

define i16 @f() {
entry:
  %a = alloca [64 x i8], align 64
  call void @use(ptr %a)
  %i = ptrtoint ptr %a to i16
  %m = and i16 %i, 63
  ret i16 %m
}
";

#[test]
fn a_local_aligned_past_what_the_stack_keeps_realigns_it_at_every_level() {
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let text = assembly(OVER_ALIGNED, &Request::new(level)).unwrap_or_else(|error| panic!("{level:?}: {error}"));
        assert!(text.contains("and #192"), "{level:?}: no rounding of rs0 to 64:\n{text}");
    }
}

#[test]
fn a_local_at_the_alignment_the_c_abi_states_still_compiles() {
    let text = OVER_ALIGNED.replace("[64 x i8], align 64", "i16, align 2");
    assert!(assembly(&text, &Request::new(Level::O2)).is_ok());
}
