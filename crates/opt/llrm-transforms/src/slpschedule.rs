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
//! edges are the order of the writes, and each read between the writes it
//! stood between, with no alias analysis.
//!
//! # Runs
//! Under `slptree` (`schedulable_phis`, `emit`).
//!
//! # Used by
//! `slptree`.
//!
//! # Example
//! Once a tree is vectorized, the vector of each bundle is placed where the
//! last scalar of its bundle stood, and what waits on it follows.
//!
//! ```text
//! store float %f0 / %f1 / %f2 / %f3   (four scalar stores, in this order)
//!         =>
//! the <4 x float> chain is emitted in the place of the last scalar it
//! replaces; the one vector store stands at the last scalar store
//! ```

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

/// The divides of `block` that cannot trap: a constant divisor that is not
/// zero, nor -1 for a signed one (`isSafeToSpeculativelyExecute`). Like a pure
/// call, they are not barriers to the loads around them.
pub(crate) fn safe_divides(
    context: &llrm_mir::context::Context,
    function: &Function,
    block: BlockId,
) -> IdSet<InstId> {
    use llrm_mir::context::ConstantKind;
    use llrm_mir::opcode::BinaryOp;
    function
        .iter_block(block)
        .filter(|inst| {
            let instruction = function.instruction(*inst);
            let Opcode::Binary(op) = instruction.opcode else { return false };
            let signed = match op {
                BinaryOp::SDiv | BinaryOp::SRem => true,
                BinaryOp::UDiv | BinaryOp::URem => false,
                _ => return false,
            };
            let Some(Operand::Constant(divisor)) = instruction.operands.get(1) else { return false };
            let constant = context.get(*divisor);
            let ConstantKind::Int(bits) = constant.kind else { return false };
            let width = context.types.int_bits(constant.ty).unwrap_or(0);
            let ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
            bits != 0 && !(signed && bits == ones)
        })
        .collect()
}

/// Whether `inst` is a computation of its operands alone, which may stand
/// anywhere its operands do. `pure_calls` are the calls and divides that are.
pub(crate) fn is_pure(
    function: &Function,
    inst: InstId,
    pure_calls: &IdSet<InstId>,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Binary(op) => {
            !matches!(
                op,
                llrm_mir::opcode::BinaryOp::UDiv
                    | llrm_mir::opcode::BinaryOp::SDiv
                    | llrm_mir::opcode::BinaryOp::URem
                    | llrm_mir::opcode::BinaryOp::SRem
            ) || pure_calls.contains(&inst)
        }
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

/// The order the memory accesses of `insts` keep (`calculateDependencies`'
/// memory edges): each access after the last write before it, each write after
/// the reads since; two reads need no order. `at` gives an instruction's node.
fn memory_edges(
    function: &Function,
    insts: &[InstId],
    pure_calls: &IdSet<InstId>,
    at: &dyn Fn(InstId) -> usize,
) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    let mut last_write: Option<usize> = None;
    let mut reads: Vec<usize> = Vec::new();
    for inst in insts {
        let node = at(*inst);
        let reading = matches!(
            function.instruction(*inst).opcode,
            Opcode::Load { volatile: false, .. }
        );
        if !reading && is_pure(function, *inst, pure_calls) {
            continue;
        }
        if let Some(write) = last_write
            && write != node
        {
            edges.push((write, node));
        }
        if reading {
            reads.push(node);
        } else {
            for read in reads.drain(..) {
                if read != node {
                    edges.push((read, node));
                }
            }
            last_write = Some(node);
        }
    }
    edges
}

/// A topological order of `nodes` (their edges `reads` and `chain` made into a
/// graph), the earliest `priority` first; none where they wait on each other.
fn topological(
    nodes: &[usize],
    edges: &[(usize, usize)],
    priority: &dyn Fn(usize) -> (usize, usize),
) -> Option<Vec<usize>> {
    // Node numbers are small and dense (instruction positions, then bundles):
    // vectors by number, not hash maps, which cost a SipHash per edge.
    let size = nodes.iter().chain(edges.iter().flat_map(|(from, to)| [from, to])).max().map_or(0, |most| most + 1);
    let mut waiting: Vec<Option<usize>> = vec![None; size];
    for node in nodes {
        waiting[*node] = Some(0);
    }
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); size];
    for (from, to) in edges {
        if from == to {
            return None;
        }
        *waiting[*to].as_mut()? += 1;
        after[*from].push(*to);
    }
    let mut ready: BinaryHeap<Reverse<((usize, usize), usize)>> =
        nodes.iter().filter(|node| waiting[**node] == Some(0)).map(|node| Reverse((priority(*node), *node))).collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(Reverse((_, node))) = ready.pop() {
        order.push(node);
        for next in &after[node] {
            let count = waiting[*next].as_mut().expect("a node");
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
    pure_calls: &IdSet<InstId>,
) -> bool {
    let insts: Vec<InstId> = function
        .iter_block(block)
        .filter(|inst| {
            function.instruction(*inst).opcode != Opcode::Phi && !function.instruction(*inst).opcode.is_terminator()
        })
        .collect();
    let mut node_of: IdMap<InstId, usize> = IdMap::new();
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
    // Writes keep their order, and the reads stay between them.
    edges.extend(memory_edges(function, &insts, pure_calls, &|inst| node_of[&inst]));
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
    pure_calls: &IdSet<InstId>,
    original: &IdMap<InstId, usize>,
) -> bool {
    let Some(terminator) = function.terminator(block) else { return false };
    let insts: Vec<InstId> = function
        .iter_block(block)
        .filter(|inst| function.instruction(*inst).opcode != Opcode::Phi && *inst != terminator)
        .collect();
    // An instruction made for the tree takes the place of the one before it.
    let mut key: IdMap<InstId, (usize, usize)> = IdMap::new();
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
    let node_of: IdMap<InstId, usize> = insts.iter().enumerate().map(|(at, inst)| (*inst, at)).collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for inst in &insts {
        let to = node_of[inst];
        for def in reads(function, block, *inst) {
            edges.push((node_of[&def], to));
        }
    }
    edges.extend(memory_edges(function, &insts, pure_calls, &|inst| node_of[&inst]));
    let nodes: Vec<usize> = (0..insts.len()).collect();
    let Some(order) = topological(&nodes, &edges, &|node| key[&insts[node]]) else { return false };
    for node in order {
        function.move_to(insts[node], Position::Before(terminator)).expect("a placed instruction");
    }
    true
}

#[cfg(test)]
mod topological_tests {
    use super::topological;

    fn by_number(node: usize) -> (usize, usize) {
        (node, 0)
    }

    /// Of the nodes ready, the one with the lowest priority goes first.
    #[test]
    fn the_earliest_ready_node_goes_first() {
        let order = topological(&[0, 1, 2, 3], &[(2, 0), (1, 3)], &by_number).unwrap();
        assert_eq!(order, vec![1, 2, 0, 3]);
    }

    /// Node numbers are positions in a block and bundles after them: a sparse
    /// set of them, the large ones last, is ordered as the dense one is.
    #[test]
    fn sparse_node_numbers_are_ordered_as_dense_ones() {
        let order = topological(&[5, 90_000, 7], &[(90_000, 7), (7, 5)], &by_number).unwrap();
        assert_eq!(order, vec![90_000, 7, 5]);
    }

    #[test]
    fn nodes_that_wait_on_each_other_have_no_order() {
        assert_eq!(topological(&[0, 1, 2], &[(0, 1), (1, 2), (2, 0)], &by_number), None);
        assert_eq!(topological(&[0, 1], &[(1, 1)], &by_number), None);
    }

    /// An edge to a node that is not listed is no schedule, as it was.
    #[test]
    fn an_edge_to_an_unlisted_node_has_no_order() {
        assert_eq!(topological(&[0, 1], &[(0, 9)], &by_number), None);
        assert_eq!(topological(&[0, 1], &[(9, 0)], &by_number), None);
    }
}
