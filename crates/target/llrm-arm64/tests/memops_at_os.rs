//! A fill or copy expands in line within the pin's limit for the function:
//! 8 `q` stores for a fill and 4 for a copy under -Os, 32 and 16 otherwise.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn ir(
    name: &str,
    bytes: u32,
) -> String {
    let body = match name {
        "memset" => format!("call void @llvm.memset.p0.i64(ptr align 8 %p, i8 0, i64 {bytes}, i1 false)"),
        _ => format!("call void @llvm.{name}.p0.p0.i64(ptr align 8 %p, ptr align 8 %q, i64 {bytes}, i1 false)"),
    };
    format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         define void @f(ptr %p, ptr %q) {{\n  {body}\n  ret void\n}}\n\
         declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
         declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
         declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n"
    )
}

fn calls(
    name: &str,
    bytes: u32,
    level: Level,
) -> bool {
    let asm = driver::assembly(&ir(name, bytes), Platform::Linux, level).expect("lowers");
    asm.lines().any(|line| {
        let line = line.trim();
        (line.starts_with("bl ") || line.starts_with("b ")) && line.contains(name)
    })
}

/// QuickrBasic symb_vars_init -Os: a 512-byte fill became 16 `stp q` (+56 B a
/// function against the pin's `b memset`); symbols.o +128 B. Both sides of the
/// pin's -Os limit, and -O2 where the fill stays inline.
#[test]
fn a_fill_over_eight_q_stores_is_a_call_at_os() {
    assert!(!calls("memset", 128, Level::Os), "8 stores");
    assert!(calls("memset", 129, Level::Os), "9 stores");
    assert!(calls("memset", 512, Level::Os));
    assert!(!calls("memset", 512, Level::O2), "32 stores");
    assert!(calls("memset", 576, Level::O2));
}

#[test]
fn a_copy_over_four_q_accesses_is_a_call_at_os() {
    for name in ["memcpy", "memmove"] {
        assert!(!calls(name, 64, Level::Os), "{name}: 4 accesses");
        assert!(calls(name, 72, Level::Os), "{name}: 5 accesses");
        assert!(!calls(name, 256, Level::O2), "{name}: 16 accesses");
        assert!(calls(name, 288, Level::O2), "{name}: 17 accesses");
    }
}
