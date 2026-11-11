//! Frames keep 4 and nothing realigns them, but the optimizer took an
//! `alloca ... align 8` at its word: `ptr & 7` folded to 0 on a frame that
//! gives an address ending in 4 (KNOWN-WRONG-ALIGN-M68K), and an object
//! aligned to 16 was laid out misaligned without a word. Up to 8 is clamped
//! to what the frame keeps; past 8 the frame realigns the stack (`realign.rs`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const MASKED: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @use(ptr)
define i32 @f() {
  %a = alloca double, align 8
  call void @use(ptr %a)
  %i = ptrtoint ptr %a to i32
  %m = and i32 %i, 7
  ret i32 %m
}";

#[test]
fn an_object_aligned_past_what_the_frame_keeps_is_not_assumed_aligned() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(MASKED, cpu_of(cpu).unwrap(), level).expect("compiles");
            // clamped to 4, only bit 2 of the address is unknown: `ptr & 7` is `ptr & 4`, not 0
            assert!(out.contains("and.l") && !out.contains("moveq\t#0, %d0"), "{cpu} {level:?}: `ptr & 7` folded:\n{out}");
        }
    }
}
