//! `va_start` + `va_arg` keep the list pointer in a register.

use std::process::Command;

fn assembly(ir: &str) -> String {
    let guard = tempfile::tempdir().unwrap();
    let dir = guard.path().to_path_buf();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n{ir}"),
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-Os", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `f(int n, ...)` reading one int took an `ix` frame and two stores into a `va_list` slot nothing read
/// back (51 B for `walk`, SDCC 20 B); the list now lives in a register.
#[test]
fn a_va_list_needs_no_stack_slot() {
    let text = assembly(
        "declare void @llvm.va_start.p0(ptr)\ndeclare void @llvm.va_end.p0(ptr)\n\
         define i16 @f(i16 %n, ...) {\n  %ap = alloca ptr, align 2\n  call void @llvm.va_start.p0(ptr %ap)\n  \
         %v = va_arg ptr %ap, i16\n  call void @llvm.va_end.p0(ptr %ap)\n  ret i16 %v\n}\n",
    );
    assert!(!text.contains("(ix"), "{text}");
}
