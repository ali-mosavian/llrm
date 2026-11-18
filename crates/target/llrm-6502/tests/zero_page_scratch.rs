//! The runtime's scratch cells (`__sa`, `__st`..) were BSS: three-byte
//! addresses in every routine that works in them (`__mulsi3` 83 B where 65 B
//! do, `__rt_udivmod32` 144 B where 107 B do). A platform row that has the
//! bytes gives them to zero page.

use llrm_6502::platform::{Platform, ROWS};

#[test]
fn the_linked_rows_give_the_runtime_its_scratch_and_the_full_ones_do_not() {
    for row in ROWS {
        let platform = Platform::parse(row).expect("a row");
        let linked = platform.name.starts_with("sim65");
        assert_eq!(platform.scratch_zero_page >= 15, linked, "{}", platform.name);
    }
}
