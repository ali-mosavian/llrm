//! `fastisel::parse` reads the pin's `-gen-fast-isel` output into rows.

use llrm_dagisel_gen::fastisel::{self, Shape};

const EXCERPT: &str = include_str!("fixtures/fastisel_excerpt.inc");

/// The first parser kept only the first arm of `ADD i32 rr`: the table asked for
/// `ADD32rr_ND` on a CPU without NDD. Arms keep the pin's order and conditions.
#[test]
fn add_i32_rr_keeps_both_arms_in_the_pins_order() {
    let table = fastisel::parse(EXCERPT).unwrap();
    let rows = table.rows("ISD", "ADD", "i32", Shape::Rr);
    assert_eq!(rows.ret, Some("i32".to_owned()));
    let ops: Vec<_> = rows.arms.iter().map(|a| (a.opcode.as_str(), a.class.as_str())).collect();
    assert_eq!(ops, [("ADD32rr_ND", "GR32"), ("ADD32rr", "GR32")]);
    assert_eq!(rows.arms[0].cond.as_deref(), Some("Subtarget->hasNDD()"));
    assert_eq!(rows.arms[1].cond.as_deref(), Some("!Subtarget->hasNDD()"));
}

/// A `_Predicate_` variant is a different function from the plain one: the
/// immediate must satisfy the named test.
#[test]
fn an_immediate_predicate_is_part_of_the_key() {
    let table = fastisel::parse(EXCERPT).unwrap();
    let plain = table.rows("ISD", "ADD", "i32", Shape::Ri);
    assert!(plain.imm_predicate.is_none());
    let wide = table.rows("ISD", "ADD", "i64", Shape::RiPredicated);
    assert_eq!(wide.imm_predicate.as_deref(), Some("i64immSExt32"));
}

/// Dispatchers (`fastEmit_ISD_ADD_rr`) are derived from the rows, not stored;
/// a function the parser cannot read is counted, never dropped.
#[test]
fn dispatchers_are_skipped_and_the_count_adds_up() {
    let table = fastisel::parse(EXCERPT).unwrap();
    assert_eq!(table.parsed(), 4);
    assert_eq!(table.dispatchers(), 1);
    assert_eq!(table.unread(), 0);
}
