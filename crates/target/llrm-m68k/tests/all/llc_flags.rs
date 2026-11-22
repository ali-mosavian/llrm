//! `llrm-cc -fno-inline-functions` (and the other mid-end flags a program's `// flags:` line names)
//! reached `llrm-m68k-llc`, which answered `unrecognized argument`: 19 corpus programs failed to
//! compile at all, and the sweep counted them as unmeasured cells.

use std::process::{Command, Output};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @h(i32 %a) {
  %r = mul i32 %a, 3
  ret i32 %r
}
define i32 @f(i32 %a) {
  %x = call i32 @h(i32 %a)
  %y = call i32 @h(i32 %x)
  ret i32 %y
}
";

fn llc(flags: &[&str]) -> Output {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.ll"), IR).unwrap();
    Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .current_dir(&dir)
        .args(["-S", "-O2", "-o", "-"])
        .args(flags)
        .arg("f.ll")
        .output()
        .expect("the driver runs")
}

fn calls_h(flags: &[&str]) -> bool {
    let done = llc(flags);
    assert!(done.status.success(), "{flags:?}: {}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8_lossy(&done.stdout).contains("jsr\t(h,%pc)")
}

#[test]
fn the_pass_flags_of_a_header_reach_the_mid_end() {
    assert!(!calls_h(&[]), "inlined by default");
    assert!(calls_h(&["-fno-inline-functions", "-fno-inline-functions-called-once"]));
    assert!(calls_h(&["-fno-inline=h"]));
}

#[test]
fn a_flag_it_does_not_know_is_an_error_not_a_silent_drop() {
    let done = llc(&["-fno-such-pass"]);
    assert!(!done.status.success());
    assert!(String::from_utf8_lossy(&done.stderr).contains("unrecognized argument -fno-such-pass"));
    assert!(!llc(&["-fno-inline=nothing"]).status.success(), "a name no function has");
}

#[test]
fn excess_precision_is_taken_as_there_is_none_to_keep() {
    assert!(llc(&["-fexcess-precision=standard"]).status.success());
}

/// `-fsanitize=bounds` (the `idioms` variant) answered `unrecognized argument`, the
/// text of a typo. It is refused by name: a sanitizer is never silently dropped.
#[test]
fn a_sanitizer_is_refused_by_name() {
    for flag in ["-fsanitize=bounds", "-fno-sanitize=bounds"] {
        let done = llc(&[flag]);
        let stderr = String::from_utf8_lossy(&done.stderr);
        assert!(!done.status.success(), "{flag}");
        assert!(stderr.contains(flag) && stderr.contains("sanitizer checks are not implemented"), "{flag}: {stderr}");
        assert!(!stderr.contains("unrecognized argument"), "{flag}: {stderr}");
    }
}
