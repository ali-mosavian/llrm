//! The IR a stage dump ends with is the module the object was made from. At
//! -Os a trial that is kept (INLINEHINT-GROWTH-TRIAL) is compiled without
//! stage files; the files were the base compile's, so the last stage had the
//! call inlined that the object makes (QB operands.c: every comparison made on
//! the dump, `hot_int` inlined in it and a call in the object, was wrong).

use std::process::Command;

const IR: &str = "target triple = \"x86_64-unknown-linux-gnu\"
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"
@tab = external global [64 x i32]
define internal i32 @esize(ptr %p, i32 %k) inlinehint nounwind optsize {
  %a0 = load i32, ptr %p
  %a1 = getelementptr i32, ptr %p, i32 1
  %b0 = load i32, ptr %a1
  %c0 = and i32 %k, 7
  %t0 = getelementptr [64 x i32], ptr @tab, i32 0, i32 %c0
  %d0 = load i32, ptr %t0
  %m0 = mul i32 %a0, %d0
  %s0 = add i32 %m0, %b0
  %x0 = icmp ult i32 %s0, 100
  %y0 = select i1 %x0, i32 %s0, i32 100
  %c1 = lshr i32 %k, 3
  %t1 = getelementptr [64 x i32], ptr @tab, i32 0, i32 %c1
  %d1 = load i32, ptr %t1
  %m1 = mul i32 %y0, %d1
  %s1 = xor i32 %m1, %k
  %x1 = icmp sgt i32 %s1, 7
  %y1 = select i1 %x1, i32 %s1, i32 %m0
  %r = sub i32 %y1, %b0
  ret i32 %r
}
define i32 @one(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  ret i32 %r
}
define i32 @two(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = add i32 %r, 1
  ret i32 %s
}
define i32 @three(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = shl i32 %r, 1
  ret i32 %s
}
define i32 @four(ptr %p, i32 %k) nounwind optsize {
  %r = call i32 @esize(ptr %p, i32 %k)
  %s = sub i32 0, %r
  ret i32 %s
}
";

#[test]
fn the_last_dumped_stage_has_the_calls_the_kept_trial_keeps() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    // A dump of thousands of files is refused on tmpfs.
    let stages = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("stages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stages);
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .env("LLRM_MIR_STAGES", &stages)
        .env("LLRM_DEBUG", "trial")
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let log = String::from_utf8_lossy(&said.stderr);
    assert!(log.contains("keep-hinted-calls") && log.contains("kept"), "the premise: the trial is kept\n{log}");
    let mut last: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(&stages)
        .unwrap()
        .filter_map(|one| one.ok().map(|one| one.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "ll"))
        .filter_map(|path| Some((path.file_name()?.to_str()?.split('-').next()?.parse().ok()?, path)))
        .collect();
    last.sort();
    let dumped = std::fs::read_to_string(&last.last().expect("a stage was dumped").1).unwrap();
    assert!(dumped.contains("@esize("), "the last stage has the call inlined that the object makes:\n{dumped}");
    assert!(dumped.matches("call").count() >= 4, "{dumped}");
}
