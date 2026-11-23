//! A selector that morphs a node into one that already exists must replace
//! the node's uses (`MorphNodeTo`, then `ReplaceUses`): `any_extend` of a word
//! read by two users left the second reading the unselected node, and
//! QuickrBasic `cngfx.c` was refused "operand AnyExtend before its definition".

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn an_any_extend_equal_to_a_selected_one_is_replaced() {
    let source = include_str!("../data/any_extend_shared.ll");
    for cpu in ["68000", "68020"] {
        assembly(source, cpu_of(cpu).unwrap(), Level::O2).unwrap_or_else(|why| panic!("{cpu}: {why}"));
    }
}
