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
