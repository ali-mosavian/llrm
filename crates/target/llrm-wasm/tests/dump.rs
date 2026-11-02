use std::process::Command;

const LOOP: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  %j = add i32 %i, 1
  %c = icmp ult i32 %j, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %j
}
";

/// The mid-end's steps could not be seen on the wasm path (`llrm-wasm-llc` took
/// no `--dump`), so a pass that departs from the pin's order had to be found
/// by guessing: `LLRM_WASM_DUMP=DIR` writes the step files.
#[test]
fn the_mid_end_steps_are_written_where_llrm_wasm_dump_says() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-dump-{}", std::process::id()));
    let ir = dir.join("in.ll");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&ir, LOOP).unwrap();
    let steps = dir.join("steps");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(&ir)
        .env("LLRM_WASM_DUMP", &steps)
        .env("LLRM_DUMP_INPUTS", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let files = std::fs::read_dir(&steps).map(|entries| entries.count()).unwrap_or(0);
    std::fs::remove_dir_all(&dir).ok();
    assert!(files > 0, "no step was written");
}
