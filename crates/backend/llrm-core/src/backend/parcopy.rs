//! Port of `qbopt/backend/parcopy.py`: scheduling a parallel copy, moves
//! that happen at once, written in an order.
//!
//! After allocation: which moves conflict is a question about locations. A
//! move may go once nothing left in the group reads what it writes; a cycle
//! is exchanged or rotated through the machine stack: every cycle can be.

use std::fmt;
use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::{IndexMap, IndexSet};

use crate::backend::target;
use crate::model::ir::{self, Loc, Mem, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::pyrepr::Repr;

/// Something in a copy group that is not a move of one place to another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Malformed(pub String);

impl fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Malformed {}

pub struct ParallelCopy;

impl LIRTransform for ParallelCopy {
    fn class_name(&self) -> &'static str {
        "ParallelCopy"
    }

    fn name(&self) -> &str {
        "parcopy"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        scheduled(&body).map_err(|refused| refused.to_string())
    }

    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        scheduled(&body).map_err(|malformed| Exception::defined_in("qbopt.backend.parcopy", "Malformed", malformed.to_string()))
    }
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics {
        name: Some(name.to_owned()),
        dests,
        sources,
        ..Semantics::new(op)
    }
}

fn _width(one: &Loc) -> u32 {
    match one {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        Loc::Imm(one) => one.width,
        Loc::Held(one) => one.width,
        Loc::Address(_) | Loc::St(_) => panic!("{} has no width", one.repr()),
    }
}

/// Python `list.remove`: drop the first element equal to `one`.
fn remove(left: &mut Vec<Arc<Insn>>, one: &Arc<Insn>) {
    let index = left.iter().position(|each| **each == **one).expect("list.remove(x): x not in list");
    left.remove(index);
}

/// `body` with every copy group written in an order that computes it.
pub fn scheduled(body: &LirBody) -> Result<LirBody, Malformed> {
    if !body.blocks.iter().any(|block| block.insns.iter().any(|one| one.group.is_some())) {
        return Ok(body.clone());
    }
    let mut out_body = body.clone();
    for block in &mut out_body.blocks {
        let mut out: Vec<Arc<Insn>> = Vec::new();
        let mut run: Vec<Arc<Insn>> = Vec::new();
        for one in block.insns.iter().map(Some).chain([None]) {
            let group = one.and_then(|one| one.group);
            if !run.is_empty() && group != run[0].group {
                for part in _ordered(&run)? {
                    out.extend(_expanded(&part)?);
                }
                run = Vec::new();
            }
            let Some(one) = one else { break };
            if group.is_none() {
                out.push(Arc::clone(one));
                continue;
            }
            run.push(Arc::clone(one));
        }
        block.insns = out;
    }
    Ok(out_body)
}

/// An ordered frame copy needs no scratch register and preserves flags.
fn _expanded(one: &Arc<Insn>) -> Result<Vec<Arc<Insn>>, Malformed> {
    if one.what.as_ref().is_some_and(|what| what.op != Operation::Move) {
        return Ok(vec![Arc::clone(one)]);
    }
    let what = one.what.as_ref().expect("a move in a copy group");
    let (into, source) = (&what.dests[0], &what.sources[0]);
    let (Loc::Mem(into), Loc::Mem(source)) = (into, source) else {
        return Ok(vec![Arc::clone(one)]);
    };
    if into.width != source.width
        || !matches!(into.width, 1 | 2 | 4)
        || [into, source].iter().any(|cell| cell.through != Register::BP)
    {
        return Err(Malformed("memory parallel copy needs equal-width frame slots".to_owned()));
    }
    // A copy moves held values, so its cells are spill slots, and a slot
    // owns at least a word: a byte's is moved whole.
    let [into, source] = [into, source].map(|cell| Mem { width: cell.width.max(2), ..cell.clone() });
    let mut push = (**one).clone();
    push.what = Some(semantics(Operation::Push, "push", vec![], vec![Loc::Mem(source)]));
    push.defines = vec![];
    push.uses = vec![];
    let mut pop = (**one).clone();
    pop.what = Some(semantics(Operation::Pop, "pop", vec![Loc::Mem(into)], vec![]));
    pop.covers = Some((one.at, one.at));
    pop.op = None;
    pop.defines = vec![];
    pop.uses = vec![];
    pop.spread = vec![];
    Ok(vec![Arc::new(push), Arc::new(pop)])
}

fn ungrouped(one: &Insn) -> Arc<Insn> {
    let mut out = one.clone();
    out.group = None;
    Arc::new(out)
}

/// One group, in an order where no move reads what an earlier one wrote.
fn _ordered(moves: &[Arc<Insn>]) -> Result<Vec<Arc<Insn>>, Malformed> {
    let mut identities = Vec::new();
    for one in moves {
        if _into(one)? == _outof(one)? {
            identities.push(Arc::clone(one));
        }
    }
    let mut left = Vec::new();
    for one in moves {
        if _into(one)? != _outof(one)? {
            left.push(Arc::clone(one));
        }
    }
    // An identity emits no machine instruction but still defines the
    // virtual value its destination represents: keep a zero-cost marker.
    let mut out: Vec<Arc<Insn>> = identities.into_iter().map(lir::anchor).collect();
    while !left.is_empty() {
        // Free where nothing still to come reads the place it writes.
        let wanted: IndexSet<String> = left.iter().map(|one| _outof(one)).collect::<Result<_, _>>()?;
        let mut ready = Vec::new();
        for one in &left {
            if !wanted.contains(&_into(one)?) {
                ready.push(Arc::clone(one));
            }
        }
        if ready.is_empty() {
            let (made, used) = _rotated(&left)?;
            out.extend(made);
            for one in &used {
                remove(&mut left, one);
            }
            continue;
        }
        for one in &ready {
            out.push(ungrouped(one));
            remove(&mut left, one);
        }
    }
    Ok(out)
}

type Rotation = (Vec<Arc<Insn>>, Vec<Arc<Insn>>);

/// One cycle out of `left`, broken without a free register: by exchanges
/// where the target can exchange every adjacent pair, else through the
/// machine stack.
///
/// A cycle `p1 <- p2 <- ... <- pn <- p1` is `xchg p1,p2` then `xchg p2,p3`
/// and so on. Otherwise save the place the closing move reads on the stack,
/// perform the remaining moves in order, then pop into the closing move's
/// destination.
fn _rotated(left: &[Arc<Insn>]) -> Result<Rotation, Malformed> {
    let mut writes: IndexMap<String, &Arc<Insn>> = IndexMap::default();
    for one in left {
        writes.insert(_into(one)?, one);
    }
    let start = &left[0];
    let mut cycle: Vec<Arc<Insn>> = vec![Arc::clone(start)];
    let mut place = _outof(start)?;
    while place != _into(start)? {
        let one = writes.get(&place).copied().filter(|one| !cycle.iter().any(|each| **each == ***one));
        let Some(one) = one else {
            return Err(Malformed(format!("{:#06x} is in a copy group that is not a permutation of its places", start.at)));
        };
        cycle.push(Arc::clone(one));
        place = _outof(one)?;
    }

    let dest = |one: &Arc<Insn>| one.what.as_ref().expect("a move").dests[0].clone();
    let source = |one: &Arc<Insn>| one.what.as_ref().expect("a move").sources[0].clone();
    let operands: Vec<Loc> = cycle.iter().map(dest).collect();
    // One width across the whole chain: `_named` keys a register by its
    // root, so `mov ax,bx` and `mov ebx,eax` walk as one clean 2-cycle
    // whose exchange would be `xchg ax,ebx`.
    let widths: IndexSet<u32> = operands.iter().map(_width).collect();
    let pairs: Vec<(&Loc, &Loc)> = operands.iter().zip(operands.iter().skip(1)).collect();
    if widths.len() == 1 && pairs.iter().all(|(one, other)| target::exchangeable(one, other)) {
        let mut made: Vec<Arc<Insn>> = cycle
            .iter()
            .zip(&pairs)
            .map(|(one, (a, b))| {
                let mut exchange = (**one).clone();
                exchange.what = Some(semantics(
                    Operation::Exchange,
                    "xchg",
                    vec![(*a).clone(), (*b).clone()],
                    vec![(*b).clone(), (*a).clone()],
                ));
                exchange.group = None;
                Arc::new(exchange)
            })
            .collect();
        // The closing logical move contributes no machine instruction, but
        // it still defines the virtual value consumed after this edge.
        made.push(lir::anchor(Arc::clone(&cycle[cycle.len() - 1])));
        return Ok((made, cycle));
    }

    // Close the cycle on a move the stack can carry: its source is saved
    // first, and every other move, in order, writes only what was read.
    let closing = cycle
        .iter()
        .rposition(|one| target::pushed_width(&source(one)).is_some() && target::popped_width(&dest(one)).is_some())
        .ok_or_else(|| Malformed(format!("{:#06x} is in a copy cycle no move of which the stack can carry", start.at)))?;
    let count = cycle.len();
    cycle.rotate_left((closing + 1) % count);
    let (first, last) = (&cycle[0], &cycle[count - 1]);
    let mut made = vec![push_of(first, source(last))];
    made.extend(cycle[..count - 1].iter().map(|one| ungrouped(one)));
    made.push(pop_into(last, dest(last)));
    Ok((made, cycle))
}

/// `place` at the width the stack carries it: a frame cell's byte moves whole.
fn _stacked(place: Loc) -> Loc {
    match (&place, target::pushed_width(&place)) {
        (Loc::Mem(cell), Some(width)) => Loc::Mem(Mem { width, ..cell.clone() }),
        _ => place,
    }
}

fn push_of(start: &Insn, saved: Loc) -> Arc<Insn> {
    let mut push = start.clone();
    push.what = Some(semantics(Operation::Push, "push", vec![], vec![_stacked(saved)]));
    push.group = None;
    push.defines = vec![];
    push.uses = vec![];
    Arc::new(push)
}

fn pop_into(last: &Insn, into: Loc) -> Arc<Insn> {
    let mut pop = last.clone();
    pop.what = Some(semantics(Operation::Pop, "pop", vec![_stacked(into)], vec![]));
    pop.group = None;
    Arc::new(pop)
}

pub fn _into(one: &Insn) -> Result<String, Malformed> {
    _place(one, one.what.as_ref().map_or(&[][..], |what| what.dests.as_slice()))
}

pub fn _outof(one: &Insn) -> Result<String, Malformed> {
    _place(one, one.what.as_ref().map_or(&[][..], |what| what.sources.as_slice()))
}

/// The location an operand names, as one comparable thing.
fn _place(one: &Insn, where_: &[Loc]) -> Result<String, Malformed> {
    let what = one.what.as_ref();
    if what.is_none_or(|what| what.op != Operation::Move || what.dests.len() != 1 || what.sources.len() != 1) {
        return Err(Malformed(format!("{:#06x} is in a copy group and is not a move", one.at)));
    }
    if where_.len() != 1 {
        return Err(Malformed(format!("{:#06x} names {} places on one side", one.at, where_.len())));
    }
    _named(&where_[0])
}

pub fn _named(one: &Loc) -> Result<String, Malformed> {
    match one {
        Loc::Reg(reg) => Ok(format!("r{}", ir::root(reg.register) as u32)),
        Loc::Mem(memory) => Ok(format!(
            "m{}:{}:{}",
            memory.addr.repr(),
            memory.through as u32,
            memory.offset
        )),
        Loc::Imm(imm) => Ok(format!("i{}", imm.value)),
        _ => Err(Malformed(format!("a copy group names {}, which is not a place", one.repr()))),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{_into, _named, _outof, Malformed, scheduled};
    use crate::backend::verify;
    use crate::model::ir::{Addr, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
    }

    fn _move(into: Loc, out_of: Loc, group: Option<i64>, at: i64) -> Insn {
        let mut one = Insn::new(at, Some((at, at)), semantics(Operation::Move, "mov", vec![into], vec![out_of]), vec![], vec![]);
        one.group = group;
        one
    }

    fn grouped(into: Loc, out_of: Loc, group: i64) -> Insn {
        _move(into, out_of, Some(group), 0x100)
    }

    fn _reg(one: Register) -> Loc {
        Loc::Reg(Reg { register: one, width: 2 })
    }

    fn reg(one: Register, width: u32) -> Loc {
        Loc::Reg(Reg { register: one, width })
    }

    /// Python passes the string `f"[bp-{offset:#x}]"` as the address; a
    /// frame `Addr` names the same distinct place.
    fn _slot(offset: i64) -> Loc {
        Loc::Mem(Mem {
            through: Register::BP,
            offset: 0,
            disp_width: 2,
            ..Mem::new(Some(Addr::new(Space::Frame, -offset)), 2)
        })
    }

    fn _other(at: i64) -> Insn {
        Insn::new(at, Some((at, at + 1)), semantics(Operation::Push, "push", vec![], vec![_reg(Register::AX)]), vec![], vec![])
    }

    fn _body(insns: Vec<Insn>) -> LirBody {
        let block = LirBlock::new(0, insns.into_iter().map(Arc::new).collect());
        LirBody::new("one", 0, vec![block], IndexMap::default(), IndexMap::default())
    }

    fn name(one: &Insn) -> String {
        one.what.as_ref().unwrap().name.clone().unwrap()
    }

    fn names(insns: &[Arc<Insn>]) -> Vec<String> {
        insns.iter().map(|one| name(one)).collect()
    }

    fn _order(body: &LirBody) -> Vec<String> {
        scheduled(body)
            .unwrap()
            .insns()
            .iter()
            .filter(|one| {
                let what = one.what.as_ref().unwrap();
                what.op != Operation::Nothing || what.name.as_deref().is_some_and(|name| !name.is_empty())
            })
            .map(|one| {
                if one.group.is_none() && one.what.as_ref().unwrap().op == Operation::Move {
                    format!("{}<-{}", _into(one).unwrap(), _outof(one).unwrap())
                } else {
                    name(one)
                }
            })
            .collect()
    }

    fn named(one: &Loc) -> String {
        _named(one).unwrap()
    }

    #[test]
    fn test_memory_copy_expands_after_dependency_ordering() {
        // NESTED needs a spilled phi copied before another move overwrites its source.
        let (source, destination) = (_slot(4), _slot(8));
        let result = scheduled(&_body(vec![
            grouped(source.clone(), _reg(Register::AX), 1),
            grouped(destination.clone(), source.clone(), 1),
        ]))
        .unwrap();
        let instructions = &result.blocks[0].insns;
        assert_eq!(names(instructions), ["push", "pop", "mov"]);
        assert_eq!(instructions[0].what.as_ref().unwrap().sources, vec![source]);
        assert_eq!(instructions[1].what.as_ref().unwrap().dests, vec![destination]);
        assert!(instructions.iter().all(|one| one.group.is_none()));
    }

    /// sc.c's `sc_alloc` spilled a byte phi: its slot copy was refused as
    /// "memory parallel copy needs equal-width frame slots".
    #[test]
    fn test_a_byte_slot_copy_moves_its_word() {
        let byte = |offset: i64| match _slot(offset) {
            Loc::Mem(cell) => Loc::Mem(Mem { width: 1, ..cell }),
            _ => unreachable!(),
        };
        let result = scheduled(&_body(vec![grouped(byte(0x1e), byte(0x1a), 1)])).unwrap();
        let instructions = &result.blocks[0].insns;
        assert_eq!(names(instructions), ["push", "pop"]);
        assert_eq!(instructions[0].what.as_ref().unwrap().sources, vec![_slot(0x1a)]);
        assert_eq!(instructions[1].what.as_ref().unwrap().dests, vec![_slot(0x1e)]);
    }

    #[test]
    fn test_a_move_goes_after_everything_that_reads_what_it_writes() {
        // pressx's own shape: the reload into r24 must come last.
        let got = _order(&_body(vec![
            grouped(_reg(Register::DI), _slot(8), 1),
            grouped(_reg(Register::BP), _reg(Register::DI), 1),
        ]));
        let (di, bp) = (named(&_reg(Register::DI)), named(&_reg(Register::BP)));
        let slot = named(&_slot(8));
        assert_eq!(got, [format!("{bp}<-{di}"), format!("{di}<-{slot}")]);
    }

    #[test]
    fn test_a_move_of_a_place_into_itself_is_dropped() {
        let got = _order(&_body(vec![
            grouped(_reg(Register::AX), _reg(Register::AX), 1),
            grouped(_reg(Register::BX), _reg(Register::CX), 1),
        ]));
        assert_eq!(got, [format!("{}<-{}", named(&_reg(Register::BX)), named(&_reg(Register::CX)))]);
    }

    #[test]
    fn test_what_is_not_in_a_group_keeps_its_place() {
        let got = _order(&_body(vec![_other(0x100), grouped(_reg(Register::AX), _reg(Register::CX), 1), _other(0x300)]));
        let moved = format!("{}<-{}", named(&_reg(Register::AX)), named(&_reg(Register::CX)));
        assert_eq!(got, ["push".to_owned(), moved, "push".to_owned()]);
    }

    #[test]
    fn test_two_groups_are_scheduled_apart() {
        // One group's move may write what another's reads; they are not simultaneous.
        let got = _order(&_body(vec![
            grouped(_reg(Register::DI), _slot(8), 1),
            grouped(_reg(Register::BP), _reg(Register::DI), 2),
        ]));
        let (di, bp) = (named(&_reg(Register::DI)), named(&_reg(Register::BP)));
        assert_eq!(got, [format!("{di}<-{}", named(&_slot(8))), format!("{bp}<-{di}")]);
    }

    #[test]
    fn test_register_and_spilled_cycles_are_preserved() {
        // A register cycle uses xchg; a slot cycle uses the balanced machine stack.
        let swapped = scheduled(&_body(vec![
            grouped(_reg(Register::AX), _reg(Register::CX), 1),
            grouped(_reg(Register::CX), _reg(Register::AX), 1),
        ]))
        .unwrap()
        .blocks[0]
            .insns
            .clone();
        let machine: Vec<String> = swapped
            .iter()
            .filter(|one| one.what.as_ref().unwrap().op != Operation::Nothing)
            .map(|one| name(one))
            .collect();
        assert_eq!(machine, ["xchg"]);
        assert_eq!(swapped[swapped.len() - 1].what.as_ref().unwrap().op, Operation::Nothing);
        let spilled = scheduled(&_body(vec![grouped(_slot(4), _slot(8), 1), grouped(_slot(8), _slot(4), 1)]))
            .unwrap()
            .blocks[0]
            .insns
            .clone();
        assert_eq!(names(&spilled), ["push", "push", "pop", "pop"]);
        assert_eq!(spilled[0].what.as_ref().unwrap().sources, vec![_slot(4)]);
        assert_eq!(spilled[spilled.len() - 1].what.as_ref().unwrap().dests, vec![_slot(8)]);
    }

    #[test]
    fn test_register_cycle_retains_every_virtual_definition() {
        // R_WALK stopped at parcopy: values 11 and 12 were read but undefined.
        let mut first = grouped(_reg(Register::AX), _reg(Register::CX), 1);
        first.defines = vec![11];
        first.uses = vec![1];
        let mut closing = grouped(_reg(Register::CX), _reg(Register::AX), 1);
        closing.defines = vec![12];
        closing.uses = vec![2];
        let consumer = Insn::new(0x102, Some((0x102, 0x102)), semantics(Operation::Push, "push", vec![], vec![_reg(Register::CX)]), vec![], vec![12]);
        let mut body = _body(vec![first, closing, consumer]);
        body.inputs = BTreeSet::from([1, 2]);

        let got = scheduled(&body).unwrap();

        assert_eq!(verify::verify(&got, false), Vec::<String>::new());
        let defined: BTreeSet<u32> = got.insns().iter().flat_map(|one| one.defines.clone()).collect();
        assert_eq!(defined, BTreeSet::from([11, 12]));
    }

    #[test]
    fn test_register_cycle_retains_covered_bytes_as_an_anchor() {
        // sieve's shared array base made a three-register phi cycle whose final move owned original bytes.
        let mut closing = grouped(_reg(Register::CX), _reg(Register::DX), 1);
        closing.covers = Some((0x100, 0x102));
        let body = _body(vec![
            grouped(_reg(Register::DX), _reg(Register::SI), 1),
            grouped(_reg(Register::SI), _reg(Register::CX), 1),
            closing,
        ]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["xchg", "xchg", ""]);
        assert_eq!(got[got.len() - 1].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(got[got.len() - 1].covers, Some((0x100, 0x102)));
    }

    /// `conc7` at `--cpu Core` was refused ("need a temporary", #106): a slot
    /// written from edx and read back into dx as a word is a two-place cycle
    /// no exchange takes, since the places are read at two widths.
    #[test]
    fn test_a_slot_written_as_a_dword_and_read_as_a_word_cycles_through_the_stack() {
        let dword_slot = Loc::Mem(Mem { width: 4, ..match _slot(32) { Loc::Mem(cell) => cell, _ => unreachable!() } });
        let body = _body(vec![
            grouped(dword_slot.clone(), reg(Register::EDX, 4), 1),
            grouped(reg(Register::DX, 2), _slot(32), 1),
        ]);
        let got = scheduled(&body).unwrap().blocks[0].insns.clone();
        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![_slot(32)], "the word the closing move reads is saved");
        assert_eq!(got[2].what.as_ref().unwrap().dests, vec![reg(Register::DX, 2)]);
    }

    /// No `xchg` names a segment register.
    #[test]
    fn test_a_cycle_through_a_segment_register_goes_through_the_stack() {
        let body = _body(vec![grouped(reg(Register::ES, 2), _slot(4), 1), grouped(_slot(4), reg(Register::ES, 2), 1)]);
        let got = scheduled(&body).unwrap().blocks[0].insns.clone();
        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(Register::ES, 2)]);
    }

    #[test]
    fn test_mixed_width_register_cycle_uses_a_balanced_temporary() {
        // sieve rotates EDX->CX->SI->EDX without exchanging incompatible register widths.
        let body = _body(vec![
            grouped(reg(Register::EDX, 4), reg(Register::ESI, 4), 1),
            grouped(reg(Register::CX, 2), reg(Register::DX, 2), 1),
            grouped(reg(Register::SI, 2), reg(Register::CX, 2), 1),
        ]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["push", "mov", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(Register::DX, 2)]);
        assert_eq!(got[got.len() - 1].what.as_ref().unwrap().dests, vec![reg(Register::CX, 2)]);
    }

    #[test]
    fn test_a_mixed_width_cycle_closing_on_a_byte_saves_a_word() {
        // rcflip's AX <- CX, CL <- AL was refused: its temporary was `push al`.
        let body = _body(vec![grouped(reg(Register::AX, 2), reg(Register::CX, 2), 1), grouped(reg(Register::CL, 1), reg(Register::AL, 1), 1)]);

        let got = scheduled(&body).unwrap().blocks[0].insns.clone();

        assert_eq!(names(&got), ["push", "mov", "pop"]);
        assert_eq!(got[0].what.as_ref().unwrap().sources, vec![reg(Register::CX, 2)]);
        assert_eq!(got[1].what.as_ref().unwrap().dests, vec![reg(Register::CL, 1)]);
        assert_eq!(got[2].what.as_ref().unwrap().dests, vec![reg(Register::AX, 2)]);
    }

    #[test]
    fn named_places_match_python() {
        // Expected strings printed by Python's parcopy._named.
        use crate::model::ir::Imm;
        assert_eq!(named(&_reg(Register::DI)), "r44");
        assert_eq!(named(&_reg(Register::BP)), "r42");
        assert_eq!(named(&_slot(8)), "m[bp-0x8]:26:0");
        assert_eq!(named(&Loc::Imm(Imm { value: -3, width: 2, address: None })), "i-3");
    }

    #[test]
    fn test_something_that_is_not_a_move_in_a_group_is_refused() {
        let mut other = _other(0x100);
        other.group = Some(1);
        assert!(matches!(scheduled(&_body(vec![other])), Err(Malformed(_))));
    }
}
