//! A `<8 x i16>` load and store: arm64 refused the function ("a vector value")
//! and the vectorizer had nothing to emit. Hand IR, since no pass makes a
//! vector yet (ARM64-VEC POC, scroll's copy loops).

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_VECTORS", "1")
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

#[test]
fn a_vector_copy_is_a_q_load_and_store() {
    let text = assembly(
        "define void @copy(ptr %d, ptr %s) {\n\
         \x20 %v = load <8 x i16>, ptr %s, align 16\n\
         \x20 store <8 x i16> %v, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let reg = |op: &str| {
        text.lines().find_map(|l| {
            l.trim().strip_prefix(op)?.trim_start().strip_prefix('q')?.split(',').next().map(str::to_owned)
        })
    };
    let (loaded, stored) = (reg("ldr"), reg("str"));
    assert!(loaded.is_some() && loaded == stored, "{text}");
}

/// A copy loop whose store is 80 elements ahead of its load (loop-idiom leaves
/// it alone) stayed 1920 scalar `ldrh`/`strh` iterations; the vectorizer makes
/// it eight lanes per `ldr q`/`str q`.
#[test]
fn an_overlapping_copy_loop_runs_on_q_registers() {
    let text = assembly(
        "define void @f(ptr noalias %a) {\n\
         entry:\n  br label %loop\n\
         loop:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n\
         \x20 %s = getelementptr inbounds i16, ptr %a, i64 %i\n\
         \x20 %v = load i16, ptr %s, align 2\n\
         \x20 %j = add nuw nsw i64 %i, 80\n\
         \x20 %d = getelementptr inbounds i16, ptr %a, i64 %j\n\
         \x20 store i16 %v, ptr %d, align 2\n\
         \x20 %next = add nuw nsw i64 %i, 1\n\
         \x20 %done = icmp eq i64 %next, 1920\n\
         \x20 br i1 %done, label %exit, label %loop\n\
         exit:\n  ret void\n}\n",
    );
    assert!(text.contains("ldr\tq") || text.contains("ldr q"), "{text}");
    assert!(!text.contains("ldrh"), "{text}");
}
