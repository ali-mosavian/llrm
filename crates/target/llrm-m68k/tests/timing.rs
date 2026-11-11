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

/// `cmpi.b #0,(1,%a0,%d1)` ends the `strlen` loop. The 68020 model had no
/// immediate-to-memory row and returned an error that the cycles tool counted
/// as zero clocks on the 68020 only: strlen read 213 for 59 instructions when
/// the instruction before it read 283. MC68020UM 8.2.2 (p. 8-14) and 8.2.9
/// (p. 8-31): `CMPI` 2 plus `#<data>.W,(d8,An,Xn)` 9, `ADDI` to memory 4 plus
/// the fetch.
#[test]
fn a_68020_immediate_to_memory_is_priced() {
    assert_eq!(clocks(Cpu::M68020, "cmpi.b #0, (1,%a0,%d1)"), 2 + 9);
    assert_eq!(clocks(Cpu::M68020, "cmpi.l #0, (%a0)"), 2 + 4);
    assert_eq!(clocks(Cpu::M68020, "addi.l #5, (4,%a0)"), 4 + 7);
    assert_eq!(clocks(Cpu::M68020, "add.w #5, -(%a0)"), 4 + 5);
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

/// objdump prints an absolute address bare, as it prints a branch target; the
/// bare hex of a non-branch was read as a target and the instruction was
/// "unpriced": libgcc's `lea 80023c20,%a0` (4146 of 173773 executed
/// instructions of its add routine) was left out of a total.
#[test]
fn a_bare_hex_operand_of_a_non_branch_is_an_absolute_address() {
    assert_eq!(gnu("lea 80023c20,%a0"), "lea 0x80023c20:l, %a0");
    assert_eq!(clocks(Cpu::M68000, &gnu("lea 80023c20,%a0")), 12);
    assert_eq!(gnu("jsr 80000200 <foo>"), "jsr 0x80000200:l");
    assert_eq!(gnu("bsrs 80000200"), "bsr 80000200");
}

/// Cost through the conversion, the way the cycles tool prices code.
#[test]
fn a_gnu_line_is_priced_after_conversion() {
    let line = gnu("addql #1,%d0");
    assert_eq!(clocks(Cpu::M68000, &line), 8);
    assert_eq!(clocks(Cpu::M68020, &line), 2);
}

/// objdump prints the 32 / 32 `divu.l %d5, %d2` as `divull %d5,%d2,%d2`, three
/// operands. The model took only the two-operand form, so every long divide in
/// a disassembled executable was unpriced and the hardware-divide routines of
/// softfloat looked 3-4x cheaper than they are (68020: 720 clocks per f32
/// division, the divides' own 76 each missing).
#[test]
fn a_gnu_long_divide_is_the_two_operand_form() {
    assert_eq!(gnu("divull %d5,%d2,%d2"), "divu.l %d5, %d2");
    assert_eq!(gnu("divsll %d5,%d2,%d2"), "divs.l %d5, %d2");
    assert_eq!(gnu("divul %d5,%d2,%d3"), "divu.l %d5, %d3");
    assert_eq!(gnu("divsl %a0@,%d2,%d3"), "divs.l (%a0), %d3");
    // the cache-case cell (MC68020UM 8.2.8: 78 for DIVU.L, 90 for DIVS.L) plus
    // the extension word the `**` mark adds
    assert_eq!(clocks(Cpu::M68020, &gnu("divull %d5,%d2,%d2")), 78 + 2);
    assert_eq!(clocks(Cpu::M68020, &gnu("divsll %d5,%d2,%d2")), 90 + 2);
    assert_eq!(clocks(Cpu::M68020, &gnu("divull %d5,%d2,%d2")), clocks(Cpu::M68020, "divu.l %d5, %d2"));
}

/// objdump prints the target of a PC-relative `lea` as an absolute address,
/// `lea %pc@(80010000),%a0`; the bare hex was not a displacement the model
/// knows, so 6972 of 63218 instructions of a 68020 program were "unpriced".
#[test]
fn a_gnu_pc_relative_target_address_is_a_displacement() {
    assert_eq!(gnu("lea %pc@(80010000),%a0"), "lea (0,%pc), %a0");
    assert_eq!(clocks(Cpu::M68020, &gnu("lea %pc@(80010000),%a0")), clocks(Cpu::M68020, "lea (0x20,%pc), %a0"));
    assert_eq!(clocks(Cpu::M68000, &gnu("lea %pc@(80010000),%a0")), clocks(Cpu::M68000, "lea (0x20,%pc), %a0"));
}

/// objdump annotates an operand with its symbol in mid-line,
/// `lea %pc@(80000062) <a>,%a0`; the text from `<` on was dropped, destination
/// included, and the `lea` stayed unpriced.
#[test]
fn a_symbol_annotation_inside_the_operands_keeps_the_rest() {
    assert_eq!(gnu("lea %pc@(80000062) <a>,%a0"), "lea (0,%pc), %a0");
    assert_eq!(gnu("jsr 80004064 <kmain>"), "jsr 0x80004064:l");
    assert_eq!(clocks(Cpu::M68020, &gnu("lea %pc@(80000062) <a>,%a0")), clocks(Cpu::M68020, "lea (0x20,%pc), %a0"));
}

/// objdump prints the `pea 0:w` the callpush pass pushes for a zero argument
/// as `pea 0 <f>` and `pea 4 <f+0x4>`, and the Os `clr.l -(%sp)` as `clrl
/// %sp@-`; `cycles` listed both as uncovered (`?`) in the O2/Os clock tables of
/// #7715 and priced neither.
#[test]
fn a_pushed_zero_and_a_short_absolute_pea_are_priced() {
    for (cpu, pea, clr) in [(Cpu::M68000, 16, 22), (Cpu::M68020, 7, 6)] {
        assert_eq!(clocks(cpu, &gnu("pea 0 <f>")), pea, "{cpu:?} pea 0");
        assert_eq!(clocks(cpu, &gnu("pea 4 <f+0x4>")), pea, "{cpu:?} pea 4");
        assert_eq!(clocks(cpu, &gnu("clrl %sp@-")), clr, "{cpu:?} clr.l -(%sp)");
    }
}

/// A register list that ends at `%fp` (`movem.l %d2-%d7/%a2-%fp, -(%sp)`, a
/// function that keeps eleven registers) was "not in the tables": every
/// executed run of such a prologue or epilogue was left out of a program's
/// clocks, on both sides of a comparison (nbody_single: `?2`).
#[test]
fn a_register_list_ending_at_the_frame_pointer_is_priced() {
    assert_eq!(clocks(Cpu::M68000, "movem.l %d2-%d7/%a2-%fp, -(%sp)"), 8 + 8 * 11);
    assert_eq!(
        clocks(Cpu::M68020, "movem.l (%sp)+, %d2-%d7/%a2-%fp"),
        clocks(Cpu::M68020, "movem.l (%sp)+, %d2-%d7/%a2-%a6")
    );
}

/// M68020UM Table 8.2.17 (p. 8-39): `TRAP #n` 20 clocks, the cache case. The
/// harness's `trap #0` was not in the 68020 tables, so every program that
/// exits that way read UNMEASURED.
#[test]
fn a_68020_trap_is_the_exception_table_row() {
    assert_eq!(clocks(Cpu::M68020, "trap #0"), 20);
}

/// M68020UM Table 8.2.8 (p. 8-30) `MUL.L EA,Dn` 43 and Table 8.2.2 (p. 8-14)
/// `#<data>.W,#<data>.L` 6 (the extension word and the 32-bit operand); the
/// same for `DIVS.L` 90 + 6. `mulu.l #imm` was UNMEASURED.
#[test]
fn a_68020_long_multiply_by_an_immediate_adds_the_immediate_fetch() {
    assert_eq!(clocks(Cpu::M68020, "mulu.l #1000, %d0"), 43 + 6);
    assert_eq!(clocks(Cpu::M68020, "muls.l #1000, %d1"), 43 + 6);
    assert_eq!(clocks(Cpu::M68020, "divs.l #10, %d1"), 90 + 6);
}

/// Table 8.2.9 `CMPI #<data>,EA` 2 plus Table 8.2.2 `#<data>.L,(bd,An)` 7;
/// the PC form is the address-register form, as in 8.2.1 and 8.2.3 where the
/// manual prints one row for both.
#[test]
fn a_68020_compare_immediate_to_a_pc_relative_operand_is_priced() {
    assert_eq!(clocks(Cpu::M68020, "cmpi.l #5, (0,%pc)"), 2 + 7);
    assert_eq!(clocks(Cpu::M68020, "cmpi.l #5, (0,%pc)"), clocks(Cpu::M68020, "cmpi.l #5, (0,%a0)"));
}

/// Table 8.2.3 (p. 8-15): `LEA` 2 plus calculate `(d8,An,Xn)` 4, `(d16,An,Xn)`
/// 6, `(B)` 6, `(d32,B)` 12. A base displacement past 8 bits was priced as the
/// brief format's `(d8,An,Xn)` (6 clocks for the 16-bit one, 8 right), and
/// `(%a0,%d1.l*4)` was UNMEASURED. Scale and size of Xn do not affect timing.
#[test]
fn a_68020_full_format_index_is_priced_by_its_base_displacement() {
    assert_eq!(clocks(Cpu::M68020, "lea (100,%a0,%d1.l*4), %a1"), 2 + 4);
    assert_eq!(clocks(Cpu::M68020, "lea (1000,%a0,%d1.l*4), %a1"), 2 + 6);
    assert_eq!(clocks(Cpu::M68020, "lea (-1000,%a0,%d1.w), %a1"), 2 + 6);
    assert_eq!(clocks(Cpu::M68020, "lea (%a0,%d1.l*4), %a1"), 2 + 6);
    assert_eq!(clocks(Cpu::M68020, "lea (100000,%a0,%d1.l*4), %a1"), 2 + 12);
    assert_eq!(clocks(Cpu::M68020, "lea (0x12345,%a0,%d1.l), %a1"), 2 + 12);
}

/// MC68000UM Table 8-7: a shift by a register is `8+2n` long, `n` the count
/// register modulo 64; the pricer saw `n = 0` and called the total a bound.
#[test]
fn a_68000_shift_by_a_register_costs_two_clocks_a_place_once_the_count_is_known() {
    use llrm_m68k::isa::Cpu as Model;
    use llrm_m68k::timing::{DataRegisters, cost_on_counted};
    let by_register = |places| cost_on_counted(Model::M68000, "lsr.l %d2, %d0", 2, false, places).unwrap();
    assert_eq!((by_register(None).clocks, by_register(None).partial), (8, true));
    assert_eq!((by_register(Some(16)).clocks, by_register(Some(16)).partial), (40, false));
    // derived by hand from the immediate form: `lsr.l #8` is 8 + 2 x 8
    assert_eq!(by_register(Some(8)).clocks, clocks(Cpu::M68000, "lsr.l #8, %d0"));
    let mut registers = DataRegisters::new();
    registers.execute("moveq #-1,%d2");
    assert_eq!(registers.places_of("lsr.l %d2, %d0"), Some(63), "-1 is 63 places modulo 64");
    registers.execute("addq.l #1, %d2");
    assert_eq!(registers.places_of("lsr.l %d2, %d0"), None, "an add writes the register");
    registers.execute("move.l #0x10,%d2");
    assert_eq!(registers.places_of("lsr.l %d2, %d0"), Some(16));
    registers.execute("move.l %d2, %d0");
    assert_eq!(registers.places_of("lsr.l %d2, %d0"), Some(16), "a read leaves it");
    registers.execute("movem.l (%sp)+, %d2-%d5");
    assert_eq!(registers.places_of("lsr.l %d2, %d0"), None, "a register list reloads it");
}

/// MC68010 Table 9-19 (printed 9-13): `TRAP Instruction` 38 (4/4). The row was
/// in the table but its column header printed blank, so the lookup by
/// "Periods" found nothing and every 68010 program that exits by `trap #0`
/// read UNMEASURED (368 of 556 cells at -O2).
#[test]
fn a_68010_trap_is_the_exception_table_row() {
    assert_eq!(clocks(Cpu::M68010, "trap #0"), 38);
}

/// MC68030UM Table 11.6.17 (printed 11-50): `TRAP #n`, I-cache case 18 (1/0/4).
/// The row was transcribed and never read: 363 cells UNMEASURED.
#[test]
fn a_68030_trap_is_the_exception_table_row() {
    assert_eq!(clocks(Cpu::M68030, "trap #0"), 18);
}

/// Table 11.6.8 `MULU.L`/`MULS.L EA,Dn` 44 plus Table 11.6.2 `#<data>.W,#<data>.L`
/// 6, as the 68020's 43 + 6. `mulu.l #imm` was UNMEASURED (17 cells).
#[test]
fn a_68030_long_multiply_by_an_immediate_adds_the_immediate_fetch() {
    assert_eq!(clocks(Cpu::M68030, "mulu.l #1000, %d0"), 44 + 6);
    assert_eq!(clocks(Cpu::M68030, "muls.l #1000, %d1"), 44 + 6);
}

/// Table 11.6.16 `LEA` 2 plus Table 11.6.3: `(d16,An,Xn)` 6, `(B)` 6, `(d32,B)`
/// 12, as for the 68020. A full-format index was UNMEASURED.
#[test]
fn a_68030_full_format_index_is_priced_by_its_base_displacement() {
    assert_eq!(clocks(Cpu::M68030, "lea (3619,%a0,%d1), %a0"), 2 + 6);
    assert_eq!(clocks(Cpu::M68030, "lea (%a0,%d1.l*4), %a1"), 2 + 6);
    assert_eq!(clocks(Cpu::M68030, "lea (100000,%a0,%d1.l*4), %a1"), 2 + 12);
}

/// MC68060UM Table 10-10 prints `CMPI` by destination without a PC column;
/// Table 10-5 prints `(d16,PC)` at the time of `(d16,An)` (0), so the PC form
/// is the address-register form, as for the 68020. UNMEASURED (6 cells).
#[test]
fn a_68060_compare_immediate_to_a_pc_relative_operand_is_priced_as_the_address_form() {
    assert_eq!(clocks(Cpu::M68060, "cmpi.l #5, (0,%pc)"), clocks(Cpu::M68060, "cmpi.l #5, (0,%a0)"));
    assert_eq!(clocks(Cpu::M68060, "cmpi.l #5, (0,%pc)"), 2);
}

/// binutils prints an index displacement as bare hex (`%a1@(100,%d1:w)` is 256,
/// `%a2@(10,%d0:w)` is 16, a negative is sign-extended to 64 bits) and a
/// suppressed base as `@(ffffffff800020a0,%d0:l)`. All-digit hex was read as
/// decimal (256 priced as a brief index, 16 as 10) and the rest was unpriced:
/// 84 exes at 68000 and 103 at 68020 carried `operand ... is not in the tables`.
#[test]
fn an_objdump_index_displacement_is_hex() {
    assert_eq!(gnu("moveb %a1@(7e,%d1:w),%d0"), "move.b (126,%a1,%d1), %d0");
    assert_eq!(gnu("moveb %a2@(10,%d0:w),%d1"), "move.b (16,%a2,%d0), %d1");
    assert_eq!(gnu("moveb %a3@(ffffffffffffffff,%d1:l),%d0"), "move.b (-1,%a3,%d1), %d0");
    // Table 8.2.3, LEA = 2 + calculate: (d8,An,Xn) 4, (d16,An,Xn) 6, (d32,B) 12
    let lea = |operand: &str| clocks(Cpu::M68020, &gnu(&format!("lea {operand},%a0")));
    assert_eq!(lea("%a2@(10,%d0:w)"), 2 + 4, "0x10 is 16");
    assert_eq!(lea("%a3@(ffffffffffffffff,%d1:l)"), 2 + 4);
    assert_eq!(lea("%a3@(ffffffffffffff80,%d1:l)"), 2 + 4, "-128 is the brief format's lowest");
    assert_eq!(lea("%a1@(80,%d1:w)"), 2 + 6, "0x80 is 128, past the brief format");
    assert_eq!(lea("%a1@(100,%d1:w)"), 2 + 6, "0x100 is 256, a base displacement");
    assert_eq!(lea("@(ffffffff800020a0,%d0:l)"), 2 + 12, "a suppressed base with a 32-bit displacement");
    assert!(cost(Cpu::M68000, &gnu("moveb %a1@(7e,%d1:w),%d0"), 2, false).is_ok());
}

/// A displacement the parser cannot read is a parser fault, not a missing table
/// row: it was reported as `?1` unpriced and the total looked merely partial.
#[test]
fn an_unreadable_index_displacement_is_a_parser_fault() {
    let why = cost(Cpu::M68020, "lea (zz,%a0,%d1), %a1", 2, false).unwrap_err();
    assert!(why.starts_with(llrm_m68k::timing::UNPARSEABLE), "{why}");
}
