//! Move a register copy out of a loop to its exit when only code after the
//! loop reads it.
//!
//! # How it works
//! A phi whose value is computed in a loop and read after it becomes a
//! copy inside the loop, run every trip to hand over a value nothing in
//! the loop looks at. The copy belongs at the top of the exit block.
//!
//! ```text
//!   H: ...                           H: ...
//!      mov dx, ax                       mov dx, ax
//!      mov di, dx    (read only    =>   cmp ax, bx
//!      cmp ax, bx     after the         jl  H
//!      jl  H          loop)          X: mov di, dx     <- once
//!   X: ... di ...                       ... di ...
//! ```
//!
//! A loop is considered when it has one way out, from one block to one exit
//! block that has no other predecessor. Within it a candidate is a plain
//! `mov reg,reg` (same width, distinct, no constraints, relocation or spill
//! marker) in a block that dominates the exiting block, so every way out
//! has run it. Without that a loop left from its header before the first
//! trip never ran the copy, and the destination must keep what it held.
//! Lane sets (byte lanes of the registers, from decoded effects) decide the
//! rest:
//!
//! - The destination is dead on every way round the loop, nested loops
//!   included: a backward liveness over the loop's blocks, with the exit edge
//!   left out because the exit's read is what the sink is for.
//! - Between the copy and the exit, in its block and every loop block on a path
//!   that avoids the copy's block, no instruction writes the destination or the
//!   source, or reads the destination.
//!
//! A copy that passes is removed and put first in the exit block, in the
//! order found.
//!
//! # Example
//! No honest example: the pass runs inside the peephole phase, whose dump mixes
//! it with every other rewrite, and its unit tests build the loop in Rust. They
//! assert where each copy
//! ends up: `test_copy_read_only_after_its_loop_moves_to_the_exit`.
//!
//! # Runs
//! Inside the peephole phase, after copy propagation and extension
//! rewrites, before `postrasink`. Once per peephole run. No flag of its own.
//!
//! # Needs
//! [`crate::analysis::loops`] for loops and dominance, and
//! [`crate::backend::liveness`] for the backward walk.
//!
//! # See also
//! [`crate::backend`]. [`copy_of`] and [`touches`] are shared with
//! `postrasink`.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::analysis::dataflow::{self, Direction};
use crate::analysis::loops as loopy;
use crate::backend::liveness::{_backwards, _declared, _terminator, _universe, with_effect};
use crate::backend::peephole::{_lanes, _register_effects, Lanes, id};
use crate::model::ir::{Loc, Operation, Reg};
use crate::model::lir::{self, Block, Insn, LirBody};
use crate::support::hash::IndexMap;
use crate::support::hash::{HashMap, HashSet};

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
pub fn copy_of(one: &Insn) -> Option<(Reg, Reg)> {
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
pub fn touches(
    regs: Regs,
    bits: u32,
    one: &Insn,
    lanes: &Lanes,
    reading: bool,
) -> bool {
    if _terminator(one.what.as_ref()) {
        return false;
    }
    let mut effects = _register_effects(regs, bits, one, true, true);
    if effects.is_none() {
        effects = _declared(regs, one);
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
fn _between(
    at_of: &IndexMap<i64, &Block>,
    predecessors: &IndexMap<i64, Vec<i64>>,
    inside: &BTreeSet<i64>,
    copy_at: i64,
    exit_from: i64,
) -> Option<BTreeSet<i64>> {
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
        // The blocks that lead to `at`, by its predecessors: not every block of
        // the loop asked whether it does (the square of the loop).
        queue.extend(predecessors[&at].iter().copied().filter(|one| *one != copy_at && forward.contains(one)));
    }
    Some(forward.intersection(&backward).copied().collect())
}

/// What a run of instructions reads and writes, as `touches` asks it of each:
/// the union of the lanes, and whether any instruction's are not known.
#[derive(Clone, Copy, Default)]
struct Touched {
    reads: Lanes,
    writes: Lanes,
    unknown: bool,
}

impl Touched {
    fn of(
        regs: Regs,
        bits: u32,
        one: &Insn,
    ) -> Self {
        if _terminator(one.what.as_ref()) {
            return Self::default();
        }
        let mut effects = _register_effects(regs, bits, one, true, true);
        if effects.is_none() {
            effects = _declared(regs, one);
        }
        match effects {
            Some((reads, writes)) => Self { reads, writes, unknown: false },
            None => Self { unknown: true, ..Self::default() },
        }
    }

    fn of_block(
        regs: Regs,
        bits: u32,
        block: &Block,
    ) -> Self {
        block.insns.iter().fold(Self::default(), |so_far, one| so_far.with(&Self::of(regs, bits, one)))
    }

    fn with(
        mut self,
        other: &Self,
    ) -> Self {
        self.reads = self.reads.or(&other.reads);
        self.writes = self.writes.or(&other.writes);
        self.unknown |= other.unknown;
        self
    }

    /// Whether some instruction `touches` either of the two ways the sink asks.
    fn blocks(
        &self,
        both: &Lanes,
        written: &Lanes,
    ) -> bool {
        self.unknown || !both.is_disjoint(&self.writes) || !written.is_disjoint(&self.reads)
    }
}

/// What each suffix of `block` touches, by where it starts (the last: nothing).
fn _suffixes(
    regs: Regs,
    bits: u32,
    block: &Block,
) -> Vec<Touched> {
    let mut touched = vec![Touched::default(); block.insns.len() + 1];
    for (at, one) in block.insns.iter().enumerate().rev() {
        touched[at] = Touched::of(regs, bits, one).with(&touched[at + 1]);
    }
    touched
}

/// The lanes live after each instruction of `block`, given those live after
/// the block.
fn _live_after(
    regs: Regs,
    bits: u32,
    block: &Block,
    after: Lanes,
    universe: &Lanes,
) -> Vec<Lanes> {
    let mut live = vec![after; block.insns.len()];
    let mut current = after;
    for (at, one) in block.insns.iter().enumerate().rev() {
        live[at] = current;
        current = with_effect(regs, bits, one, |effect| {
            effect.map_or_else(|| *universe, |effect| effect.live_before(&current))
        });
    }
    live
}

/// Per loop block, the lanes live on entry along paths that stay in the loop.
///
/// The exit is left out: a lane only the exit reads is what the sunk copy is
/// for, and a lane a nested loop reads again is not.
fn _round(
    regs: Regs,
    bits: u32,
    at_of: &IndexMap<i64, &Block>,
    inside: &BTreeSet<i64>,
    universe: &Lanes,
) -> IndexMap<i64, Lanes> {
    let nodes: Vec<&Block> = inside.iter().map(|at| at_of[at]).collect();
    dataflow::solve(
        &nodes,
        Direction::Backward,
        |_| Lanes::new(),
        |at, into| {
            at_of[&at].succ.iter().filter(|to| inside.contains(to)).flat_map(|to| into[to].iter().copied()).collect()
        },
        |at, after| _backwards(regs, bits, at_of[&at], after.clone(), universe),
    )
    .output
}

/// `body` with each such copy moved from inside its loop to the exit.
pub fn sunk(body: &LirBody) -> LirBody {
    let regs = body.regs();
    // `loops.loops` reads only `at` and `succ`.
    let graph = &body.blocks;
    let found = loopy::loops(&graph, Some(body.entry));
    if found.is_empty() {
        return body.clone();
    }
    let dominance = loopy::dominance(&graph, Some(body.entry));
    let universe = _universe(regs);
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut predecessors: IndexMap<i64, Vec<i64>> = at_of.keys().map(|at| (*at, Vec::new())).collect();
    for block in &body.blocks {
        for at in &block.succ {
            if let Some(found) = predecessors.get_mut(at) {
                found.push(block.at);
            }
        }
    }

    // What a whole block touches, made once for the body.
    let mut whole: HashMap<i64, Touched> = HashMap::default();
    let mut suffixes: HashMap<i64, Vec<Touched>> = HashMap::default();
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
        // Solved when a copy first needs it: most loops have none to move.
        let round_cell = std::cell::OnceCell::new();
        let round_into = || round_cell.get_or_init(|| _round(regs, body.bits, &at_of, &inside, &universe));
        // What a block's copies read is made once for the block, not for each
        // copy: the rest of the block cloned and walked for each was the square
        // of the block.
        let mut lives: HashMap<i64, Vec<Lanes>> = HashMap::default();
        let mut between: HashMap<i64, Option<Touched>> = HashMap::default();
        // Every way out runs the copy first: a loop left from its header
        // before any trip never ran its latch.
        for block in inside.iter().map(|at| at_of[at]).filter(|block| dominance.dominates(block.at, source_at)) {
            for (index, one) in block.insns.iter().enumerate() {
                let pair = copy_of(one);
                let Some((dest, register)) = pair else {
                    continue;
                };
                if removed.contains(&id(one)) {
                    continue;
                }
                let (written, read) = (_lanes(regs, dest.register), _lanes(regs, register.register));
                if written.is_empty() || read.is_empty() || !written.is_disjoint(&read) {
                    continue;
                }
                // Between the copy and the exit nothing writes either side or
                // reads the copy: local to the blocks, asked before the
                // liveness of the whole loop, which only a copy
                // that passes needs.
                let touched = suffixes.entry(block.at).or_insert_with(|| _suffixes(regs, body.bits, block));
                let both: Lanes = written.or(&read);
                // The rest of its own block alone settles most copies.
                if touched[index + 1].blocks(&both, &written) {
                    continue;
                }
                // What the blocks on the way to the exit touch, together.
                let Some(rest) = *between.entry(block.at).or_insert_with(|| {
                    let blocks = _between(&at_of, &predecessors, &inside, block.at, source_at)?;
                    Some(blocks.iter().fold(Touched::default(), |so_far, at| {
                        let one = whole.entry(*at).or_insert_with(|| Touched::of_block(regs, body.bits, at_of[at]));
                        so_far.with(one)
                    }))
                }) else {
                    continue;
                };
                if touched[index + 1].with(&rest).blocks(&both, &written) {
                    continue;
                }
                // Dead on every way round, nested loops included. Asked only at
                // the header, PRECALCULATIONS' inner loop read the copy again
                // and the copy left both loops.
                let live = lives
                    .entry(block.at)
                    .or_insert_with(
                        || {
                            let after: Lanes = block
                                .succ
                                .iter()
                                .filter(|to| inside.contains(to))
                                .flat_map(|to| round_into()[to].iter().copied())
                                .collect();
                            _live_after(regs, body.bits, block, after, &universe)
                        },
                    );
                if !written.is_disjoint(&live[index]) {
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
        let mut insns = block.insns.to_vec();
        if insns.iter().any(|one| removed.contains(&id(one))) {
            insns = lir::without(&insns, |one| removed.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>);
        }
        if let Some(moved) = moved.get(&block.at) {
            insns = moved.iter().cloned().chain(insns).collect();
        }
        blocks.push(block.with_insns(insns));
    }
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::sunk;
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::HashMap;
    use crate::support::hash::IndexMap;

    fn r(register: RegId) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn _insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Arc<Insn> {
        // Inserted, as the C path's are: an instruction standing for BC's bytes
        // stays where `lir.without` finds no heir.
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) }),
            vec![],
            vec![],
        ))
    }

    fn _move(
        at: i64,
        dest: RegId,
        source: RegId,
    ) -> Arc<Insn> {
        _insn(at, Operation::Move, "mov", vec![r(dest)], vec![r(source)], None)
    }

    fn _compare(
        at: i64,
        left: RegId,
        right: RegId,
    ) -> Arc<Insn> {
        _insn(at, Operation::Compare, "cmp", vec![], vec![r(left), r(right)], None)
    }

    fn _branch(
        at: i64,
        name: &str,
        target: i64,
    ) -> Arc<Insn> {
        _insn(at, Operation::Branch, name, vec![], vec![], Some(target))
    }

    fn _jump(
        at: i64,
        target: i64,
    ) -> Arc<Insn> {
        _insn(at, Operation::Jump, "jmp", vec![], vec![], Some(target))
    }

    fn _return(at: i64) -> Arc<Insn> {
        _insn(at, Operation::Return, "ret", vec![], vec![], None)
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
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
                            what.name.as_deref() == Some("mov") && what.sources == [r(RegId::DX)]
                        })
                        .map(|one| one.what.as_ref().unwrap().dests[0].clone())
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn test_copy_read_by_an_inner_loop_stays() {
        // Shellsort's gap loop: `mov cx,dx` in the inner loop's header saved
        // `i` for the inner loop's latch; it must not leave for the
        // outer exit.
        const AX: RegId = RegId::AX;
        const BX: RegId = RegId::BX;
        const CX: RegId = RegId::CX;
        const DX: RegId = RegId::DX;
        const SI: RegId = RegId::SI;
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(1, vec![_jump(1, 31)], vec![31]).into(),
                block(31, vec![_compare(31, AX, BX), _branch(32, "jle", 91)], vec![35, 91]).into(),
                block(35, vec![_move(35, DX, AX)], vec![38]).into(),
                block(38, vec![_move(38, CX, DX), _compare(39, DX, BX), _branch(40, "jge", 86)], vec![42, 86]).into(),
                block(42, vec![_move(42, DX, BX)], vec![77]).into(),
                block(77, vec![_move(77, DX, CX), _jump(78, 38)], vec![38]).into(),
                block(86, vec![_move(86, SI, AX), _jump(87, 31)], vec![31]).into(),
                block(91, vec![_return(91)], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        assert_eq!(_copies(&sunk(&body))[&38], vec![r(RegId::from(CX))]);
    }

    #[test]
    fn test_copy_read_only_after_its_loop_moves_to_the_exit() {
        // Plasmablobs: `mov di,dx` on the way back to the header ran every pass
        // for one read after the loop.
        const AX: RegId = RegId::AX;
        const BX: RegId = RegId::BX;
        const DI: RegId = RegId::DI;
        const DX: RegId = RegId::DX;
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(1, vec![_jump(1, 5)], vec![5]).into(),
                block(
                    5,
                    vec![_move(5, DX, AX), _move(6, DI, DX), _compare(7, AX, BX), _branch(8, "jl", 5)],
                    vec![5, 9],
                )
                .into(),
                block(9, vec![_return(9)], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let copies = _copies(&sunk(&body));
        assert!(copies[&5].is_empty() && copies[&9] == vec![r(RegId::from(DI))]);
    }

    #[test]
    fn test_a_copy_a_zero_trip_loop_never_ran_stays_in_it() {
        // #111: the latch's `mov di,dx` left for the exit of a loop tested at
        // its header; run no times, the loop left DI the value DX happened to
        // hold, not the one DI held before it (5812 where 0 is right on DOS).
        const AX: RegId = RegId::AX;
        const BX: RegId = RegId::BX;
        const DI: RegId = RegId::DI;
        const DX: RegId = RegId::DX;
        let body = LirBody::new(
            "f",
            1,
            vec![
                block(1, vec![_jump(1, 3)], vec![3]).into(),
                block(3, vec![_compare(3, AX, BX), _branch(4, "jge", 9)], vec![5, 9]).into(),
                block(5, vec![_move(5, DX, AX), _move(6, DI, DX), _jump(7, 3)], vec![3]).into(),
                block(9, vec![_return(9)], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let copies = _copies(&sunk(&body));
        assert!(copies[&5] == vec![r(RegId::from(DI))] && copies[&9].is_empty(), "{copies:?}");
    }

    fn _raw_insn(
        at: i64,
        what: Semantics,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![]))
    }

    fn sem(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) }
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 2, address: None })
    }

    /// What the pushes along `path` write, running register moves and adds.
    fn _pushed(
        body: &LirBody,
        path: &[i64],
    ) -> Vec<i64> {
        let mut held: HashMap<RegId, i64> = HashMap::default();
        let mut pushed = Vec::new();
        let blocks: HashMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
        for at in path {
            for one in &blocks[at].insns {
                let what = one.what.as_ref().unwrap();
                let read = |held: &HashMap<RegId, i64>, operand: &Loc| match operand {
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
        // PRECALCULATIONS' map index was copied back once per row instead of
        // once per pixel, and deedlines drew its minimap from stale
        // plasma data.
        let (si, bx, dx) = (r(RegId::SI), r(RegId::BX), r(RegId::DX));
        let body = LirBody::new(
            "one",
            0,
            vec![
                block(
                    0,
                    vec![_raw_insn(0, sem(Operation::Move, "mov", vec![si.clone()], vec![imm(0)], None))],
                    vec![0x10],
                )
                .into(),
                block(
                    0x10,
                    vec![
                        _raw_insn(0x10, sem(Operation::Push, "push", vec![], vec![si.clone()], None)),
                        _raw_insn(0x12, sem(Operation::Move, "mov", vec![bx.clone()], vec![si.clone()], None)),
                        _raw_insn(
                            0x14,
                            sem(Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), imm(1)], None),
                        ),
                        _raw_insn(0x16, sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), imm(3)], None)),
                        _raw_insn(0x18, sem(Operation::Move, "mov", vec![si], vec![bx], None)),
                        _raw_insn(0x1A, sem(Operation::Branch, "jl", vec![], vec![], Some(0x10))),
                    ],
                    vec![0x10, 0x20],
                )
                .into(),
                block(
                    0x20,
                    vec![
                        _raw_insn(0x20, sem(Operation::Compare, "cmp", vec![], vec![dx, imm(0)], None)),
                        _raw_insn(0x22, sem(Operation::Branch, "jne", vec![], vec![], Some(0))),
                    ],
                    vec![0, 0x30],
                )
                .into(),
                block(0x30, vec![_raw_insn(0x30, sem(Operation::Return, "ret", vec![], vec![], None))], vec![]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        assert_eq!(_pushed(&sunk(&body), &[0, 0x10, 0x10, 0x10]), vec![0, 1, 2]);
    }
}
