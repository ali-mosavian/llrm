//! The store limits a memset, memcpy or memmove is inlined for come from the
//! target's tuning table, not from numbers in the builder.

const MEMOPS: &str = include_str!("builder/memops.rs");

/// A limit written as a literal in `memops.rs` is a tuning constant outside the
/// table: m16's 4 and 2 sat in the builder's `memset_stores_size` sentinel and
/// the pin's 16/8/8/4 as bare numbers beside it, two homes for one rule.
#[test]
fn memops_has_no_store_limit_outside_the_tuning_table() {
    for (at, line) in MEMOPS.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for bare in ["=> 4,", "=> 8,", "=> 16,", "{ 4 } else { 8 }"] {
            assert!(!code.contains(bare), "memops.rs:{}: a store limit as a literal (`{bare}`): {line}", at + 1);
        }
    }
}
