//! `(x >> 23) & 255` was `moveq #23; lsr.l; and.l #255` plus a callee-saved
//! register (fix_usi 68020: 7 clocks a call over gcc, which uses `bfextu`).

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @exponent(i32 %x) {
  %s = lshr i32 %x, 23
  %e = and i32 %s, 255
  ret i32 %e
}
define i32 @whole(i32 %x) {
  %s = lshr i32 %x, 5
  %e = and i32 %s, 1023
  ret i32 %e
}";

#[test]
fn a_shifted_low_mask_is_one_bfextu_from_the_68020() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let out = assembly(IR, cpu_of("68020").unwrap(), level).expect("compiles");
        assert!(out.contains("{#1:#8}"), "{level:?}: exponent field\n{out}");
        assert!(out.contains("{#17:#10}"), "{level:?}: a 10-bit field\n{out}");
        assert!(!out.contains("lsr.l"), "{level:?}: the shift stays\n{out}");
    }
}

#[test]
fn the_68000_has_no_bitfield_instruction() {
    let out = assembly(IR, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("bfextu"), "{out}");
}

/// GNU as: `bfextu %d0{#1:#8},%d1` is `e9c0 1048`, `%d3{#17:#10},%d7` is `e9c3
/// 744a`.
#[test]
fn bfextu_encodes_as_gnu_as_does() {
    let inst = |dst, src, off, wid| Inst {
        opcode: "BFEXTU32dd",
        operands: vec![Operand::Reg(dst), Operand::Reg(src), Operand::Imm(off), Operand::Imm(wid)],
    };
    assert_eq!(encode(&inst(reg::D1, reg::D0, 1, 8)).unwrap().bytes, [0xe9, 0xc0, 0x10, 0x48]);
    assert_eq!(encode(&inst(reg::D7, reg::D3, 17, 10)).unwrap().bytes, [0xe9, 0xc3, 0x74, 0x4a]);
}
