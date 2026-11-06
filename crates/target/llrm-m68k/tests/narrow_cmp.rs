//! hex tests `v & 15 < 10` with `cmpi.l #10,%d2`, 6 B and 14 clk at 68000; the
//! register is below 16 and the constant fits a byte, so `cmpi.b` is 4 B and 8
//! clk and gcc narrows. The compare is narrowed after allocation, in place: as
//! a DAG truncate it was a copy into a second register (hex 88 -> 90 B).
//! A reader of N or V (a signed condition) keeps the long compare.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// `if ((x & MASK) COND BOUND) 1 else 2`, with the masked value also stored so
/// it keeps its other use, as hex's digit does.
fn compare(
    mask: u32,
    cond: &str,
    bound: u32,
) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         define i32 @f(i32 %x, ptr %p) {{\n  %m = and i32 %x, {mask}\n  store i32 %m, ptr %p\n  \
         %c = icmp {cond} i32 %m, {bound}\n  br i1 %c, label %a, label %b\na:\n  ret i32 1\nb:\n  ret i32 2\n}}"
    )
}

fn asm(
    ir: &str,
    cpu: &str,
    level: Level,
) -> String {
    assembly(ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_masked_byte_is_compared_at_byte_width() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = asm(&compare(15, "ult", 10), cpu, level);
            assert!(out.contains("cmpi.b\t#10,") && !out.contains("cmpi.l\t#10,"), "{cpu} {level:?}:\n{out}");
            assert!(!out.contains("movem"), "a narrowed compare must not cost a register: {cpu} {level:?}:\n{out}");
        }
    }
}

/// A value below 512 against a bound a byte cannot hold is a word compare.
#[test]
fn a_constant_too_big_for_a_byte_is_compared_at_word_width() {
    let out = asm(&compare(511, "ult", 300), "68000", Level::O2);
    assert!(out.contains("cmpi.w\t#300,") && !out.contains("cmpi.l\t#300,"), "{out}");
}

/// Nothing is known of the register: the compare stays long.
#[test]
fn an_unbounded_register_is_compared_long() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
              define i32 @f(i32 %x) {\n  %c = icmp ult i32 %x, 10\n  br i1 %c, label %a, label %b\na:\n  ret i32 1\nb:\n  ret i32 2\n}";
    let out = asm(ir, "68000", Level::O2);
    assert!(out.contains("cmpi.l\t#10,") && !out.contains("cmpi.b"), "{out}");
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
