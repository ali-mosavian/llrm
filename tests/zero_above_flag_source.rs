//! A zero made between a flag source and its compare moves above the source,
//! so it is an `xor`, not a five-byte `mov`.

mod common;

/// `__fixunssfsi` and the other `fix_*` conversions kept `mov eax, 0` between
/// `or` and `jne` (3 bytes over the pin's `xor eax, eax` ahead of the `or`); the
/// pin's `optimizeCompareInstr` moves the `MOV32r0` above the flag source.
#[test]
fn a_zero_between_a_flag_source_and_its_branch_is_an_xor_ahead_of_the_source() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "unsigned f(unsigned x, unsigned e) { if (e < 127 || (int)x < 0) return 0; if (e > 158) return -1; return x * 3 + e; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O2", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    assert!(text.contains("xor eax, eax") && !text.contains("mov eax, 0"), "{text}");
}
