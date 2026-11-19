//! `a - umax(b, 1)` is `cmp b, 1; sbb`, not an `adc` into a register and a
//! `sub`.

mod common;

/// `a - umax(b, 1)` came out as `mov r, b; adc r, 0; sub a, r`, one register
/// and one instruction over the pin's `cmp b, 1; sbb a, b`.
#[test]
fn a_difference_with_a_maximum_of_one_is_a_borrow_subtract() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), "unsigned f(unsigned a, unsigned b) { return a - (b > 1 ? b : 1); }\n")
        .unwrap();
    for target in ["--target=x86_64-linux", "--target=x86_32-linux"] {
        let output = common::cc::llrm_c()
            .current_dir(directory)
            .args([target, "-O2", "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
        assert!(text.contains("sbb ") && !text.contains("adc "), "{target}: {text}");
    }
}
