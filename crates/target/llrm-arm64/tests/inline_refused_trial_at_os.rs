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
        instructions(&inlined) + 20 < instructions(&kept),
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

/// A function that ends in `unreachable` is not a call: counting it as one
/// left the module's call-weighted work unreadable (NaN), and every refused
/// inline of the module was put back (gline.c with a trap path: 1,436 B, not
/// 1,292 B; in QuickrBasic double.c, file.c, sema.c, exec.c and using.c).
#[test]
fn a_function_with_an_unreachable_end_does_not_stop_the_trial() {
    let asm = assembly(include_str!("data/gline_os_unreachable.ll"), true);
    assert!(!asm.contains("\ncut:") && !asm.contains("\noutside:"), "{asm}");
}

fn trial_trace(ir: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, ir).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .env("LLRM_DEBUG", "trial")
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8_lossy(&said.stderr).into_owned()
}

/// A refused site the byte model reads as far larger inlined (a callee two
/// callers share, 36 B of copy in a 40 B caller) is not built: the second
/// compile cost 69 of 112 QuickrBasic modules +14% compile time for 11 wins,
/// and those wins all had a site within the band.
#[test]
fn a_refused_call_the_bytes_say_is_far_larger_is_not_built() {
    let far = trial_trace(include_str!("data/refused_inline_far_larger_os.ll"));
    assert!(!far.contains("inline-refused"), "{far}");
    let near = trial_trace(include_str!("data/gline_os.ll"));
    assert!(near.contains("inline-refused"), "premise: a site within the band is built:\n{near}");
}
