//! `BlockScheduling` for a tree of registers: the order the instructions of
//! the block stand in once a bundle is one instruction.
//!
//! # How it works
//! `BoUpSLP::BlockScheduling` (SLPVectorizer.cpp l.19930) makes each bundle's
//! scalars one `ScheduleBundle`, finds what each instruction waits for
//! (`calculateDependencies`: the values it reads, and the memory the ones
//! before it write), and `scheduleBlock` places the ready ones, bundles whole,
//! in the order the block had where it can. A bundle whose lanes wait on each
//! other cannot be scheduled and the tree is not made. Here the instructions
//! of the block are the nodes, the values they read and the order of those that
//! touch memory are the edges, and the vectors made for the tree are placed
//! where the last scalar of their bundle stood, with whatever waits on them
//! after.
//!
//! DEVIATE SLP6 (owner arm64, exit 2026-12-01): `calculateDependencies`' memory
//! edges are the order of every instruction that is not a pure computation,
//! with no alias analysis: a tree of phis has no memory of its own.
//!
//! # Runs
//! Under `slptree` (`schedulable_phis`, `emit`).
//!
//! # Used by
//! `slptree`.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

/// Whether `inst` is a computation of its operands alone, which may stand
/// anywhere its operands do. `pure_calls` are the calls that are.
pub(crate) fn is_pure(
    function: &Function,
    inst: InstId,
    pure_calls: &HashSet<InstId>,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Binary(op) => !matches!(
            op,
            llrm_mir::opcode::BinaryOp::UDiv
                | llrm_mir::opcode::BinaryOp::SDiv
                | llrm_mir::opcode::BinaryOp::URem
                | llrm_mir::opcode::BinaryOp::SRem
        ),
        Opcode::Cast(_)
        | Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::Select
        | Opcode::FNeg
        | Opcode::ExtractElement
        | Opcode::InsertElement
        | Opcode::ShuffleVector(_)
        | Opcode::GetElementPtr { .. } => true,
        Opcode::Call(_) => pure_calls.contains(&inst),
        _ => false,
    }
}

/// The values `inst` reads that other instructions of `block` (not its phis)
/// make.
fn reads(
    function: &Function,
    block: BlockId,
    inst: InstId,
) -> Vec<InstId> {
    function
        .instruction(inst)
        .operands
        .iter()
        .filter_map(|operand| {
            let Operand::Value(value) = operand else { return None };
            match function.value(*value).def {
                ValueDef::Instruction(def)
                    if function.parent(def) == Some(block) && function.instruction(def).opcode != Opcode::Phi =>
                {
                    Some(def)
                }
                _ => None,
            }
        })
        .collect()
}

/// A topological order of `nodes` (their edges `reads` and `chain` made into a
/// graph), the earliest `priority` first; none where they wait on each other.
fn topological(
    nodes: &[usize],
    edges: &[(usize, usize)],
    priority: &dyn Fn(usize) -> (usize, usize),
) -> Option<Vec<usize>> {
    let mut waiting: HashMap<usize, usize> = nodes.iter().map(|node| (*node, 0)).collect();
    let mut after: HashMap<usize, Vec<usize>> = HashMap::new();
    for (from, to) in edges {
        if from == to {
            return None;
        }
        *waiting.get_mut(to)? += 1;
        after.entry(*from).or_default().push(*to);
    }
    let mut ready: BinaryHeap<Reverse<((usize, usize), usize)>> =
        nodes.iter().filter(|node| waiting[*node] == 0).map(|node| Reverse((priority(*node), *node))).collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(Reverse((_, node))) = ready.pop() {
        order.push(node);
        for next in after.get(&node).into_iter().flatten() {
            let count = waiting.get_mut(next).expect("a node");
            *count -= 1;
            if *count == 0 {
                ready.push(Reverse((priority(*next), *next)));
            }
        }
    }
    (order.len() == nodes.len()).then_some(order)
}

/// Whether the instructions of `block`, each bundle (`bundles`: the scalars of
/// each, and the values each reads through a gather) one node, can be ordered:
/// the bundles' lanes do not wait on each other, nor round a cycle of bundles.
pub(crate) fn acyclic(
    function: &Function,
    block: BlockId,
    bundles: &[(Vec<InstId>, Vec<InstId>)],
    pure_calls: &HashSet<InstId>,
) -> bool {
    let insts: Vec<InstId> = function
        .iter_block(block)
        .filter(|inst| {
            function.instruction(*inst).opcode != Opcode::Phi && !function.instruction(*inst).opcode.is_terminator()
        })
        .collect();
    let mut node_of: HashMap<InstId, usize> = HashMap::new();
    for (at, inst) in insts.iter().enumerate() {
        node_of.insert(*inst, at);
    }
    let base = insts.len();
    for (bundle, (lanes, _)) in bundles.iter().enumerate() {
        for lane in lanes {
            if node_of.contains_key(lane) {
                node_of.insert(*lane, base + bundle);
            }
        }
    }
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for inst in &insts {
        let to = node_of[inst];
        for def in reads(function, block, *inst) {
            let from = node_of[&def];
            if from == to {
                // Two lanes of one bundle, one reading the other.
                return false;
            }
            edges.push((from, to));
        }
    }
    // What a bundle reads through a gather is read before it.
    for (bundle, (_, gathered)) in bundles.iter().enumerate() {
        for def in gathered {
            if let Some(from) = node_of.get(def) {
                edges.push((*from, base + bundle));
            }
        }
    }
    // The instructions that are not pure computations keep their order.
    let mut last: Option<usize> = None;
    for inst in &insts {
        if !is_pure(function, *inst, pure_calls) {
            let at = node_of[inst];
            if let Some(before) = last
                && before != at
            {
                edges.push((before, at));
            }
            last = Some(at);
        }
    }
    let mut nodes: Vec<usize> =
        (0..base).filter(|at| !bundles.iter().any(|(lanes, _)| lanes.contains(&insts[*at]))).collect();
    nodes.extend((0..bundles.len()).map(|bundle| base + bundle));
    topological(&nodes, &edges, &|node| (node, 0)).is_some()
}

/// The instructions of `block` after its phis, in the earliest order that
/// keeps each after what it reads and the impure ones as they were; the
/// instructions made for the tree stand where the instruction before them did.
pub(crate) fn schedule(
    function: &mut Function,
    block: BlockId,
    pure_calls: &HashSet<InstId>,
    original: &HashMap<InstId, usize>,
) -> bool {
    let Some(terminator) = function.terminator(block) else { return false };
    let insts: Vec<InstId> = function
        .iter_block(block)
        .filter(|inst| function.instruction(*inst).opcode != Opcode::Phi && *inst != terminator)
        .collect();
    // An instruction made for the tree takes the place of the one before it.
    let mut key: HashMap<InstId, (usize, usize)> = HashMap::new();
    let mut inherited = 0usize;
    let mut sequence = 0usize;
    for inst in &insts {
        match original.get(inst) {
            Some(place) => {
                inherited = *place;
                sequence = 0;
                key.insert(*inst, (*place, 0));
            }
            None => {
                sequence += 1;
                key.insert(*inst, (inherited, sequence));
            }
        }
    }
    let node_of: HashMap<InstId, usize> = insts.iter().enumerate().map(|(at, inst)| (*inst, at)).collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for inst in &insts {
        let to = node_of[inst];
        for def in reads(function, block, *inst) {
            edges.push((node_of[&def], to));
        }
    }
    let mut last: Option<usize> = None;
    for inst in &insts {
        if !is_pure(function, *inst, pure_calls) {
            let at = node_of[inst];
            if let Some(before) = last {
                edges.push((before, at));
            }
            last = Some(at);
        }
    }
    let nodes: Vec<usize> = (0..insts.len()).collect();
    let Some(order) = topological(&nodes, &edges, &|node| key[&insts[node]]) else { return false };
    for node in order {
        function.move_to(insts[node], Position::Before(terminator)).expect("a placed instruction");
    }
    true
}
