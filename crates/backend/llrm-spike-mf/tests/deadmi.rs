//! `dead_machine_instruction_elim`: what nothing reads goes, with the phis and
//! chains only it kept.

mod common;

use common::{op, v};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, Reg, dead_machine_instruction_elim};

fn function(blocks: Vec<Block>) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 8];
    function.blocks = blocks;
    function
}

fn names(function: &MFunction<()>) -> Vec<Vec<&'static str>> {
    function
        .blocks
        .iter()
        .map(|block| block.insts.iter().filter_map(|inst| if let Kind::Op(name) = inst.kind { Some(name) } else { None }).collect())
        .collect()
}

/// The high word of `trunc (phi i64 ..)` has no reader: the phi, and the
/// selects feeding it in both predecessors, were left in (`shl`/`lshr` by a
/// variable amount: 14 instructions and two branches a copy).
#[test]
fn a_phi_nothing_reads_goes_with_the_chain_that_fed_it() {
    let use_of = |reg| Operand::Use(v(reg));
    let mut ret = op("rts", vec![use_of(1)]);
    ret.terminator = true;
    ret.barrier = true;
    let mut f = function(vec![
        Block { id: 0, insts: vec![op("lo", vec![Operand::Def(v(0))]), op("hi", vec![Operand::Def(v(2)), use_of(0)])], phis: vec![] },
        Block {
            id: 1,
            insts: vec![op("lo2", vec![Operand::Def(v(3)), use_of(0)])],
            phis: vec![],
        },
        Block {
            id: 2,
            insts: vec![ret],
            phis: vec![
                Phi { result: v(1), incoming: vec![(0, v(0)), (1, v(3))] },
                Phi { result: v(4), incoming: vec![(0, v(2)), (1, v(2))] },
            ],
        },
    ]);
    assert!(dead_machine_instruction_elim(&mut f));
    assert_eq!(names(&f), [vec!["lo"], vec!["lo2"], vec!["rts"]]);
    assert_eq!(f.blocks[2].phis.len(), 1);
}

/// A call, a store and a write of a physical register stay though nothing
/// reads their results.
#[test]
fn side_effects_stay() {
    let mut call = op("jsr", vec![Operand::Def(v(0))]);
    call.call = true;
    let mut store = op("st", vec![Operand::Use(v(1))]);
    store.may_store = true;
    let mut volatile = op("ld", vec![Operand::Def(v(2))]);
    volatile.side_effects = true;
    let phys = op("mv", vec![Operand::Def(Reg::Phys(llrm_lir::registers::RegId::from_raw(0)))]);
    let mut f = function(vec![Block { id: 0, insts: vec![call, store, volatile, phys, op("dead", vec![Operand::Def(v(3))])], phis: vec![] }]);
    assert!(dead_machine_instruction_elim(&mut f));
    assert_eq!(names(&f), [vec!["jsr", "st", "ld", "mv"]]);
}

/// A shift that sets the flags a branch reads stays; with no reader after it,
/// or another setter between, it goes.
#[test]
fn flag_setters_go_only_where_the_flags_are_dead() {
    let setter = |name| {
        let mut inst = op(name, vec![Operand::Def(v(0))]);
        inst.sets_flags = true;
        inst
    };
    let mut branch = op("bcc", vec![]);
    branch.reads_flags = true;
    branch.terminator = true;
    let mut kept = function(vec![Block { id: 0, insts: vec![setter("lsl"), branch.clone()], phis: vec![] }]);
    assert!(!dead_machine_instruction_elim(&mut kept));
    assert_eq!(names(&kept), [vec!["lsl", "bcc"]]);

    let mut shadowed = function(vec![Block { id: 0, insts: vec![setter("lsl"), setter("lsr"), branch], phis: vec![] }]);
    assert!(dead_machine_instruction_elim(&mut shadowed));
    assert_eq!(names(&shadowed), [vec!["lsr", "bcc"]]);
}
