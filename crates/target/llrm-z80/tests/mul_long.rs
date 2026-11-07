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
