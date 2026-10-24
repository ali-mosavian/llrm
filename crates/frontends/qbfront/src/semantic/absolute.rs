//! POKEs into memory no variable lives in.
//!
//! `b$seg` is a run-time cell, so a POKE may write any variable: DEF SEG
//! with no argument, or with VARSEG(x), points it at the program's own data.
//! Where a literal DEF SEG has run on every path to the POKE, and no call
//! since may have set the segment to anything else, the segment is a
//! constant the program does not own. The POKE then stays
//! apart from every place, as a far array's accesses do (the `allocation`
//! tag); an optimizer may keep variables and `b$seg` in registers across it.

use std::collections::{BTreeMap, BTreeSet};

use super::tags::Tag;
use super::{Compiler, Function, Operand};

pub(super) fn applied(compiler: &mut Compiler) {
    let defined: BTreeSet<String> = compiler.functions.iter().map(|function| function.name.clone()).collect();
    // The procedures that may leave the segment unknown: one that sets it to
    // other than a literal, or calls such a one or code outside the module.
    let mut clobbers: BTreeSet<String> = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for function in &compiler.functions {
            if !clobbers.contains(&function.name)
                && function.blocks.iter().flat_map(|block| &block.instructions).any(|one| {
                    sets_unknown(one) || escapes(one, &defined, &clobbers)
                })
            {
                clobbers.insert(function.name.clone());
                changed = true;
            }
        }
    }
    for function in &mut compiler.functions {
        mark(function, &defined, &clobbers);
    }
}

fn sets_unknown(one: &super::Instruction) -> bool {
    matches!(one.tag, Some(Tag::SetSegment)) && !matches!(one.operands.as_slice(), [Operand::Place(_), Operand::Constant(..)])
}

/// Whether an instruction calls code that may leave the segment unknown.
fn escapes(
    one: &super::Instruction,
    defined: &BTreeSet<String>,
    clobbers: &BTreeSet<String>,
) -> bool {
    match (&one.tag, &one.callee) {
        (Some(Tag::Invoke { .. }), Some(callee)) => !defined.contains(callee) || clobbers.contains(callee),
        (Some(Tag::Invoke { .. }), None) => true,
        _ => false,
    }
}

fn mark(
    function: &mut Function,
    defined: &BTreeSet<String>,
    clobbers: &BTreeSet<String>,
) {
    let segment = function.blocks.iter().flat_map(|block| &block.instructions).find_map(|one| match (&one.tag, one.operands.first()) {
        (Some(Tag::SetSegment), Some(Operand::Place(place))) => Some(*place),
        _ => None,
    });
    let Some(segment) = segment else { return };
    let index: BTreeMap<u32, usize> = function.blocks.iter().enumerate().map(|(at, block)| (block.id, at)).collect();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
    for (at, block) in function.blocks.iter().enumerate() {
        for target in block.terminator.iter().flat_map(|one| &one.targets) {
            if let Some(&to) = index.get(target) {
                predecessors[to].push(at);
            }
        }
    }
    // Must-analysis: a literal segment is set on every path in. The entry
    // and every block nothing jumps to (an error handler) start unknown.
    let mut after = vec![true; function.blocks.len()];
    let entering = |after: &[bool], at: usize| {
        at != 0 && !predecessors[at].is_empty() && predecessors[at].iter().all(|&from| after[from])
    };
    let mut changed = true;
    while changed {
        changed = false;
        for at in 0..function.blocks.len() {
            let mut known = entering(&after, at);
            for one in &function.blocks[at].instructions {
                if sets_unknown(one) || escapes(one, defined, clobbers) {
                    known = false;
                } else if matches!(one.tag, Some(Tag::SetSegment)) {
                    known = true;
                }
            }
            if after[at] != known {
                after[at] = known;
                changed = true;
            }
        }
    }
    let mut marked = Vec::new();
    for at in 0..function.blocks.len() {
        let mut known = entering(&after, at);
        for one in &function.blocks[at].instructions {
            if sets_unknown(one) || escapes(one, defined, clobbers) {
                known = false;
            } else if matches!(one.tag, Some(Tag::SetSegment)) {
                known = true;
            } else if matches!(one.tag, Some(Tag::Poke)) && known {
                if let Some(Operand::Indirect { base, .. }) = one.operands.first() {
                    marked.push(*base);
                }
            }
        }
    }
    function.absolute.extend(marked.into_iter().map(|base| (base, segment)));
}
