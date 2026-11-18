//! A switch whose default path pays the peeled, table and last tests in turn
//! tests the range of all its cases first.

use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::Line;

/// switch_peeled_table -Os: a value in none of the cases (225 of 256 calls)
/// paid three range tests, 163 T a call: the cell ran 88.6K T against SDCC's
/// 80.1K. The first test is now the span of every case, 28 (192..=219), not
/// the peeled run's 22, and the last case (219) is what the tests leave.
#[test]
fn a_value_in_no_case_pays_one_range_test_at_os() {
    let ir = include_str!("data/switch_root_range.ll");
    let lines = llrm_z80::compile::lines(ir, "pick", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let first = lines
        .iter()
        .find_map(|line| match line {
            Line::Inst(text) if text.contains("sub a, #") => Some(text.as_str()),
            _ => None,
        })
        .expect("a range test");
    assert!(first.ends_with("#28"), "the first test is `{first}`; the span of the cases is 28, the peeled run's 22");
    let last = lines.iter().any(|line| matches!(line, Line::Inst(text) if text.contains("#219")));
    assert!(!last, "the cases fill 192..=219, so 219 is what is left and needs no test of its own (8 B)");
}
