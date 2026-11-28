//! A long shifted right by one is a carry chain, not a funnel of two words.

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
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

fn body(text: &str) -> Vec<&str> {
    text.lines().filter(|l| l.starts_with('\t') && !l.starts_with("\t.")).map(str::trim).collect()
}

/// `x >> 1` on a `u32` was 14 instructions: `srl h; rr l`, the low bit of the
/// high word moved through `rrca; and #128; or` into the top of the low word,
/// then `srl d; rr e`. It is `ld a,e; rra; rr h; rr l; srl d; rr e; ret`:
/// that bit goes through the carry.
#[test]
fn a_long_shifted_right_by_one_passes_the_bit_through_the_carry() {
    for op in ["lshr", "ashr"] {
        let text = assembly(&format!("define i32 @g(i32 %x) {{\n  %r = {op} i32 %x, 1\n  ret i32 %r\n}}\n"));
        let insts = body(&text);
        assert!(insts.len() <= 7 && !insts.contains(&"rrca"), "{op}: {} instructions:\n{text}", insts.len());
    }
}

/// A funnel of two words by an amount other than one stays the shifts and an
/// or.
#[test]
fn a_word_funnel_by_another_amount_still_compiles() {
    let text = assembly(
        "declare i16 @llvm.fshr.i16(i16, i16, i16)\ndefine i16 @g(i16 %a, i16 %b) {\n  %r = call i16 @llvm.fshr.i16(i16 %a, i16 %b, i16 3)\n  ret i16 %r\n}\n",
    );
    assert!(body(&text).len() > 4, "{text}");
}
