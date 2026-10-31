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

/// Table 9-6 (p. 9-5): ADD Dn,<M> long is 12(1/2)+, plus 8 for `(An)` long (Table 9-1).
#[test]
fn m68010_add_long_to_memory() {
    assert_eq!(m68010("add.l %d0, (%a0)"), 20);
}

/// Table 9-6 (p. 9-5): MULS 42(1/0)+*, DIVU 108(1/0)+, DIVS 122(1/0)+; the 68000's are 70, 140, 158.
#[test]
fn m68010_multiply_and_divide_are_faster_than_the_68000s() {
    assert_eq!(m68010("muls.w %d1, %d0"), 42);
    assert_eq!(m68010("divu.w (%a0), %d0"), 108 + 4);
    assert_eq!(m68010("divs.w %d1, %d0"), 122);
}

/// Table 9-10 (p. 9-7): CLR.W (An) is 8(1/1) with no read of the operand; the 68000's is 12.
#[test]
fn m68010_clear_does_not_read_its_operand() {
    assert_eq!(m68010("clr.w (%a0)"), 8);
    assert_eq!(clocks(Cpu::M68000, "clr.w (%a0)"), 12);
    assert_eq!(m68010("clr.l 0x1000:l"), 20);
}

/// Table 9-15 (p. 9-10): Bcc byte not taken 6(1/0), word not taken 10(2/0), taken 10(2/0); DBcc cc false: taken 10, expired 16.
#[test]
fn m68010_branches() {
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 2, false).unwrap().clocks, 6);
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 4, false).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "bne .L1", 2, true).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "dbf %d0, .L1", 4, true).unwrap().clocks, 10);
    assert_eq!(cost_on(Cpu::M68010, "dbf %d0, .L1", 4, false).unwrap().clocks, 16);
}

/// Table 9-16 (p. 9-10): MOVEM M->R long (An)+ is 12+8n, R->M long -(An) is 8+8n.
#[test]
fn m68010_movem() {
    assert_eq!(m68010("movem.l (%sp)+, %d2-%d4"), 12 + 8 * 3);
    assert_eq!(m68010("movem.l %d2-%d4, -(%sp)"), 8 + 8 * 3);
}

/// Table 9-16 (p. 9-10) prints MOVEM.L (An),<list> as 24+8n; DEVIATE (12+8n, as (An)+ and the 68000).
#[test]
fn m68010_movem_long_from_address_register_indirect_follows_its_neighbour() {
    assert_eq!(m68010("movem.l (%a0), %d2-%d4"), 12 + 8 * 3);
}

/// Table 9-18 (p. 9-12): RTS 16(4/0), RTE short 24(6/0), LINK 16(2/2), UNLK 12(3/0).
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
    for (suffix, csv) in [("w", include_str!("../src/timing/m68010/t9-2.csv")), ("l", include_str!("../src/timing/m68010/t9-4.csv"))] {
        let rows: Vec<Vec<String>> = csv_rows(csv);
        let destinations: Vec<&str> = rows[0][1..].iter().map(|name| name.trim_start_matches("Destination / ")).collect();
        for row in &rows[1..] {
            for (column, cell) in row[1..].iter().enumerate() {
                let expected: u32 = cell.split('(').next().unwrap().parse().unwrap();
                let text = format!("move.{suffix} {}, {}", operand_for(&row[0]), operand_for(destinations[column]));
                assert_eq!(m68010(&text), expected, "{text}: Table 9-{} source {} destination {}", if suffix == "w" { 2 } else { 4 }, row[0], destinations[column]);
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
