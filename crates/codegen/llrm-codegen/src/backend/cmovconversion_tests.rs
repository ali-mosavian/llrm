use std::sync::Arc;

use super::*;
use crate::model::ir::{Mem, Semantics};
use crate::support::hash::IndexMap;

fn held(value: u32) -> Loc {
    Loc::Held(Held { value, width: 8, offset: 0 })
}

fn insn(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
}

fn block(
    at: i64,
    succ: Vec<i64>,
    insns: Vec<Arc<Insn>>,
) -> Block {
    LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }.into()
}

/// `v10 := cmovl v3, second` in a loop of one block, `second` a register or a
/// cell, or in a block outside any loop.
fn body(
    second: Loc,
    uses: Vec<u32>,
    in_loop: bool,
) -> LirBody {
    let cmp = insn(10, Operation::Compare, "cmp", vec![], vec![held(1), held(2)], vec![], vec![1, 2]);
    let cmov = insn(11, Operation::Select, "cmovl", vec![held(10)], vec![held(3), second], vec![10], uses);
    let sum = insn(12, Operation::Binary, "add", vec![held(11)], vec![held(10), held(5)], vec![11], vec![10, 5]);
    let branch = insn(13, Operation::Branch, "jne", vec![], vec![], vec![], vec![]);
    let ret = insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
    let jump = insn(15, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
    let blocks = if in_loop {
        vec![
            block(0, vec![1], vec![jump]),
            block(1, vec![1, 2], vec![cmp, cmov, sum, branch]),
            block(2, vec![], vec![ret]),
        ]
    } else {
        vec![block(0, vec![1], vec![cmp, cmov, sum, jump]), block(1, vec![], vec![ret])]
    };
    let mut body = LirBody::new("f", 0, blocks, IndexMap::default(), IndexMap::default());
    body.inputs = [1, 2, 3, 4, 5].into_iter().collect();
    body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
    body.bits = 64;
    body
}

fn cpu() -> &'static Profile {
    crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu")
}

fn cell() -> Loc {
    Loc::Mem(Mem { base: Some(Held { value: 4, width: 8, offset: 0 }), ..Mem::new(None, 8) })
}

fn names(block: &Block) -> Vec<String> {
    block.insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

/// A `cmov` of a load waits for the condition before the load starts. In a
/// loop it became a branch over the load and a phi at the join; the load
/// stayed ahead of the compare's flags and the loop ran at the load's latency.
#[test]
fn a_cmov_of_a_load_in_a_loop_is_a_branch() {
    let done = converted(&body(cell(), vec![3, 4], true), cpu());
    assert_eq!(done.blocks.len(), 5);
    // The loop block ends in the branch over the load, which `l` skips to.
    assert_eq!(names(&done.blocks[1]), ["cmp", "jge", "jmp"]);
    let (skip, sink) = (&done.blocks[2], &done.blocks[3]);
    assert_eq!(names(skip), ["mov", "jmp"]);
    assert_eq!(sink.phis.len(), 1);
    assert_eq!(sink.phis[0].result, 10);
    let loaded = skip.insns[0].defines[0];
    assert_eq!(sink.phis[0].incoming, [(skip.at, loaded), (1, 3)]);
    // The join keeps the rest of the block and its edges.
    assert_eq!(names(sink).last().map(String::as_str), Some("jne"));
    assert_eq!(sink.succ, [1, 2]);
    assert_eq!(done.blocks[1].succ, [skip.at, sink.at]);
}

/// A register `cmov` outside every loop is left alone: only the loops and the
/// loads are converted.
#[test]
fn a_register_cmov_outside_a_loop_stays() {
    let before = body(held(4), vec![3, 4], false);
    let done = converted(&before, cpu());
    assert_eq!(done, before);
}
