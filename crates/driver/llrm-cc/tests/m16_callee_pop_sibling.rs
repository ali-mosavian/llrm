//! m16 -Os: a near function that ends in a call with its own arguments keeps
//! the caller cleaning, so that call stays a jump. As callee-pop its `ret 4`
//! forced the call, its pushes and its cleanup (`twice`, 200 to 205 B in
//! far_pointer_call). Needs the m16 clang (`LLRM_CLANG_M16`) and the Open
//! Watcom headers (tools/ow-fetch.sh).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const TWICE: &str = "extern void report(long v);
__attribute__((noinline)) static void __far twice(long v) { report(v); report(v); }
int main(void)
{
    twice(3);
    twice(4);
    return 0;
}
";

#[test]
fn a_function_that_ends_in_a_call_with_its_own_arguments_keeps_the_jump_at_os() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_CLANG_M16 is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("m16sibling-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("twice.c"), TWICE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S", "-o"])
        .arg(dir.join("twice.s"))
        .arg(dir.join("twice.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("twice.s")).unwrap();
    assert!(
        text.lines().any(|line| line.trim().starts_with("jmp") && line.contains("_report")) && !text.contains("ret 4"),
        "twice pops its argument and calls report where it could jump:\n{text}"
    );
}
