//! A copy of an address of registers is the address again
//! (`reMaterializeTrivialDef`).

mod common;

/// vsgcc's x_isqrt at -O2: the sum `r + b` is read by the compare and, on the
/// taken path, by the subtract. The coalescer kept one `lea` and copied it
/// across the compare (`mov eax, edx`); the pin makes the `lea` again where
/// the copy was, and the loop is one instruction shorter.
#[test]
fn a_lea_read_on_one_path_only_is_made_again_not_copied() {
    let source = r#"unsigned isqrt(unsigned x)
{
    unsigned r = 0, b = 1u << 30;
    while (b > x)
        b >>= 2;
    while (b) {
        if (x >= r + b) {
            x -= r + b;
            r = (r >> 1) + b;
        } else
            r >>= 1;
        b >>= 2;
    }
    return r;
}
"#;
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let leas = asm.lines().filter(|line| line.trim_start().starts_with("lea ")).count();
    assert_eq!(leas, 2, "{asm}");
}
