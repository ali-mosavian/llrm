//! Port of `qbopt/backend/copysink.py`: move a copy out of a loop when only
//! what is after the loop reads it.
//!
//! A phi whose value is computed in a loop and read after it becomes a copy on
//! the edge back to the header, so it runs every iteration to hand over a value
//! nothing inside the loop looks at. The copy belongs on the exit edge. That is
//! sound while the destination is read nowhere in the loop and the source is
//! not written between the copy and the exit, which is what this checks.

use std::collections::BTreeSet;
use crate::support::hash::HashSet;
use std::sync::Arc;

use crate::support::hash::IndexMap;

use crate::analysis::loops as loopy;
use crate::backend::liveness::{_backwards, _declared, _terminator, _universe};
use crate::backend::peephole::{Lanes, _lanes, _register_effects, id};
use crate::model::ir::{Loc, Operation, Reg};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::mir::MirBlock;

fn _plain(one: &Insn) -> bool {
    one.what.is_some()
        && one.clobbers.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
        && !one.frame_adjust
        && !one.spill_reload
        && !one.spill_store
}

/// The (destination, source) of a plain register-to-register move.
fn _copy(one: &Insn) -> Option<(Reg, Reg)> {
    if !_plain(one) {
        return None;
    }
    let what = one.what.as_ref()?;
    if let (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Reg(source)]) =
        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
    {
        if dest.width == source.width && dest != source {
            return Some((*dest, *source));
        }
    }
    None
}

/// Whether `one` reads (or writes) any of `lanes`, conservatively.
fn _touches(one: &Insn, lanes: &Lanes, reading: bool) -> bool {
    if _terminator(one.what.as_ref()) {
        return false;
    }
    let mut effects = _register_effects(one, true, true);
    if effects.is_none() {
        effects = _declared(one);
    }
    let Some(effects) = effects else {
        return true;
    };
    !lanes.is_disjoint(if reading { &effects.0 } else { &effects.1 })
}

/// Blocks on a path from the copy's block to the exit that avoids it.
///
/// A path that comes back to the copy's block runs the copy again, so what it
/// writes on the way cannot be why the last copy before the exit is wrong.
fn _between(at_of: &IndexMap<i64, &LirBlock>, inside: &BTreeSet<i64>, copy_at: i64, exit_from: i64) -> Option<BTreeSet<i64>> {
    let mut forward = BTreeSet::new();
    let mut queue: Vec<i64> =
        at_of[&copy_at].succ.iter().copied().filter(|to| inside.contains(to) && *to != copy_at).collect();
    while let Some(at) = queue.pop() {
        if forward.contains(&at) || at == copy_at {
            continue;
        }
        forward.insert(at);
        queue.extend(at_of[&at].succ.iter().copied().filter(|to| inside.contains(to) && *to != copy_at));
    }
    if exit_from != copy_at && !forward.contains(&exit_from) {
        return None;
    }
    let mut backward = BTreeSet::new();
    let mut queue = if exit_from != copy_at { vec![exit_from] } else { Vec::new() };
    while let Some(at) = queue.pop() {
        if backward.contains(&at) || at == copy_at {
            continue;
        }
        backward.insert(at);
        queue.extend(
            inside
                .iter()
                .copied()
                .filter(|one| at_of[one].succ.contains(&at) && *one != copy_at && forward.contains(one)),
        );
    }
    Some(forward.intersection(&backward).copied().collect())
}

/// Per loop block, the lanes live on entry along paths that stay in the loop.
///
/// The exit is left out: a lane only the exit reads is what the sunk copy is
/// for, and a lane a nested loop reads again is not.
fn _round(at_of: &IndexMap<i64, &LirBlock>, inside: &BTreeSet<i64>, universe: &Lanes) -> IndexMap<i64, Lanes> {
    let mut into: IndexMap<i64, Lanes> = inside.iter().map(|at| (*at, Lanes::new())).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for at in inside {
            let after: Lanes = at_of[at]
                .succ
                .iter()
                .filter(|to| inside.contains(to))
                .flat_map(|to| into[to].iter().copied())
                .collect();
            let before = _backwards(at_of[at], after, universe);
            if before != into[at] {
                into.insert(*at, before);
                changing = true;
            }
        }
    }
    into
}

/// `body` with each such copy moved from inside its loop to the exit.
pub fn sunk(body: &LirBody) -> LirBody {
    // `loops.loops` reads only `at` and `succ`.
    let graph: Vec<MirBlock> =
        body.blocks.iter().map(|block| MirBlock::new(block.at, Vec::new(), Vec::new(), block.succ.clone())).collect();
    let found = loopy::loops(&graph, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let universe = _universe();
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut predecessors: IndexMap<i64, Vec<i64>> = at_of.keys().map(|at| (*at, Vec::new())).collect();
    for block in &body.blocks {
        for at in &block.succ {
            if let Some(found) = predecessors.get_mut(at) {
                found.push(block.at);
            }
        }
    }

    let mut moved: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut removed: HashSet<usize> = HashSet::default();
    for found_loop in &found {
        let inside: BTreeSet<i64> = found_loop.body.iter().copied().filter(|at| at_of.contains_key(at)).collect();
        let leaving: Vec<(i64, i64)> = inside
            .iter()
            .flat_map(|at| at_of[at].succ.iter().filter(|to| !inside.contains(to)).map(|to| (*at, *to)))
            .collect();
        // One way out, reached from one place. Any other shape needs the copy
        // on each edge, and an exit block with another predecessor needs that
        // edge split first -- neither is worth inventing for the case at hand.
        if leaving.len() != 1 {
            continue;
        }
        let (source_at, exit_at) = leaving[0];
        if !at_of.contains_key(&exit_at) || predecessors[&exit_at] != [source_at] {
            continue;
        }
        let round_into = _round(&at_of, &inside, &universe);
        for block in inside.iter().map(|at| at_of[at]) {
            for (index, one) in block.insns.iter().enumerate() {
                let pair = _copy(one);
                let Some((dest, register)) = pair else {
                    continue;
                };
                if removed.contains(&id(one)) {
                    continue;
                }
                let (written, read) = (_lanes(dest.register), _lanes(register.register));
                if written.is_empty() || read.is_empty() || !written.is_disjoint(&read) {
                    continue;
                }
                // Dead on every way round, nested loops included. Asked only at
                // the header, PRECALCULATIONS' inner loop read the copy again
                // and the copy left both loops.
                let after: Lanes = block
                    .succ
                    .iter()
                    .filter(|to| inside.contains(to))
                    .flat_map(|to| round_into[to].iter().copied())
                    .collect();
                let rest_of_block = LirBlock { insns: block.insns[index + 1..].to_vec(), ..block.clone() };
                if !written.is_disjoint(&_backwards(&rest_of_block, after, &universe)) {
                    continue;
                }
                let Some(rest) = _between(&at_of, &inside, block.at, source_at) else {
                    continue;
                };
                let later: Vec<&Arc<Insn>> = block.insns[index + 1..]
                    .iter()
                    .chain(rest.iter().flat_map(|at| at_of[at].insns.iter()))
                    .collect();
                let both: Lanes = written.union(&read).copied().collect();
                if later
                    .iter()
                    .any(|other| _touches(other, &both, false) || _touches(other, &written, true))
                {
                    continue;
                }
                removed.insert(id(one));
                moved.entry(exit_at).or_default().push(Arc::clone(one));
            }
        }
    }

    if removed.is_empty() {
        return body.clone();
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = block.insns.clone();
        if insns.iter().any(|one| removed.contains(&id(one))) {
            insns = lir::without(&insns, |one| removed.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
        }
        if let Some(moved) = moved.get(&block.at) {
            insns = moved.iter().cloned().chain(insns).collect();
        }
        blocks.push(LirBlock { insns, ..block.clone() });
    }
    LirBody { blocks, ..body.clone() }
}

#[cfg(test)]
mod tests {
    use crate::support::hash::HashMap;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::sunk;
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn r(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn _insn(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Arc<Insn> {
        // Inserted, as the C path's are: an instruction standing for BC's bytes stays where `lir.without` finds no heir.
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }),
            vec![],
            vec![],
        ))
    }

    fn _move(at: i64, dest: Register, source: Register) -> Arc<Insn> {
        _insn(at, Operation::Move, "mov", vec![r(dest)], vec![r(source)], None)
    }

    fn _compare(at: i64, left: Register, right: Register) -> Arc<Insn> {
        _insn(at, Operation::Compare, "cmp", vec![], vec![r(left), r(right)], None)
    }

    fn _branch(at: i64, name: &str, target: i64) -> Arc<Insn> {
        _insn(at, Operation::Branch, name, vec![], vec![], Some(target))
    }

    fn _jump(at: i64, target: i64) -> Arc<Insn> {
        _insn(at, Operation::Jump, "jmp", vec![], vec![], Some(target))
    }

    fn _return(at: i64) -> Arc<Insn> {
        _insn(at, Operation::Return, "ret", vec![], vec![], None)
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    fn _copies(body: &LirBody) -> HashMap<i64, Vec<Loc>> {
        body.blocks
            .iter()
            .map(|block| {
                (
                    block.at,
                    block
                        .insns
                        .iter()
                        .filter(|one| {
                            let what = one.what.as_ref().unwrap();
                            what.name.as_deref() == Some("mov") && what.sources == [r(Register::DX)]
                        })
                        .map(|one| one.what.as_ref().unwrap().dests[0].clone())
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn test_copy_read_by_an_inner_loop_stays() {
        // Shellsort's gap loop: `mov cx,dx` in the inner loop's header saved `i`
        // for the inner loop's latch; it must not leave for the outer exit.
        use Register::{AX, BX, CX, DX, SI};
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(1, vec![_jump(1, 31)], vec![31]),
                block(31, vec![_compare(31, AX, BX), _branch(32, "jle", 91)], vec![35, 91]),
                block(35, vec![_move(35, DX, AX)], vec![38]),
                block(38, vec![_move(38, CX, DX), _compare(39, DX, BX), _branch(40, "jge", 86)], vec![42, 86]),
                block(42, vec![_move(42, DX, BX)], vec![77]),
                block(77, vec![_move(77, DX, CX), _jump(78, 38)], vec![38]),
                block(86, vec![_move(86, SI, AX), _jump(87, 31)], vec![31]),
                block(91, vec![_return(91)], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        assert_eq!(_copies(&sunk(&body))[&38], vec![r(CX)]);
    }

    #[test]
    fn test_copy_read_only_after_its_loop_moves_to_the_exit() {
        // Plasmablobs: `mov di,dx` on the way back to the header ran every pass for one read after the loop.
        use Register::{AX, BX, DI, DX};
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(1, vec![_jump(1, 3)], vec![3]),
                block(3, vec![_compare(3, AX, BX), _branch(4, "jge", 9)], vec![5, 9]),
                block(5, vec![_move(5, DX, AX), _move(6, DI, DX), _jump(7, 3)], vec![3]),
                block(9, vec![_return(9)], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let copies = _copies(&sunk(&body));
        assert!(copies[&5].is_empty() && copies[&9] == vec![r(DI)]);
    }

    fn _raw_insn(at: i64, what: Semantics) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![]))
    }

    fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 2, address: None })
    }

    /// What the pushes along `path` write, running register moves and adds.
    fn _pushed(body: &LirBody, path: &[i64]) -> Vec<i64> {
        let mut held: HashMap<Register, i64> = HashMap::default();
        let mut pushed = Vec::new();
        let blocks: HashMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
        for at in path {
            for one in &blocks[at].insns {
                let what = one.what.as_ref().unwrap();
                let read = |held: &HashMap<Register, i64>, operand: &Loc| match operand {
                    Loc::Imm(value) => value.value,
                    Loc::Reg(register) => held[&register.register],
                    _ => unreachable!(),
                };
                if [Operation::Move, Operation::Binary].contains(&what.op) {
                    let Loc::Reg(dest) = &what.dests[0] else { unreachable!() };
                    let value = what.sources.iter().map(|source| read(&held, source)).sum();
                    held.insert(dest.register, value);
                } else if what.op == Operation::Push {
                    pushed.push(read(&held, &what.sources[0]));
                }
            }
        }
        pushed
    }

    #[test]
    fn test_a_copy_an_inner_loop_reads_again_stays_in_it() {
        // PRECALCULATIONS' map index was copied back once per row instead of once
        // per pixel, and deedlines drew its minimap from stale plasma data.
        let (si, bx, dx) = (r(Register::SI), r(Register::BX), r(Register::DX));
        let body = LirBody::new(
            "one",
            0,
            vec![
                block(0, vec![_raw_insn(0, sem(Operation::Move, "mov", vec![si.clone()], vec![imm(0)], None))], vec![0x10]),
                block(
                    0x10,
                    vec![
                        _raw_insn(0x10, sem(Operation::Push, "push", vec![], vec![si.clone()], None)),
                        _raw_insn(0x12, sem(Operation::Move, "mov", vec![bx.clone()], vec![si.clone()], None)),
                        _raw_insn(0x14, sem(Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), imm(1)], None)),
                        _raw_insn(0x16, sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), imm(3)], None)),
                        _raw_insn(0x18, sem(Operation::Move, "mov", vec![si], vec![bx], None)),
                        _raw_insn(0x1A, sem(Operation::Branch, "jl", vec![], vec![], Some(0x10))),
                    ],
                    vec![0x10, 0x20],
                ),
                block(
                    0x20,
                    vec![
                        _raw_insn(0x20, sem(Operation::Compare, "cmp", vec![], vec![dx, imm(0)], None)),
                        _raw_insn(0x22, sem(Operation::Branch, "jne", vec![], vec![], Some(0))),
                    ],
                    vec![0, 0x30],
                ),
                block(0x30, vec![_raw_insn(0x30, sem(Operation::Return, "ret", vec![], vec![], None))], vec![]),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        assert_eq!(_pushed(&sunk(&body), &[0, 0x10, 0x10, 0x10]), vec![0, 1, 2]);
    }
}
