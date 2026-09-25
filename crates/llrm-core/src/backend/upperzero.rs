//! Which 32-bit register roots hold zero in their upper half before each
//! allocated instruction.
//!
//! A word or byte write leaves the upper half as it was; `movzx`, `xor r,r`
//! and a `mov` of a word-sized constant set it to zero; any other write of
//! the upper lanes, and anything whose effects are unknown, loses it. The
//! entry knows nothing. A 32-bit effective address names the same byte as
//! the 16-bit one only through registers this proves.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::intervals::_graph;
use crate::analysis::loops;
use crate::backend::liveness;
use crate::backend::peephole::{_register_effects, id};
use crate::model::ir::{Loc, Operation, Reg};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{HashMap, IndexMap};

/// The roots a general register names, one bit each.
pub const ROOTS: [Register; 7] =
    [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI, Register::EBP];

/// A set of roots, one bit per `ROOTS` entry.
pub type Roots = u8;

const ALL: Roots = (1 << ROOTS.len()) - 1;

/// The bit of `register`'s root, if it has one here.
pub fn bit(register: Register) -> Option<Roots> {
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
        (Operation::Move, Some("mov"), [Loc::Imm(constant)]) => constant.address.is_none() && (0..=0xFFFF).contains(&constant.value),
        _ => false,
    };
    if zeroes { root } else { 0 }
}

/// The roots whose upper half `one` may leave other than it found it.
fn disturbed(one: &Insn) -> Roots {
    // A jump or branch writes no register; no decoder answers for it.
    if liveness::_terminator(one.what.as_ref()) {
        return 0;
    }
    let effects = _register_effects(one, true, false).or_else(|| liveness::_declared(one));
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
pub fn after(one: &Insn, zero: Roots) -> Roots {
    let zeroed = zeroing(one);
    (zero & !disturbed(one)) | zeroed
}

/// Before each instruction, by `id`, the roots whose upper half is zero.
pub fn before(body: &LirBody) -> HashMap<usize, Roots> {
    let graph = _graph(&body.blocks);
    let predecessors = loops::predecessors(&graph);
    let mut into: IndexMap<i64, Roots> =
        body.blocks.iter().map(|block| (block.at, if block.at == body.entry { 0 } else { ALL })).collect();
    let mut out: IndexMap<i64, Roots> = body.blocks.iter().map(|block| (block.at, ALL)).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            let known: BTreeSet<i64> = out.keys().copied().collect();
            let mut zero = if block.at == body.entry { 0 } else { ALL };
            for from in predecessors.get(&block.at).into_iter().flatten().filter(|at| known.contains(at)) {
                zero &= out[from];
            }
            into.insert(block.at, zero);
            let leaving = block.insns.iter().fold(zero, |zero, one| after(one, zero));
            if leaving != out[&block.at] {
                out.insert(block.at, leaving);
                changing = true;
            }
        }
    }
    let mut result = HashMap::default();
    for block in &body.blocks {
        let mut zero = into[&block.at];
        for one in &block.insns {
            result.insert(id(one), zero);
            zero = after(one, zero);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;

    use super::{before, bit};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn one(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    #[test]
    fn test_a_zeroed_upper_half_survives_word_writes_and_jumps() {
        // A `jmp` decoded as unknown lost every root, so no loop kept the
        // preheader's `movzx`.
        let (ebx, bx) = (Loc::Reg(Reg { register: Register::EBX, width: 4 }), Loc::Reg(Reg { register: Register::BX, width: 2 }));
        let word = || Loc::Imm(Imm { value: 5, width: 2, address: None });
        let entry = LirBlock {
            succ: vec![1],
            ..LirBlock::new(0, vec![
                one(0, Operation::Extend, "movzx", vec![ebx], vec![bx.clone()], None),
                one(1, Operation::Jump, "jmp", vec![], vec![], Some(1)),
            ])
        };
        let looped = LirBlock {
            succ: vec![1, 2],
            ..LirBlock::new(1, vec![
                one(2, Operation::Move, "mov", vec![bx.clone()], vec![word()], None),
                one(3, Operation::Compare, "cmp", vec![], vec![bx.clone(), word()], None),
                one(4, Operation::Branch, "jne", vec![], vec![], Some(1)),
            ])
        };
        let exit = LirBlock::new(2, vec![one(5, Operation::Move, "mov", vec![bx], vec![word()], None)]);
        let body = LirBody::new("zero", 0, vec![entry, looped, exit], IndexMap::default(), IndexMap::default());

        let zero = before(&body);

        let ebx = bit(Register::EBX).unwrap();
        for block in &body.blocks[1..] {
            for insn in &block.insns {
                assert_ne!(zero[&crate::backend::peephole::id(insn)] & ebx, 0, "{:?}", insn.what);
            }
        }
    }
}
