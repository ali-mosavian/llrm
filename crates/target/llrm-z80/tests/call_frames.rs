//! A call's stack arguments are the words pushed for that call alone.

use std::process::Command;

/// Two soft-float multiplies and two conversions: the `ILP` schedule opened
/// `__fixdfsi`'s one-word frame inside `__muldf3`'s six, so `__muldf3` ran with
/// seven words pushed and its arguments at the wrong addresses (tests/run/c
/// `object_extent_loop` reported 33564 for 82).
#[test]
fn a_call_has_only_its_own_stack_arguments_pushed() {
    let dir = std::env::temp_dir().join("llrm-call-frames-z80");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\ntarget triple = \"z80-unknown-none\"\n\
         @x = global double 1.5\n@z = global double 5.5\n\
         define i16 @main() {\n\
           %1 = load volatile double, ptr @x\n  %2 = fmul double %1, 4.0\n  %3 = fptosi double %2 to i32\n\
           %4 = load volatile double, ptr @z\n  %5 = fmul double %4, 16.0\n  %6 = fptosi double %5 to i32\n\
           call void @report(i32 %3)\n  call void @report(i32 %6)\n  ret i16 0\n}\n\
         declare void @report(i32)\n",
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let mut pushed = 0i32;
    for line in text.lines().map(str::trim) {
        if line == "push ix" || line == "pop ix" {
            continue;
        }
        if line.starts_with("push ") {
            pushed += 1;
        } else if line.starts_with("pop ") {
            pushed -= 1;
        } else if let Some(callee) = line.strip_prefix("call ") {
            let words = match callee {
                "__muldf3" => 6,
                "__fixdfsi" => 1,
                _ => 0,
            };
            assert_eq!(pushed, words, "{callee} runs with {pushed} words pushed:\n{text}");
        }
    }
}
