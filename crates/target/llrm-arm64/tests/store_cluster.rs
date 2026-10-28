//! Stores to neighbouring slots made adjacent.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/store_cluster.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// Two stores with the second value computed between them were two `str`: the allocator reused the first
/// value's register for the second. clang computes both, then `stp`.
#[test]
fn stores_with_a_value_computed_between_them_pair() {
    assert!(body("g").contains("stp x9, x10, [x0]"), "{}", body("g"));
    assert!(body("f").contains("stp x"), "{}", body("f"));
}

/// A store through another pointer between them may alias: they stay apart.
#[test]
fn a_store_that_may_alias_keeps_them_apart() {
    assert!(!body("across_a_store_that_may_alias").contains("stp"), "{}", body("across_a_store_that_may_alias"));
}
