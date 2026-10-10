//! Which 32-bit register roots hold zero in their upper half before each
//! allocated instruction.
//!
//! A word or byte write leaves the upper half as it was; `movzx`, `xor r,r`
//! and a `mov` of a word-sized constant set it to zero; any other write of
//! the upper lanes, and anything whose effects are unknown, loses it. The
//! entry knows nothing. A 32-bit effective address names the same byte as
//! the 16-bit one only through registers this proves.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::dataflow::{self, Direction};
use crate::analysis::loops;
use crate::backend::peephole::{_register_effects, id};
use crate::backend::{liveness, target};
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, IndexMap};

/// The roots a general register names, one bit each.
pub const ROOTS: [RegId; 7] = llrm_x86::registers::ROOTS;

/// A set of roots, one bit per `ROOTS` entry.
pub type Roots = u8;

const ALL: Roots = (1 << ROOTS.len()) - 1;

/// The bit of `register`'s root, if it has one here.
pub fn bit(register: RegId) -> Option<Roots> {
    let root = crate::model::ir::root(register);
    ROOTS.iter().position(|one| *one == root).map(|index| 1 << index)
}

/// The roots whose upper half `one` sets to zero.
fn zeroing(one: &Insn) -> Roots {
    let Some(what) = &one.what else {
        return 0;
    };
    let wide = |loc: &Loc| match loc {
        Loc::Reg(Reg { register, width: 4 }) => bit(*register),
        _ => None,
    };
    let [destination] = what.dests.as_slice() else {
        return 0;
    };
    let Some(root) = wide(destination) else {
        return 0;
    };
    let zeroes = match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Extend, Some("movzx"), [Loc::Reg(Reg { width: 1 | 2, .. }) | Loc::Mem(_)]) => true,
        (_, Some("xor" | "sub"), [left, right]) => left == destination && right == destination,
        (Operation::Move, Some("mov"), [Loc::Imm(constant)]) => {
            constant.address.is_none() && (0..=0xFFFF).contains(&constant.value)
        }
        _ => false,
    };
    if zeroes { root } else { 0 }
}

/// The roots whose upper half `one` may leave other than it found it.
fn disturbed(
    bits: u32,
    one: &Insn,
) -> Roots {
    // A jump or branch writes no register; no decoder answers for it.
    if liveness::_terminator(one.what.as_ref()) {
        return 0;
    }
    let effects = _register_effects(bits, one, true, false).or_else(|| liveness::_declared(one));
    let Some((_, writes)) = effects else {
        return ALL;
    };
    ROOTS
        .iter()
        .enumerate()
        .filter(|(_, root)| writes.contains(&(**root, 2)) || writes.contains(&(**root, 3)))
        .fold(0, |roots, (index, _)| roots | 1 << index)
}

/// What `one` makes of `zero`, the roots known zero before it.
pub fn after(
    bits: u32,
    one: &Insn,
    zero: Roots,
) -> Roots {
    if let Some(swapped) = exchanged(one, zero) {
        return swapped;
    }
    let zeroed = zeroing(one) | copied(one, zero);
    (zero & !disturbed(bits, one)) | zeroed
}

/// `zero` after `xchg a, b` of two whole registers: each now holds the other's
/// upper half.
fn exchanged(
    one: &Insn,
    zero: Roots,
) -> Option<Roots> {
    let Some(Semantics { op: Operation::Exchange, dests, .. }) = &one.what else {
        return None;
    };
    let [Loc::Reg(Reg { register: left, width: 4 }), Loc::Reg(Reg { register: right, width: 4 })] = dests.as_slice()
    else {
        return None;
    };
    let (left, right) = (bit(*left)?, bit(*right)?);
    let (had_left, had_right) = (zero & left != 0, zero & right != 0);
    let rest = zero & !(left | right);
    Some(rest | if had_right { left } else { 0 } | if had_left { right } else { 0 })
}

/// The root a whole-register copy writes, where the root it copies from has a
/// zero upper half.
fn copied(
    one: &Insn,
    zero: Roots,
) -> Roots {
    let Some(Semantics { op: Operation::Move, dests, sources, .. }) = &one.what else {
        return 0;
    };
    match (dests.as_slice(), sources.as_slice()) {
        ([Loc::Reg(Reg { register: into, width: 4 })], [Loc::Reg(Reg { register: from, width: 4 })]) => {
            match (bit(*into), bit(*from)) {
                (Some(into), Some(from)) if zero & from != 0 => into,
                _ => 0,
            }
        }
        _ => 0,
    }
}

/// Before each instruction, by `id`, the roots whose upper half is zero.
pub fn before(body: &LirBody) -> HashMap<usize, Roots> {
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    let nodes: Vec<&LirBlock> = body.blocks.iter().collect();
    let blocks: IndexMap<i64, &LirBlock> = nodes.iter().map(|block| (block.at, *block)).collect();
    let solved = dataflow::solve(
        &nodes,
        Direction::Forward,
        |_| ALL,
        |at, out| {
            let mut zero = if at == body.entry { 0 } else { ALL };
            for from in predecessors.get(&at).into_iter().flatten().filter(|from| out.contains_key(*from)) {
                zero &= out[from];
            }
            zero
        },
        |at, zero| blocks[&at].insns.iter().fold(*zero, |zero, one| after(body.bits, one, zero)),
    );
    let into = solved.input;
    let mut result = HashMap::default();
    for block in &body.blocks {
        let mut zero = into[&block.at];
        for one in &block.insns {
            result.insert(id(one), zero);
            zero = after(body.bits, one, zero);
        }
    }
    result
}

/// `movzx root,word` for each root in `roots`, before `block`'s terminator.
pub fn extended(
    block: &LirBlock,
    roots: Roots,
) -> LirBlock {
    let mut insns = block.insns.to_vec();
    let at = insns.last().map_or(block.at, |one| one.at);
    let position = if insns.last().is_some_and(|one| liveness::_terminator(one.what.as_ref())) {
        insns.len() - 1
    } else {
        insns.len()
    };
    for (index, root) in ROOTS.iter().enumerate() {
        if roots & 1 << index == 0 {
            continue;
        }
        let word = target::named(*root, 2);
        let what = Semantics {
            name: Some("movzx".into()),
            dests: vec![Loc::Reg(Reg { register: *root, width: 4 })],
            sources: vec![Loc::Reg(Reg { register: word, width: 2 })],
            ..Semantics::new(Operation::Extend)
        };
        insns.insert(position, Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())));
    }
    block.with_insns(insns)
}

/// `body` with the upper half of each loop's `roots` zeroed on entry, where
/// every way in may take it: one successor, and those lanes dead there
/// unless `held` says no value lives in them.
pub fn preheaded(
    body: &LirBody,
    wanted: &IndexMap<i64, Roots>,
    held: bool,
) -> (LirBody, IndexMap<i64, Roots>) {
    let graph = &body.blocks;
    let found = loops::loops(&graph, Some(body.entry));
    let predecessors = loops::predecessors(&graph);
    let exits = liveness::dead_at_exit(body);
    let mut placed: IndexMap<i64, Roots> = IndexMap::default();
    let mut per_block: IndexMap<i64, Roots> = IndexMap::default();
    for (header, roots) in wanted {
        let Some(inside) = found.iter().find(|one| one.header == *header).map(|one| &one.body) else {
            continue;
        };
        let entries: Vec<i64> =
            predecessors.get(header).into_iter().flatten().copied().filter(|at| !inside.contains(at)).collect();
        let mut allowed = *roots;
        for (index, root) in ROOTS.iter().enumerate() {
            let upper = [(*root, 2), (*root, 3)];
            let takes = !entries.is_empty()
                && entries.iter().all(|at| {
                    body.blocks.iter().find(|block| block.at == *at).is_some_and(|block| block.succ == [*header])
                        && (!held || upper.iter().all(|lane| exits[at].contains(lane)))
                });
            if !takes {
                allowed &= !(1 << index);
            }
        }
        if allowed == 0 {
            continue;
        }
        placed.insert(*header, allowed);
        for at in entries {
            *per_block.entry(at).or_default() |= allowed;
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match per_block.get(&block.at) {
            Some(roots) => extended(block, *roots),
            None => block.clone(),
        })
        .collect();
    (body.with_blocks(blocks), placed)
}

/// The roots `one`'s cells read 32 bits wide through a register holding no
/// value, the frame pointer: nothing defines their upper half.
pub(crate) fn unheld(one: &Insn) -> Roots {
    let Some(what) = &one.what else {
        return 0;
    };
    let wide = |register: RegId, held: bool| {
        if !held && target::width_of(register) == Some(4) { bit(register).unwrap_or(0) } else { 0 }
    };
    what.dests
        .iter()
        .chain(&what.sources)
        .filter_map(|operand| match operand {
            Loc::Mem(cell) => {
                Some(wide(cell.through, cell.base.is_some()) | wide(cell.index_through, cell.index.is_some()))
            }
            _ => None,
        })
        .fold(0, |roots, one| roots | one)
}

/// `body` with the upper half of every unheld root a cell reads zero: one
/// `movzx` in the preheader of the outermost loop around the cell that
/// leaves that half alone, or before the first cell of a block no
/// preheader proves.
pub fn established(body: &LirBody) -> LirBody {
    let graph = &body.blocks;
    let natural = loops::loops(&graph, Some(body.entry));
    let untouched = |inside: &BTreeSet<i64>, roots: Roots| {
        body.blocks
            .iter()
            .filter(|block| inside.contains(&block.at))
            .all(|block| block.insns.iter().all(|one| disturbed(body.bits, one) & roots == 0))
    };
    let outermost = |at: i64, roots: Roots| {
        let around = natural.iter().filter(|one| one.body.contains(&at));
        around
            .clone()
            .filter(|one| untouched(&one.body, roots))
            .max_by_key(|one| one.body.len())
            .or_else(|| around.min_by_key(|one| one.body.len()))
            .map(|one| one.header)
    };
    let mut body = body.clone();
    let mut preheaders = true;
    loop {
        let zero = before(&body);
        let missing: Vec<(i64, usize, Roots)> = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter().map(move |one| (block.at, one)))
            .map(|(at, one)| (at, id(one), unheld(one) & !zero[&id(one)]))
            .filter(|(_, _, roots)| *roots != 0)
            .collect();
        if missing.is_empty() {
            return body;
        }
        if std::mem::take(&mut preheaders) {
            let mut wanted: IndexMap<i64, Roots> = IndexMap::default();
            for (at, _, roots) in &missing {
                if let Some(header) = outermost(*at, *roots) {
                    *wanted.entry(header).or_default() |= roots;
                }
            }
            if !wanted.is_empty() {
                body = preheaded(&body, &wanted, false).0;
                continue;
            }
        }
        let mut first: IndexMap<i64, (usize, Roots)> = IndexMap::default();
        for (at, position, roots) in missing {
            first.entry(at).or_insert((position, roots));
        }
        let blocks = body
            .blocks
            .iter()
            .map(|block| match first.get(&block.at) {
                None => block.clone(),
                Some((position, roots)) => {
                    let at = block.insns.iter().position(|one| id(one) == *position).expect("the cell");
                    let mut insns = block.insns.to_vec();
                    let zeroed = extended(&LirBlock::new(block.at, Vec::new()), *roots).insns.to_vec();
                    let when = insns[at].at;
                    insns.splice(at..at, zeroed.into_iter().map(|one| Arc::new(Insn { at: when, ..(*one).clone() })));
                    block.with_insns(insns)
                }
            })
            .collect();
        body = body.with_blocks(blocks);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{before, bit};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn one(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    #[test]
    fn test_a_zeroed_upper_half_survives_word_writes_and_jumps() {
        // A `jmp` decoded as unknown lost every root, so no loop kept the
        // preheader's `movzx`.
        let (ebx, bx) =
            (Loc::Reg(Reg { register: RegId::EBX, width: 4 }), Loc::Reg(Reg { register: RegId::BX, width: 2 }));
        let word = || Loc::Imm(Imm { value: 5, width: 2, address: None });
        let entry = LirBlock {
            succ: vec![1],
            ..LirBlock::new(
                0,
                vec![
                    one(0, Operation::Extend, "movzx", vec![ebx], vec![bx.clone()], None),
                    one(1, Operation::Jump, "jmp", vec![], vec![], Some(1)),
                ],
            )
        };
        let looped = LirBlock {
            succ: vec![1, 2],
            ..LirBlock::new(
                1,
                vec![
                    one(2, Operation::Move, "mov", vec![bx.clone()], vec![word()], None),
                    one(3, Operation::Compare, "cmp", vec![], vec![bx.clone(), word()], None),
                    one(4, Operation::Branch, "jne", vec![], vec![], Some(1)),
                ],
            )
        };
        let exit = LirBlock::new(2, vec![one(5, Operation::Move, "mov", vec![bx], vec![word()], None)]);
        let body = LirBody::new("zero", 0, vec![entry, looped, exit], IndexMap::default(), IndexMap::default());

        let zero = before(&body);

        let ebx = bit(RegId::EBX).unwrap();
        for block in &body.blocks[1..] {
            for insn in &block.insns {
                assert_ne!(zero[&crate::backend::peephole::id(insn)] & ebx, 0, "{:?}", insn.what);
            }
        }
    }

    /// `xchg esi, ecx` lost both registers' zero upper halves, so a counter
    /// that began as `mov ecx, 2` was copied by `movzx` before it indexed
    /// (bench/sieve with EBP free: one more instruction a trip).
    #[test]
    fn test_an_exchange_swaps_the_zero_upper_halves() {
        let (ecx, esi) =
            (Loc::Reg(Reg { register: RegId::ECX, width: 4 }), Loc::Reg(Reg { register: RegId::ESI, width: 4 }));
        let two = Loc::Imm(Imm { value: 2, width: 4, address: None });
        let entry = LirBlock::new(
            0,
            vec![
                one(0, Operation::Move, "mov", vec![ecx.clone()], vec![two], None),
                one(
                    1,
                    Operation::Exchange,
                    "xchg",
                    vec![esi.clone(), ecx.clone()],
                    vec![ecx.clone(), esi.clone()],
                    None,
                ),
                one(2, Operation::Nothing, "nop", vec![], vec![], None),
            ],
        );
        let body = LirBody::new("swap", 0, vec![entry], IndexMap::default(), IndexMap::default());

        let zero = before(&body);

        let id = |at: usize| crate::backend::peephole::id(&body.blocks[0].insns[at]);
        assert_ne!(zero[&id(2)] & bit(RegId::ESI).unwrap(), 0);
        assert_eq!(zero[&id(2)] & bit(RegId::ECX).unwrap(), 0);
    }
}
