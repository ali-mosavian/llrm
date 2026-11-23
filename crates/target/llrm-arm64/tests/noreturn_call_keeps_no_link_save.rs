//! A call that never returns and cannot unwind does not make `x30` saved: the
//! pin's `MachineRegisterInfo::isNoReturnDef` (a call in a block with no
//! successor to a `noreturn nounwind` function, from a function without
//! `uwtable`) is no modification of the link register. Saving it anyway cost
//! `str x30, [sp, #-16]!` in about 50 functions of QB operands.c (+4 B each).

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\ntarget datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\ndeclare void @ext(i32) nounwind\n";

/// The assembly of `f` in `body` (a module without its header) at -Os.
fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

/// Whether the assembly stores `x30`.
fn saves_link(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        (line.starts_with("str ") || line.starts_with("stp ")) && line.contains("x30")
    })
}

/// `f(a, p)` loads through `p`, or calls `@die` where `a` is zero.
fn caller(
    die: &str,
    attrs: &str,
) -> String {
    format!(
        "{die}\ndefine i32 @f(i32 %a, ptr %p) {attrs} {{\n  %c = icmp eq i32 %a, 0\n  br i1 %c, label %bad, label %ok\nbad:\n  call void @die(i32 10)\n  unreachable\nok:\n  %v = load i32, ptr %p\n  ret i32 %v\n}}\n"
    )
}

#[test]
fn a_noreturn_nounwind_call_saves_no_link_register() {
    let text = assembly(&caller("declare void @die(i32) noreturn nounwind", "nounwind optsize"));
    assert!(!saves_link(&text), "the link register is saved for a call that never returns:\n{text}");
}

#[test]
fn a_noreturn_call_that_may_unwind_saves_the_link_register() {
    let text = assembly(&caller("declare void @die(i32) noreturn", "nounwind optsize"));
    assert!(saves_link(&text), "{text}");
}

#[test]
fn a_function_with_a_unwind_table_saves_the_link_register() {
    let text = assembly(&caller("declare void @die(i32) noreturn nounwind", "nounwind optsize uwtable"));
    assert!(saves_link(&text), "{text}");
}

#[test]
fn a_returning_call_beside_the_noreturn_one_saves_the_link_register() {
    let text = assembly(
        "declare void @die(i32) noreturn nounwind\ndefine i32 @f(i32 %a, ptr %p) nounwind optsize {\n  %c = icmp eq i32 %a, 0\n  br i1 %c, label %bad, label %ok\nbad:\n  call void @die(i32 10)\n  unreachable\nok:\n  call void @ext(i32 %a)\n  %v = load i32, ptr %p\n  ret i32 %v\n}\n",
    );
    assert!(saves_link(&text), "{text}");
}

#[test]
fn a_frame_record_still_saves_the_link_register() {
    let text =
        assembly(&caller("declare void @die(i32) noreturn nounwind", "nounwind optsize \"frame-pointer\"=\"all\""));
    assert!(saves_link(&text), "{text}");
}
