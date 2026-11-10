//! The carry of a long add or subtract stays in the flag.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
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
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `a + b` on longs rebuilt the carry by a compare, `jr c` and a byte added to
/// the high word (11 instructions and 3 branches beyond the two adds); the
/// high word is `adc hl, rr`.
#[test]
fn a_long_add_carries_in_the_flag() {
    let text = assembly("add", "define i32 @g(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}\n");
    assert!(text.contains("adc hl, ") && !text.contains("jr c"), "{text}");
}

/// The same for the borrow: `sbc hl, rr` after `sbc hl, rr`.
#[test]
fn a_long_subtract_borrows_in_the_flag() {
    let text = assembly("sub", "define i32 @g(i32 %a, i32 %b) {\n  %r = sub i32 %a, %b\n  ret i32 %r\n}\n");
    assert!(text.matches("sbc hl, ").count() == 2 && !text.contains("jr c"), "{text}");
}

/// A long grown by a widened word: the high word has no addend of its own and
/// the carry was still rebuilt by a compare (mem_length_boundaries_m32 -O2: the
/// 32-bit sum of a loop, 11 instructions a trip beyond `adc hl, rr`).
#[test]
fn a_long_grown_by_a_word_carries_in_the_flag() {
    let text = assembly(
        "widened",
        "define i32 @g(i32 %a, i16 %b) {\n  %e = zext i16 %b to i32\n  %r = add i32 %a, %e\n  ret i32 %r\n}\n",
    );
    assert!(text.contains("adc hl, ") && !text.contains("jr c"), "{text}");
}

/// Eight long arguments on the stack, five adds of products: five of the seven carries were rebuilt by a compare (`pack` of
/// os_locals_between_pushes at -O2: 9 instructions and a branch each, 362 B against SDCC's 270 B) where the same chain with seven
/// arguments, or each add alone, took `adc hl, rr`.
#[test]
fn the_carries_of_a_chain_of_long_adds_over_stack_arguments_stay_in_the_flag() {
    let text = assembly(
        "chain",
        "define i32 @g(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e, i32 %f, i32 %g, i32 %h) {
  %a10 = mul i32 %a, 10
  %s1 = add i32 %a10, %b
  %s1m = mul i32 %s1, 10
  %s2 = add i32 %s1m, %c
  %s2m = mul i32 %s2, 10
  %s3 = add i32 %s2m, %d
  %s3m = mul i32 %s3, 10
  %s4 = add i32 %s3m, %e
  %s4m = mul i32 %s4, 10
  %s5 = add i32 %s4m, %f
  %s5m = mul i32 %s5, 100
  %g10 = mul i32 %g, 10
  %s6 = add i32 %s5m, %g10
  %r = add i32 %s6, %h
  ret i32 %r
}\n",
    );
    assert!(!text.contains("jr c"), "{text}");
}
