//! Which constant-bound arrays stay in the runtime's far heap.
//!
//! A frame array is the stack's, reached `[bp+index+disp]`: no DIM or ERASE
//! call, no `es:` and no selector, and no register to hold its base. A far
//! array starts at offset 0 of its own segment, so its base is a segment
//! register (`es`, `fs`, `gs`: free ones, the integer registers untouched)
//! and its address is the index alone. A loop with more live values than the
//! target has registers is the case where that matters: the frame array's
//! address wants a register the loop has not got (an array parameter's base,
//! the temporary that adds two subscripts) and the loop spills where the far
//! form would not.
//!
//! The model reads the first compile, where every candidate is a frame
//! array. In each innermost loop (a back edge's natural loop) the scalars live
//! on entry to its header are held in the integer registers, and the variables
//! its subscripts name in the address registers, each class less the target's
//! `reserve`. A near access adds one for an array parameter (its base) and one
//! for a subscript that reads an array or calls (`composite`: not affine in a
//! counter, so its sum is formed by an instruction); each addition past a
//! class is a spill. Far arrays in the loop beyond the target's `segments`
//! cost the same. Where the frame spills more than the runtime's would, the
//! arrays of the groups in the loop (the arrays one array parameter receives,
//! transitively) go far together. The spill is paid on every trip and the
//! DIM and ERASE it would save once, and the trips are not known, so the
//! DIM's saving does not offset a predicted spill.

use std::collections::{BTreeMap, BTreeSet};

use super::tags::Tag;
use super::{Compiler, Function, INTEGER, LONG, Operand};

/// What the target holds and what the runtime's array calls cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Room {
    /// Integer registers; 0 leaves the model off.
    pub registers: usize,
    /// Registers an address is formed from (`bx`, `si`, `di`).
    pub addresses: usize,
    /// Segment registers a far array's base is held in.
    pub segments: usize,
    /// Registers a two-address target keeps back for the copy an operation
    /// makes where an operand stays live (`spill.rs`, `copied`).
    pub reserve: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Array {
    /// Procedure, array.
    Local(String, String),
    /// Procedure, parameter position.
    Formal(String, usize),
}

pub(super) struct Access {
    pub procedure: String,
    pub pointer: u32,
    pub array: Array,
    /// The variables the subscripts name.
    pub names: BTreeSet<String>,
    /// A subscript reads an array or calls: its value is not affine in any
    /// loop counter, so the address is formed by instructions and not
    /// stepped.
    pub composite: bool,
}

/// What the emitter saw of the arrays, for the model.
#[derive(Default)]
pub(super) struct Usage {
    pub accesses: Vec<Access>,
    /// An array handed to a parameter.
    pub flows: Vec<(Array, Array)>,
    /// A procedure's array parameters by name: their position.
    pub formals: BTreeMap<(String, String), usize>,
}

/// The variables the subscripts name, and whether any reads an array or calls.
pub(super) fn subscripts(indices: &[crate::syntax::Expr]) -> (BTreeSet<String>, bool) {
    use crate::syntax::Expr;
    fn walk(
        one: &Expr,
        names: &mut BTreeSet<String>,
        data: &mut bool,
    ) {
        match one {
            Expr::Name(name, _) => {
                names.insert(super::array_key(name));
            }
            Expr::Apply { arguments, .. } => {
                *data = true;
                arguments.iter().for_each(|inner| walk(inner, names, data));
            }
            Expr::Index { base, indices, .. } => {
                *data = true;
                walk(base, names, data);
                indices.iter().for_each(|inner| walk(inner, names, data));
            }
            Expr::Field { base, .. } => walk(base, names, data),
            Expr::Unary { operand, .. } => walk(operand, names, data),
            Expr::Binary { left, right, .. } => {
                walk(left, names, data);
                walk(right, names, data);
            }
            _ => {}
        }
    }
    let (mut names, mut data) = (BTreeSet::new(), false);
    indices.iter().for_each(|one| walk(one, &mut names, &mut data));
    (names, data)
}

struct Loop {
    header: u32,
    body: BTreeSet<u32>,
    /// No other loop lies inside it.
    innermost: bool,
}

fn loops(function: &Function) -> Vec<Loop> {
    let index: BTreeMap<u32, usize> = function.blocks.iter().enumerate().map(|(at, block)| (block.id, at)).collect();
    let successors = |id: u32| -> Vec<u32> {
        index
            .get(&id)
            .and_then(|&at| function.blocks[at].terminator.as_ref())
            .map(|terminator| terminator.targets.clone())
            .unwrap_or_default()
    };
    let Some(entry) = function.blocks.first().map(|block| block.id) else { return Vec::new() };
    let mut predecessors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for block in &function.blocks {
        for target in successors(block.id) {
            predecessors.entry(target).or_default().push(block.id);
        }
    }
    // Depth-first from the entry: an edge to a block still on the path is a
    // back edge.
    let mut state: BTreeMap<u32, bool> = BTreeMap::new();
    let mut back: Vec<(u32, u32)> = Vec::new();
    let mut stack: Vec<(u32, Vec<u32>)> = vec![(entry, successors(entry))];
    state.insert(entry, true);
    while let Some((block, pending)) = stack.last_mut() {
        let block = *block;
        match pending.pop() {
            Some(next) => match state.get(&next) {
                Some(true) => back.push((block, next)),
                Some(false) => {}
                None => {
                    state.insert(next, true);
                    stack.push((next, successors(next)));
                }
            },
            None => {
                state.insert(block, false);
                stack.pop();
            }
        }
    }
    let mut by_header: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for (tail, header) in back {
        let body = by_header.entry(header).or_default();
        body.insert(header);
        let mut work = vec![tail];
        while let Some(block) = work.pop() {
            if body.insert(block) {
                work.extend(predecessors.get(&block).into_iter().flatten().copied());
            }
        }
    }
    let all: Vec<(u32, BTreeSet<u32>)> = by_header.into_iter().collect();
    all.iter()
        .map(|(header, body)| Loop {
            header: *header,
            body: body.clone(),
            innermost: !all.iter().any(|(_, other)| other.len() < body.len() && other.is_subset(body)),
        })
        .collect()
}

/// The scalar integer locals live on entry to a loop's header, whether or not
/// the loop names them: its own counters and what the loops around it keep.
fn live_at(
    function: &Function,
    header: u32,
) -> usize {
    let scalar = |operand: &Operand| -> Option<u32> {
        let place = match operand {
            Operand::Place(place) | Operand::Element(place, _) => *place,
            Operand::Projection { place, .. } => *place,
            _ => return None,
        };
        let found = function.places.iter().find(|one| one.id == place)?;
        (matches!(found.type_id, INTEGER | LONG) && !found.name.starts_with("$forStep")).then_some(place)
    };
    let mut used: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut defined: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for block in &function.blocks {
        let (mut reads, mut writes) = (BTreeSet::new(), BTreeSet::new());
        for instruction in &block.instructions {
            let store = instruction.op == "store";
            for (at, operand) in instruction.operands.iter().enumerate() {
                if let Some(place) = scalar(operand) {
                    if store && at == 0 {
                        writes.insert(place);
                    } else if !writes.contains(&place) {
                        reads.insert(place);
                    }
                }
                // A reference parameter's pointee is read from before the loop.
                // KNOWN APPROXIMATION (exit 2026-11-15, when the spill-forecast
                // pricing replaces this count): an array parameter's descriptor
                // words are loaded through the same kind of operand and are
                // counted here as scalars, on top of the base `formals` adds.
                // The loop pays for the base once; the second count stands in
                // for the element addresses and values the count does not
                // see,
                // and `an_array_parameter_in_a_loop_nest_stays_the_runtimes`
                // (bubble at -Os, +5% clocks in the frame) fails without it.
                if let Operand::Indirect { base, type_id, inbounds: false, .. } = operand {
                    if matches!(*type_id, INTEGER | LONG) && function.parameters.contains(base) && !(store && at == 0) {
                        reads.insert(u32::MAX - *base);
                    }
                }
            }
        }
        used.insert(block.id, reads);
        defined.insert(block.id, writes);
    }
    let mut live: BTreeMap<u32, BTreeSet<u32>> =
        function.blocks.iter().map(|block| (block.id, BTreeSet::new())).collect();
    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let mut now = used[&block.id].clone();
            let out: BTreeSet<u32> = block
                .terminator
                .iter()
                .flat_map(|terminator| terminator.targets.iter())
                .filter_map(|target| live.get(target))
                .flatten()
                .copied()
                .collect();
            now.extend(out.difference(&defined[&block.id]));
            if now != live[&block.id] {
                live.insert(block.id, now);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    live.get(&header).map_or(0, |set| set.len())
}

fn find(
    group_of: &mut BTreeMap<Array, Array>,
    array: &Array,
) -> Array {
    let parent = group_of.get(array).cloned().unwrap_or_else(|| array.clone());
    if parent == *array {
        return parent;
    }
    let root = find(group_of, &parent);
    group_of.insert(array.clone(), root.clone());
    root
}

pub(super) fn kept_far(compiler: &Compiler) -> BTreeSet<String> {
    let room = compiler.options.room;
    if room.registers == 0 {
        return BTreeSet::new();
    }
    let key = |function: &Function| {
        if function.name == "__main" { String::new() } else { super::array_key(&function.name) }
    };
    let by_key: BTreeMap<String, &Function> = compiler.functions.iter().map(|one| (key(one), one)).collect();
    // The frame arrays.
    let mut frame: BTreeSet<Array> = BTreeSet::new();
    for (name, function) in &by_key {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(Tag::Allocate(shape)) = &instruction.tag {
                    if shape.frame {
                        frame.insert(Array::Local(name.clone(), shape.name.clone()));
                    }
                }
            }
        }
    }
    // Groups: the arrays one parameter receives.
    let mut group_of: BTreeMap<Array, Array> = BTreeMap::new();
    for (from, to) in &compiler.usage.flows {
        let (a, b) = (find(&mut group_of, from), find(&mut group_of, to));
        if a != b {
            group_of.insert(a, b);
        }
    }
    let trace = std::env::var_os("LLRM_QB_PLACEMENT_TRACE").is_some();
    let mut far_groups: BTreeSet<Array> = BTreeSet::new();
    for (name, function) in &by_key {
        for one in loops(function).iter().filter(|one| one.innermost) {
            let mut results: BTreeSet<u32> = BTreeSet::new();
            for block in function.blocks.iter().filter(|block| one.body.contains(&block.id)) {
                for instruction in &block.instructions {
                    results.extend(instruction.results.iter().copied());
                }
            }
            let touched: Vec<&Access> = compiler
                .usage
                .accesses
                .iter()
                .filter(|access| access.procedure == *name && results.contains(&access.pointer))
                .collect();
            if touched.is_empty() {
                continue;
            }
            let arrays: BTreeSet<Array> = touched.iter().map(|access| access.array.clone()).collect();
            let formals = arrays.iter().filter(|array| matches!(array, Array::Formal(..))).count();
            let composite = touched.iter().any(|access| access.composite);
            let indices: BTreeSet<&String> = touched.iter().flat_map(|access| access.names.iter()).collect();
            // Two classes of register run short. The scalars live on entry to
            // the loop, with an array parameter's base and the temporary a
            // composite subscript's sum wants, are held in the integer
            // registers; the subscript variables, with the same additions, in
            // the address registers. Each addition past what a class holds,
            // less the reserve, is a spill. A far array's base is in a segment
            // register instead, which holds `segments` of them.
            let extra = formals + usize::from(composite);
            let overflow = |wanted: usize, held: usize| wanted.saturating_sub(held.saturating_sub(room.reserve));
            let live = live_at(function, one.header);
            let general = overflow(live + extra, room.registers) - overflow(live, room.registers);
            let address = overflow(indices.len() + extra, room.addresses) - overflow(indices.len(), room.addresses);
            let near = general.max(address);
            let far = arrays.len().saturating_sub(room.segments);
            if trace {
                eprintln!(
                    "placement: {name} loop indices {:?} live {live} formals {formals} composite {composite} arrays {} general {general} address {address} far {far}",
                    indices,
                    arrays.len(),
                );
            }
            // A loop's trips are not known, and the spill is paid on each,
            // the DIM and ERASE it saves once: where the frame spills more, the
            // arrays of the loop's groups go far together.
            if near > far {
                far_groups.extend(arrays.iter().map(|array| find(&mut group_of, array)));
            }
        }
    }
    let kept: BTreeSet<String> = frame
        .iter()
        .filter(|array| far_groups.contains(&find(&mut group_of, array)))
        .filter_map(|array| match array {
            Array::Local(procedure, name) => Some(format!("{procedure}/{name}")),
            Array::Formal(..) => None,
        })
        .collect();
    if trace {
        eprintln!("placement: relief {far_groups:?} kept {kept:?}");
    }
    kept
}
