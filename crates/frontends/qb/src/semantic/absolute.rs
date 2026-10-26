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
                && function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|one| sets_unknown(one) || escapes(one, &defined, &clobbers))
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
    matches!(one.tag, Some(Tag::SetSegment))
        && !matches!(
            one.operands.as_slice(),
            [Operand::Place(_), Operand::Constant(..)]
        )
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
    let segment = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(
            |one| match (&one.tag, one.operands.first()) {
                (Some(Tag::SetSegment), Some(Operand::Place(place))) => Some(*place),
                _ => None,
            },
        );
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

/// The place whose VARSEG a DEF SEG stores, where the stored value is
/// `POINTER_SEGMENT` of that place's `ADDRESS`.
fn varseg_of(
    function: &Function,
    defs: &BTreeMap<u32, (usize, usize)>,
    one: &super::Instruction,
) -> Option<u32> {
    let [Operand::Place(_), Operand::Value(value)] = one.operands.as_slice() else { return None };
    let (at, index) = *defs.get(value)?;
    let segment = &function.blocks[at].instructions[index];
    let [Operand::Value(address)] = segment.operands.as_slice() else { return None };
    (segment.op == "pointer_segment").then_some(())?;
    let (at, index) = *defs.get(address)?;
    let address = &function.blocks[at].instructions[index];
    match (address.op, address.operands.as_slice()) {
        ("address", [Operand::Place(place)]) => Some(*place),
        _ => None,
    }
}

/// POKEs through a DEF SEG = VARSEG(x) that nothing since has changed, at
/// `VARPTR(x) + k` inside x: the pointer is x's own address plus `k`, which an
/// optimizer sees apart from every other variable and from `b$seg`. The
/// pointer the POKE built from the segment cell is the one `ADDRESS(x)` makes.
pub(super) fn varseg(compiler: &mut Compiler) {
    let defined: BTreeSet<String> = compiler.functions.iter().map(|function| function.name.clone()).collect();
    let mut clobbers: BTreeSet<String> = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for function in &compiler.functions {
            if !clobbers.contains(&function.name)
                && function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|one| sets_unknown(one) || escapes(one, &defined, &clobbers))
            {
                clobbers.insert(function.name.clone());
                changed = true;
            }
        }
    }
    for function in &mut compiler.functions {
        let extents: BTreeMap<u32, usize> = function.places.iter().map(|place| (place.id, place.extent)).collect();
        varseg_pokes(function, &defined, &clobbers, &extents);
    }
}

fn varseg_pokes(
    function: &mut Function,
    defined: &BTreeSet<String>,
    clobbers: &BTreeSet<String>,
    extents: &BTreeMap<u32, usize>,
) {
    if !function.blocks.iter().flat_map(|block| &block.instructions).any(|one| matches!(one.tag, Some(Tag::Poke))) {
        return;
    }
    let mut defs: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for (at, block) in function.blocks.iter().enumerate() {
        for (index, one) in block.instructions.iter().enumerate() {
            for &result in &one.results {
                defs.insert(result, (at, index));
            }
        }
    }
    let index: BTreeMap<u32, usize> = function.blocks.iter().enumerate().map(|(at, block)| (block.id, at)).collect();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
    for (at, block) in function.blocks.iter().enumerate() {
        for target in block.terminator.iter().flat_map(|one| &one.targets) {
            if let Some(&to) = index.get(target) {
                predecessors[to].push(at);
            }
        }
    }
    // Must-analysis over the place a DEF SEG = VARSEG(x) named: `None` is any
    // other segment. Blocks start optimistic (`Top`) except the entry and the
    // blocks nothing jumps to, so a loop keeps what its entry set.
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Top,
        Place(u32),
        Other,
    }
    let step = |function: &Function, state: State, one: &super::Instruction| -> State {
        if matches!(one.tag, Some(Tag::SetSegment)) {
            return match varseg_of(function, &defs, one) {
                Some(place) => State::Place(place),
                None => State::Other,
            };
        }
        if escapes(one, defined, clobbers) {
            return State::Other;
        }
        state
    };
    let met = |a: State, b: State| match (a, b) {
        (State::Top, any) | (any, State::Top) => any,
        (State::Place(x), State::Place(y)) if x == y => State::Place(x),
        _ => State::Other,
    };
    let mut after = vec![State::Top; function.blocks.len()];
    let entering = |after: &[State], at: usize| {
        if at == 0 || predecessors[at].is_empty() {
            State::Other
        } else {
            predecessors[at].iter().fold(State::Top, |state, &from| met(state, after[from]))
        }
    };
    let mut changed = true;
    while changed {
        changed = false;
        for at in 0..function.blocks.len() {
            let mut state = entering(&after, at);
            for one in &function.blocks[at].instructions {
                state = step(function, state, one);
            }
            if after[at] != state {
                after[at] = state;
                changed = true;
            }
        }
    }
    // The POKEs to rewrite: (block, instruction, address value, offset).
    let mut rewrites: Vec<(usize, usize, u32, usize)> = Vec::new();
    for at in 0..function.blocks.len() {
        let mut state = entering(&after, at);
        for (position, one) in function.blocks[at].instructions.iter().enumerate() {
            if matches!(one.tag, Some(Tag::Poke)) {
                if let (State::Place(place), Some(Operand::Indirect { base, offset: 0, .. })) =
                    (state, one.operands.first())
                {
                    if let Some((address, k)) = poke_offset(function, &defs, *base, place) {
                        let width = 1;
                        if extents.get(&place).is_some_and(|&extent| k + width <= extent) {
                            rewrites.push((at, position, address, k));
                        }
                    }
                }
            }
            state = step(function, state, one);
        }
    }
    for (at, position, address, k) in rewrites {
        if let Some(Operand::Indirect { base, offset, .. }) =
            function.blocks[at].instructions[position].operands.first_mut()
        {
            *base = address;
            *offset = k;
        }
    }
}

/// The `ADDRESS` value and the bytes past it that a POKE's far pointer `base`
/// names: `CONCAT(segment cell, OFFSET(ADDRESS(place)) + k)`.
fn poke_offset(
    function: &Function,
    defs: &BTreeMap<u32, (usize, usize)>,
    base: u32,
    place: u32,
) -> Option<(u32, usize)> {
    let at = |value: u32| -> Option<&super::Instruction> {
        let (block, index) = *defs.get(&value)?;
        Some(&function.blocks[block].instructions[index])
    };
    let concat = at(base).filter(|one| one.op == "concat")?;
    let [Operand::Value(segment), Operand::Value(offset)] = concat.operands.as_slice() else { return None };
    at(*segment).filter(|one| matches!(one.tag, Some(Tag::ReadSegment)))?;
    // OFFSET(ADDRESS(place)), plus a constant.
    let (low, k) = match at(*offset) {
        Some(one) if one.op == "add" => match one.operands.as_slice() {
            [Operand::Value(low), Operand::Constant(_, super::Number::Integer(k))] if *k >= 0 => (*low, *k as usize),
            _ => return None,
        },
        Some(one) if one.op == "pointer_offset" => (*offset, 0),
        _ => return None,
    };
    let pointer_offset = at(low).filter(|one| one.op == "pointer_offset")?;
    let [Operand::Value(address)] = pointer_offset.operands.as_slice() else { return None };
    let made = at(*address)?;
    match (made.op, made.operands.as_slice()) {
        ("address", [Operand::Place(named)]) if *named == place => Some((*address, k)),
        _ => None,
    }
}
