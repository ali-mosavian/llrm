//! DEVIATE OS-INLINE-TRIAL: at -Os a call the size guard refuses and the pin's
//! cost accepts is built inlined too, and kept where the finished code is
//! smaller. QuickrBasic gline.c -Os: `outside` (3 sites) and `cut` (2) stayed
//! calls, 1,436 B against 1,292 B with them inlined.

use std::process::Command;

fn assembly(
    ir: &str,
    trial: bool,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let mut llc = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.args(["-S", "-Os", "-o"]).arg(&output).arg(&input);
    if !trial {
        llc.env("LLRM_ARM64_NO_TRIAL", "1");
    }
    let said = llc.output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

fn instructions(asm: &str) -> usize {
    asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

/// Without the trial the two small callees stay, and the object is larger.
/// With it they are inlined at every site and the object is the smaller one.
#[test]
fn the_calls_the_guard_refuses_are_inlined_where_the_object_is_smaller() {
    let ir = include_str!("data/gline_os.ll");
    let kept = assembly(ir, false);
    assert!(kept.contains("\ncut:") && kept.contains("\noutside:"), "premise: the guard refuses both:\n{kept}");
    let inlined = assembly(ir, true);
    assert!(!inlined.contains("\ncut:") && !inlined.contains("\noutside:"), "{inlined}");
    assert!(
        instructions(&inlined) + 30 < instructions(&kept),
        "{} against {}",
        instructions(&inlined),
        instructions(&kept)
    );
}

/// Three calls of a ten-instruction callee the pin's cost accepts: inlined
/// they are larger than three calls and the callee, so the trial puts them
/// back.
#[test]
fn a_refused_call_stays_where_inlining_it_grows_the_object() {
    let asm = assembly(include_str!("data/refused_inline_grows_os.ll"), true);
    assert!(asm.lines().any(|line| line == "mix:"), "{asm}");
    assert_eq!(asm.matches("\tbl mix").count(), 3, "{asm}");
}
