//! `%a6` became allocatable and the `cmpa.l` rewrite still looked at `%a0` to
//! `%a5`: a compare whose destination was put in `%a6` stayed `cmp.l`, which
//! the encoder wrote as `cmp.l %d0, %d6`. `spill_store_order` printed four
//! wrong lines of five at 68020 -O1, while the assembly text read right.

use std::process::Command;

use llrm_m68k::encode::encode_final;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

#[test]
fn a_compare_held_in_a6_is_cmpa_in_the_text_and_the_object() {
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/a6_compare.ll");
    let build = |kind: &str| {
        let out = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
            .args([kind, "-O1", "-mcpu=68020", "-o", "-", ir])
            .output()
            .expect("runs");
        assert!(out.status.success(), "{kind}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let text = build("-S");
    let data_compare_of_address: Vec<&str> = text
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("cmp.l") && line.rsplit(", ").next().is_some_and(|to| to.starts_with("%a"))
        })
        .collect();
    assert_eq!(data_compare_of_address, Vec::<&str>::new());
    assert!(text.contains("cmpa."), "premise: a compare lives in an address register:\n{text}");
    build("-c");
}

#[test]
fn the_object_writers_refuse_a_register_of_the_other_bank() {
    let compare = |dest| Inst { opcode: "CMP32dr", operands: vec![Operand::Reg(reg::D0), Operand::Reg(dest)] };
    assert!(encode_final(&compare(reg::D6)).is_ok());
    assert!(encode_final(&compare(reg::A6)).is_err(), "%a6 was written as %d6");
}
