//! Port of `qbopt/backend/machinecse.py`: eliminate repeated deterministic
//! computations after register allocation.
//!
//! MIR GVN sees values, not the physical instructions lowering and allocation
//! create.  At this point the question is entirely physical: value-number the
//! byte lanes read and written by independently reproducible instructions, and
//! retain an anchor for the virtual/data-source ownership of a redundant
//! occurrence.

use crate::support::hash::{HashMap, HashSet};
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::peephole::{Lane, Lanes, _lanes, id};
use crate::backend::target;
use crate::model::ir::{Addr, Loc, Operation, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::pyrepr::Repr;

const _REPRODUCIBLE: [(Operation, &str); 4] = [
    (Operation::Move, "mov"),
    (Operation::Extend, "movsx"),
    (Operation::Extend, "movzx"),
    (Operation::Address, "lea"),
];

/// Remove an exact physical recomputation whose inputs still agree.
pub struct MachineCSE;

impl LIRTransform for MachineCSE {
    fn class_name(&self) -> &'static str {
        "MachineCSE"
    }

    fn name(&self) -> &str {
        "machine-cse"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        eliminated(&body)
    }
}

fn _relocated(where_: &Loc) -> bool {
    match where_ {
        Loc::Imm(one) => one.address.is_some(),
        Loc::Address(one) => one.addr.is_some_and(|addr| addr.space != Space::Frame),
        _ => false,
    }
}

/// `_shape`'s tuples: every encoded field of a source.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Shape {
    Reg(Register, u32),
    Imm(i64, u32, Option<Addr>),
    Address(Option<Addr>, Register, Register, i64, i64, u32),
}

/// Every encoded field of a source, including compare=False address fields.
fn _shape(where_: &Loc) -> Result<Shape, String> {
    match where_ {
        Loc::Reg(one) => Ok(Shape::Reg(one.register, one.width)),
        Loc::Imm(one) => Ok(Shape::Imm(one.value, one.width, one.address)),
        Loc::Address(one) => {
            Ok(Shape::Address(one.addr, one.through, one.index, one.scale, one.offset, one.disp_width))
        }
        _ => Err(format!("machine CSE source is not independently reproducible: {}", where_.repr())),
    }
}

/// Physical input lanes, or None for a register this tracker omits.
fn _source_lanes(where_: &Loc) -> Option<Lanes> {
    let registers: Vec<Register> = match where_ {
        Loc::Reg(one) => vec![one.register],
        Loc::Address(one) => {
            [one.through, one.index].into_iter().filter(|register| *register != Register::None).collect()
        }
        Loc::Imm(_) => Vec::new(),
        _ => return None,
    };
    let mut lanes = Lanes::new();
    for register in registers {
        let found = _lanes(register);
        if found.is_empty() {
            return None;
        }
        lanes.extend(found);
    }
    Some(lanes)
}

/// `(what.op, what.name, tuple(map(_shape, what.sources)), destination.width)`.
pub type Expression = (Operation, Option<String>, Vec<Shape>, u32);

/// The pure register result this occurrence can independently reproduce.
///
/// These are the x86 forms whose result is independent of their destination's
/// previous contents and which do not set flags.  Memory reads are excluded:
/// proving a cell unchanged is MIR's MemorySSA job, while this pass exists for
/// artifacts created below that boundary.  Relocations are excluded because
/// every emitted symbolic occurrence owns a fixup as well as instruction
/// bytes.
fn _candidate(one: &Insn) -> Result<Option<(Expression, Vec<Lane>, Vec<Lane>)>, String> {
    let Some(what) = &one.what else {
        return Ok(None);
    };
    if !_REPRODUCIBLE.iter().any(|(op, name)| what.op == *op && what.name.as_deref() == Some(*name))
        || what.dests.len() != 1
        || !matches!(what.dests[0], Loc::Reg(_))
        || what.sources.iter().any(|arg| !matches!(arg, Loc::Reg(_) | Loc::Imm(_) | Loc::Address(_)))
        || what.dests.iter().chain(&what.sources).any(_relocated)
        || what.target.is_some()
        || what.indirect
        || !one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.symbol == Some(true)
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.op.as_ref().is_some_and(|op| op.barrier())
    {
        return Ok(None);
    }
    let read_sets: Vec<Option<Lanes>> = what.sources.iter().map(_source_lanes).collect();
    if read_sets.iter().any(Option::is_none) {
        return Ok(None);
    }
    let reads: Lanes = read_sets.into_iter().flatten().flatten().collect();
    let Loc::Reg(destination) = &what.dests[0] else {
        unreachable!("checked above");
    };
    let writes = _lanes(destination.register);
    if writes.is_empty() || !reads.is_disjoint(&writes) {
        return Ok(None);
    }
    // Destination placement is deliberately absent.  The computed value is
    // identified by the operation, its explicit inputs and the values currently
    // occupying their physical lanes; the output tokens below say where it is.
    let shapes = what.sources.iter().map(_shape).collect::<Result<Vec<Shape>, String>>()?;
    let expression = (what.op, what.name.clone(), shapes, destination.width);
    Ok(Some((expression, reads.into_iter().collect(), writes.into_iter().collect())))
}

/// Explicit and declared physical writes, or None for an opaque boundary.
fn _written(one: &Insn) -> Option<Lanes> {
    let what = one.what.as_ref()?;
    if [Operation::Barrier, Operation::Call, Operation::Return, Operation::Fill, Operation::Leave].contains(&what.op) {
        return None;
    }
    let mut writes = Lanes::new();
    for dest in &what.dests {
        if let Loc::Reg(dest) = dest {
            writes.extend(_lanes(dest.register));
        }
    }
    for (held, register) in &one.delivers {
        writes.extend(_lanes(target::named(*register, i64::from(held.width))));
    }
    for register in &one.clobbers {
        writes.extend(_lanes(*register));
    }
    for register in &one.clobbers_high {
        writes.extend(_lanes(*register).into_iter().filter(|lane| lane.1 >= 2));
    }
    Some(writes)
}

/// The tuples a lane's value is named by.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    /// `("block-entry", block.at, lane)`.
    BlockEntry(i64, Lane),
    /// `(("expression", expression, inputs), byte)`.
    Value(Arc<(Expression, Vec<(Lane, Token)>)>, usize),
    /// `("written", id(one), lane)`.
    Written(usize, Lane),
    /// `("entry", lane)`.
    Entry(Lane),
}

pub type State = HashMap<Lane, Token>;

/// Physical values leaving one block and redundant occurrences within it.
fn _transfer(block: &LirBlock, incoming: &State) -> Result<(State, HashSet<usize>), String> {
    let mut state = incoming.clone();
    let mut redundant: HashSet<usize> = HashSet::default();
    for one in &block.insns {
        let candidate = _candidate(one)?;
        if let Some((expression, reads, writes)) = candidate {
            let inputs: Vec<(Lane, Token)> = reads
                .iter()
                .map(|lane| (*lane, state.get(lane).cloned().unwrap_or(Token::BlockEntry(block.at, *lane))))
                .collect();
            let value = Arc::new((expression, inputs));
            let wanted: Vec<(Lane, Token)> =
                writes.iter().enumerate().map(|(byte, lane)| (*lane, Token::Value(Arc::clone(&value), byte))).collect();
            if wanted.iter().all(|(lane, token)| state.get(lane) == Some(token)) {
                redundant.insert(id(one));
                continue;
            }
            state.extend(wanted);
            continue;
        }
        let Some(writes) = _written(one) else {
            state.clear();
            continue;
        };
        for lane in writes {
            state.insert(lane, Token::Written(id(one), lane));
        }
    }
    Ok((state, redundant))
}

/// The physical lane values every incoming edge agrees on.
fn _merged(states: &[&State]) -> State {
    let Some(first) = states.first() else {
        return State::default();
    };
    let mut common = (*first).clone();
    for state in &states[1..] {
        common.retain(|lane, token| state.get(lane) == Some(token));
    }
    common
}

fn _lanes_used(body: &LirBody) -> Result<Lanes, String> {
    let mut lanes = Lanes::new();
    for one in body.insns() {
        if let Some((_expression, reads, writes)) = _candidate(&one)? {
            lanes.extend(reads);
            lanes.extend(writes);
        } else if let Some(writes) = _written(&one) {
            lanes.extend(writes);
        }
    }
    Ok(lanes)
}

/// Value-number deterministic register computations across the CFG.
pub fn eliminated(body: &LirBody) -> Result<LirBody, String> {
    let mut predecessors: HashMap<i64, HashSet<i64>> =
        body.blocks.iter().map(|block| (block.at, HashSet::default())).collect();
    for block in &body.blocks {
        for successor in &block.succ {
            if let Some(found) = predecessors.get_mut(successor) {
                found.insert(block.at);
            }
        }
    }
    let entry: State = _lanes_used(body)?.into_iter().map(|lane| (lane, Token::Entry(lane))).collect();
    let empty = State::default();
    let mut outgoing: HashMap<i64, State> = HashMap::default();
    let mut redundant: HashMap<i64, HashSet<usize>> = HashMap::default();
    // Acyclic facts normally settle in layout order in one pass. Reversed
    // blocks and conservative loop joins may need more; refusal to converge
    // keeps the body unchanged rather than trusting a partial physical state.
    let mut converged = false;
    for _round in 0..std::cmp::max(1, body.blocks.len() * 4) {
        let mut changed = false;
        for block in &body.blocks {
            let mut states: Vec<&State> =
                predecessors[&block.at].iter().filter_map(|at| outgoing.get(at)).collect();
            if states.len() != predecessors[&block.at].len() {
                states.push(&empty);
            }
            if block.at == body.entry {
                states.push(&entry);
            }
            let incoming = _merged(&states);
            let (after, gone) = _transfer(block, &incoming)?;
            if outgoing.get(&block.at) != Some(&after) || redundant.get(&block.at) != Some(&gone) {
                outgoing.insert(block.at, after);
                redundant.insert(block.at, gone);
                changed = true;
            }
        }
        if !changed {
            converged = true;
            break;
        }
    }
    if !converged {
        return Ok(body.clone());
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match redundant.get(&block.at) {
            Some(gone) if !gone.is_empty() => block.with_insns(block
                    .insns
                    .iter()
                    .map(|one| if gone.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                    .collect()),
            _ => block.clone(),
        })
        .collect();
    Ok(body.with_blocks(blocks))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::eliminated;
    use crate::model::ir::{Addr, Address, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn what(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
    }

    fn bx() -> Loc {
        Loc::Reg(Reg { register: Register::BX, width: 2 })
    }

    fn _lea(value: u32, at: i64) -> Arc<Insn> {
        let address = Address {
            through: Register::BP,
            offset: -100,
            disp_width: 1,
            ..Address::new(Some(Addr::new(Space::Frame, -100)))
        };
        Arc::new(Insn::new(
            at,
            Some((at, at + 3)),
            what(Operation::Address, "lea", vec![bx()], vec![Loc::Address(address)]),
            vec![value],
            vec![],
        ))
    }

    fn _body(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new("machine-cse", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default())
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    fn overwrite(at: i64, covers: (i64, i64), defines: u32) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some(covers),
            what(Operation::Move, "mov", vec![bx()], vec![Loc::Imm(Imm { value: 7, width: 2, address: None })]),
            vec![defines],
            vec![],
        ))
    }

    fn successor(result: &LirBody) -> &LirBlock {
        result.blocks.iter().find(|block| block.at == 5).unwrap()
    }

    #[test]
    fn test_repeated_frame_address_is_anchored_when_registers_are_unchanged() {
        // C nbody emitted `lea bx,[bp-100]` twice around one x87 update.
        let (first, repeated) = (_lea(1, 0), _lea(2, 5));
        let store = Arc::new(Insn::new(
            3,
            Some((3, 5)),
            what(
                Operation::Move,
                "mov",
                vec![Loc::Mem(Mem {
                    through: Register::BP,
                    disp_width: 1,
                    ..Mem::new(Some(Addr::new(Space::Frame, -2)), 2)
                })],
                vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
            ),
            vec![],
            vec![],
        ));

        let result = eliminated(&_body(vec![Arc::clone(&first), store, Arc::clone(&repeated)])).unwrap();

        let insns = result.insns();
        assert_eq!(insns[0].what, first.what);
        assert_eq!(insns[2].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(insns[2].defines, repeated.defines, "the virtual definition and byte ownership must survive");
    }

    #[test]
    fn test_repeated_frame_address_is_kept_after_destination_changes() {
        let (first, repeated) = (_lea(1, 0), _lea(2, 5));
        let result = eliminated(&_body(vec![first, overwrite(3, (3, 5), 3), Arc::clone(&repeated)])).unwrap();
        assert_eq!(result.insns()[2].what, repeated.what);
    }

    #[test]
    fn test_repeated_frame_address_is_eliminated_across_a_cfg_edge() {
        // Lowering rebuilt the same frame address in two consecutive blocks.
        let (first, repeated) = (_lea(1, 0), _lea(2, 5));
        let body = LirBody::new(
            "machine-cse",
            0,
            vec![block(0, vec![first], vec![5]), block(5, vec![Arc::clone(&repeated)], vec![])],
            IndexMap::default(),
            IndexMap::default(),
        );

        let result = eliminated(&body).unwrap();

        assert_eq!(successor(&result).insns[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(successor(&result).insns[0].defines, repeated.defines);
    }

    #[test]
    fn test_repeated_frame_address_is_eliminated_after_agreeing_branches() {
        let (first, second, repeated) = (_lea(1, 1), _lea(2, 2), _lea(3, 5));
        let body = LirBody::new(
            "machine-cse",
            0,
            vec![
                block(0, vec![], vec![1, 2]),
                block(1, vec![first], vec![5]),
                block(2, vec![second], vec![5]),
                block(5, vec![repeated], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );

        let result = eliminated(&body).unwrap();

        assert_eq!(successor(&result).insns[0].what.as_ref().unwrap().op, Operation::Nothing);
    }

    #[test]
    fn test_repeated_frame_address_is_kept_when_one_branch_disagrees() {
        let (first, repeated) = (_lea(1, 1), _lea(3, 5));
        let body = LirBody::new(
            "machine-cse",
            0,
            vec![
                block(0, vec![], vec![1, 2]),
                block(1, vec![first], vec![5]),
                block(2, vec![overwrite(2, (2, 4), 2)], vec![5]),
                block(5, vec![Arc::clone(&repeated)], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );

        let result = eliminated(&body).unwrap();

        assert_eq!(successor(&result).insns[0].what, repeated.what);
    }

    #[test]
    fn test_repeated_frame_address_is_kept_across_an_opaque_call() {
        // A callee may replace every physical input and output register.
        let (first, repeated) = (_lea(1, 0), _lea(3, 5));
        let call = Arc::new(Insn::new(
            3,
            Some((3, 5)),
            Some(Semantics { name: Some("call".to_owned()), target: Some(10), ..Semantics::new(Operation::Call) }),
            vec![2],
            vec![],
        ));
        let body = LirBody::new(
            "machine-cse",
            0,
            vec![block(0, vec![first], vec![3]), block(3, vec![call], vec![5]), block(5, vec![Arc::clone(&repeated)], vec![])],
            IndexMap::default(),
            IndexMap::default(),
        );

        let result = eliminated(&body).unwrap();

        assert_eq!(successor(&result).insns[0].what, repeated.what);
    }

    #[test]
    fn test_address_coefficients_are_part_of_the_expression_identity() {
        // `ax + si*2` and `si + ax*2` read the same lanes and compute different numbers.
        let lea = |at: i64, covers: (i64, i64), through: Register, index: Register, defines: u32| {
            Arc::new(Insn::new(
                at,
                Some(covers),
                what(
                    Operation::Address,
                    "lea",
                    vec![bx()],
                    vec![Loc::Address(Address { through, index, scale: 2, ..Address::new(None) })],
                ),
                vec![defines],
                vec![],
            ))
        };
        let first = lea(0, (0, 3), Register::AX, Register::SI, 1);
        let different = lea(3, (3, 6), Register::SI, Register::AX, 2);

        let result = eliminated(&_body(vec![first, Arc::clone(&different)])).unwrap();

        assert_eq!(result.insns()[1].what, different.what);
    }
}
