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
use crate::backend::{coalesce, twoaddr};
use crate::model::ir::{Held, Loc, Space};
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
    one.what.as_ref().is_some_and(twoaddr::ties)
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

/// For each value, the instructions it lives through that take registers (what they require,
/// deliver or clobber), as (block, registers).
pub(crate) fn through_taken(body: &LirBody) -> IndexMap<u32, Vec<(i64, BTreeSet<Register>)>> {
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let (_, live_out) = allocate::live(body);
    let mut out: IndexMap<u32, Vec<(i64, BTreeSet<Register>)>> = IndexMap::default();
    for block in &body.blocks {
        let mut live = live_out[&block.at].clone();
        for one in block.insns.iter().rev() {
            let taken = crate::backend::ssaassign::takes(one, &general);
            if !taken.is_empty() {
                for value in live.iter().filter(|value| !one.defines.contains(value)) {
                    out.entry(*value).or_default().push((block.at, taken.clone()));
                }
            }
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
    }
    out
}

/// Each use of a value as an address half, with the registers that can form the address and the block it is in.
pub(crate) fn address_demands(body: &LirBody) -> Vec<(u32, BTreeSet<Register>, i64)> {
    let bx: BTreeSet<Register> = target::WORD_BASES.iter().map(|one| _whole(*one)).collect();
    let indexes: BTreeSet<Register> = target::WORD_INDEXES.iter().map(|one| _whole(*one)).collect();
    let bases: BTreeSet<Register> = target::ADDRESSING.iter().map(|one| _whole(*one)).filter(|one| *one != Register::EBP).collect();
    let pair: BTreeSet<Register> = bx.union(&indexes).copied().collect();
    let mut out: Vec<(u32, BTreeSet<Register>, i64)> = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                let Loc::Mem(cell) = place else { continue };
                let word = |held: &Option<Held>| held.filter(|held| held.width == 2);
                match (word(&cell.base), word(&cell.index)) {
                    (Some(base), None) => {
                        let allowed = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) { &indexes } else { &bases };
                        out.push((base.value, allowed.clone(), block.at));
                    }
                    (Some(base), Some(index)) => {
                        out.push((base.value, pair.clone(), block.at));
                        out.push((index.value, pair.clone(), block.at));
                    }
                    (None, Some(index)) => out.push((index.value, indexes.clone(), block.at)),
                    (None, None) => {}
                }
            }
        }
    }
    out
}

/// The registers each value used as an address half may take there, over all its uses.
fn addressing(body: &LirBody) -> IndexMap<u32, BTreeSet<Register>> {
    let mut out: IndexMap<u32, BTreeSet<Register>> = IndexMap::default();
    for (value, allowed, _) in address_demands(body) {
        let entry = out.entry(value).or_insert_with(|| allowed.clone());
        let narrowed: BTreeSet<Register> = entry.intersection(&allowed).copied().collect();
        if !narrowed.is_empty() {
            *entry = narrowed;
        }
    }
    out
}

/// The registers each value confined to byte registers may take: the only values with no other home.
pub(crate) fn byte_classes(body: &LirBody, segments: &Segments) -> IndexMap<u32, BTreeSet<Register>> {
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    let bytes: BTreeSet<Register> = [Register::EAX, Register::EBX, Register::ECX, Register::EDX].into_iter().collect();
    allocate::classes(body, &BTreeSet::new(), segments)
        .into_iter()
        .filter_map(|(value, class)| {
            let registers: BTreeSet<Register> = class.iter().map(|one| _whole(*one)).filter(|one| general.contains(one)).collect();
            (!registers.is_empty() && registers.is_subset(&bytes)).then_some((value, registers))
        })
        .collect()
}

/// Each value a phi, a copy or a tie names, with every value they join it to.
fn webs(body: &LirBody) -> IndexMap<u32, BTreeSet<u32>> {
    let mut root: IndexMap<u32, u32> = IndexMap::default();
    fn find(root: &mut IndexMap<u32, u32>, value: u32) -> u32 {
        let parent = *root.entry(value).or_insert(value);
        if parent == value {
            return value;
        }
        let top = find(root, parent);
        root.insert(value, top);
        top
    }
    let mut join = |root: &mut IndexMap<u32, u32>, a: u32, b: u32| {
        let (a, b) = (find(root, a), find(root, b));
        root.insert(a, b);
    };
    for block in &body.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                join(&mut root, phi.result, *value);
            }
        }
        // A copy, and a tied result with its first source, cost nothing in one register.
        for one in &block.insns {
            if let Some((into, out_of)) = coalesce::_copy(one) {
                join(&mut root, into, out_of);
            } else if let Some(what) = one.what.as_ref().filter(|what| twoaddr::ties(what)) {
                if let (Some(Loc::Held(first)), [made]) = (what.sources.first(), &one.defines[..]) {
                    join(&mut root, *made, first.value);
                }
            }
        }
    }
    let values: Vec<u32> = root.keys().copied().collect();
    let mut groups: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for value in &values {
        let top = find(&mut root, *value);
        groups.entry(top).or_default().insert(*value);
    }
    values.iter().map(|value| (*value, groups[&find(&mut root, *value)].clone())).collect()
}

/// A register for each value of `body` that wants one: `skip` names those that do not.
pub fn coloured(body: &LirBody, skip: &BTreeSet<u32>, fixed: &IndexMap<u32, Register>, segments: &Segments) -> IndexMap<u32, Register> {
    let confined = allocate::classes(body, &BTreeSet::new(), segments);
    let general: Vec<Register> = target::AVAILABLE.iter().map(|one| _whole(*one)).collect();
    // A value any register holds takes first the registers the fewest confined values may take.
    let bytes_first: BTreeSet<Register> = [Register::EAX, Register::EBX, Register::ECX, Register::EDX].into_iter().collect();
    let demand = |register: &Register| confined.values().filter(|class| class.iter().any(|one| _whole(*one) == *register)).count();
    let mut roomy: Vec<Register> = general.clone();
    roomy.sort_by_key(demand);
    // A byte value lives in a register with byte halves; an address role is met
    // where the value acts, by a copy if need be, so it only orders the choice.
    let bytes = &bytes_first;
    // A value of no class of its own prefers its phi web's: the web joins in one register
    // where no copy separates it, and its first member is coloured before its uses are seen.
    // An address half's registers are a preference too: a copy meets the role, but in a loop it runs each time.
    let mut leaning: IndexMap<u32, BTreeSet<Register>> = addressing(body);
    for (value, class) in &confined {
        leaning.insert(*value, class.iter().map(|one| _whole(*one)).collect());
    }
    let mut web = webs(body);
    for value in leaning.keys() {
        web.entry(*value).or_insert_with(|| BTreeSet::from([*value]));
    }
    let joined: IndexMap<u32, Vec<Register>> = web
        .iter()
        .filter(|(value, _)| !confined.contains_key(*value))
        .filter_map(|(value, members)| {
            let classes: Vec<&BTreeSet<Register>> = members.iter().filter_map(|member| leaning.get(member)).collect();
            let shared: Vec<Register> = roomy.iter().copied().filter(|register| !classes.is_empty() && classes.iter().all(|class| class.contains(register))).collect();
            (!shared.is_empty()).then_some((*value, shared))
        })
        .collect();
    let class_of = |value: u32| -> Vec<Register> {
        match confined.get(&value) {
            None => {
                let first: Vec<Register> = joined.get(&value).cloned().unwrap_or_default();
                first.iter().chain(roomy.iter().filter(|register| !first.contains(register))).copied().collect()
            }
            Some(class) => {
                let mut out: Vec<Register> = Vec::new();
                for register in target::order(Some(class), segments) {
                    let root = _whole(register);
                    if general.contains(&root) && !out.contains(&root) {
                        out.push(root);
                    }
                }
                let hard = out.iter().all(|register| bytes.contains(register));
                if !hard {
                    for register in &roomy {
                        if !out.contains(register) {
                            out.push(*register);
                        }
                    }
                }
                out
            }
        }
    };
    let wanted = |value: u32| !skip.contains(&value) && confined.get(&value).is_none_or(|class| class.iter().any(|one| general.contains(&_whole(*one))));
    let wish = wishes(body);
    // Registers a value would have to leave, and come back to, where an instruction it lives through takes them.
    let avoid: IndexMap<u32, BTreeSet<Register>> = through_taken(body).into_iter().map(|(value, spots)| (value, spots.into_iter().flat_map(|(_, taken)| taken).collect())).collect();
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
            // A pinned value has its register, taken or not: a clash is the assignment's to refuse.
            if let Some(register) = fixed.get(&value) {
                return Some(_whole(*register));
            }
            let mut order: Vec<Register> = Vec::new();
            order.extend(wish.get(&value).copied());
            order.extend(prefer.iter().copied());
            order.extend(partners.get(&value).into_iter().flatten().filter_map(|other| colour.get(other).copied()));
            order.extend(class.iter().copied());
            let free = |register: &Register| class.contains(register) && !taken.contains_key(register);
            let kept = avoid.get(&value);
            order.iter().copied().find(|register| free(register) && kept.is_none_or(|set| !set.contains(register)) || wish.get(&value) == Some(register) && free(register))
                .or_else(|| class.iter().copied().find(|register| free(register) && kept.is_none_or(|set| !set.contains(register))))
                .or_else(|| order.into_iter().find(free))
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
            let first = twoaddr::tie_source(one, &after[position]);
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
                    None => {
                        UNCOLOURED.with(|count| count.set(count.get() + 1));
                        llrm_support::debug!("ssaassign", "{}: no colour for value#{value} at {:#06x}: taken {:?}, class {:?}", body.name, one.at, pool.keys().collect::<Vec<_>>(), class_of(value));
                    }
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
