//! Cells of the published timing tables of the CPUs after the 68000 and 68020,
//! each with its table and page.

use llrm_m68k::isa::Cpu;
use llrm_m68k::timing::cost_on;

fn clocks(
    cpu: Cpu,
    text: &str,
) -> u32 {
    cost_on(cpu, text, 2, true).unwrap_or_else(|why| panic!("{why}")).clocks
}

fn m68010(text: &str) -> u32 {
    clocks(Cpu::M68010, text)
}

/// MC68000UM Table 9-2 (p. 9-3), `(d16,An)` to `(d16,An)`: 20(4/1).
#[test]
fn m68010_move_word_between_displacements() {
    assert_eq!(m68010("move.w (4,%a0), (8,%a1)"), 20);
}

/// Table 9-4 (p. 9-4), long, `-(An)` source to `(xxx).L` destination.
#[test]
fn m68010_move_long_predecrement_to_absolute() {
    assert_eq!(m68010("move.l -(%a0), 0x1000:l"), 30);
}

/// Table 9-6 (p. 9-5): ADD Dn,<M> long is 12(1/2)+, plus 8 for `(An)` long
/// (Table 9-1).
#[test]
fn m68010_add_long_to_memory() {
    assert_eq!(m68010("add.l %d0, (%a0)"), 20);
}

/// Table 9-6 (p. 9-5): MULS 42(1/0)+*, DIVU 108(1/0)+, DIVS 122(1/0)+; the
/// 68000's are 70, 140, 158.
#[test]
fn m68010_multiply_and_divide_are_faster_than_the_68000s() {
    assert_eq!(m68010("muls.w %d1, %d0"), 42);
    assert_eq!(m68010("divu.w (%a0), %d0"), 108 + 4);
    assert_eq!(m68010("divs.w %d1, %d0"), 122);
}

/// Table 9-10 (p. 9-7): CLR.W (An) is 8(1/1) with no read of the operand; the
/// 68000's is 12.
#[test]
fn m68010_clear_does_not_read_its_operand() {
    assert_eq!(m68010("clr.w (%a0)"), 8);
    assert_eq!(clocks(Cpu::M68000, "clr.w (%a0)"), 12);
    assert_eq!(m68010("clr.l 0x1000:l"), 20);
}

/// Table 9-15 (p. 9-10): Bcc byte not taken 6(1/0), word not taken 10(2/0),
/// taken 10(2/0); DBcc cc false: taken 10, expired 16.
#[test]
fn m68010_branches() {
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 2, false).unwrap().clocks, 6);
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 4, false).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 2, true).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "dbf %d0, .L1", 4, true).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "dbf %d0, .L1", 4, false).unwrap().clocks, 16);
}

/// Table 9-16 (p. 9-10): MOVEM M->R long (An)+ is 12+8n, R->M long -(An) is
/// 8+8n.
#[test]
fn m68010_movem() {
    assert_eq!(m68010("movem.l (%sp)+, %d2-%d4"), 12 + 8 * 3);
    assert_eq!(m68010("movem.l %d2-%d4, -(%sp)"), 8 + 8 * 3);
}

/// Table 9-16 (p. 9-10) prints MOVEM.L (An),<list> as 24+8n; DEVIATE (12+8n, as
/// (An)+ and the 68000).
#[test]
fn m68010_movem_long_from_address_register_indirect_follows_its_neighbour() {
    assert_eq!(m68010("movem.l (%a0), %d2-%d4"), 12 + 8 * 3);
}

/// Table 9-18 (p. 9-12): RTS 16(4/0), RTE short 24(6/0), LINK 16(2/2), UNLK
/// 12(3/0).
#[test]
fn m68010_control_flow() {
    assert_eq!(m68010("rts"), 16);
    assert_eq!(m68010("rte"), 24);
    assert_eq!(m68010("link %a6, #-4"), 16);
    assert_eq!(m68010("unlk %a6"), 12);
}

/// Table 9-12 (p. 9-8): shifts are 6+2n (byte/word) and 8+2n (long).
#[test]
fn m68010_shifts() {
    assert_eq!(m68010("lsl.w #3, %d0"), 12);
    assert_eq!(m68010("asr.l #2, %d1"), 12);
}

/// Table 9-14 (p. 9-9): BTST Dn,<ea> byte 4(1/0)+; BSET #,Dn long 12(2/0)*.
#[test]
fn m68010_bit_operations() {
    assert_eq!(m68010("btst %d0, (%a0)"), 4 + 4);
}

/// An operand in the assembler's syntax for the mode a manual label names.
fn operand_for(label: &str) -> &'static str {
    match label.replace('\u{2013}', "-").replace([' ', '*'], "").as_str() {
        "Dn" => "%d0",
        "An" => "%a0",
        "(An)" => "(%a0)",
        "(An)+" => "(%a0)+",
        "-(An)" => "-(%a0)",
        "(d16,An)" => "(4,%a0)",
        "(d8,An,Xn)" => "(4,%a0,%d1)",
        "(xxx).W" => "0x10:w",
        "(xxx).L" => "0x1000:l",
        "(d16,PC)" => "(4,%pc)",
        "(d8,PC,Xn)" => "(4,%pc,%d1)",
        "#<data>" => "#5",
        other => panic!("no operand for {other:?}"),
    }
}

/// Every cell of Tables 9-2 and 9-4 (pp. 9-3, 9-4), the whole move grid.
#[test]
fn m68010_whole_move_grid() {
    for (suffix, csv) in
        [("w", include_str!("../src/timing/m68010/t9-2.csv")), ("l", include_str!("../src/timing/m68010/t9-4.csv"))]
    {
        let rows: Vec<Vec<String>> = csv_rows(csv);
        let destinations: Vec<&str> =
            rows[0][1..].iter().map(|name| name.trim_start_matches("Destination / ")).collect();
        for row in &rows[1..] {
            for (column, cell) in row[1..].iter().enumerate() {
                let expected: u32 = cell.split('(').next().unwrap().parse().unwrap();
                let text = format!("move.{suffix} {}, {}", operand_for(&row[0]), operand_for(destinations[column]));
                assert_eq!(
                    m68010(&text),
                    expected,
                    "{text}: Table 9-{} source {} destination {}",
                    if suffix == "w" { 2 } else { 4 },
                    row[0],
                    destinations[column]
                );
            }
        }
    }
}

/// The CSV rows after the title row, quotes honoured.
fn csv_rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1)
        .map(|line| {
            let mut fields = vec![String::new()];
            let mut quoted = false;
            for c in line.chars() {
                match c {
                    '"' => quoted = !quoted,
                    ',' if !quoted => fields.push(String::new()),
                    c => fields.last_mut().unwrap().push(c),
                }
            }
            fields
        })
        .collect()
}

fn m68030(text: &str) -> u32 {
    clocks(Cpu::M68030, text)
}

/// MC68030UM 11.6.6 (p. 11-37), I-cache case: MOVE Rn,Dn 2; MOVE SOURCE,(An)+ 4
/// plus the fetch of a `(An)+` source (11.6.1, p. 11-26: 3).
#[test]
fn m68030_moves() {
    assert_eq!(m68030("move.w %d0, %d1"), 2);
    assert_eq!(m68030("move.l (%a0)+, (%a1)+"), 4 + 3);
}

/// 11.6.8 (p. 11-40): ADD Dn,EA 3 plus fea `(An)` 3; DIVU.W 44; MULU.L 44 plus
/// the immediate fetch `#<data>.W,Dn` 2 (11.6.2, p. 11-28).
#[test]
fn m68030_arithmetic() {
    assert_eq!(m68030("add.l %d0, (%a0)"), 3 + 3);
    assert_eq!(m68030("divu.w %d1, %d0"), 44);
    assert_eq!(m68030("mulu.l %d1, %d0"), 44 + 2);
}

/// 11.6.16 (p. 11-49): RTS 9, UNLK 5, LINK.W 4; 11.6.15 (p. 11-48): Bcc taken
/// 6, not taken byte 4, DBcc expired 10.
#[test]
fn m68030_control() {
    assert_eq!(m68030("rts"), 9);
    assert_eq!(m68030("unlk %a6"), 5);
    assert_eq!(m68030("link.w %a6, #-4"), 4);
    assert_eq!(cost_on(Cpu::M68030, "bne .L1", 2, true).unwrap().clocks, 6);
    assert_eq!(cost_on(Cpu::M68030, "bne .L1", 2, false).unwrap().clocks, 4);
    assert_eq!(cost_on(Cpu::M68030, "dbf %d0, .L1", 4, false).unwrap().clocks, 10);
}

/// 11.6.14 (p. 11-47): BFTST Dn 8; BFEXTU Mem (<5 bytes) 12 plus the immediate
/// address `#<data>.W,(An)` (11.6.4, p. 11-33: 2).
#[test]
fn m68030_bit_fields() {
    assert_eq!(m68030("bftst %d0{4:8}"), 8);
    assert_eq!(m68030("bfextu (%a0){4:8}, %d0"), 12 + 2);
}

/// 11.6.12 (p. 11-45): LSd #,Dy 4; ASL Dx,Dy 8; 11.6.13 (p. 11-46): BTST #,Dn
/// 4, BSET Dn,Mem 6 plus fea `(An)` 3; 11.6.7 (p. 11-39): MOVEM RL,EA 4+2n plus
/// the register-mask address `#<data>.W,-(An)` (2).
#[test]
fn m68030_shifts_bits_and_movem() {
    assert_eq!(m68030("lsl.l #2, %d0"), 4);
    assert_eq!(m68030("asl.l %d1, %d0"), 8);
    assert_eq!(m68030("btst #3, %d0"), 4);
    assert_eq!(m68030("bset %d0, (%a0)"), 6 + 3);
    assert_eq!(m68030("movem.l %d2-%d4, -(%sp)"), 4 + 2 * 3 + 2);
}

fn m68040(text: &str) -> u32 {
    clocks(Cpu::M68040, text)
}

/// MC68040UM Section 10: the clocks are the busiest stage, `max(<ea> calculate,
/// execute base)`; the lead time is hidden. Cells below are `<ea> calculate`,
/// `execute` of the printed tables. 10.6 (p. 10-13): ADD (An) 1, 1; ADDA (An)+
/// 2, 1L + 2. 10.4 (p. 10-9): MOVE (An)+ to (An)+ 2, 1L + 1.
#[test]
fn m68040_integer_cells() {
    assert_eq!(m68040("add.l (%a0), %d0"), 1);
    assert_eq!(m68040("adda.l (%a0)+, %a1"), 2);
    assert_eq!(m68040("move.l (%a0)+, (%a1)+"), 2);
    assert_eq!(m68040("move.w %d0, %d1"), 1);
}

/// 10.5 (pp. 10-11, 10-12): RTS 5, 5; UNLK 2, 1L + 1; LINK 3, 2L + 1; BSR 2, 1L
/// + 1; Bcc taken 2, 2; not taken 3, 3.
#[test]
fn m68040_control_flow() {
    assert_eq!(m68040("rts"), 5);
    assert_eq!(m68040("unlk %a6"), 2);
    assert_eq!(m68040("link %a6, #-4"), 3);
    assert_eq!(m68040("bsr .L1"), 2);
    assert_eq!(cost_on(Cpu::M68040, "bne .L1", 2, true).unwrap().clocks, 2);
    assert_eq!(cost_on(Cpu::M68040, "bne .L1", 2, false).unwrap().clocks, 3);
}

/// 10.6: DIVS.W Dn 8, 27 (p. 10-20); MULS.W Dn 1, 16/20 and MULU.L 1, 14/20 (p.
/// 10-25, word/long); ASL Dn 1, 3/4 and ASR/LSL/LSR Dn 1, 2/3 (p. 10-14,
/// immediate/register count).
#[test]
fn m68040_multiply_divide_and_shifts() {
    assert_eq!(m68040("divs.w %d1, %d0"), 27);
    assert_eq!(m68040("muls.w %d1, %d0"), 16);
    assert_eq!(m68040("mulu.l %d1, %d0"), 20);
    assert_eq!(m68040("asl.l #2, %d0"), 3);
    assert_eq!(m68040("asl.l %d1, %d0"), 4);
    assert_eq!(m68040("lsl.l #2, %d0"), 2);
    assert_eq!(m68040("lsl.l %d1, %d0"), 3);
}

/// 10.6 (p. 10-23): MOVEM <list>,-(An) calculates `2 + D' + A'` and executes
/// `1L + 1 + D' + A'`: n = 3 gives 5.
#[test]
fn m68040_movem_counts_its_registers() {
    assert_eq!(m68040("movem.l %d2-%d4, -(%sp)"), 5);
}

/// The 68040 tables have no NOP row; it is an error, not zero clocks.
#[test]
fn m68040_nop_is_not_in_the_tables() {
    assert!(cost_on(Cpu::M68040, "nop", 2, true).is_err());
}

fn m68060(text: &str) -> u32 {
    clocks(Cpu::M68060, text)
}

fn m68060_branch(
    text: &str,
    taken: bool,
) -> u32 {
    cost_on(Cpu::M68060, text, 2, taken).unwrap().clocks
}

/// M68060UM Tables 10-6 and 10-7 (p. 10-15): MOVE (An)->(An) 2(1/1), Dn->Dn
/// 1(0/0), #<data>->(d16,An) 2(0/1), (xxx).L->(d8,An,Xi*SF) 2(1/1). Table 10-5
/// (p. 10-14) adds 0 for these modes.
#[test]
fn m68060_move_cells() {
    assert_eq!(m68060("move.w (%a0), (%a1)"), 2);
    assert_eq!(m68060("move.l %d0, %d1"), 1);
    assert_eq!(m68060("move.l #5, (4,%a0)"), 2);
    assert_eq!(m68060("move.l 0x1000:l, (4,%a0,%d1)"), 2);
    assert_eq!(m68060("movea.l (%a0)+, %a1"), 1);
    assert_eq!(m68060("moveq #1, %d0"), 1);
}

/// Every full-format-free cell of Tables 10-6 and 10-7 (p. 10-15), the whole
/// move grid.
#[test]
fn m68060_whole_move_grid() {
    let operand = |label: &str| operand_for(&label.replace("Xi*SF", "Xn").replace("(xxx).WL", "(xxx).L"));
    for (suffix, csv) in
        [("w", include_str!("../src/timing/m68060/t10-6.csv")), ("l", include_str!("../src/timing/m68060/t10-7.csv"))]
    {
        let rows = csv_rows(csv);
        let destinations: Vec<&str> =
            rows[0][1..].iter().map(|name| name.trim_start_matches("Destination / ")).collect();
        for row in rows[1..].iter().filter(|row| !row[0].contains("bd")) {
            for (column, cell) in
                row[1..].iter().enumerate().filter(|(column, _)| !destinations[*column].contains("bd"))
            {
                let expected: u32 = cell.split('(').next().unwrap().parse().unwrap();
                let mnemonic = if destinations[column] == "An" { "movea" } else { "move" };
                let text = format!("{mnemonic}.{suffix} {}, {}", operand(&row[0]), operand(destinations[column]));
                assert_eq!(m68060(&text), expected, "{text}: source {} destination {}", row[0], destinations[column]);
            }
        }
    }
}

/// Table 10-9 (p. 10-16): ADD <ea>,Dn 1(1/0), ADD Dn,<M> 1(1/1); DIVU.L 38,
/// MULU.L 2 with one more cycle for an absolute operand (footnote 3); DIVS.W is
/// "<=22".
#[test]
fn m68060_standard_cells() {
    assert_eq!(m68060("add.l (%a0), %d0"), 1);
    assert_eq!(m68060("add.l %d0, (%a0)"), 1);
    assert_eq!(m68060("sub.w %d1, %d0"), 1);
    assert_eq!(m68060("divu.l %d1, %d0"), 38);
    assert_eq!(m68060("mulu.l %d1, %d0"), 2);
    assert_eq!(m68060("muls.l 0x1000:l, %d0"), 3);
    assert_eq!(m68060("muls.w 0x1000:l, %d0"), 2);
    assert_eq!(m68060("divs.w %d1, %d0"), 22);
    assert!(cost_on(Cpu::M68060, "divs.w %d1, %d0", 2, true).unwrap().partial);
    assert!(!cost_on(Cpu::M68060, "divu.l %d1, %d0", 4, true).unwrap().partial);
}

/// Tables 10-10, 10-12 to 10-16 (pp. 10-17 to 10-20): ADDI.L #,(d16,An) 2;
/// CLR.L (An) 1; shift Dn 1, shift (An) 1; BSET #,Dn 1; BTST Dn,(An) 1; BFEXTU
/// (An) "<5 bytes" 6; BFCHG Dn 8.
#[test]
fn m68060_immediate_shift_and_bit_cells() {
    assert_eq!(m68060("addi.l #5, (4,%a0)"), 2);
    assert_eq!(m68060("addq.l #1, %a0"), 1);
    assert_eq!(m68060("clr.l (%a0)"), 1);
    assert_eq!(m68060("lsl.l #2, %d0"), 1);
    assert_eq!(m68060("asr.w (%a0)"), 1);
    assert_eq!(m68060("bset #3, %d0"), 1);
    assert_eq!(m68060("btst %d1, (%a0)"), 1);
    assert_eq!(m68060("bfextu (%a0){0:8}, %d0"), 6);
    assert_eq!(m68060("bfchg %d0{0:8}"), 8);
    assert!(cost_on(Cpu::M68060, "bfchg %d0{0:8}", 4, true).unwrap().partial);
}

/// Tables 10-17 to 10-19 (p. 10-21): the predicted columns: Bcc taken 0, not
/// taken 1; BRA 0; BSR 1; DBRA 1; RTS 7(1/0), RTD 7, RTR 8; JSR (xxx).WL 1,
/// remaining JMP 5.
#[test]
fn m68060_control_flow() {
    assert_eq!(m68060_branch("bne .L1", true), 0);
    assert_eq!(m68060_branch("bne .L1", false), 1);
    assert_eq!(m68060("bra .L1"), 0);
    assert_eq!(m68060("bsr .L1"), 1);
    assert_eq!(m68060_branch("dbf %d0, .L1", true), 1);
    assert_eq!(m68060("rts"), 7);
    assert_eq!(m68060("rtr"), 8);
    assert_eq!(m68060("jsr .L1"), 1);
    assert_eq!(m68060("jmp (%a0)"), 5);
    assert!(cost_on(Cpu::M68060, "bne .L1", 2, true).unwrap().partial);
}

/// Table 10-20 (p. 10-22): MOVEM Mem->Reg (An)+ is n, (d8,An,Xi*SF) 1+n;
/// Reg->Mem -(An) n; LEA (d16,An) 1; PEA (d16,An) 2.
#[test]
fn m68060_movem_lea_pea() {
    assert_eq!(m68060("movem.l (%sp)+, %d2-%d4"), 3);
    assert_eq!(m68060("movem.l (4,%a0,%d1), %d2-%d5"), 5);
    assert_eq!(m68060("movem.l %d2-%d4, -(%sp)"), 3);
    assert_eq!(m68060("lea (4,%a0), %a1"), 1);
    assert_eq!(m68060("pea (4,%a0)"), 2);
}

/// Tables 10-21 to 10-24 (pp. 10-22 to 10-24): ADDX Dy,Dx 1, -(Ay),-(Ax) 2;
/// CHK.W <ea> 2; ANDI to CCR 1; MOVE from SR 1; ORI to SR 5; MOVEC SFC 11/12,
/// CACR 14/15; NOP 9; EXT.W 1; MOVE16 18.
#[test]
fn m68060_status_and_miscellaneous_cells() {
    assert_eq!(m68060("addx.l %d0, %d1"), 1);
    assert_eq!(m68060("addx.l -(%a0), -(%a1)"), 2);
    assert_eq!(m68060("chk.w (%a0), %d0"), 2);
    assert_eq!(m68060("andi.b #1, %ccr"), 1);
    assert_eq!(m68060("move.w %sr, %d0"), 1);
    assert_eq!(m68060("ori.w #1, %sr"), 5);
    assert_eq!(m68060("movec %sfc, %d0"), 11);
    assert_eq!(m68060("movec %d0, %cacr"), 15);
    assert_eq!(m68060("nop"), 9);
    assert_eq!(m68060("ext.w %d0"), 1);
    assert_eq!(m68060("move16 (%a0)+, (%a1)+"), 18);
}

/// The 68060 tables have no cell for floating point: an error, not zero clocks.
#[test]
fn m68060_unlisted_is_an_error() {
    assert!(cost_on(Cpu::M68060, "fadd.x %fp0, %fp1", 4, true).is_err());
    assert!(cost_on(Cpu::M68060, "add.l (%a0)+, %d0", 2, true).is_ok());
}

/// MC68040UM 10.5 (pp. 10-11, 10-12): PACK Dx 1, 3 and -(Ay) 3, 2L + 3; UNPK
/// -(Ay) 3, 2L + 4; MOVE16 (Ax)+,(Ay)+ 6, 1L + 7 and xxx.L,(An) 4, 7; 10.6 (p.
/// 10-24) MOVES Rn,(An); (p. 10-18) CMP2 (An).
#[test]
fn m68040_pack_move16_moves_and_cmp2() {
    assert_eq!(m68040("pack %d0, %d1, #0"), 3);
    assert_eq!(m68040("unpk -(%a0), -(%a1), #0"), 4);
    assert_eq!(m68040("move16 (%a0)+, (%a1)+"), 7);
    assert_eq!(m68040("move16 0x1000:l, (%a0)"), 7);
    assert!(cost_on(Cpu::M68040, "moves.l %d0, (%a0)", 4, true).is_ok());
    assert!(cost_on(Cpu::M68040, "cmp2.l (%a0), %d0", 4, true).is_ok());
}
