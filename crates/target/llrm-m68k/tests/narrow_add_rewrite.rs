//! `add.l #imm, %dN` becomes `add.w` where the register's upper half is zero
//! before and after (hex: `add.l #87` after `& 15`, 6 B and 16 clk against 4
//! and 8 on the 68000). The rewrite must keep the register and the codes a
//! later row reads.

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::dagisel::generated::instrinfo;
use llrm_m68k::dagisel::generated::m68k_m68000::MACHINE_OPCODES;
use llrm_m68k::dagisel::local;
use llrm_m68k::mfunc::{Class, MFunction, op_inst};
use llrm_m68k::narrow_add::narrow_adds;
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::mfunc::Reg;
use llrm_spike_mf::{Block, Inst, Kind, Operand};

const HEX: &str = include_str!("../bench/library/hex.ll");

fn imm_row(
    name: &'static str,
    value: i64,
) -> Inst {
    let desc = local::code_of(name).and_then(local::desc).expect(name);
    op_inst(name, desc, vec![Operand::Def(Reg::Virt(0)), Operand::Use(Reg::Virt(0)), Operand::Imm(value)])
}

fn constant(value: i64) -> Inst {
    let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == "MOVI32ri").map(|(code, _)| *code).expect("MOVI32ri");
    op_inst("MOVI32ri", instrinfo::desc(code).expect("described"), vec![Operand::Def(Reg::Virt(0)), Operand::Imm(value)])
}

fn and_self() -> Inst {
    let desc = local::code_of("AND32dd").and_then(local::desc).expect("AND32dd");
    op_inst("AND32dd", desc, vec![Operand::Def(Reg::Virt(0)), Operand::Use(Reg::Virt(0)), Operand::Use(Reg::Virt(0))])
}

fn branch(to: u32) -> Inst {
    let desc = local::code_of("Bne8").and_then(local::desc).expect("Bne8");
    op_inst("Bne8", desc, vec![Operand::Block(to)])
}

fn function(blocks: Vec<Vec<Inst>>) -> MFunction {
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32];
    for (id, insts) in blocks.into_iter().enumerate() {
        function.blocks.push(Block { id: id as u32, insts, phis: vec![] });
    }
    function
}

/// The kinds of one block's rows after the rewrite.
fn rewritten(mut function: MFunction) -> Vec<Vec<&'static str>> {
    narrow_adds(&mut function, &Assignment(vec![llrm_m68k::abi::register("d0")]));
    function
        .blocks
        .iter()
        .map(|block| block.insts.iter().map(|inst| if let Kind::Op(name) = inst.kind { name } else { "COPY" }).collect())
        .collect()
}

/// hex: `15 & x` then `+ 87`, the sum below 256.
#[test]
fn a_masked_value_plus_an_immediate_becomes_a_word_add_with_the_same_immediate() {
    let mut f = function(vec![vec![constant(15), and_self(), imm_row("ADD32di", 87)]]);
    narrow_adds(&mut f, &Assignment(vec![llrm_m68k::abi::register("d0")]));
    let add = &f.blocks[0].insts[2];
    assert_eq!(add.kind, Kind::Op("ADD16di"));
    assert_eq!(add.operands[2], Operand::Imm(87));
}

/// `0xFFFF + 0xFFFF` carries out of the word: the long add keeps the carry bit.
#[test]
fn a_sum_that_leaves_the_word_stays_long() {
    let kinds = rewritten(function(vec![vec![imm_row("AND32di", 0xFFFF), imm_row("ADD32di", 0xFFFF)]]));
    assert_eq!(kinds, vec![vec!["AND32di", "ADD32di"]]);
}

/// Nothing is known of the register: the upper half may be anything.
#[test]
fn an_unbounded_register_stays_long() {
    assert_eq!(rewritten(function(vec![vec![imm_row("ADD32di", 48)]])), vec![vec!["ADD32di"]]);
}

/// `0x7FF0 + 0x20 = 0x8010`: the word add sets N where the long add does not,
/// so a branch on the codes after it keeps the long add; below 0x8000 it does not.
#[test]
fn a_later_reader_of_the_codes_blocks_a_sum_in_the_top_half_only() {
    let top = vec![imm_row("AND32di", 0x7FF0), imm_row("ADD32di", 0x20), branch(0)];
    assert_eq!(rewritten(function(vec![top]))[0][1], "ADD32di");
    let low = vec![imm_row("AND32di", 0x7F00), imm_row("ADD32di", 0x20), branch(0)];
    assert_eq!(rewritten(function(vec![low]))[0][1], "ADD16di");
    let free = vec![imm_row("AND32di", 0x7FF0), imm_row("ADD32di", 0x20)];
    assert_eq!(rewritten(function(vec![free]))[0][1], "ADD16di");
}

/// A function whose first block is a loop head: the bound at the end of the
/// loop is not the bound at entry (the caller's value), so nothing is narrowed.
#[test]
fn an_entry_block_that_loops_back_starts_with_no_bound() {
    let body = vec![imm_row("ADD32di", 48)];
    let tail = vec![imm_row("AND32di", 15), branch(0)];
    assert_eq!(rewritten(function(vec![body, tail]))[0], vec!["ADD32di"]);
}

/// hex printed `add.l #87,%d2` after `and.l`: 6 B and 16 clk for the word add's 4 and 8.
#[test]
fn hex_adds_its_digit_base_with_a_word_add() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(HEX, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(out.contains("add.w\t#87,"), "{cpu} {level:?}: no word add:\n{out}");
            assert!(!out.contains("add.l\t#87,"), "{cpu} {level:?}: the long add is left:\n{out}");
        }
    }
}
