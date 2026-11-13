//! Borland's `<stdarg.h>` takes `&parmN`; clang keeps that parameter in a local copy, so `va_arg` read the stack below the arguments
//! (sum(3, 1, 2, 3) summed garbage at -O0). The bcc row's shim uses the builtins, and `ap` starts at the first variadic word.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

#[test]
fn va_start_points_at_the_first_variadic_argument_not_a_local_copy_of_the_last_named() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || std::env::var_os("LLRM_BORLAND_INC").is_none() {
        eprintln!("skipped: needs LLRM_CLANG_M16 and LLRM_BORLAND_INC");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-bcc")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-S", "-O0", "-o", "/dev/stdout"])
        .arg(root.join("tests/cc/stdarg/sum_bcc.c"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    // far return address (4) + saved bp (2) + n (2): the first `...` word is [bp+8]
    assert!(text.contains("lea ax, [bp+8]"), "{text}{}", String::from_utf8_lossy(&out.stderr));
}
