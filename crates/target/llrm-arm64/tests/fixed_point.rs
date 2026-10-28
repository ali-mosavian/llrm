//! Fixed-point conversions.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/fixed_point.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// `(double)w * 0.5`, `(long)(d * 16)` and `0.25 * (double)x` were a conversion and a multiply by a constant
/// kept in a callee-saved float register; clang makes one `ucvtf d, w, #1`, `fcvtzs x, d, #4`, `scvtf d, x, #2`.
#[test]
fn a_power_of_two_scale_beside_a_conversion_is_fractional_bits() {
    assert!(body("half").contains("ucvtf d0, w0, #1"));
    assert!(body("sixteen").contains("fcvtzs x0, d0, #4"));
    assert!(body("quarter").contains("scvtf d0, x0, #2"));
}

/// Not a power of two, more fractional bits than the integer has, a conversion read elsewhere: all stay a multiply.
#[test]
fn a_scale_that_is_no_fixed_point_stays_a_multiply() {
    for function in ["not_a_power_of_two", "too_many_bits", "converted_twice"] {
        let asm = body(function);
        assert!(asm.contains("fmul"), "{function}:\n{asm}");
    }
}
