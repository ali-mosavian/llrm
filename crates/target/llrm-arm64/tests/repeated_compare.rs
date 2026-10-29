//! A comparison the flags already hold.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn count(
    text: &str,
    function: &str,
    mnemonic: &str,
) -> usize {
    let asm = driver::assembly(text, Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{function}:")).expect("the function is there");
    let rest = &asm[start..];
    let body = &rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)];
    body.lines().filter(|line| line.trim_start().starts_with(mnemonic)).count()
}

/// Two `select`s on `a < b` and `a > b` each made their `fcmp`; clang makes one
/// and reads its flags twice.
#[test]
fn two_conditions_on_one_comparison_share_it() {
    assert_eq!(count(include_str!("data/repeated_compare.ll"), "both", "fcmp"), 1);
}

/// Operands that changed in between are a different comparison: both stay.
#[test]
fn a_comparison_of_a_changed_operand_is_made_again() {
    assert_eq!(count(include_str!("data/repeated_compare.ll"), "redefined", "subs"), 2);
}
