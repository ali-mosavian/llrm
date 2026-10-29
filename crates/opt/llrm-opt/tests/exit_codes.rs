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

/// 2008-08-13-CmpStride from the pin: a module with no triple is x86-64 to
/// `opt`. With a neutral cost model LSR left the `shl` in the loop and
/// the test's `grep add | count 2` saw one add.
const CMP_STRIDE: &str = "target datalayout = \"n8:16:32:64\"\n@g = common global i16 0\n\
define void @func_1() nounwind {\nentry:\n  br label %bb\nbb:\n  %i = phi i16 [ 0, %entry ], [ %t1, %bb ]\n  \
%t0 = shl i16 %i, 1\n  store volatile i16 %t0, ptr @g, align 2\n  %t1 = add i16 %i, -3\n  \
%t2 = icmp slt i16 %t1, 1\n  br i1 %t2, label %bb, label %return\nreturn:\n  ret void\n}\n";

#[test]
fn a_module_without_a_triple_is_reduced_with_the_x86_64_cost_model() {
    let (code, out, err) = run(&["-S", "-loop-reduce"], CMP_STRIDE);
    assert_eq!(code, Some(0), "{err}");
    assert!(!out.contains("shl"), "{out}");
}

/// The pin's sink-from-preheader.ll runs `indvars -indvars-predicate-loops=0`:
/// the flag was unknown, so the test exited 77 and was never run.
#[test]
fn indvars_predicate_loops_and_the_debug_checks_are_accepted() {
    for flag in ["-indvars-predicate-loops=0", "-indvars-predicate-loops=1", "-verify-scev", "-verify-loop-info"] {
        let (code, _, err) = run(&["-S", "-passes=indvars", flag], LOOP);
        assert_eq!(code, Some(0), "{flag}: {err}");
    }
    let (code, _, err) = run(&["-S", "-passes=indvars", "-indvars-no-such-flag"], LOOP);
    assert_eq!((code, err.trim()), (Some(77), "unsupported: -indvars-no-such-flag"));
}
