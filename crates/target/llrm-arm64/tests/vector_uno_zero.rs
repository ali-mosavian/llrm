//! `fcmp uno <4 x float> %x, zeroinitializer` lowered as `fcmge` + `fcmlt` +
//! `orr` of the two masks (3 instructions): `LowerVSETCC` never asked
//! `isKnownNeverNaN`, so the zero operand stayed; the pin compares `x` with
//! itself, one `fcmeq` (the soft-float loops of sfm16_float_mul: 2453
//! instructions against clang's 1955).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn kernel() -> Vec<String> {
    let asm = driver::assembly(include_str!("data/vector_uno_zero.ll"), Platform::Linux, Level::O2).expect("lowers");
    asm.lines().skip_while(|line| *line != "f:").take_while(|line| !line.starts_with("\tret")).map(String::from).collect()
}

#[test]
fn an_unordered_compare_against_zero_is_one_fcmeq_of_the_value_with_itself() {
    let kernel = kernel();
    let count = |mnemonic: &str| kernel.iter().filter(|line| line.starts_with(mnemonic)).count();
    assert_eq!(count("\tfcmeq "), 1, "{}", kernel.join("\n"));
    assert_eq!(count("\tfcmge "), 0, "{}", kernel.join("\n"));
    assert_eq!(count("\tfcmlt "), 0, "{}", kernel.join("\n"));
}

/// With the mask one `fcmeq`, the select was `eor` (of the mask with a zero
/// vector `getNode` did not see through) + `bic` + `and` + `orr`; the pin's is
/// one `bsl`.
#[test]
fn a_select_on_an_unordered_compare_is_one_bsl() {
    let kernel = kernel();
    let count = |mnemonic: &str| kernel.iter().filter(|line| line.starts_with(mnemonic)).count();
    assert_eq!(count("\tbsl "), 1, "{}", kernel.join("\n"));
    for gone in ["\teor ", "\tbic ", "\tand ", "\torr "] {
        assert_eq!(count(gone), 0, "{}", kernel.join("\n"));
    }
}
