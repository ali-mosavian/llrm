//! `MemLocFragmentFill`: after a location of some bits of a variable, the
//! memory locations of its other bits are stated again.
//!
//! A location of a variable's bits `[32, 64)` ends the locations of the bits
//! outside them in a debugger. When a memory location covered those, this
//! pass adds a location for them after the new one, so that "the variable is
//! in memory" survives a store of one field.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use llrm_mir::{BlockId, DwOp, Function, InstId, MetadataId, Module, Operand};

use super::{
    DebugVariable, Fragment, FunctionVarLocsBuilder, InsertPt, VarLocInfo, fragment_of, variable_size_in_bits,
};

/// The bits of one variable that are in memory: half-open intervals, with the
/// base address (an id, 0 for no memory location) each is at. Adjacent
/// intervals of one base are one (`IntervalMap`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Frags(Vec<(u32, u32, u32)>);

impl Frags {
    fn overlaps(
        &self,
        start: u32,
        stop: u32,
    ) -> bool {
        self.0.iter().any(|&(a, b, _)| a < stop && b > start)
    }

    /// The interval holding `bit` (`IntervalMap::find` for a bit in one).
    fn at(
        &self,
        bit: u32,
    ) -> Option<(u32, u32, u32)> {
        self.0.iter().copied().find(|&(a, b, _)| a <= bit && bit < b)
    }

    /// Adds `[start, stop)`, joined to a neighbour with the same base; it
    /// must not overlap one.
    fn insert(
        &mut self,
        mut start: u32,
        mut stop: u32,
        base: u32,
    ) {
        if let Some(at) = self.0.iter().position(|&(_, b, v)| b == start && v == base) {
            start = self.0.remove(at).0;
        }
        if let Some(at) = self.0.iter().position(|&(a, _, v)| a == stop && v == base) {
            stop = self.0.remove(at).1;
        }
        let at = self.0.partition_point(|&(a, _, _)| a < start);
        self.0.insert(at, (start, stop, base));
    }

    /// `meetFragments`: the bits both have at the same nonzero base.
    fn meet(
        &self,
        other: &Frags,
    ) -> Frags {
        let mut result = Frags::default();
        for &(a, b, v) in &self.0 {
            if v == 0 {
                continue;
            }
            for &(c, d, w) in &other.0 {
                let (start, stop) = (a.max(c), b.min(d));
                if v == w && start < stop {
                    result.insert(start, stop, v);
                }
            }
        }
        result
    }
}

type VarFragMap = HashMap<MetadataId, Frags>;

/// `meetVars`: the variables both have, their bits met.
fn meet_vars(
    a: &mut VarFragMap,
    b: &VarFragMap,
) {
    a.retain(|variable, _| b.contains_key(variable));
    for (variable, frags) in a.iter_mut() {
        *frags = frags.meet(&b[variable]);
    }
}

/// `getDerefOffsetInBytes`: the offset of an expression that is a memory
/// location, `[plus_uconst n | constu n plus|minus] deref [fragment]`.
fn deref_offset_in_bytes(ops: &[DwOp]) -> Option<i64> {
    use llrm_mir::module::DwBinary::{Minus, Plus};
    let (offset, rest) = match ops {
        [DwOp::PlusUconst(n), rest @ ..] if !rest.is_empty() => (*n as i64, rest),
        [DwOp::Constu(n), DwOp::Binary(Plus), rest @ ..] if !rest.is_empty() => (*n as i64, rest),
        [DwOp::Constu(n), DwOp::Binary(Minus), rest @ ..] if !rest.is_empty() => (-(*n as i64), rest),
        [DwOp::Constu(_), ..] => return None,
        _ => (0, ops),
    };
    match rest {
        [DwOp::Deref] | [DwOp::Deref, DwOp::Fragment { .. }] => Some(offset),
        _ => None,
    }
}

/// What the pass adds before an insertion point: a variable's bits at a base.
struct FragMemLoc {
    variable: MetadataId,
    base: u32,
    offset: u32,
    size: u32,
    location: Option<MetadataId>,
}

struct Fill<'a> {
    module: &'a Module,
    with_slot: &'a HashSet<MetadataId>,
    coalesce: bool,
    /// The bases (the values a memory location is at), numbered from 1.
    bases: Vec<Vec<Operand>>,
    /// What each block adds, by point, in the order it was found; cleared
    /// each time the block is processed again.
    inserts: HashMap<BlockId, Vec<(InsertPt, FragMemLoc)>>,
}

impl Fill<'_> {
    fn base_of(
        &mut self,
        values: &[Operand],
    ) -> u32 {
        let at = self
            .bases
            .iter()
            .position(|one| one == values)
            .unwrap_or_else(
                || {
                    self.bases.push(values.to_vec());
                    self.bases.len() - 1
                },
            );
        at as u32 + 1
    }

    fn insert_mem_loc(
        &mut self,
        block: BlockId,
        before: InsertPt,
        variable: MetadataId,
        range: (u32, u32),
        base: u32,
        location: Option<MetadataId>,
    ) {
        if base == 0 {
            return;
        }
        let loc = FragMemLoc { variable, base, offset: range.0, size: range.1 - range.0, location };
        self.inserts.entry(block).or_default().push((before, loc));
    }

    /// `addDef`.
    fn add_def(
        &mut self,
        block: BlockId,
        before: InsertPt,
        def: &VarLocInfo,
        builder: &FunctionVarLocsBuilder,
        live: &mut VarFragMap,
    ) {
        let variable = builder.variable(def.variable).variable;
        let Some(size) = variable_size_in_bits(self.module, variable) else { return };
        if !self.with_slot.contains(&variable) {
            return;
        }
        let (start, end) = match fragment_of(&def.expression) {
            Some(Fragment { offset, size }) => (offset as u32, (offset + size) as u32),
            None => (0, size as u32),
        };
        // Only a plain memory location whose fragment starts where its offset
        // does is filled; any other is a value location as it stands.
        let base = match deref_offset_in_bytes(&def.expression) {
            Some(offset) if offset * 8 == i64::from(start) => self.base_of(&def.values),
            _ => 0,
        };
        let Some(frags) = live.get_mut(&variable) else {
            let mut frags = Frags::default();
            frags.insert(start, end, base);
            live.insert(variable, frags);
            return;
        };
        if !frags.overlaps(start, end) {
            frags.insert(start, end, base);
            self.coalesced(block, before, variable, (start, end), base, def.location, frags);
            return;
        }
        // The intervals the new one cuts are shortened, and what stands
        // outside it is stated again; those inside it are gone.
        let overlapping: Vec<(u32, u32, u32)> =
            frags.0.iter().copied().filter(|&(a, b, _)| a < end && b > start).collect();
        frags.0.retain(|&(a, b, _)| !(a < end && b > start));
        for (a, b, v) in overlapping {
            if a < start {
                frags.insert(a, start, v);
                self.insert_mem_loc(block, before, variable, (a, start), v, def.location);
            }
            if b > end {
                frags.insert(end, b, v);
                self.insert_mem_loc(block, before, variable, (end, b), v, def.location);
            }
        }
        frags.insert(start, end, base);
        self.coalesced(block, before, variable, (start, end), base, def.location, frags);
    }

    /// `coalesceFragments`: where the new interval joined its neighbours, the
    /// joined one is stated.
    #[allow(clippy::too_many_arguments)]
    fn coalesced(
        &mut self,
        block: BlockId,
        before: InsertPt,
        variable: MetadataId,
        range: (u32, u32),
        base: u32,
        location: Option<MetadataId>,
        frags: &Frags,
    ) {
        if !self.coalesce {
            return;
        }
        let Some((a, b, _)) = frags.at(range.0) else { return };
        if (a, b) != range {
            self.insert_mem_loc(block, before, variable, (a, b), base, location);
        }
    }
}

/// Adds to `builder` the locations that state the memory locations again
/// (`MemLocFragmentFill::run`). `slots` is the number of records standing
/// before an instruction.
///
/// Complexity: the pin's: blocks again while a state changes (the states only
/// shrink), each pass O(wedges x intervals); the interval map is a sorted list
/// where the pin has a B-tree, so O(k) per operation for k intervals of a
/// variable, which a variable keeps few of.
pub(super) fn run(
    module: &Module,
    function: &Function,
    builder: &mut FunctionVarLocsBuilder,
    with_slot: &HashSet<MetadataId>,
    slots: &dyn Fn(InstId) -> u32,
    coalesce: bool,
) {
    let mut fill = Fill { module, with_slot, coalesce, bases: Vec::new(), inserts: HashMap::new() };
    let order = llrm_mir::blockorder::reverse_post_order_traversal(function);
    let number: HashMap<BlockId, usize> = order.iter().enumerate().map(|(at, &b)| (b, at)).collect();
    let mut live_in: HashMap<BlockId, VarFragMap> = HashMap::new();
    let mut live_out: HashMap<BlockId, VarFragMap> = HashMap::new();
    let mut visited: HashSet<BlockId> = HashSet::new();
    let mut worklist: BTreeSet<usize> = (0..order.len()).collect();
    let mut pending: BTreeSet<usize> = BTreeSet::new();
    while !worklist.is_empty() {
        while let Some(at) = worklist.pop_first() {
            let block = order[at];
            let mut changed = meet(function, block, &visited, &mut live_in, &live_out);
            changed |= visited.insert(block);
            if !changed {
                continue;
            }
            let mut live = live_in[&block].clone();
            fill.inserts.remove(&block);
            for inst in function.iter_block(block) {
                for slot in 0..=slots(inst) {
                    let Some(defs) = builder.wedge((inst, slot)).map(<[VarLocInfo]>::to_vec) else { continue };
                    for def in &defs {
                        fill.add_def(block, (inst, slot), def, builder, &mut live);
                    }
                }
            }
            if live_out.get(&block) != Some(&live) {
                live_out.insert(block, live);
                for successor in function.successors(block).iter() {
                    if let Some(&n) = number.get(successor) {
                        pending.insert(n);
                    }
                }
            }
        }
        std::mem::swap(&mut worklist, &mut pending);
    }
    let mut added: BTreeMap<usize, Vec<(InsertPt, FragMemLoc)>> = BTreeMap::new();
    for (block, list) in fill.inserts.drain() {
        added.insert(number[&block], list);
    }
    for (_, list) in added {
        for (at, loc) in list {
            let size = variable_size_in_bits(module, loc.variable).unwrap_or(0);
            let mut ops = Vec::new();
            let fragment = (u64::from(loc.size) != size)
                .then_some(Fragment { offset: u64::from(loc.offset), size: u64::from(loc.size) });
            if loc.offset != 0 {
                ops.push(DwOp::PlusUconst(u64::from(loc.offset / 8)));
            }
            ops.push(DwOp::Deref);
            if let Some(fragment) = fragment {
                ops.push(DwOp::Fragment { offset: fragment.offset, size: fragment.size });
            }
            let variable = DebugVariable { variable: loc.variable, fragment, inlined_at: None };
            builder.add_var_loc(at, variable, ops, loc.location, fill.bases[loc.base as usize - 1].clone());
        }
    }
}

/// `meet`: the state at the top of `block` from its predecessors visited so
/// far; whether it changed.
fn meet(
    function: &Function,
    block: BlockId,
    visited: &HashSet<BlockId>,
    live_in: &mut HashMap<BlockId, VarFragMap>,
    live_out: &HashMap<BlockId, VarFragMap>,
) -> bool {
    let mut joined = VarFragMap::new();
    let mut first = true;
    for pred in function.predecessors(block).iter().filter(|p| visited.contains(p)) {
        if first {
            joined = live_out[pred].clone();
            first = false;
        } else {
            meet_vars(&mut joined, &live_out[pred]);
        }
        if joined.is_empty() {
            break;
        }
    }
    if live_in.get(&block) == Some(&joined) {
        return false;
    }
    live_in.insert(block, joined);
    true
}
