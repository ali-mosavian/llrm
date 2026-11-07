//! A `mul i32 %n, 3` at -O2 was refused ("e has no byte halves"): #7893 named
//! a spilled byte by its lane in the wider register, and z80's `e` is not a
//! half of a register the allocator may take for it. The lane naming is the
//! target's to ask for (`spills_named_by_lane`); z80 numbers the value.

#[test]
fn a_long_multiply_by_a_constant_compiles_at_o2() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join(format!("llrm-z80-mul-long-{}.s", std::process::id()));
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/data/mul_long_by_three.ll")])
        .output()
        .expect("llrm-z80-llc runs");
    std::fs::remove_file(&out).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

/// `x * 3` on a long expanded through byte lanes: 49 instructions (about
/// 140 B) where `x << 1` and `x +` are two `add hl` / `adc hl` pairs.
/// register_upper_half `n - descend(n-1) + 2*n` was 164 instructions against
/// SDCC's 118.
#[test]
fn a_long_times_three_is_shifts_and_adds() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let asm = text(include_str!("data/mul_long_by_three.ll"), "m3", 0, &Request::new(Level::O2)).expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(count <= 30 && asm.contains("adc hl"), "{count} instructions:\n{asm}");
}

/// #7940 decomposed `x * 2^n +- 1` before the size routine could be chosen: in a module with two long
/// multiplies a function tuned for size called `__mulsi3` (14 B a site), and the shifts and adds
/// grew 12 Os cells (mul_constants +227 B, int64_division +96, call_keeps +153 against 7cfc6eabbf~1).
#[test]
fn a_size_function_in_a_module_of_long_multiplies_calls_the_routine_for_x_times_three() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i32 @m(i32 %n, i32 %k) optsize {\n %r = mul i32 %n, 3\n %s = mul i32 %k, 5\n %t = add i32 %r, %s\n ret i32 %t\n}";
    let asm = llrm_z80::compile::text(ir, "m", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::Os)).expect("compiles");
    assert_eq!(asm.matches("call __mulsi3").count(), 2, "{asm}");
}

/// #7940 decomposed `x * 3` for a zero-extended byte too: 21 instructions where the byte-lane multiply of
/// a value whose three high bytes are zero is 15 (byte_multiply -O2 +36 B, +38,149 T). A zero-extended
/// word keeps the decomposition (23 against 44).
#[test]
fn a_long_times_three_of_a_zero_extended_byte_keeps_the_byte_lane_multiply() {
    let count = |width: u32| {
        let ir = format!(
            "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
             define i32 @f(i{width} %a) {{\n %x = zext i{width} %a to i32\n %r = mul i32 %x, 3\n ret i32 %r\n}}"
        );
        let asm = llrm_z80::compile::text(&ir, "f", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::O2)).expect("compiles");
        asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
    };
    assert!(count(8) <= 16, "byte: {} instructions", count(8));
    assert!(count(16) <= 30, "word: {} instructions", count(16));
}
