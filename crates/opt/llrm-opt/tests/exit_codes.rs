use std::io::Write;
use std::process::{Command, Stdio};

const LOOP: &str = "define i32 @f(i32 %x) {\nentry:\n  %dead = mul i32 %x, 3\n  ret i32 %x\n}\n";

fn run(
    args: &[&str],
    input: &str,
) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_llrm-opt"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stdout).into(), String::from_utf8_lossy(&out.stderr).into())
}

#[test]
fn an_unknown_pass_exits_77_and_names_it() {
    let (code, out, err) = run(&["-S", "-passes=loop-term-fold"], LOOP);
    assert_eq!((code, out.as_str(), err.trim()), (Some(77), "", "unsupported: loop-term-fold"));
}

#[test]
fn an_unknown_flag_exits_77_and_names_it() {
    let (code, _, err) = run(&["-S", "-lsr-insns-cost=1", "-passes=dce"], LOOP);
    assert_eq!((code, err.trim()), (Some(77), "unsupported: -lsr-insns-cost=1"));
}

#[test]
fn a_known_pass_reads_stdin_and_writes_the_module() {
    let (code, out, _) = run(&["-S", "-passes=dce", "-"], LOOP);
    assert_eq!(code, Some(0));
    assert!(out.contains("ret i32 %x") && !out.contains("%dead"), "{out}");
    let (code, out, _) = run(&["-S", "-dce"], LOOP);
    assert_eq!(code, Some(0));
    assert!(!out.contains("%dead"), "{out}");
}

#[test]
fn text_the_parser_cannot_read_is_78_not_a_pass() {
    let (code, _, err) = run(&["-passes=dce"], "this is not IR\n");
    assert_eq!(code, Some(78), "{err}");
}
