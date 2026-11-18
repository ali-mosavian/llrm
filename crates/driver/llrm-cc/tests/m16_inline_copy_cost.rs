//! At `-Os` a private callee called three times is not inlined when its copies are bigger than its body
//! (AM-RC-1: QCport `pl_trace.c` `pl_box_axis`, 21 size units a copy of ~70 B, +181 B inlined). The
//! callee that goes with its last call is inlined only where the byte-priced trial (SZ2) says the
//! caller is smaller. Needs the pinned clang (`LLRM_PIN_CLANG`) and the Open Watcom headers.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn assembly() -> Option<String> {
    assembly_of("x86_16-dos", "m16_inline_copy_cost.c", &["-march=i386"])
}

fn assembly_of(
    target: &str,
    source: &str,
    extra: &[&str],
) -> Option<String> {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() || !llrm_cc::ow_present() {
        eprintln!("skipped: LLRM_PIN_CLANG is not set or the Open Watcom tree is missing (tools/ow-fetch.sh)");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inl-{target}-{}-{}", source.replace('/', "_"), std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-Os", "-w", "-S"])
        .args(extra)
        .arg("-o")
        .arg(dir.join("d.s"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    Some(std::fs::read_to_string(dir.join("d.s")).unwrap())
}

fn calls_to(
    asm: &str,
    callee: &str,
) -> usize {
    asm.lines().filter(|line| line.trim().starts_with("call") && line.trim().ends_with(callee)).count()
}

/// Three copies of a float callee with nine arguments were +181 B at `-Os`; the callee stays, called three times.
#[test]
fn a_callee_whose_copies_outgrow_its_body_stays_a_call_at_os() {
    let Some(asm) = assembly() else { return };
    assert_eq!(calls_to(&asm, "_box_axis"), 3, "{asm}");
}

/// The control: a callee that is smaller than the work around its call is still inlined.
#[test]
fn a_callee_smaller_than_its_calls_is_still_inlined_at_os() {
    let Some(asm) = assembly() else { return };
    assert_eq!(calls_to(&asm, "_small_axis"), 0, "{asm}");
}

/// D13 is x86-m16's: at x86-m32 the units are bytes, and a private loop called three times is inlined and
/// its body goes (`memmove_overlap` -Os: 321 B inlined, 350 B when the credit was dropped on every target).
#[test]
fn an_m32_callee_that_goes_with_its_last_call_is_still_inlined_at_os() {
    let Some(asm) = assembly_of("x86_32-linux", "../../../../../tests/run/c/memmove_overlap.c", &[]) else { return };
    assert_eq!(calls_to(&asm, "sum"), 0, "{asm}");
}
