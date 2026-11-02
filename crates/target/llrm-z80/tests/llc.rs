//! `llrm-z80-llc`: the program `llrm-cc`'s z80 row drives.

use std::process::Command;

fn llc(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc")).args(args).output().expect("llrm-z80-llc runs")
}

#[test]
fn a_kernel_is_written_as_sdasz80_text() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join("llrm-z80-llc-add16.s");
    let done = llc(&["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/kernels/ir/add16.ll")]);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&out).expect("the output file");
    assert!(text.contains(".area _CODE") && text.contains("_add16::") && text.contains("add hl, de"), "{text}");
}

#[test]
fn a_refused_function_fails_the_exit_status() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let source = std::env::temp_dir().join("llrm-z80-llc-refused.ll");
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i1 @g(ptr %p) {\n  %v = load i1, ptr %p\n  ret i1 %v\n}\n";
    std::fs::write(&source, ir).unwrap();
    let out = std::env::temp_dir().join("llrm-z80-llc-refused.s");
    let done = llc(&["-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()]);
    assert_eq!(done.status.code(), Some(1), "{dir}");
    assert!(String::from_utf8_lossy(&done.stderr).contains("REFUSED @g"));
}
