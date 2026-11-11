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

/// A register `cmov` on a loop-carried chain: `x := cmov(x < v7, x, v4)`, the
/// flags waiting on `x` and on two chained loads that are off the chain.
fn body_carried() -> LirBody {
    let load = insn(8, Operation::Move, "mov", vec![held(6)], vec![cell()], vec![6], vec![4]);
    let chased = Loc::Mem(Mem { base: Some(Held { value: 6, width: 8, offset: 0 }), ..Mem::new(None, 8) });
    let again = insn(9, Operation::Move, "mov", vec![held(7)], vec![chased], vec![7], vec![6]);
    let cmp = insn(10, Operation::Compare, "cmp", vec![], vec![held(20), held(7)], vec![], vec![20, 7]);
    let cmov = insn(11, Operation::Select, "cmovl", vec![held(21)], vec![held(20), held(4)], vec![21], vec![20, 4]);
    let branch = insn(13, Operation::Branch, "jne", vec![], vec![], vec![], vec![]);
    let ret = insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![]);
    let jump = insn(15, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
    let mut looped = LirBlock { succ: vec![1, 2].into(), ..LirBlock::new(1, vec![load, again, cmp, cmov, branch]) };
    looped.phis = vec![Phi { result: 20, incoming: vec![(0, 3), (1, 21)] }].into();
    let blocks = vec![block(0, vec![1], vec![jump]), looped.into(), block(2, vec![], vec![ret])];
    let mut body = LirBody::new("f", 0, blocks, IndexMap::default(), IndexMap::default());
    body.inputs = [3, 4].into_iter().collect();
    body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
    body.bits = 64;
    body
}

/// x86-64's latencies are the SandyBridge model's (cmov 2, load 5, imul 4):
/// the converter priced a load at 1 and a `cmov` at 1, so a loop whose flags
/// wait on a load never reached the gain threshold and no register `cmov`
/// became a branch (jpeg_fixed -O2: 42 cmovs, the pin 33).
#[test]
fn a_carried_register_cmov_is_a_branch_at_the_sandybridge_latencies() {
    let mut profile = cpu().clone();
    profile.mispredict_penalty = Some(16);
    profile.speed_rate = true;
    let before = body_carried();
    assert_ne!(converted(&before, &profile), before);
    let old: Vec<(String, i64)> = [("mov_rm", 1), ("cmov", 1), ("alu_rm", 1)].map(|(a, b)| (a.to_owned(), b)).to_vec();
    std::sync::Arc::make_mut(&mut profile._latencies).retain(|(name, _)| !old.iter().any(|(other, _)| other == name));
    std::sync::Arc::make_mut(&mut profile._latencies).extend(old);
    assert_eq!(converted(&before, &profile), before);
}

/// The table the test above reads. A register `imul` is 3 (SandyBridge `WriteIMul32Reg`, load 5 more
/// for the memory form), not the one-operand form's 4: at 4 every `imul` in a cmov loop
/// deepened the chain by one more clock than the pin's model.
#[test]
fn the_x86_64_table_states_the_sandybridge_latencies() {
    let profile = cpu();
    assert_eq!(
        ["cmov", "mov_rm", "alu_rm", "imul_r32", "imul_m32"].map(|form| profile.latency(form).unwrap()),
        [2, 5, 6, 3, 8]
    );
}

/// m64's table stated 1000 at every level, so jpeg_fixed -O2 kept 42 cmovs where
/// the pin's SandyBridge penalty (16) leaves 33. -O2/-O3 take the pin's penalty;
/// -O1, -Os and -Oz keep the 1000 (CMC3: the conversion cost -Os +2.6% on Zen 5).
#[test]
fn a_carried_register_cmov_is_a_branch_at_o2_and_stays_at_o1_and_size_at_m64() {
    let before = body_carried();
    let mut at = cpu().clone();
    at.speed_rate = true;
    assert_ne!(converted(&before, &at), before, "-O2/-O3");
    let mut o1 = cpu().clone();
    o1.speed_rate = false;
    assert_eq!(converted(&before, &o1), before, "-O1");
    let mut os = at.clone();
    os.size = true;
    assert_eq!(converted(&before, &os), before, "-Os");
    os.size = false;
    os.min_size = true;
    assert_eq!(converted(&before, &os), before, "-Oz");
}

/// A `movzx` from memory is a load (SandyBridge `WriteLoad`, 5), not the register form's 1: the
/// YCbCr loop of jpeg_fixed -O2 kept three clamp `cmovs` the pin turns into branches, its byte
/// loads priced 4 clocks short.
#[test]
fn a_movzx_from_memory_costs_a_load_in_the_depth_walk() {
    let load = insn(1, Operation::Extend, "movzx", vec![held(6)], vec![cell()], vec![6], vec![4]);
    assert_eq!(latency_of(&load, cpu()), 5);
}
