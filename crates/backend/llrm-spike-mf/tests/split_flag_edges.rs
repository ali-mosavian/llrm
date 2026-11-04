//! 1000 compare-and-branch edges into phis (3000 blocks) took 20 s in
//! `split_flag_edges`: `successors` found each branch target by scanning all
//! blocks, and the pass counted every block's predecessors the same way, once
//! per split. They take 0.15 s.

mod common;

use common::{Two, v};
use llrm_spike_mf::regalloc::split_flag_edges;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi};

fn op(name: &'static str) -> Inst {
    Inst::new(Kind::Op(name), vec![])
}

/// `cmp; branch J` falling into `B`, which falls into the join `J` whose phi
/// reads the compare's block: the copy for that input cannot go after the
/// compare, so the edge is split.
fn diamonds(count: u32) -> MFunction<()> {
    let mut function = MFunction::new("f");
    for group in 0..count {
        let (a, b, join) = (3 * group, 3 * group + 1, 3 * group + 2);
        let mut compare = op("cmp");
        compare.sets_flags = true;
        let mut jump = common::branch(join);
        jump.reads_flags = true;
        function.blocks.push(Block { id: a, insts: vec![compare, jump], phis: vec![] });
        function.blocks.push(Block { id: b, insts: vec![op("work")], phis: vec![] });
        let (result, from_a, from_b) = (3 * group, 3 * group + 1, 3 * group + 2);
        function.blocks.push(Block {
            id: join,
            insts: vec![op("use")],
            phis: vec![Phi { result: v(result), incoming: vec![(a as usize, v(from_a)), (b as usize, v(from_b))] }],
        });
    }
    function.blocks.push(Block { id: 3 * count, insts: vec![op("ret")], phis: vec![] });
    function.classes = vec![(); 3 * count as usize + 3];
    function
}

#[test]
fn many_flag_edges_are_split_in_linear_work_per_pass() {
    let mut function = diamonds(1000);
    let before = function.blocks.len();
    let start = std::time::Instant::now();
    split_flag_edges::<Two, _>(&mut function);
    let took = start.elapsed();
    assert!(function.blocks.len() > before, "no edge was split: the test measures nothing");
    assert!(took.as_secs() < 1, "{before} blocks took {took:?}");
}
