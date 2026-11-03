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
//! The model prices that on the first compile, where every candidate is a
//! frame array. In each innermost loop (a back edge's natural loop) the
//! variables the subscripts name are address registers, of the target's
//! `addresses` less one kept for a temporary. A near access adds one for an
//! array parameter (its base) and one for a subscript that reads an array or
//! calls (`composite`: not affine in a counter, so its sum is formed by an
//! instruction); each addition past the registers is a spill (`Room::spill`
//! clocks a trip). Far arrays in the loop beyond the target's `segments`
//! cost the same. A loop that spills more near than far is worth its trips
//! (`TRIP` to the power of its depth, times how often its procedure runs) of
//! that. The arrays of the groups in the loop (the arrays one array parameter
//! receives, transitively) go far together where that outweighs the DIM and
//! ERASE calls the frame saves them (`Room::dim` clocks per execution of the
//! DIM, counted by the call graph and the loops around the call).

use std::collections::{BTreeMap, BTreeSet};

use super::tags::Tag;
use super::{Compiler, Function};

/// Each loop level runs its body this many times as often: the pin's
/// branch-probability estimate for a loop with no other word, 124 back edges
/// to 4 exits (`LoopBranchHeuristic`).
const TRIP: i64 = 32;

/// What the target holds and what the runtime's array calls cost.
#[derive(Clone, Copy, Debug, Default)]
pub struct Room {
    /// Registers an address is formed from (`bx`, `si`, `di`); 0 leaves the
    /// model off. One stays free for the temporary an address that no
    /// single instruction forms needs.
    pub addresses: usize,
    /// Segment registers a far array's base is held in.
    pub segments: usize,
    /// Clocks one value spilled in a loop costs per iteration.
    pub spill: i64,
    /// Clocks the DIM and the ERASE of one array cost together.
    pub dim: i64,
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
        .map(|(_, body)| Loop {
            body: body.clone(),
            innermost: !all.iter().any(|(_, other)| other.len() < body.len() && other.is_subset(body)),
        })
        .collect()
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
    if room.addresses == 0 {
        return BTreeSet::new();
    }
    let key = |function: &Function| {
        if function.name == "__main" { String::new() } else { super::array_key(&function.name) }
    };
    let by_key: BTreeMap<String, &Function> = compiler.functions.iter().map(|one| (key(one), one)).collect();
    let loops_of: BTreeMap<String, Vec<Loop>> =
        by_key.iter().map(|(name, function)| (name.clone(), loops(function))).collect();
    let weight = |name: &str, block: u32| -> i64 {
        let depth = loops_of[name].iter().filter(|one| one.body.contains(&block)).count().min(6) as u32;
        TRIP.pow(depth)
    };
    let block_of = |function: &Function, instruction: u32| -> Option<u32> {
        function.blocks.iter().find(|block| block.instructions.iter().any(|one| one.id == instruction)).map(|b| b.id)
    };
    // How often each procedure runs: once for the main code, and for the rest
    // the weight of each call site in a caller times the caller's.
    let by_symbol: BTreeMap<u32, String> = compiler
        .functions
        .iter()
        .filter_map(|function| Some((compiler.signatures.get(super::canonical(&function.name))?.symbol, key(function))))
        .collect();
    let seed = || {
        let mut runs: BTreeMap<String, i64> = by_key.keys().map(|name| (name.clone(), 0)).collect();
        runs.insert(String::new(), 1);
        runs
    };
    let mut runs = seed();
    for _ in 0..8 {
        let mut next = seed();
        for (name, function) in &by_key {
            for call in &function.calls {
                let Some(callee) = call.callee.and_then(|symbol| by_symbol.get(&symbol)) else { continue };
                let at = block_of(function, call.instruction).map_or(1, |block| weight(name, block));
                let entry = next.entry(callee.clone()).or_default();
                *entry = entry.saturating_add(runs[name].saturating_mul(at)).min(1 << 40);
            }
        }
        if next == runs {
            break;
        }
        runs = next;
    }
    // The frame arrays, with how often their DIM runs.
    let mut frame: BTreeMap<Array, i64> = BTreeMap::new();
    for (name, function) in &by_key {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(Tag::Allocate(shape)) = &instruction.tag {
                    if shape.frame {
                        let at = runs[name].saturating_mul(weight(name, block.id));
                        frame.insert(Array::Local(name.clone(), shape.name.clone()), at);
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
    // Per loop, who is accessed and what the loop's scalars come to.
    let mut relief: Vec<(i64, BTreeSet<Array>)> = Vec::new();
    for (name, function) in &by_key {
        for one in loops_of[name].iter().filter(|one| one.innermost) {
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
            // Each index is a register; an array parameter's base is another,
            // and a composite subscript wants the temporary that sums it.
            let usable = room.addresses.saturating_sub(1);
            let spills = |wanted: usize| wanted.saturating_sub(usable);
            let near = spills(indices.len() + formals + usize::from(composite)) - spills(indices.len());
            let far = arrays.len().saturating_sub(room.segments);
            if std::env::var_os("LLRM_QB_PLACEMENT_TRACE").is_some() {
                eprintln!(
                    "placement: {name} loop indices {:?} formals {formals} composite {composite} arrays {} near {near} far {far} runs {}",
                    indices,
                    arrays.len(),
                    runs[name]
                );
            }
            if near <= far {
                continue;
            }
            let hot = one.body.iter().map(|&block| weight(name, block)).max().unwrap_or(1);
            let worth = runs[name].saturating_mul(hot).saturating_mul(room.spill).saturating_mul((near - far) as i64);
            let roots: BTreeSet<Array> = arrays.iter().map(|array| find(&mut group_of, array)).collect();
            relief.push((worth, roots));
        }
    }
    // A loop's spills end only where all its arrays go far together. Take the
    // loops dearest first: one whose spills outweigh the DIM and ERASE calls
    // of the groups it would move sends them all.
    let mut saved: BTreeMap<Array, i64> = BTreeMap::new();
    for (array, at) in &frame {
        let root = find(&mut group_of, array);
        *saved.entry(root).or_default() += at.saturating_mul(room.dim);
    }
    relief.sort_by(|left, right| right.0.cmp(&left.0));
    let mut far_groups: BTreeSet<Array> = BTreeSet::new();
    for (worth, roots) in &relief {
        let moving: i64 = roots
            .iter()
            .filter(|root| !far_groups.contains(*root))
            .map(|root| saved.get(root).copied().unwrap_or(0))
            .sum();
        if *worth > moving {
            far_groups.extend(roots.iter().cloned());
        }
    }
    let mut kept = BTreeSet::new();
    for array in frame.keys() {
        if far_groups.contains(&find(&mut group_of, array)) {
            if let Array::Local(procedure, name) = array {
                kept.insert(format!("{procedure}/{name}"));
            }
        }
    }
    if std::env::var_os("LLRM_QB_PLACEMENT_TRACE").is_some() {
        eprintln!("placement: relief {relief:?} saved {saved:?} kept {kept:?}");
    }
    kept
}
