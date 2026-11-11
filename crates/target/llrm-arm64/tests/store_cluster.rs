//! Stores to neighbouring slots made adjacent.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn body(name: &str) -> String {
    let asm = driver::assembly(include_str!("data/store_cluster.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)].to_owned()
}

/// Two stores with the second value computed between them were two `str`: the
/// allocator reused the first value's register for the second. clang computes
/// both, then `stp`.
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

/// Loads of neighbouring slots, each used before the next, were `ldp` for the
/// first pair and two `ldr` after: the dead value's register was reused for
/// the next load (matmul's checksum: 63 `ldr` where the pin has 33 `ldp`). The
/// pin's scheduler clusters the loads.
#[test]
fn loads_with_work_between_them_pair() {
    let text = body("a_chain_of_loads");
    assert_eq!(text.matches("ldp").count(), 2, "{text}");
}

/// The same for a frame slot, which the checksum's accesses were.
#[test]
fn slot_loads_with_work_between_them_pair() {
    let text = body("a_chain_of_slot_loads");
    let data_pairs = text.lines().filter(|line| line.contains("ldp") && (line.contains("[sp]") || line.contains("[sp, #16]"))).count();
    assert_eq!(data_pairs, 2, "{text}");
}

/// A store that may alias keeps the loads apart.
#[test]
fn a_store_that_may_alias_keeps_the_loads_apart() {
    let text = body("loads_across_a_store_that_may_alias");
    assert!(!text.contains("ldp"), "{text}");
}

/// Stores to neighbouring offsets of a frame slot, with the other slot's
/// store between them, were four `str`: `cluster` looked at registers only
/// and never saw a slot as a base (fpbench's checksum: 12 `str s` where the
/// pin has 6 `stp`).
#[test]
fn interleaved_slot_stores_pair() {
    let text = body("slot_stores_interleaved");
    assert_eq!(text.matches("stp s").count(), 2, "{text}");
}
