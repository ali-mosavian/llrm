//! Two soft-float libcalls whose stack arguments overlap in time.

use std::process::Command;

/// `x*4` and `z*16` each make a `__muldf3` and a `__fixdfsi` call. The
/// scheduler began the `__fixdfsi` frame inside the second `__muldf3` frame
/// (no call-sequence resource on z80): its argument stores landed in the
/// wrong slots and `x87rotate` in tests/run/c printed wrong output. Every
/// call must see only its own frame on the stack.
#[test]
fn libcall_frames_do_not_nest() {
    let dir = std::env::temp_dir().join("llrm-call-sequences-z80");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("m.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\ntarget triple = \"z80-unknown-none\"\n\
         @x = global double 1.5\n@z = global double 5.5\n\
         declare void @report(i32)\n\
         define i16 @main() {\n\
           %1 = load volatile double, ptr @x\n  %2 = fmul double %1, 4.0\n  %3 = fptosi double %2 to i32\n\
           %4 = load volatile double, ptr @z\n  %5 = fmul double %4, 16.0\n  %6 = fptosi double %5 to i32\n\
           call void @report(i32 %3)\n  call void @report(i32 %6)\n  ret i16 0\n}\n",
    )
    .unwrap();
    let out = dir.join("m.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(&out).unwrap();
    let mut depth = 0;
    for line in asm.lines().map(str::trim) {
        if line.starts_with("push ") {
            depth += 1;
        } else if line.starts_with("pop ") {
            depth -= 1;
        } else if line.starts_with("ld sp,") {
            depth = 0;
        } else if line == "call __muldf3" {
            assert_eq!(depth, 6, "a __muldf3 frame holds six words:\n{asm}");
        } else if line == "call __fixdfsi" {
            assert_eq!(depth, 1, "a __fixdfsi frame holds one word:\n{asm}");
        }
    }
}
