//! `commute_tied` and the hoisted phi copy: which source of `a + b` the
//! result is tied to.

mod common;

use common::{Two, op, v};
use llrm_spike_mf::regalloc::{eliminate_phis_with, successors};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, commute_tied};

fn sum_into(
    result: u32,
    tied: u32,
    other: u32,
) -> Inst {
    let mut add = op("add", vec![Operand::Def(v(result)), Operand::Use(v(tied)), Operand::Use(v(other))]);
    add.ties = vec![(0, 1)];
    add.commutable = true;
    add
}

fn loop_function(insts: Vec<Inst>) -> MFunction<()> {
    let mut f = MFunction::new("f");
    f.classes = vec![(); 24];
    f.blocks.push(Block { id: 0, insts: vec![], phis: vec![] });
    f.blocks.push(Block { id: 1, insts, phis: vec![] });
    f
}

fn tied_source(f: &MFunction<()>) -> Operand {
    let add = f.blocks[1].insts.iter().find(|inst| inst.kind == Kind::Op("add")).unwrap();
    add.operands[add.ties[0].1].clone()
}

/// `s = a + b` where `s` flows on into `b`'s register: tied to `b`.
/// (v6 is b's loop register, v7 its copy read by the add; v9 is a's copy.)
fn rotating() -> Vec<Inst> {
    vec![
        Inst::copy(v(6), v(20)),
        Inst::copy(v(19), v(6)),
        Inst::copy(v(7), v(6)),
        Inst::copy(v(9), v(8)),
        sum_into(10, 9, 7),
        Inst::copy(v(11), v(10)),
        Inst::copy(v(20), v(11)),
    ]
}

/// The add took `a`'s register and the pair rotated through a temporary: a
/// move more than tying the result to `b`, the register the sum flows into.
#[test]
fn the_source_whose_register_the_sum_flows_into_is_tied() {
    let mut f = loop_function(rotating());
    commute_tied::<Two>(&mut f);
    assert_eq!(tied_source(&f), Operand::Use(v(7)));
}

/// With no copy chain from `b` back to the result the order stays.
#[test]
fn without_a_copy_chain_to_the_result_the_order_stays() {
    let mut f = loop_function(vec![
        Inst::copy(v(7), v(6)),
        Inst::copy(v(9), v(8)),
        sum_into(10, 9, 7),
        Inst::copy(v(11), v(10)),
        Inst::copy(v(21), v(11)),
    ]);
    commute_tied::<Two>(&mut f);
    assert_eq!(tied_source(&f), Operand::Use(v(9)));
}

/// The phi's copy of a value goes before that value's last read, so the read
/// ends the register; at the end of the block it kept the register live.
#[test]
fn a_phi_copy_goes_before_the_last_read_of_its_source() {
    let build = || {
        let mut f = MFunction::<()>::new("f");
        f.classes = vec![(); 4];
        let mut jump = Inst::new(Kind::Op("jump"), vec![Operand::Block(1)]);
        jump.terminator = true;
        jump.barrier = true;
        f.blocks.push(Block {
            id: 0,
            insts: vec![
                op("def", vec![Operand::Def(v(0))]),
                op("read", vec![Operand::Use(v(0))]),
                op("other", vec![Operand::Def(v(2))]),
                jump,
            ],
            phis: vec![],
        });
        f.blocks.push(Block { id: 1, insts: vec![], phis: vec![Phi { result: v(1), incoming: vec![(0, v(0))] }] });
        f
    };
    let position = |hoist| {
        let mut f = build();
        eliminate_phis_with(&mut f, hoist);
        let _ = successors(&f);
        f.blocks[0].insts.iter().position(|inst| inst.kind == Kind::Copy).unwrap()
    };
    assert_eq!(position(false), 3);
    assert_eq!(position(true), 1);
}
