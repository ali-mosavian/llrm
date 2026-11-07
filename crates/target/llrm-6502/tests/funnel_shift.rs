//! A wide value shifted by one bit carries one bit from word to word through the carry of a shift, not
//! through a byte made of it and merged with `ora`.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// `k_<op>`: `op` of an `iN` loaded through a pointer by one bit, stored back.
fn shift_by_one(op: &str, bits: u32) -> String {
    format!(
        "define void @k_{op}{bits}(ptr %p) {{
  %x = load i{bits}, ptr %p
  %y = {op} i{bits} %x, 1
  store i{bits} %y, ptr %p
  ret void
}}
"
    )
}

fn bytes_of(op: &str, bits: u32, limit: u32) {
    let name = format!("k_{op}{bits}");
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), shift_by_one(op, bits));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= limit, "{name}: {bytes} B\n{body}");
}

/// Each word's top bit was a `asl a; lda #0; rol a` byte, `ora`ed into the next word.
#[test]
fn a_long_shifted_left_by_one_carries_between_its_words() {
    bytes_of("shl", 32, 60);
}

#[test]
fn a_long_shifted_right_by_one_carries_between_its_words() {
    bytes_of("lshr", 32, 60);
}

#[test]
fn a_long_long_shifted_left_by_one_carries_between_its_words() {
    bytes_of("shl", 64, 124);
}

#[test]
fn a_long_long_shifted_right_by_one_carries_between_its_words() {
    bytes_of("lshr", 64, 124);
}

/// The combiner after type legalization handed `funnel_by_one` every node, and a
/// node with one operand indexed past its end: 42 corpus programs refused at
/// -O1 with "index out of bounds: the len is 1 but the index is 1".
#[test]
fn a_late_combine_reads_only_the_nodes_it_matches() {
    let ir = include_str!("data/late_combine_node.ll");
    assembly(ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
}
