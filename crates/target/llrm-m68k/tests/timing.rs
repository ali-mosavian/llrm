//! The clock model against cells of Motorola's published tables: MC68000UM
//! section 8 (16-bit timing, the MC68000 column) and MC68020UM section 8 (the
//! cache case). Each test names the table and page of the number it checks.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::timing::cost;

fn clocks(
    cpu: Cpu,
    text: &str,
) -> u32 {
    cost(cpu, text, 2, false).unwrap_or_else(|why| panic!("{text}: {why}")).clocks
}

fn taken(
    cpu: Cpu,
    text: &str,
) -> u32 {
    cost(cpu, text, 2, true).unwrap().clocks
}

/// MC68000UM Table 8-3 (p. 8-3), Move Long: `Dn,Dn` 4; `(d16,An),(An)+` 24;
/// `#<data>,(xxx).L` 28 is not reachable here, `(An)+,-(An)` 20.
#[test]
fn a_68000_move_long_is_the_published_cell() {
    assert_eq!(clocks(Cpu::M68000, "move.l %d0, %d1"), 4);
    assert_eq!(clocks(Cpu::M68000, "move.l (12,%a0), (%a1)+"), 24);
    assert_eq!(clocks(Cpu::M68000, "move.l (%a0)+, -(%a1)"), 20);
    assert_eq!(clocks(Cpu::M68000, "move.l #5, %d0"), 12);
}

/// MC68000UM Table 8-2 (p. 8-2), Move Byte and Word: `(An),(d16,An)` 16;
/// `Dn,(An)` 8; `-(An),Dn` 10; `(d8,An,Xn),Dn` 14.
#[test]
fn a_68000_move_word_is_the_published_cell() {
    assert_eq!(clocks(Cpu::M68000, "move.w (%a0), (4,%a1)"), 16);
    assert_eq!(clocks(Cpu::M68000, "move.b %d0, (%a1)"), 8);
    assert_eq!(clocks(Cpu::M68000, "move.w -(%a0), %d0"), 10);
    assert_eq!(clocks(Cpu::M68000, "move.b (0,%a0,%d2), %d1"), 14);
}

/// MC68000UM Table 8-4 (p. 8-4), Standard: `ADD.L <ea>,Dn` 6+ea, 8+ea for a
/// register or immediate source; `ADD.W Dn,<M>` 8+ea; `CMP.W <ea>,Dn` 4+ea.
#[test]
fn a_68000_standard_arithmetic_is_the_published_base_plus_the_address() {
    assert_eq!(clocks(Cpu::M68000, "add.l (%a0), %d0"), 6 + 8);
    assert_eq!(clocks(Cpu::M68000, "add.l %d1, %d0"), 8);
    assert_eq!(clocks(Cpu::M68000, "add.w %d0, (4,%a0)"), 8 + 8);
    assert_eq!(clocks(Cpu::M68000, "cmp.w (%a0), %d0"), 4 + 4);
}

/// MC68000UM Table 8-5 (p. 8-5), Immediate: `ADDI.L #,Dn` 16, `ANDI.L #,Dn`
/// 14, `CMPI.L #,Dn` 14, `MOVEQ` 4.
#[test]
fn a_68000_immediate_form_is_the_published_cell() {
    assert_eq!(clocks(Cpu::M68000, "add.l #1, %d0"), 16);
    assert_eq!(clocks(Cpu::M68000, "and.l #255, %d0"), 14);
    assert_eq!(clocks(Cpu::M68000, "cmpi.l #0, %d0"), 14);
    assert_eq!(clocks(Cpu::M68000, "moveq #1, %d0"), 4);
}

/// MC68000UM Table 8-7 (p. 8-6), Shift: `LSL.L #n,Dn` 8+2n, `ASR.W #n,Dn` 6+2n.
#[test]
fn a_68000_shift_costs_two_clocks_per_place() {
    assert_eq!(clocks(Cpu::M68000, "lsl.l #1, %d0"), 10);
    assert_eq!(clocks(Cpu::M68000, "lsl.l #8, %d0"), 24);
    assert_eq!(clocks(Cpu::M68000, "asr.w #3, %d1"), 12);
}

/// `llvm-objdump` prints the quick count 8 as `#0` (the encoded field).
#[test]
fn a_shift_count_of_zero_in_a_disassembly_is_eight() {
    assert_eq!(clocks(Cpu::M68000, "lsl.l #0, %d0"), 24);
}

/// MC68000UM Table 8-9 (p. 8-7), Conditional: `Bcc` taken 10, not taken 8
/// (byte) or 12 (word); `BRA` 10; `BSR` 18.
#[test]
fn a_68000_branch_depends_on_taken_and_on_its_size() {
    assert_eq!(taken(Cpu::M68000, "bne $20"), 10);
    assert_eq!(clocks(Cpu::M68000, "bne $20"), 8);
    assert_eq!(cost(Cpu::M68000, "bne $20", 4, false).unwrap().clocks, 12);
    assert_eq!(clocks(Cpu::M68000, "bra $20"), 10);
    assert_eq!(clocks(Cpu::M68000, "bsr $20"), 18);
}

/// MC68000UM Tables 8-10 and 8-12 (pp. 8-8, 8-10): `JSR (xxx).L` 20, `LEA
/// (d16,An)` 8, `MOVEM.L R→M, -(An)` 8+8n, `RTS` 16, `LINK` 16, `UNLK` 12.
#[test]
fn a_68000_call_frame_and_movem_are_the_published_cells() {
    assert_eq!(clocks(Cpu::M68000, "jsr g"), 20);
    assert_eq!(clocks(Cpu::M68000, "lea (8,%a0), %a1"), 8);
    assert_eq!(clocks(Cpu::M68000, "movem.l %d2-%d4, -(%sp)"), 8 + 8 * 3);
    assert_eq!(clocks(Cpu::M68000, "movem.l (4,%sp), %d2-%d4/%a2"), 16 + 8 * 4);
    assert_eq!(clocks(Cpu::M68000, "rts"), 16);
    assert_eq!(clocks(Cpu::M68000, "link.w %a6, #-4"), 16);
    assert_eq!(clocks(Cpu::M68000, "unlk %a6"), 12);
}

/// MC68020UM section 8.2.6 (cache case, p. 8-23): `Rn,Dn` 2, `(An),Dn` 6,
/// `(An)+,(An)+` 7, `(d16,An),(An)+` 8, `#<data>.L,Dn` 6.
#[test]
fn a_68020_move_is_the_cache_case_cell() {
    assert_eq!(clocks(Cpu::M68020, "move.l %d0, %d1"), 2);
    assert_eq!(clocks(Cpu::M68020, "move.l (%a0), %d1"), 6);
    assert_eq!(clocks(Cpu::M68020, "move.l (%a0)+, (%a1)+"), 7);
    assert_eq!(clocks(Cpu::M68020, "move.l (4,%a0), (%a1)+"), 8);
    assert_eq!(clocks(Cpu::M68020, "move.l #5, %d1"), 6);
}

/// MC68020UM 8.2.8 (p. 8-30) with 8.2.1 (p. 8-13), cache case: `ADD EA,Dn` 2
/// plus the fetch of the address; `(d16,An)` costs 5; `ADD Dn,EA` 4 plus the
/// address.
#[test]
fn a_68020_add_is_its_base_plus_the_fetched_address() {
    assert_eq!(clocks(Cpu::M68020, "add.l %d1, %d0"), 2);
    assert_eq!(clocks(Cpu::M68020, "add.l (4,%a0), %d0"), 2 + 5);
    assert_eq!(clocks(Cpu::M68020, "add.l %d0, (%a0)"), 4 + 4);
}

/// MC68020UM 8.2.15-8.2.16 (pp. 8-37, 8-38), cache case: `Bcc` taken 6, `Bcc.B`
/// not taken 4, `RTS` 10, `LEA` 2 plus the calculated address (`(d16,An)` 2),
/// `JSR` 5 plus the jump address (`(xxx).L` 2).
#[test]
fn a_68020_control_flow_is_the_cache_case_cell() {
    assert_eq!(taken(Cpu::M68020, "bne $20"), 6);
    assert_eq!(clocks(Cpu::M68020, "bne $20"), 4);
    assert_eq!(clocks(Cpu::M68020, "rts"), 10);
    assert_eq!(clocks(Cpu::M68020, "lea (8,%a0), %a1"), 4);
    assert_eq!(clocks(Cpu::M68020, "jsr g"), 7);
}

/// The prologue of a function that keeps one callee-saved register is `movem.l
/// %d2, (0,%sp)`: its list is one register, which reads as a register operand.
/// The first cycle run left it unpriced and the totals short by its clocks.
#[test]
fn a_movem_of_one_register_is_priced() {
    assert_eq!(clocks(Cpu::M68000, "movem.l %d2, (0,%sp)"), 12 + 8);
    assert_eq!(clocks(Cpu::M68000, "movem.l (0,%sp), %d2"), 16 + 8);
    assert_eq!(clocks(Cpu::M68020, "movem.l %d2, (0,%sp)"), 4 + 3 + 4);
}

/// An instruction the tables do not cover is an error, never zero clocks.
#[test]
fn an_unmodelled_instruction_is_an_error() {
    assert!(cost(Cpu::M68000, "frobnicate %d0", 2, false).is_err());
    assert!(cost(Cpu::M68000, "move.l ($12345678).l, %d0", 6, false).is_err());
}

fn gnu(text: &str) -> String {
    llrm_m68k::timing::from_gnu(text).unwrap_or_else(|why| panic!("{text}: {why}"))
}

/// binutils prints `addql #1,%d0`; the pin's syntax is `addq.l #1, %d0`. The
/// first cycle runs read the overlay-era code through the pin's own objdump,
/// which shows `addq` and `tst` as `<unknown>`, and priced none of them.
#[test]
fn a_gnu_mnemonic_loses_its_size_letter_to_a_suffix() {
    assert_eq!(gnu("addql #1,%d0"), "addq.l #1, %d0");
    assert_eq!(gnu("tstl %d1"), "tst.l %d1");
    assert_eq!(gnu("moveal %a1,%a0"), "movea.l %a1, %a0");
    assert_eq!(gnu("moveml %d2-%d4,%sp@-"), "movem.l %d2-%d4, -(%sp)");
    assert_eq!(gnu("lsll #8,%d0"), "lsl.l #8, %d0");
    assert_eq!(gnu("moveq #1,%d0"), "moveq #1, %d0");
    assert_eq!(gnu("rts"), "rts");
    assert_eq!(gnu("linkw %fp,#-4"), "link.w %fp, #-4");
}

#[test]
fn a_gnu_operand_is_the_pins_addressing_syntax() {
    assert_eq!(gnu("movel %a0@(12),%d2"), "move.l (12,%a0), %d2");
    assert_eq!(gnu("movel %a0@+,%d1"), "move.l (%a0)+, %d1");
    assert_eq!(gnu("movel %d1,%a0@"), "move.l %d1, (%a0)");
    assert_eq!(gnu("movel %a0@(0,%d2:l),%d3"), "move.l (0,%a0,%d2), %d3");
    assert_eq!(gnu("lea %pc@(0x20),%a1"), "lea (0x20,%pc), %a1");
    assert_eq!(gnu("bnes 0x20"), "bne 0x20");
    assert_eq!(gnu("bnew 0x20"), "bne 0x20");
    // the symbol annotation of a branch target is dropped; priced as a target
    assert_eq!(gnu("bnes 80000074 <k+0x8>"), "bne 80000074");
    assert_eq!(clocks(Cpu::M68000, &gnu("bras 80000074 <k+0x8>")), 10);
}

/// Cost through the conversion, the way the cycles tool prices code.
#[test]
fn a_gnu_line_is_priced_after_conversion() {
    let line = gnu("addql #1,%d0");
    assert_eq!(clocks(Cpu::M68000, &line), 8);
    assert_eq!(clocks(Cpu::M68020, &line), 2);
}
