//! hex tests `v & 15 < 10` with `cmpi.l #10,%d2`, 6 B and 14 clk at 68000; the
//! register is below 16 and the constant fits a byte, so `cmpi.b` is 4 B and 8
//! clk and gcc narrows. The compare is narrowed after allocation, in place: as
//! a DAG truncate it was a copy into a second register (hex 88 -> 90 B).
//! A reader of N or V (a signed condition) keeps the long compare.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const HEX: &str = include_str!("../bench/library/hex.ll");

fn asm(
    ir: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

/// hex: `v & 15` against 10. The compare is a byte compare in the register
/// the value is in; the DAG version copied it to a second saved register.
#[test]
fn hexs_masked_digit_is_compared_as_a_byte_in_place() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = asm(HEX, cpu, level);
            assert!(out.contains("cmpi.b\t#10,") && !out.contains("cmpi.l\t#10,"), "{cpu} {level:?}:\n{out}");
            assert!(!out.contains("movem"), "a narrowed compare must not cost a register: {cpu} {level:?}:\n{out}");
        }
    }
}

/// Nothing is known of the register: the compare stays long.
#[test]
fn an_unbounded_register_is_compared_long() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
              define i32 @f(i32 %x, ptr %p) {\n  store i32 %x, ptr %p\n  %c = icmp ult i32 %x, 10\n  br i1 %c, label %a, label %b\na:\n  ret i32 1\nb:\n  ret i32 2\n}";
    let out = asm(ir, "68000", Level::O2);
    assert!(!out.contains("cmpi.b") && !out.contains("cmpi.w"), "{out}");
}

/// A signed branch reads N and V, which the byte compare sets from bit 7.
#[test]
fn a_signed_reader_keeps_the_long_compare() {
    use llrm_m68k::dagisel::local;
    use llrm_m68k::mfunc::{Class, MFunction, op_inst};
    use llrm_m68k::narrow_add::narrow_compares;
    use llrm_m68k::regalloc::Assignment;
    use llrm_spike_mf::mfunc::Reg;
    use llrm_spike_mf::{Block, Kind, Operand};
    let row = |name: &'static str, operands: Vec<Operand>| {
        op_inst(name, local::code_of(name).and_then(local::desc).expect(name), operands)
    };
    let mut kinds = Vec::new();
    for branch in ["Bne8", "Blt8"] {
        let mut function = MFunction::new("f");
        function.classes = vec![Class::Dr32];
        let insts = vec![
            row("MOVI32ri", vec![Operand::Def(Reg::Virt(0)), Operand::Imm(15)]),
            row("CMP32di", vec![Operand::Imm(10), Operand::Use(Reg::Virt(0))]),
            row(branch, vec![Operand::Block(0)]),
        ];
        function.blocks.push(Block { id: 0, insts, phis: vec![] });
        narrow_compares(&mut function, &Assignment(vec![llrm_m68k::abi::register("d0")]));
        kinds.push(function.blocks[0].insts[1].kind);
    }
    assert_eq!(kinds, [Kind::Op("CMP8di"), Kind::Op("CMP32di")]);
}

/// A constant a byte cannot hold, in a register below 64K: the word compare.
#[test]
fn a_constant_too_big_for_a_byte_is_compared_at_word_width() {
    use llrm_m68k::dagisel::local;
    use llrm_m68k::mfunc::{Class, MFunction, op_inst};
    use llrm_m68k::narrow_add::narrow_compares;
    use llrm_m68k::regalloc::Assignment;
    use llrm_spike_mf::mfunc::Reg;
    use llrm_spike_mf::{Block, Kind, Operand};
    let row = |name: &'static str, operands: Vec<Operand>| {
        op_inst(name, local::code_of(name).and_then(local::desc).expect(name), operands)
    };
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32];
    let insts = vec![
        row("MOVI32ri", vec![Operand::Def(Reg::Virt(0)), Operand::Imm(500)]),
        row("CMP32di", vec![Operand::Imm(300), Operand::Use(Reg::Virt(0))]),
        row("Bne8", vec![Operand::Block(0)]),
    ];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    narrow_compares(&mut function, &Assignment(vec![llrm_m68k::abi::register("d0")]));
    assert_eq!(function.blocks[0].insts[1].kind, Kind::Op("CMP16di"));
}
