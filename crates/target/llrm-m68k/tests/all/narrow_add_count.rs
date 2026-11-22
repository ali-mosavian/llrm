//! The soundness cases of counting `add.l #imm, %dN` rows a word add could do
//! (`narrow_add::sites`): the byte-width add gcc prints for hex (`addi.b #48`
//! for 6 B and 16 clk of `add.l`) is exact only when the upper half of the
//! register is zero before and after, and the codes it sets are the long
//! form's or unread.

use llrm_m68k::dagisel::generated::instrinfo;
use llrm_m68k::dagisel::generated::m68k_m68000::MACHINE_OPCODES;
use llrm_m68k::dagisel::local;
use llrm_m68k::mfunc::{Class, MFunction, op_inst};
use llrm_m68k::narrow_add::{Site, sites};
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::mfunc::Reg;
use llrm_spike_mf::{Block, Inst, Operand};

fn imm_row(
    name: &'static str,
    register: u32,
    value: i64,
) -> Inst {
    let desc = local::code_of(name).and_then(local::desc).expect(name);
    op_inst(name, desc, vec![Operand::Def(Reg::Virt(register)), Operand::Use(Reg::Virt(register)), Operand::Imm(value)])
}

/// A constant load by the pin's row `name` (not a local one).
fn constant(
    name: &'static str,
    register: u32,
    value: i64,
) -> Inst {
    let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == name).map(|(code, _)| *code).expect(name);
    let desc = instrinfo::desc(code).expect("described");
    op_inst(name, desc, vec![Operand::Def(Reg::Virt(register)), Operand::Imm(value)])
}

fn branch_if_not_equal() -> Inst {
    let desc = local::code_of("Bne8").and_then(local::desc).expect("Bne8");
    op_inst("Bne8", desc, vec![Operand::Block(0)])
}

/// The sites in one block over `insts` (v0 in d0).
fn found(insts: Vec<Inst>) -> Vec<Site> {
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    sites(&function, &Assignment(vec![llrm_m68k::abi::register("d0")]))
}

/// hex: `d = v & 15` then `d + 48`: the sum is below 256.
#[test]
fn a_masked_value_plus_an_immediate_is_a_site_with_equal_codes() {
    let found = found(vec![imm_row("AND32di", 0, 15), imm_row("ADD32di", 0, 48)]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].codes_equal && found[0].sound(), "{found:?}");
}

/// A carry out of the word would change the upper half the long add changes:
/// `0xFFF0 + 0x20` must not be narrowed.
#[test]
fn a_sum_that_carries_out_of_the_word_is_not_a_site() {
    assert_eq!(found(vec![imm_row("AND32di", 0, 0xFFF0), imm_row("ADD32di", 0, 0x20)]), vec![]);
}

/// Nothing is known of the register: the upper half may be anything.
#[test]
fn an_unknown_register_is_not_a_site() {
    assert_eq!(found(vec![imm_row("ADD32di", 0, 48)]), vec![]);
}

/// `0x7FF0 + 0x20 = 0x8010`: the word add sets N and V where the long add does
/// not, so a reader of the codes after it blocks the rewrite and none allows it.
#[test]
fn a_sum_in_the_top_half_of_the_word_needs_the_codes_unread() {
    let base = vec![imm_row("AND32di", 0, 0x7FF0), imm_row("ADD32di", 0, 0x20)];
    let free = found(base.clone());
    assert!(free.len() == 1 && !free[0].codes_equal && free[0].sound(), "{free:?}");
    let mut read = base;
    read.push(branch_if_not_equal());
    let read = found(read);
    assert!(read.len() == 1 && !read[0].codes_equal && !read[0].sound(), "{read:?}");
}

/// A write between forgets the bound, and an immediate `addq` can take is not a site.
#[test]
fn a_write_between_forgets_the_bound_and_a_small_immediate_is_left_to_addq() {
    assert_eq!(found(vec![imm_row("AND32di", 0, 15), imm_row("XOR32di", 0, 0xFFFF_0000u32 as i64), imm_row("ADD32di", 0, 48)]), vec![]);
    assert_eq!(found(vec![constant("MOVI32ri", 0, 3), imm_row("ADD32di", 0, 4)]), vec![]);
}

/// The value moves in a one-predecessor chain: the second block starts from the first's end.
#[test]
fn the_bound_crosses_a_block_with_one_predecessor_only() {
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32];
    function.blocks.push(Block { id: 0, insts: vec![imm_row("AND32di", 0, 15)], phis: vec![] });
    function.blocks.push(Block { id: 1, insts: vec![imm_row("ADD32di", 0, 48)], phis: vec![] });
    let assignment = Assignment(vec![llrm_m68k::abi::register("d0")]);
    assert_eq!(sites(&function, &assignment).len(), 1);
    // a second predecessor (block 2 jumps to 1) forgets it
    let jump = local::code_of("Bne8").and_then(local::desc).expect("Bne8");
    function.blocks.push(Block { id: 2, insts: vec![op_inst("Bne8", jump, vec![Operand::Block(1)])], phis: vec![] });
    function.blocks[0].insts.push(op_inst("Bne8", jump, vec![Operand::Block(1)]));
    assert_eq!(sites(&function, &assignment), vec![]);
}

/// hex printed `add.l #87` for a byte: its 15 came from `MOVI32ri`, the row selection
/// emits for a constant, which the count did not know, so the chain
/// `move.l #15; and.l; add.l #87` was not a site (0 of hex's 1) while 113 others were.
#[test]
fn a_constant_loaded_by_the_selection_row_starts_a_bound() {
    let and = op_inst(
        "AND32dd",
        local::code_of("AND32dd").and_then(local::desc).expect("AND32dd"),
        vec![Operand::Def(Reg::Virt(0)), Operand::Use(Reg::Virt(0)), Operand::Use(Reg::Virt(0))],
    );
    let sites = found(vec![constant("MOVI32ri", 0, 15), and, imm_row("ADD32di", 0, 87)]);
    assert_eq!(sites.len(), 1, "{sites:?}");
}
