//! A long doubled in straight code stays in the pairs its words arrive in.

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

fn body(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.starts_with('\t') && !l.starts_with("\t.")).map(str::trim).collect()
}

/// `x << 1` was `add hl,hl; ld c,l; ld b,h; ld l,e; ld h,d; adc hl,de; ex
/// de,hl; ld l,c; ld h,b; ret` (10 B): the high word waited in `de` while the
/// low word went through `bc`. It is `add hl,hl; rl e; rl d; ret` (6 B), SDCC's
/// `sla e; rl d; adc hl,hl; ret` also being 6 B.
#[test]
fn a_long_doubled_in_straight_code_needs_no_third_pair() {
    for (level, op) in [("-O1", "shl i32 %x, 1"), ("-O2", "shl i32 %x, 1"), ("-Os", "add i32 %x, %x")] {
        let text = assembly(&format!("define i32 @g(i32 %x) {{\n  %r = {op}\n  ret i32 %r\n}}\n"), level);
        let insts = body(&text);
        assert!(insts.len() <= 4, "{level} {op}: {} instructions:\n{text}", insts.len());
        assert!(!text.contains("bc") && !text.contains("(ix"), "{level} {op}:\n{text}");
    }
}

/// A long doubled on each side of a call, the argument and result pairs of the
/// convention: 20 instructions (two 9-instruction trades through `bc`), it is
/// 7.
#[test]
fn a_long_doubled_around_a_call_needs_no_trade() {
    let text = assembly(
        "declare i32 @t(i32)\ndefine i32 @g(i32 %x) {\n  %a = shl i32 %x, 1\n  %s = call i32 @t(i32 %a)\n  %q = shl i32 %s, 1\n  ret i32 %q\n}\n",
        "-O2",
    );
    let insts = body(&text);
    assert!(insts.len() <= 8 && !text.contains("bc"), "{} instructions:\n{text}", insts.len());
}
