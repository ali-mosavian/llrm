//! The encoder cut a value to its field and went on: `(260,%sp,%a5)` in the
//! 8-bit displacement of an indexed operand became `(4,%sp,%a5)` in an object
//! from `llrm-cc -c` at 68020 (gas encodes the full extension word, so the text
//! path was right and the two outputs disagreed). A value that does not fit its
//! field is an error now.

use llrm_m68k::dagisel::local;
use llrm_m68k::dagisel::scaled;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

/// The operands of a row, by its text (as `scaled_rows.rs`): a memory operand
/// is `(disp,%a2,%d1)`, a register `%d3` or `%a3`, an immediate 5; `disp` is
/// the indexed displacement.
fn with_displacement(
    name: &'static str,
    disp: i64,
) -> Inst {
    let &(_, text, shape) = local::text(name).expect("a row");
    let mnemonic = text.split(['\t', ' ']).next().unwrap();
    let base = scaled::base_of(name).unwrap();
    let address = ["adda", "suba", "movea", "cmpa", "lea"].iter().any(|one| mnemonic.starts_with(one))
        || base.ends_with("af")
        || base.ends_with('a');
    let width = |method: &str| match () {
        _ if method.contains("ARII") => 3,
        _ if method.contains("ARID") || method.contains("PCI") => 2,
        _ => 1,
    };
    let count = shape.iter().map(|one| usize::from(one.at) + width(one.method)).max().unwrap_or(0);
    let mut operands = vec![Operand::Imm(5); count];
    for one in shape {
        let at = usize::from(one.at);
        if one.method.contains("ARII") {
            operands[at] = Operand::Imm(disp);
            operands[at + 1] = Operand::Reg(reg::A2);
            operands[at + 2] = Operand::Reg(reg::D1);
        } else if one.method.contains("PCI") {
            operands[at] = Operand::Imm(8);
            operands[at + 1] = Operand::Reg(reg::D1);
        } else if one.method.contains("ARID") {
            operands[at] = Operand::Imm(12);
            operands[at + 1] = Operand::Reg(reg::A2);
        } else if one.method.contains("ARI") {
            operands[at] = Operand::Reg(reg::A2);
        } else if one.method.contains("AL") {
            operands[at] = Operand::Imm(0x1234_5678);
        } else if one.method == "printOperand" {
            operands[at] = Operand::Reg(if address { reg::A3 } else { reg::D3 });
        }
    }
    Inst { opcode: name, operands }
}

#[test]
fn an_indexed_displacement_beyond_eight_bits_is_refused_not_cut() {
    let mut checked = 0;
    for name in scaled::names() {
        if !local::text(name).is_some_and(|&(_, _, shape)| shape.iter().any(|one| one.method.contains("ARII"))) {
            continue;
        }
        for ok in [-128, 0, 127] {
            encode(&with_displacement(name, ok)).unwrap_or_else(|why| panic!("{name}: {ok}: {why}"));
        }
        for bad in [128, 260, -129, 1 << 20] {
            let why = encode(&with_displacement(name, bad)).expect_err(&format!("{name}: {bad} was cut to a byte"));
            assert!(why.contains("does not fit its 8-bit field"), "{name}: {why}");
        }
        checked += 1;
    }
    assert!(checked > 50, "{checked} rows");
}
