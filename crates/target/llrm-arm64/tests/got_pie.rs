//! At Linux PIE an extern global is read through the GOT, as the pin's
//! `adrp :got:` + `ldr :got_lo12:` + `ldr`; static code reads it directly.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const BODY: &str = r#"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@ext = external global i32
define i32 @f() {
  %v = load i32, ptr @ext
  ret i32 %v
}
"#;
const PIE: &str = "!llvm.module.flags = !{!0, !1}\n!0 = !{i32 8, !\"PIC Level\", i32 2}\n!1 = !{i32 7, !\"PIE Level\", i32 2}\n";

const OTHER_FLAGS: &str = "!llvm.module.flags = !{!0}\n!0 = !{i32 7, !\"uwtable\", i32 2}\n";

fn loads(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("ldr")).count()
}

/// release/next: `adrp x9, ext@PAGE; ldr w0, [x9, ext@PAGEOFF]` at -fPIE, one
/// load where the pin has three (the GOT entry, then the value).
#[test]
fn an_extern_global_is_loaded_through_the_got_at_linux_pie() {
    let pie = driver::assembly(&format!("{BODY}{PIE}"), Platform::Linux, Level::O2).expect("lowers");
    assert_eq!(loads(&pie), 2, "{pie}");
    // Static: other flags, or none at all (the frontends' modules).
    for tail in [OTHER_FLAGS, ""] {
        let fixed = driver::assembly(&format!("{BODY}{tail}"), Platform::Linux, Level::O2).expect("lowers");
        assert_eq!(loads(&fixed), 1, "{fixed}");
    }
    assert!(pie.to_lowercase().contains("got"), "{pie}");
}
