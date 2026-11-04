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

/// A target with room for every register, so no sink is refused for pressure.
struct Wide;

impl llrm_spike_mf::Target for Wide {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<llrm_lir::registers::RegId> {
        (0..250u8).map(llrm_lir::registers::RegId::from_raw).collect()
    }

    fn view(
        root: llrm_lir::registers::RegId,
        _: (),
    ) -> llrm_lir::registers::RegId {
        root
    }

    fn root(register: llrm_lir::registers::RegId) -> llrm_lir::registers::RegId {
        register
    }

    fn reserved(_: llrm_lir::registers::RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<llrm_lir::registers::RegId> {
        vec![]
    }

    fn copy_sets_flags(
        _: llrm_lir::registers::RegId,
        _: llrm_lir::registers::RegId,
    ) -> bool {
        false
    }
}

impl llrm_spike_mf::Jump for Wide {
    fn jump(block: u32) -> Inst {
        <Two as llrm_spike_mf::Jump>::jump(block)
    }

    fn reverse(branch: &Inst) -> Option<Inst> {
        <Two as llrm_spike_mf::Jump>::reverse(branch)
    }
}

/// `count` diamonds in one loop. Each head defines a value its join alone
/// reads (so it sinks, and the liveness is solved again), and the last join
/// reads every head's other value, so hundreds of registers are live. Liveness
/// was a hash set per block: 200 diamonds took 350 ms, 20 ms now
/// (div_constants_long at m68k: 8% of its instructions in `pressure_exceeds`).
fn loop_of_diamonds(count: u32) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 2 * count as usize];
    for g in 0..count {
        let (head, mid, join) = (3 * g, 3 * g + 1, 3 * g + 2);
        let (p, x) = (2 * g, 2 * g + 1);
        function
            .blocks
            .push(
                Block {
                    id: head,
                    insts: vec![
                        op("def", vec![Operand::Def(v(p))]),
                        op("add", vec![Operand::Def(v(x)), Operand::Use(v(p)), Operand::Use(v(p))]),
                        branch(join),
                    ],
                    phis: vec![],
                },
            );
        function.blocks.push(Block { id: mid, insts: vec![], phis: vec![] });
        let mut insts = vec![op("use", vec![Operand::Use(v(x))])];
        if g + 1 == count {
            insts.extend((0..count).map(|n| op("use", vec![Operand::Use(v(2 * n))])));
            insts.push(branch(0));
        }
        function.blocks.push(Block { id: join, insts, phis: vec![] });
    }
    function.blocks.push(Block { id: 3 * count, insts: vec![op("ret", vec![])], phis: vec![] });
    function
}

#[test]
fn sinking_in_a_loop_of_many_blocks_solves_liveness_with_bit_sets() {
    let mut function = loop_of_diamonds(200);
    let start = std::time::Instant::now();
    assert!(machine_sink::<Wide>(&mut function), "nothing sank: the test measures nothing");
    let took = start.elapsed();
    assert!(took.as_millis() < 150, "{} blocks took {took:?}", function.blocks.len());
}
