//! `machine_sink` over small diamonds.

mod common;

use common::{Two, branch, op, v};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, machine_sink};

/// `b0: x = add a, a; branch b2` falls into `b1`, which falls into
/// `b2: r = phi [b0: x] [b1: a]; ret r`. `x` is used by the phi only.
fn diamond(after: Vec<Inst>) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 4];
    let mut head = vec![
        op("arg", vec![Operand::Def(v(0))]),
        op("add", vec![Operand::Def(v(1)), Operand::Use(v(0)), Operand::Use(v(0))]),
    ];
    head.extend(after);
    head.push(branch(2));
    function.blocks.push(Block { id: 0, insts: head, phis: vec![] });
    function.blocks.push(Block { id: 1, insts: vec![], phis: vec![] });
    let join = Block {
        id: 2,
        insts: vec![op("ret", vec![Operand::Use(v(2))])],
        phis: vec![Phi { result: v(2), incoming: vec![(0, v(1)), (1, v(0))] }],
    };
    function.blocks.push(join);
    function
}

fn names(block: &Block) -> Vec<&'static str> {
    block.insts.iter().map(|inst| if let Kind::Op(name) = inst.kind { name } else { "copy" }).collect()
}

/// A value only a phi on one edge reads was computed before the branch, on
/// both paths. It now sits in a block of its own on that edge.
#[test]
fn a_value_read_by_one_edges_phi_moves_onto_the_edge() {
    let mut function = diamond(vec![]);
    assert!(machine_sink::<Two>(&mut function));
    let head = &function.blocks[0];
    // `updateTerminator`: the head now jumps to the block it used to fall into
    assert_eq!(names(head), ["arg", "branch", "jump"]);
    let edge = function.blocks.iter().find(|block| block.id > 2).expect("a block on the split edge");
    assert_eq!(names(edge).first(), Some(&"add"));
    let join = function.blocks.iter().find(|block| block.id == 2).unwrap();
    let from_edge = function.blocks.iter().position(|block| block.id == edge.id).unwrap();
    assert!(join.phis[0].incoming.contains(&(from_edge, v(1))), "{:?}", join.phis[0].incoming);
}

/// A load does not cross a store below it in its block: the store may write
/// what the load reads. The same load with no store there does move.
#[test]
fn a_load_stays_above_a_store() {
    let mut store = op("store", vec![Operand::Use(v(0))]);
    store.may_store = true;
    let mut function = diamond(vec![store]);
    function.blocks[0].insts[1].may_load = true;
    assert!(!machine_sink::<Two>(&mut function));
    assert_eq!(names(&function.blocks[0]), ["arg", "add", "store", "branch"]);

    let mut free = diamond(vec![]);
    free.blocks[0].insts[1].may_load = true;
    assert!(machine_sink::<Two>(&mut free));
}

/// A value read on the path that falls through as well stays put: sinking it
/// would leave that read without a definition.
#[test]
fn a_value_read_on_both_paths_does_not_move() {
    let mut function = diamond(vec![]);
    function.blocks[1].insts.push(op("use", vec![Operand::Use(v(1))]));
    assert!(!machine_sink::<Two>(&mut function));
    assert_eq!(names(&function.blocks[0]), ["arg", "add", "branch"]);
}
