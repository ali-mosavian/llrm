//! A pool constant fills the gap before the 8-aligned one even where the
//! object has a variable of zeros (it moves to `.data?` later).

mod common;

const SOURCE: &str = "
long double x[64];
int g = 1;
long double f(long double v) { return v * 1.1L + 2.5f; }
long double h(void) { x[3] = 2.5f; return x[g]; }
";

/// m32 -O1 x_ld_scale and x_ld_poly: `g` (4 B), `align 8`, the 10-byte
/// constant, then the dword constant: 4 B of padding that the dword fits in,
/// 214 B against clang's 213. A zero variable among the data had switched the
/// gap fill off.
#[test]
fn test_a_dword_constant_fills_the_gap_before_the_extended_one_beside_a_zero_variable() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--target=x86_32-wat", "-O1", "-mabi=sysv", "-mlong-double-80", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let dword = text.find("db 000h,000h,020h,040h").expect(&text);
    let aligned = text.find("align 8").expect(&text);
    assert!(dword < aligned, "{text}");
}
