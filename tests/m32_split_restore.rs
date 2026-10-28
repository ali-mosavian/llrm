//! A late path that leaves by a conditional branch to the return the early
//! exit shares still leaves the early exit bare.

mod common;

use std::process::Command;

fn ir() -> String {
    "target datalayout = \"@LAYOUT@\"\n\
@a = internal global [64 x i32] zeroinitializer, align 4\n\
declare fastcc void @g(i32)\n\
define fastcc void @f(i32 %n, i32 %m) noinline nounwind {\n\
entry:\n  %c = icmp slt i32 %n, 2\n  br i1 %c, label %ret, label %body\n\
body:\n  call fastcc void @g(i32 %n)\n  call fastcc void @g(i32 %m)\n  %p = getelementptr i32, ptr @a, i32 %n\n  %v = load i32, ptr %p\n  %s = add i32 %v, %n\n  %d = icmp eq i32 %s, %m\n  br i1 %d, label %ret, label %loop\n\
loop:\n  %i = phi i32 [ 0, %body ], [ %j, %loop ]\n  %q = getelementptr i32, ptr @a, i32 %i\n  store i32 %s, ptr %q\n  %j = add i32 %i, 1\n  %z = icmp eq i32 %j, %m\n  br i1 %z, label %ret, label %loop\n\
ret:\n  ret void\n}\n".replace("@LAYOUT@", &common::datalayout("x86-m32"))
}

/// x_mergesort `msort` pushed ebx/esi/edi/ebp before its early `jl` to the
/// return: the registers were saved at the entry, not on the late path. The
/// test named `push ebx`, which the allocator stopped choosing: it read the
/// missing push as a failure (any callee-saved push counts).
#[test]
fn test_the_early_exit_precedes_the_pushes_when_the_late_path_branches_to_the_return() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), ir()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let body = asm.split("_f proc near").nth(1).unwrap();
    let push = body.find("push ").expect("a callee-saved register is pushed on the late path");
    assert!(body.find("jl ").unwrap() < push, "{asm}");
}
