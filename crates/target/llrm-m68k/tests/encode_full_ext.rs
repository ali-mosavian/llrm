//! The PC-relative indexed forms (`lea (disp,%pc,%d1.l*4), %a0`, `jsr`) carry
//! the 68020 full extension word. `encode` stopped at it ("operand 2 is not
//! encodable as IndexSuppress"): `encodeScale` and `encodeIndexSuppress` were
//! not translated, so `llrm-cc -c` could not emit any row that takes it.
//! Expected bytes are binutils' (`as -m68020`, a 100000 displacement so it
//! picks the 32-bit base displacement the pin always writes).

use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

fn bytes(
    opcode: &'static str,
    destination: Option<Operand>,
    index: Operand,
    scale: i64,
) -> Result<String, String> {
    let mut operands: Vec<Operand> = destination.into_iter().collect();
    operands.extend([Operand::Imm(100_000), index, Operand::Imm(scale)]);
    let encoded = encode(&Inst { opcode, operands })?;
    Ok(encoded.bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(""))
}

#[test]
fn the_pc_relative_full_extension_word_is_binutils() {
    let a0 = || Some(Operand::Reg(reg::A0));
    for (opcode, index, scale, want) in [
        ("LEA32l", Operand::Reg(reg::D1), 1, "41fb1930000186a0"),
        ("LEA32l", Operand::Reg(reg::D1), 2, "41fb1b30000186a0"),
        ("LEA32l", Operand::Reg(reg::D1), 4, "41fb1d30000186a0"),
        ("LEA32l", Operand::Reg(reg::D1), 8, "41fb1f30000186a0"),
        ("LEA32L", Operand::Reg(reg::D1), 1, "41fb1130000186a0"),
        ("LEA32l", Operand::Reg(reg::A3), 4, "41fbbd30000186a0"),
    ] {
        assert_eq!(bytes(opcode, a0(), index, scale).as_deref(), Ok(want), "{opcode} scale {scale}");
    }
    assert_eq!(bytes("CALLl", None, Operand::Reg(reg::D1), 1).as_deref(), Ok("4ebb1930000186a0"));
}

#[test]
fn a_scale_other_than_1_2_4_8_is_refused() {
    let why = bytes("LEA32l", Some(Operand::Reg(reg::A0)), Operand::Reg(reg::D1), 3).unwrap_err();
    assert!(why.contains("scale 3"), "{why}");
}
