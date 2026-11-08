//! `(bd32,%pc)`, the full-format PC-relative mode medium uses from the 68020:
//! its bytes and relocations against GNU as.

use llrm_m68k::encode::{Fixup, encode};
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

/// `lea (to+2,%pc,%zd0), %a0`: GNU as writes `41fb 0170 <bd32>` (the pin sets
/// the index's W/L bit, ignored with IS) and `R_68K_PC32 to+4`: the PC is the
/// extension word, 2 bytes before the displacement field. A fixup without them
/// put every medium access 2 bytes off and still read back what it stored.
#[test]
fn lea_with_a_suppressed_index_is_41fb_0170_and_pc32() {
    let inst = Inst {
        opcode: "LEA32l",
        operands: vec![Operand::Reg(reg::A0), Operand::Symbol("to".into(), 2), Operand::Imm(0), Operand::Imm(1)],
    };
    assert_eq!(print(&inst).unwrap(), "lea\t(to+2,%pc,%zd0), %a0");
    let done = encode(&inst).unwrap();
    assert_eq!(done.bytes, [0x41, 0xfb, 0x09, 0x70, 0, 0, 0, 0]);
    assert_eq!(done.fixups, [Fixup { offset: 4, symbol: "to".into(), addend: 4, size: 4, pc_rel: true }]);
}

/// `lea (to,%pc,%zd0)` was priced as `(d8,PC,Xn)` and `(to,%pc)` as the
/// 4-clock `(d16,PC)`: the full format with a 32-bit displacement is the
/// manuals' `(d32,B)` (68020 table 8-2, 68030 11.6.1), 13 clocks with the
/// cache against 5.
#[test]
fn a_full_format_pc_operand_is_priced_as_d32_b() {
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::timing::cost;
    let full = cost(Cpu::M68020, "lea (to,%pc,%zd0), %a0", 8, false).unwrap().clocks;
    let short = cost(Cpu::M68020, "lea (to,%pc), %a0", 4, false).unwrap().clocks;
    let indexed = cost(Cpu::M68020, "move.b (to,%pc,%d1.l), %d0", 8, false).unwrap().clocks;
    let plain = cost(Cpu::M68020, "move.b (to,%pc,%d1), %d0", 4, false).unwrap().clocks;
    assert!(full > short, "full {full}, short {short}");
    assert!(indexed > plain, "indexed {indexed}, plain {plain}");
    assert!(cost(Cpu::M68000, "lea (to,%pc,%zd0), %a0", 8, false).is_err());
}
