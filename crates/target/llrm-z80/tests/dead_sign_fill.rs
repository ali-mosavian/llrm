//! An instruction whose only results are `a` and the flags, with no reader, is dropped.

use std::process::Command;

fn assembly(
    ir: &str,
    level: &str,
) -> String {
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
        .args(["-S", level, "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// The sign of `off`, widened to 32 bits for `iv + off` whose high word nothing reads (a shift count is its low
/// bits), was still computed: `ld a, d; add a, a; sbc a, a` ahead of the loop, its result overwritten at once.
/// Only `start`'s sign fill is left.
#[test]
fn the_sign_fill_of_an_unread_high_word_is_not_computed() {
    let ir = "define i32 @g(i32 %val, i16 %start, i16 %off) {\nentry:\n  %s = sext i16 %start to i32\n  %o = sext i16 %off to i32\n  br label %loop\nloop:\n  %iv = phi i32 [ %s, %entry ], [ %next, %loop ]\n  %amt = add i32 %iv, %o\n  %sh = lshr i32 %val, %amt\n  %c = icmp ne i32 %sh, 0\n  %next = add i32 %iv, 1\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %iv\n}\n";
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly(ir, level);
        let fills = text.matches("sbc a, a").count();
        assert_eq!(fills, 1, "{level}: {fills} sign fills, one is read:\n{text}");
    }
}
