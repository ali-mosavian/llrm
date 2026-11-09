//! A frame's stack-pointer update folds into the first save only where the
//! access form can encode it.

use llrm_arm64::driver::{self, IselMode, Level};
use llrm_arm64::machine::Platform;

fn frame_of(bytes: u32) -> String {
    frame_with(bytes, "")
}

fn frame_with(
    bytes: u32,
    attributes: &str,
) -> String {
    format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
target triple = \"aarch64-unknown-linux-gnu\"\n\
declare void @g(ptr)\n\
define void @f() #0 {{\n  %a = alloca [{bytes} x i8], align 16\n  call void @g(ptr %a)\n  ret void\n}}\nattributes #0 = {{ {attributes} }}\n"
    )
}

/// `f` saves x30 alone: `str x30, [sp, #-N]!` and `ldr x30, [sp], #N` take
/// -256..255. A frame of 256..512 bytes folded the update anyway and the
/// object failed ("a pre-index offset -272 does not fit 9 bits", "a post-index
/// offset 256 ..."); the pin (`convertCalleeSaveRestoreToSPPrePostIncDec`)
/// makes the update its own `sub`/`add` when the offset is out of range.
#[test]
fn a_frame_larger_than_the_single_access_update_range_is_encoded() {
    driver::set_isel_mode(IselMode::Dag);
    for bytes in (200..=520).step_by(8) {
        for level in [Level::O1, Level::Os] {
            driver::object(&frame_of(bytes), Platform::Linux, "t", level)
                .unwrap_or_else(|why| panic!("{bytes} bytes at {level:?}: {why}"));
        }
    }
}

/// Where it does not fit, the update is a separate adjustment, not a folded
/// one: the save is at the stack pointer.
#[test]
fn an_out_of_range_update_is_a_separate_adjustment() {
    driver::set_isel_mode(IselMode::Dag);
    let text = driver::assembly(&frame_of(272), Platform::Linux, true).expect("lowers");
    assert!(text.contains("sub\tsp, sp, #") || text.contains("sub sp, sp, #"), "{text}");
    assert!(!text.contains("]!"), "a folded update that cannot be encoded:\n{text}");
}

/// Two programs of the corpus whose first save is a single register at a
/// frame past the range: a leaf saving `x29` (`str x29, [sp, #-368]!`) and
/// a function saving `x30` (`str x30, [sp, #-272]!`).
#[test]
fn the_corpus_frames_with_a_single_first_save_are_encoded() {
    driver::set_isel_mode(IselMode::Dag);
    for (name, ir) in [
        ("frame_pointer_free", include_str!("data/frame_pointer_free.ll")),
        ("store_merge_interleaved", include_str!("data/store_merge_interleaved.ll")),
    ] {
        for level in [Level::O1, Level::Os, Level::Oz] {
            driver::object(ir, Platform::Linux, "t", level).unwrap_or_else(|why| panic!("{name} at {level:?}: {why}"));
        }
    }
}
