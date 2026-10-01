//! Registers for an SSA body in dominance order (Hack, "Register Allocation
//! for Programs in SSA Form", ch. 4): once the spiller left no point with more
//! values than registers, walking the dominator tree and giving each value a
//! register free where it is defined colours the body, and what a phi joins
//! becomes a permutation of registers already given.
//!
//! The result is advice to the allocator, not a constraint: the phases
//! between here and assignment (phi elimination's copies, two-address
//! instructions, constraint copies) may make a colour impossible, and the
//! allocator then chooses otherwise.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::Register;

use crate::analysis::{intervals as ranges, loops};
use crate::backend::allocate::{self, _whole};
use crate::backend::target::{self, Segments};
use crate::backend::twoaddr;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

/// The register each value of a body was coloured with, shared with the allocator.
pub type Colours = Rc<RefCell<IndexMap<u32, Register>>>;

thread_local! {
    static UNCOLOURED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many values found no free register of their class where they were defined.
pub fn uncoloured() -> usize {
    UNCOLOURED.with(std::cell::Cell::get)
}

/// Whether `one` writes its first source's register.
fn tied(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        twoaddr::_TIED.contains(&what.op) || (what.op == Operation::Multiply && what.dests.len() == 1 && what.sources.len() == 2)
    })
}

fn first_source(one: &Insn) -> Option<u32> {
    match one.what.as_ref()?.sources.first()? {
        Loc::Held(held) => Some(held.value),
        _ => None,
    }
}

/// The register each instruction asks a value to be in.
fn wishes(body: &LirBody) -> IndexMap<u32, Register> {
    let mut out: IndexMap<u32, Register> = IndexMap::default();
    for one in body.insns() {
        for (held, register) in one.requires.iter().chain(&one.delivers) {
            out.entry(held.value).or_insert(_whole(*register));
        }
        if let Some(what) = &one.what {
            for (place, register) in target::requirements(what) {
                let side = if place.side == "dest" { &what.dests } else { &what.sources };
                if let Some(Loc::Held(held)) = side.get(place.index) {
                    out.entry(held.value).or_insert(_whole(register));
                }
            }
        }
    }
    out
}

/// A register for each value of `body` that wants one: `skip` names those that do not.
pub fn coloured(body: &LirBody, skip: &BTreeSet<u32>, segments: &Segments) -> IndexMap<u32, Register> {
    let confined = allocate::classes(body, &BTreeSet::new(), segments);
    let general: Vec<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let class_of = |value: u32| -> Vec<Register> {
        match confined.get(&value) {
            None => general.clone(),
            Some(class) => {
                let mut out: Vec<Register> = Vec::new();
                for register in target::order(Some(class), segments) {
                    let root = _whole(register);
                    if general.contains(&root) && !out.contains(&root) {
                        out.push(root);
                    }
                }
                out
            }
        }
    };
    let wanted = |value: u32| !skip.contains(&value) && confined.get(&value).is_none_or(|class| class.iter().any(|one| general.contains(&_whole(*one))));
    let wish = wishes(body);
    let (live_in, live_out) = allocate::live(body);
    let graph = ranges::_graph(&body.blocks);
    let idom = loops::immediate_dominators(&graph, Some(body.entry));
    let mut children: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        if let Some(Some(parent)) = idom.get(&block.at) {
            children.entry(*parent).or_default().push(block.at);
        }
    }
    // Copy partners: a phi's result and its arguments.
    let mut partners: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                partners.entry(phi.result).or_default().push(*value);
                partners.entry(*value).or_default().push(phi.result);
            }
        }
    }
    let by_at: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut colour: IndexMap<u32, Register> = IndexMap::default();
    let mut todo: Vec<i64> = vec![body.entry];
    while let Some(at) = todo.pop() {
        let block = by_at[&at];
        let mut taken: IndexMap<Register, u32> = IndexMap::default();
        for value in &live_in[&at] {
            if let Some(register) = colour.get(value) {
                taken.insert(*register, *value);
            }
        }
        // The values live after each instruction of the block.
        let mut after: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); block.insns.len()];
        let mut live = live_out[&at].clone();
        for (position, one) in block.insns.iter().enumerate().rev() {
            after[position] = live.clone();
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
        let choose = |value: u32, taken: &IndexMap<Register, u32>, colour: &IndexMap<u32, Register>, prefer: &[Register]| -> Option<Register> {
            let class = class_of(value);
            let mut order: Vec<Register> = Vec::new();
            order.extend(wish.get(&value).copied());
            order.extend(prefer.iter().copied());
            order.extend(partners.get(&value).into_iter().flatten().filter_map(|other| colour.get(other).copied()));
            order.extend(class.iter().copied());
            order.into_iter().find(|register| class.contains(register) && !taken.contains_key(register))
        };
        for phi in &block.phis {
            if !wanted(phi.result) {
                continue;
            }
            match choose(phi.result, &taken, &colour, &[]) {
                Some(register) => {
                    taken.insert(register, phi.result);
                    colour.insert(phi.result, register);
                }
                None => UNCOLOURED.with(|count| count.set(count.get() + 1)),
            }
        }
        for (position, one) in block.insns.iter().enumerate() {
            let dying: Vec<u32> = one.uses.iter().copied().filter(|value| !after[position].contains(value)).collect();
            let free = |taken: &mut IndexMap<Register, u32>, colour: &IndexMap<u32, Register>, value: u32| {
                if let Some(register) = colour.get(&value) {
                    if taken.get(register) == Some(&value) {
                        taken.shift_remove(register);
                    }
                }
            };
            // A tied instruction writes its first source's register while its other sources are still read.
            let first = if tied(one) { first_source(one) } else { None };
            for value in &dying {
                if !tied(one) || Some(*value) == first {
                    free(&mut taken, &colour, *value);
                }
            }
            let prefer: Vec<Register> = first.and_then(|value| colour.get(&value).copied()).into_iter().collect();
            // What an instruction clobbers is no home for a value it does not define.
            let clobbered: BTreeSet<Register> = one.clobbers.iter().map(|register| _whole(*register)).collect();
            let mut blocked = taken.clone();
            for register in &clobbered {
                blocked.entry(*register).or_insert(u32::MAX);
            }
            for value in one.defines.iter().copied().filter(|value| wanted(*value)) {
                let pool = if after[position].contains(&value) { &taken } else { &blocked };
                match choose(value, pool, &colour, &prefer) {
                    Some(register) => {
                        taken.insert(register, value);
                        blocked.insert(register, value);
                        colour.insert(value, register);
                    }
                    None => UNCOLOURED.with(|count| count.set(count.get() + 1)),
                }
            }
            for value in dying {
                free(&mut taken, &colour, value);
            }
            for value in &one.defines {
                if !after[position].contains(value) {
                    free(&mut taken, &colour, *value);
                }
            }
        }
        for child in children.get(&at).into_iter().flatten().rev() {
            todo.push(*child);
        }
    }
    colour
}
