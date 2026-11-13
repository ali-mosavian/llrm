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
    // the carry is taken from the flag: `adc hl, rr`, or `jr nc` over `inc rr`
    // for a word that adds nothing
    assert!(
        (text.contains("adc hl, ") || (text.contains("jr nc") && text.contains("inc de"))) && !text.contains("jr c"),
        "{text}"
    );
}

/// A word that adds nothing but the carry of the word before it is `jr nc` over
/// `inc rr` (3 B, at most 19 T), not `ld de, #0; adc hl, de` (5 B, 25 T) with a
/// register held for the zero: `x + 2` on a long was 12 B and took
/// the high word through the frame, add_ones in commute.rs.
#[test]
fn a_word_that_adds_only_the_carry_is_a_jump_over_an_inc() {
    let text = assembly("carry_inc", "define i32 @g(i32 %a) {\n  %r = add i32 %a, 2\n  ret i32 %r\n}\n");
    assert!(text.contains("jr nc") && text.contains("inc de") && !text.contains("adc hl, "), "{text}");
}

/// A long long add rebuilt the carry of its low 32 bits by a 32-bit compare, a
/// `setcc` and a byte added to the high half (wrapv_overflow_check -Os: 31 B in
/// the add alone, 192 B against SDCC's 101): the four words are one
/// `add hl, rr` and three `adc hl, rr`.
#[test]
fn a_long_long_add_carries_in_the_flag() {
    let text = assembly("add64", "define i64 @g(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  ret i64 %r\n}\n");
    assert!(text.matches("adc hl, ").count() >= 3 && !text.contains("jr c"), "{text}");
}

/// The same for the borrow: four words, `sbc hl, rr` in each.
#[test]
fn a_long_long_subtract_borrows_in_the_flag() {
    let text = assembly("sub64", "define i64 @g(i64 %a, i64 %b) {\n  %r = sub i64 %a, %b\n  ret i64 %r\n}\n");
    assert!(text.matches("sbc hl, ").count() >= 4 && !text.contains("jr c"), "{text}");
}

/// A saturating add or subtract selects with the carry out of the word op as
/// its condition: the first carry-op lowering left `UADDO` result 1 raw in a
/// `select`, and `usub.sat` and `uadd.sat` at 16 and 32 bits were REFUSED
/// with `CannotSelect(CMP)` (sat_builtins and sat_sub_u64_shared_compare, -O2,
/// 11:xxZ sweep).
#[test]
fn a_saturating_op_takes_the_carry_of_its_word_op() {
    for (bits, op) in [(16, "uadd"), (16, "usub"), (32, "uadd"), (32, "usub"), (64, "usub")] {
        let ir = format!(
            "declare i{bits} @llvm.{op}.sat.i{bits}(i{bits}, i{bits})\n\
             define i{bits} @g(i{bits} %a, i{bits} %b) {{\n  %r = call i{bits} @llvm.{op}.sat.i{bits}(i{bits} %a, i{bits} %b)\n  ret i{bits} %r\n}}\n"
        );
        let text = assembly("sat", &ir);
        assert!(text.contains("hl, "), "{op} {bits}:\n{text}");
    }
}

/// A long stepped by one stayed `inc hl; ld a, h; or a, l; jr nz; inc de` (5 B)
/// before the carry ops, and became `ld bc, #1; add hl, bc; ld de, #0; adc hl,
/// de` (9 B): long_step_by_one +19 B and +675 T at every level.
#[test]
fn a_long_step_by_one_keeps_its_inc_and_zero_test() {
    for (op, step) in [("add", "inc"), ("sub", "dec")] {
        let ir = format!("define i32 @g(i32 %a) {{\n  %r = {op} i32 %a, 1\n  ret i32 %r\n}}\n");
        let text = assembly("step", &ir);
        assert!(
            text.contains(&format!("{step} hl")) && !text.contains("adc hl, ") && !text.contains("sbc hl, "),
            "{op}:\n{text}"
        );
    }
}

/// A step by one in the low word and a constant in the high word, `0x80000001 +
/// x`: the high word took the step's carry from a compare no flag held, and
/// call_keeps was REFUSED with `b has no byte halves`.
#[test]
fn a_step_by_one_in_the_low_word_with_a_constant_above_compiles() {
    let text = assembly(
        "stepk",
        "define i32 @g(i32 %a) {\n  %m = mul i32 %a, 3\n  %r = add i32 %m, -2147483647\n  ret i32 %r\n}\n",
    );
    assert!(text.contains("adc hl, ") || text.contains("add hl, "), "{text}");
    let text = assembly("stepj", "define i32 @g(i32 %a) {\n  %r = add i32 %a, 65537\n  ret i32 %r\n}\n");
    assert!(!text.is_empty(), "{text}");
}

/// Eight long arguments on the stack, five adds of products: five of the seven
/// carries were rebuilt by a compare (`pack` of os_locals_between_pushes at
/// -O2: 9 instructions and a branch each, 362 B against SDCC's 270 B) where the
/// same chain with seven arguments, or each add alone, took `adc hl, rr`.
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
