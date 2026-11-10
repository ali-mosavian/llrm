//! `(x & 2^k) == 0` for a bit the `test` immediate cannot hold is `bt x, k`.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `f64` add/sub tested bit 56 as `mov rcx, 72057594037927936; and rcx, rdx; je`
/// (13 bytes) where the pin has `bt rdx, 56; jae` (5): +50 B in lib/fp/f64/add_sub.c.
#[test]
fn a_high_bit_of_an_i64_is_tested_with_bt() {
    let source = "long f(unsigned long a, unsigned long b) { return ((a + b) & (1UL << 56)) == 0 ? a : b; }\n";
    let text = listing(source, &["--target=x86_64-linux", "-O2"]);
    assert!(text.contains("bt ") && text.contains(", 56"), "{text}");
    assert!(!text.contains("72057594037927936"), "{text}");
}

/// The listing came out but the object did not: the encoder took `bt r, imm8`
/// for 16 and 32 bits only, so the f64 runtime stopped building ("Semantics(op=
/// COMPARE, name='bt' ...").
#[test]
fn a_bit_test_of_an_i64_register_assembles_to_an_object() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "long f(unsigned long a, unsigned long b) { return ((a + b) & (1UL << 56)) == 0 ? a : b; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["--target=x86_64-linux", "-O2", "-c", "-fobject-format=elf", "-o", "a.o", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
