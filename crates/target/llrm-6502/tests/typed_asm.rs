use llrm_6502::asm::{Asm, Line};

/// Every addressing form the emitter prints comes back as it went in: the
/// passes rewrite a typed line, and a form printed differently would reach ca65
/// as another instruction.
#[test]
fn a_typed_line_prints_the_text_it_was_parsed_from() {
    let text = "f:\n  lda #$ff\n  lda #<(src+198)\n  lda rc2\n  sta s_f_1+1\n  lda (rc2),y\n  sta (rc4,x)\n  jmp (vec)\n  \
                lda buf,x\n  ldx rc2,y\n  asl a\n  asl rc7\n  tya\n  bne f\n  jsr __mulhi3\n  rts\n.export f\n";
    assert_eq!(Asm::parse(text).text(), text);
    let lines = &Asm::parse(text).lines;
    assert!(matches!(lines[0], Line::Label(_)) && matches!(lines[16], Line::Raw(_)));
}

/// A line half written is not lost, and one the table does not know is kept as
/// it is.
#[test]
fn text_written_in_pieces_and_unknown_lines_survive() {
    use std::fmt::Write;
    let mut asm = Asm::new();
    write!(asm, "  lda ").unwrap();
    writeln!(asm, "rc2").unwrap();
    asm.push_str("  xyz 3\n  sta rc3\n");
    assert_eq!(asm.text(), "  lda rc2\n  xyz 3\n  sta rc3\n");
}

/// Sizes follow the addressing form: zero-page cells are two bytes, the
/// indexed forms the 6502 has in zero page too, everything the assembler
/// places is counted as three.
#[test]
fn an_instruction_is_as_long_as_its_addressing_form() {
    let bytes = |line: &str| match &Asm::parse(&format!("{line}\n")).lines[0] {
        Line::Inst(inst) => inst.bytes(),
        other => panic!("{other:?}"),
    };
    for (line, want) in [
        ("  rts", 1),
        ("  asl a", 1),
        ("  lda #1", 2),
        ("  lda rc2", 2),
        ("  lda (rc2),y", 2),
        ("  lda rc2,x", 2),
        ("  lda rc2,y", 3),
        ("  ldx rc2,y", 2),
        ("  ldx rc2,x", 3),
        ("  lda buf", 3),
        ("  lda s_f_1", 3),
        ("  jsr f", 3),
        ("  jmp f", 3),
        ("  bne f", 2),
        ("  bra f", 2),
    ] {
        assert_eq!(bytes(line), want, "{line}");
    }
}

/// The cost of a form comes from the timing tables, not from a list of
/// opcodes: a step is 1 byte and 2 clocks in `x` or `y`, 2 and 5 on a cell, 3
/// and 4 in `a` (`clc; adc #1`; one byte less on the 65C02's `inc a`), and a
/// compare costs the same in any of the three registers.
#[test]
fn a_form_is_priced_from_the_timing_tables_by_its_register() {
    use llrm_6502::alloc::form_cost;
    use llrm_6502::dagisel::hooks::Cpu;
    use llrm_6502::reg;
    use llrm_spike_mf::{Kind, Operand, Reg};

    let cell = llrm_lir::registers::RegId::from_raw(reg::RC0.index() as u8 + 3);
    let step = |cpu, kind, register| {
        let use_ = Operand::Use(Reg::Phys(register));
        let priced = form_cost(cpu, kind, &[Operand::Def(Reg::Phys(register)), use_], &[register, register]).unwrap();
        (priced.bytes, priced.cycles)
    };
    for kind in [Kind::Op("INC"), Kind::Op("DEC")] {
        assert_eq!(step(Cpu::Mos6502, kind, reg::X), (1, 2));
        assert_eq!(step(Cpu::Mos6502, kind, reg::Y), (1, 2));
        assert_eq!(step(Cpu::Mos6502, kind, cell), (2, 5));
        assert_eq!(step(Cpu::Mos6502, kind, reg::A), (3, 4));
        assert_eq!(step(Cpu::W65c02, kind, reg::A), (1, 2));
    }
    let compare = |register| {
        let flags = Operand::Def(Reg::Phys(reg::FLAGS));
        let priced = form_cost(
            Cpu::Mos6502,
            Kind::Op("CMP"),
            &[flags, Operand::Use(Reg::Phys(register)), Operand::Imm(7)],
            &[reg::FLAGS, register],
        )
        .unwrap();
        (priced.bytes, priced.cycles)
    };
    assert_eq!(compare(reg::A), compare(reg::X));
    assert_eq!(compare(reg::A), compare(reg::Y));
}
