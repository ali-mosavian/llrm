//! A word compared for equality.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-compare16-z80-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(&source, format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n{ir}"))
        .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-Os", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `a != b` on two words was `ld a, l; sub a, e; jr nz; ld a, h; sub a, d` (6 B):
/// `or a, a; sbc hl, de` is 3.
#[test]
fn two_words_are_compared_for_equality_with_sbc_hl() {
    let text = assembly(
        "ne",
        "declare void @f()\n\
         define void @g(i16 %a, i16 %b) {\n  %c = icmp ne i16 %a, %b\n  br i1 %c, label %t, label %e\n\
         t:\n  call void @f()\n  ret void\n\
         e:\n  ret void\n}\n",
    );
    assert!(text.contains("sbc hl, de") && !text.contains("sub a, e"), "{text}");
}

/// The biased copy a signed compare builds (`ld c, e; ld a, d; xor a, #128; ld b, a`) had its low byte and its
/// high byte read by nothing once the compare tested the high byte alone, and both were rebuilt below
/// (`ld c, e; ld b, d`): 2 dead loads in switch_peeled_table's loop.
#[test]
fn a_biased_copy_the_compare_no_longer_reads_is_not_loaded() {
    let text = assembly(
        "dead_bias",
        "declare void @f(i16)\n\
         define void @g(i16 %a) {\n  %c = icmp slt i16 %a, 256\n  br i1 %c, label %t, label %e\n\
         t:\n  call void @f(i16 %a)\n  ret void\n\
         e:\n  ret void\n}\n",
    );
    let loads = text.lines().filter(|line| line.trim_start().starts_with("ld c, e")).count();
    assert!(loads <= 1, "{text}");
/// `x < 256` on a signed word compared the low byte against 0 first (`ld a, c; sub a, #0; ld a, b; sbc a, #129`):
/// subtracting 0 changes nothing and leaves the carry clear, so the high bytes alone are `ld a, b; sub a, #129`
/// (switch_peeled_table's loop, 3 B and 7 T per test).
#[test]
fn a_word_compared_with_a_constant_of_a_zero_low_byte_tests_the_high_byte_only() {
    let text = assembly(
        "low_zero",
        "declare void @f()\n\
         define void @g(i16 %a) {\n  %c = icmp slt i16 %a, 256\n  br i1 %c, label %t, label %e\n\
         t:\n  call void @f()\n  ret void\n\
         e:\n  ret void\n}\n",
    );
    assert!(!text.contains("sub a, #0"), "{text}");
    assert!(text.contains("sub a, #129") || text.contains("sub a, #1\n"), "{text}");
}
