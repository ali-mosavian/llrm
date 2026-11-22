//! `llrm-m68k-llc -S -o - FILE.ll` wrote its assembly to a file named `-` in the working directory (a stray
//! `crates/target/llrm-m68k/-` was committed that way). `-` is stdout.

use std::process::Command;

#[test]
fn an_output_path_of_dash_is_stdout_not_a_file_named_dash() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    let _ = std::fs::remove_file(dir.join("-"));
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         define i32 @g(i32 %a) {\n  %r = add i32 %a, 1\n  ret i32 %r\n}\n",
    )
    .unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .current_dir(&dir)
        .args(["-S", "-o", "-", "g.ll"])
        .output()
        .expect("the driver runs");
    let stdout = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    assert!(stdout.contains("rts"), "stdout: {stdout:?}");
    assert!(!dir.join("-").exists(), "a file named `-` was written");
}
