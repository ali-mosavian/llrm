//! Adjacent stores become wider ones: LLVM's
//! `DAGCombiner::mergeConsecutiveStores` for stores of constants
//! (`tryStoreMergeOfConstants`, `merged_constants`) and of loaded values
//! (`tryStoreMergeOfLoads`, `merged_copies`).
//!
//! # How it works
//! Constants: stores of integer constants to adjacent cells of one object are
//! one store of their bytes, little-endian, when the bytes are a legal integer
//! (`getConsecutiveStores` finds the run of cells of one width, the widest
//! legal prefix is taken, and a store made is a candidate again, so a byte pair
//! and a word beside it become a dword). A load of any of the cells, another
//! store, a call or a volatile store ends the span; stores that overlap leave
//! the object alone.
//!
//! Loads:
//! In straight-line code, a store of a value another instruction loaded, the
//! only use of that load, is a copy of a cell. Copies of adjacent cells to
//! adjacent cells, `a[0] = b[0]; a[1] = b[1]`, are one wider load and one wider
//! store when the bytes they make are a legal integer (`getConsecutiveStores`
//! finds the run, the widest legal prefix is taken, the rest is tried again):
//!
//! ```text
//!   %x = load i16, ptr %b            %w = load i32, ptr %b
//!   store i16 %x, ptr %a      =>     store i32 %w, ptr %a
//!   %y = load i16, ptr %b2
//!   store i16 %y, ptr %a2
//! ```
//!
//! Between the first and the last of the copies of a span nothing else may
//! touch memory (a call, a store of anything else, a load whose value goes
//! elsewhere ends the span), and no object may be read in the span that is also
//! written in it, so the wider load and store can stand anywhere among the
//! narrow ones. The wider load goes before the load of the lowest address and
//! the wider store before the last of the narrow stores.
//!
//! DEVIATE M1 (no number): the DAG finds candidates through a shared chain root
//! and checks cycles through operands (`getStoreMergeCandidates`,
//! `checkMergeStoreCandidatesForDependencies`); MIR has no chains, so a span of
//! straight-line code with no other memory operation, in objects that are told
//! apart (`MemRef::object`), is the same fact. Exit: none; MemorySSA could
//! widen the span.
//!
//! DEVIATE M3 (pinned by `constants_that_cannot_be_one_wider_store_are_kept`):
//! a wider constant store starts at a multiple of its size. LLVM's
//! `allowsMemoryAccess` calls any x86 access fast.
//!
//! DEVIATE M4 (rows, `sum_three -Os`): tuned for size, a merge is made only
//! where the one store is no more code bytes than the ones it replaces, a
//! constant stored more than once being held in a register (a dword immediate
//! with its operand-size prefix in 16-bit code is more than two register
//! stores). LLVM has no such price.
//!
//! DEVIATE M2 (no number): no vector types, no rotate of a reversed pair, no
//! truncating store; the target has none of them for integers here
//! (`TLI.isTypeLegal` is `largest_legal_integer`), and a load or store of
//! any alignment is fast on x86.
//!
//! # Example
//! A struct of two words copied field by field in 16-bit code is a dword load
//! and a dword store.
//!
//! # Runs
//! In `loopidioms::Merge`, after the scalar passes have settled.
//!
//! # Used by
//! `loopidioms::Merge`.

use llrm_analysis::memory::{MemRef, Unit};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_support::hash::IndexMap;

use crate::profit::OperationCosts;

/// One copy of a cell: a load whose only use is a store, and where each reads
/// and writes.
#[derive(Clone)]
struct Pair {
    load: InstId,
    store: InstId,
    /// The store's place in the block.
    store_order: usize,
    src: Operand,
    dst: Operand,
    src_pointer: Operand,
    dst_pointer: Operand,
    src_low: i64,
    dst_low: i64,
    /// Bytes copied.
    width: i64,
}

/// The cell `pointer` names: its object and the displacement in it.
fn cell(
    unit: &Unit,
    pointer: Operand,
    width: u32,
) -> Option<(Operand, i64)> {
    let reference = MemRef::at(unit, pointer, width);
    let root =
        reference.root.filter(|_| reference.object && reference.base.is_none() && reference.segment.is_none())?;
    Some((root, reference.disp))
}

/// The wider copies the span's pairs make: runs of adjacent cells of one
/// width, each cut into the widest legal integers.
fn chunks(
    unit: &Unit,
    span: &[Pair],
    reads: &[Operand],
    legal: u32,
) -> Vec<Vec<Pair>> {
    // An object read in the span that is also written in it: the wider copy
    // could not stand anywhere among the narrow ones.
    if span.iter().any(|one| reads.contains(&one.dst)) {
        return Vec::new();
    }
    let mut groups: Vec<Vec<Pair>> = Vec::new();
    for pair in span {
        let space = (unit.space(pair.src_pointer), unit.space(pair.dst_pointer));
        match groups.iter_mut().find(|group| {
            group[0].src == pair.src
                && group[0].dst == pair.dst
                && (unit.space(group[0].src_pointer), unit.space(group[0].dst_pointer)) == space
        }) {
            Some(group) => group.push(pair.clone()),
            None => groups.push(vec![pair.clone()]),
        }
    }
    let mut made = Vec::new();
    for mut group in groups {
        group.sort_by_key(|one| one.dst_low);
        // Two stores to one byte keep their order: leave the object alone.
        if group.windows(2).any(|two| two[1].dst_low < two[0].dst_low + two[0].width) {
            continue;
        }
        let mut run: Vec<Pair> = Vec::new();
        let mut cut = |run: &mut Vec<Pair>| {
            let mut rest = std::mem::take(run);
            while rest.len() >= 2 {
                let width = rest[0].width as u32 * 8;
                // The widest prefix whose bytes are a legal integer.
                let take = (2..=rest.len())
                    .rev()
                    .find(|&count| matches!(width * count as u32, 16 | 32 | 64) && width * count as u32 <= legal);
                let Some(take) = take else {
                    rest.remove(0);
                    continue;
                };
                let tail = rest.split_off(take);
                made.push(std::mem::replace(&mut rest, tail));
            }
        };
        for pair in group {
            if run.last().is_some_and(|last| {
                last.dst_low + last.width != pair.dst_low
                    || last.src_low + last.width != pair.src_low
                    || last.width != pair.width
            }) {
                cut(&mut run);
            }
            run.push(pair);
        }
        cut(&mut run);
    }
    made
}

/// A run of copies of adjacent cells that `llvm.memcpy` stands for.
struct Call {
    destination: Operand,
    source: Operand,
    to: u32,
    from: u32,
    width: u32,
    length: i64,
    /// The last store of the run: the call stands there.
    last: InstId,
    stores: Vec<InstId>,
    loads: Vec<InstId>,
}

/// DEVIATE M5 (rows, `copy_runs -Os`, +163 bytes at m16): tuned for size, a run
/// of copies of adjacent cells that `rep movs` does in fewer code bytes than
/// the loads and stores `MEMCPY_MOVES` makes of it is `llvm.memcpy`. The pin
/// merges the run into dword copies and stops there (24 pairs for 96 bytes).
/// The calls made, and the pairs left.
fn string_runs(
    unit: &Unit,
    span: &mut Vec<Pair>,
    reads: &[Operand],
    costs: &OperationCosts,
) -> (Vec<Call>, Vec<Pair>) {
    let kept = std::mem::take(span);
    if kept.iter().any(|one| reads.contains(&one.dst)) {
        return (Vec::new(), kept);
    }
    let mut groups: Vec<Vec<Pair>> = Vec::new();
    for pair in &kept {
        let space = (unit.space(pair.src_pointer), unit.space(pair.dst_pointer));
        match groups.iter_mut().find(|group| {
            group[0].src == pair.src
                && group[0].dst == pair.dst
                && (unit.space(group[0].src_pointer), unit.space(group[0].dst_pointer)) == space
        }) {
            Some(group) => group.push(pair.clone()),
            None => groups.push(vec![pair.clone()]),
        }
    }
    let mut calls = Vec::new();
    let mut taken: Vec<InstId> = Vec::new();
    for mut group in groups {
        group.sort_by_key(|one| one.dst_low);
        if group.windows(2).any(|two| two[1].dst_low < two[0].dst_low + two[0].width) {
            continue;
        }
        let mut at = 0;
        while at < group.len() {
            let mut end = at + 1;
            while end < group.len()
                && group[end].dst_low == group[end - 1].dst_low + group[end - 1].width
                && group[end].src_low == group[end - 1].src_low + group[end - 1].width
                && group[end].width == group[end - 1].width
            {
                end += 1;
            }
            let run = &group[at..end];
            at = end;
            let length: i64 = run.iter().map(|one| one.width).sum();
            let pairs = length / 4 + i64::from((length % 4).count_ones());
            if pairs <= 8 || crate::loopidioms::copy_price(costs, length, false) >= pairs * (costs.load + costs.store) {
                continue;
            }
            let (Some(to), Some(from)) = (unit.space(run[0].dst_pointer), unit.space(run[0].src_pointer)) else {
                continue;
            };
            let last = run.iter().max_by_key(|one| one.store_order).expect("a run has copies");
            taken.extend(run.iter().map(|one| one.store));
            calls.push(Call {
                destination: run[0].dst_pointer,
                source: run[0].src_pointer,
                to,
                from,
                width: unit.layout.pointer(to).index_bits,
                length,
                last: last.store,
                stores: run.iter().map(|one| one.store).collect(),
                loads: run.iter().map(|one| one.load).collect(),
            });
        }
    }
    let left = kept.into_iter().filter(|one| !taken.contains(&one.store)).collect();
    (calls, left)
}

/// `function` with each run of copies of adjacent cells to adjacent cells made
/// one wider copy; whether any was.
pub fn merged_copies(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    runs: Option<(&mut Declared, &OperationCosts)>,
) -> bool {
    let (mut declared, run_costs) = match runs {
        Some((declared, costs)) => (Some(declared), Some(costs)),
        None => (None, None),
    };
    let legal = layout.largest_legal_integer();
    let mut plan: Vec<Vec<Pair>> = Vec::new();
    let mut calls: Vec<Call> = Vec::new();
    {
        let unit = Unit::within(context, layout, function, outer);
        // The loads of the span, by result: one map for the function, emptied
        // by taking out what the span put in.
        let mut loads: IdMap<ValueId, InstId> = IdMap::default();
        let mut put: Vec<ValueId> = Vec::new();
        for block in function.layout() {
            let mut span: Vec<Pair> = Vec::new();
            let mut reads: Vec<Operand> = Vec::new();
            let mut flush = |span: &mut Vec<Pair>,
                             reads: &mut Vec<Operand>,
                             loads: &mut IdMap<ValueId, InstId>,
                             put: &mut Vec<ValueId>| {
                let kept = match run_costs {
                    Some(costs) => {
                        let (taken, kept) = string_runs(&unit, span, reads, costs);
                        calls.extend(taken);
                        kept
                    }
                    None => std::mem::take(span),
                };
                plan.extend(chunks(&unit, &kept, reads, legal));
                span.clear();
                reads.clear();
                for value in put.drain(..) {
                    loads.remove(&value);
                }
            };
            for (order, inst) in function.iter_block(*block).enumerate() {
                let op = function.instruction(inst);
                match op.opcode {
                    Opcode::Load { volatile: false, .. } => {
                        let result = op.result.expect("a load's value");
                        let width = unit.int_bits(Operand::Value(result)).filter(|bits| matches!(bits, 8 | 16 | 32));
                        let users = function.users(result);
                        let single_store = users
                            .only()
                            .is_some_and(
                                |only| matches!(
                                    function.instruction(only.user).opcode,
                                    Opcode::Store { volatile: false, .. }
                                )
                                    && only.index == 0,
                            );
                        match (width, single_store, width.and_then(|bits| cell(&unit, op.operands[0], bits / 8))) {
                            (Some(_), true, Some((root, _))) => {
                                reads.push(root);
                                loads.insert(result, inst);
                                put.push(result);
                            }
                            _ => flush(&mut span, &mut reads, &mut loads, &mut put),
                        }
                    }
                    Opcode::Store { volatile: false, .. } => {
                        let paired = match op.operands[0] {
                            Operand::Value(value) => loads.get(&value).copied().map(|load| (value, load)),
                            _ => None,
                        };
                        let Some((value, load)) = paired else {
                            flush(&mut span, &mut reads, &mut loads, &mut put);
                            continue;
                        };
                        let bits = unit.int_bits(Operand::Value(value)).expect("an integer");
                        let width = bits / 8;
                        let source = function.instruction(load).operands[0];
                        let (Some((src, src_low)), Some((dst, dst_low))) =
                            (cell(&unit, source, width), cell(&unit, op.operands[1], width))
                        else {
                            flush(&mut span, &mut reads, &mut loads, &mut put);
                            continue;
                        };
                        span.push(Pair {
                            load,
                            store: inst,
                            store_order: order,
                            src,
                            dst,
                            src_pointer: source,
                            dst_pointer: op.operands[1],
                            src_low,
                            dst_low,
                            width: i64::from(width),
                        });
                    }
                    _ if memory::speculatable(unit.context, callees, function, inst) => {}
                    _ => flush(&mut span, &mut reads, &mut loads, &mut put),
                }
            }
            flush(&mut span, &mut reads, &mut loads, &mut put);
        }
    }
    for call in &calls {
        let declared = declared.as_mut().expect("calls come with a declaration set");
        let (callee, function_type) =
            crate::loopidioms::_copy(context, declared, crate::loopidioms::How::Apart, call.to, call.from, call.width);
        let length = crate::counting::constant(context, &llrm_analysis::num::Num::from(call.length), call.width);
        let pointer = context.types.ptr(0);
        let callee = Operand::Constant(context.constant(llrm_mir::context::Constant {
            ty: pointer,
            kind: llrm_mir::context::ConstantKind::Global(callee),
        }));
        let off = crate::counting::constant(context, &llrm_analysis::num::Num::from(0), 1);
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 4],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let void = context.types.void();
        let made = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![call.destination, call.source, length, off, callee],
            Flags::default(),
            None,
        );
        function.insert(made, Position::Before(call.last)).expect("a placed store");
        for &store in &call.stores {
            function.erase(store).expect("a store has no result");
        }
        for &load in &call.loads {
            function.erase(load).expect("a load with no use left");
        }
    }
    for chunk in &plan {
        let bits = (chunk[0].width * chunk.len() as i64 * 8) as u32;
        let ty = context.types.int(bits);
        let lowest = chunk.iter().min_by_key(|one| one.dst_low).expect("a run has copies");
        let last = chunk.iter().max_by_key(|one| one.store_order).expect("a run has copies");
        let load = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            ty,
            vec![lowest.src_pointer],
            Flags::default(),
            None,
        );
        function.insert(load, Position::Before(lowest.load)).expect("a placed load");
        let void = context.types.void();
        let wide = function.instruction(load).result.expect("a load's value");
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![Operand::Value(wide), lowest.dst_pointer],
            Flags::default(),
            None,
        );
        function.insert(store, Position::Before(last.store)).expect("a placed store");
        for one in chunk {
            function.erase(one.store).expect("a store has no result");
        }
        for one in chunk {
            function.erase(one.load).expect("a load with no use left");
        }
    }
    !(plan.is_empty() && calls.is_empty())
}

/// One store of a constant a merge may take.
#[derive(Clone)]
struct Constant {
    /// The stores it stands for, in the order they were found.
    stores: Vec<InstId>,
    /// The last of them in the block, and its place there.
    last: InstId,
    order: usize,
    root: Operand,
    low: i64,
    high: i64,
    value: u128,
    /// The pointer of the lowest cell.
    pointer: Operand,
}

/// A store of an integer constant to a cell of an object.
fn constant(
    unit: &Unit,
    inst: InstId,
    order: usize,
) -> Option<Constant> {
    let op = unit.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let (value, pointer) = (op.operands[0], op.operands[1]);
    let bits = unit.int_bits(value).filter(|bits| matches!(bits, 8 | 16 | 32))?;
    let value = unit.int_constant(value)?;
    let (root, low) = cell(unit, pointer, bits / 8)?;
    Some(Constant {
        stores: vec![inst],
        last: inst,
        order,
        root,
        low,
        high: low + i64::from(bits / 8),
        value: value & ((1u128 << bits) - 1),
        pointer,
    })
}

/// Whether one store of `wide` bytes is no more code bytes than `cells`'
/// stores: `costs.store` is a store of an immediate of the operand size, so a
/// register store (the form of a constant stored more than once in the
/// function, which a register holds) is that less the operand, and a store of
/// `width` bytes of immediate has them instead, each under the operand-size
/// prefix where its width is not the operand's.
fn smaller(
    unit: &Unit,
    costs: &OperationCosts,
    cells: &[Constant],
    wide: i64,
    stored: &IndexMap<(u128, u32), usize>,
) -> bool {
    let Some(space) = unit.space(cells[0].pointer) else { return false };
    let operand = i64::from(unit.layout.pointer(space).index_bits / 8);
    let register = |width: i64| costs.sized(costs.store - operand, width, operand);
    let immediate = |width: i64| register(width) + width;
    let each: i64 = cells
        .iter()
        .map(|one| {
            let width = one.high - one.low;
            let held = stored.get(&(one.value, width as u32)).is_some_and(|&count| count > 1);
            if held { register(width) } else { immediate(width) }
        })
        .sum();
    immediate(wide) <= each
}

/// The cells of `group` (one object, by address, none overlapping) after every
/// merge `getConsecutiveStores` and `tryStoreMergeOfConstants` make of them: a
/// run of adjacent cells of one width is cut into the widest legal stores that
/// start at a multiple of their size, and the stores made are candidates again.
fn constants_merged(
    unit: &Unit,
    group: Vec<Constant>,
    legal: u32,
    stored: &IndexMap<(u128, u32), usize>,
    costs: &OperationCosts,
    size: bool,
) -> Vec<Constant> {
    let mut cells = group;
    loop {
        let mut changed = false;
        let mut at = 0;
        while at + 1 < cells.len() {
            let width = cells[at].high - cells[at].low;
            // The run of adjacent cells of this width from `at`.
            let mut run = 1;
            while at + run < cells.len()
                && cells[at + run].low == cells[at + run - 1].high
                && cells[at + run].high - cells[at + run].low == width
            {
                run += 1;
            }
            // The most cells whose bytes are a legal integer, aligned to their
            // size.
            let take = (2..=run)
                .rev()
                .find(
                    |&count| {
                        let bits = width * count as i64 * 8;
                        matches!(bits, 16 | 32 | 64)
                            && bits <= i64::from(legal)
                            && cells[at].low % (width * count as i64) == 0
                            && (!size || smaller(unit, costs, &cells[at..at + count], width * count as i64, stored))
                    },
                );
            let Some(take) = take else {
                at += 1;
                continue;
            };
            let low = cells[at].low;
            let mut value = 0u128;
            let mut stores = Vec::new();
            for one in &cells[at..at + take] {
                value |= one.value << (8 * (one.low - low) as u32);
                stores.extend(one.stores.iter().copied());
            }
            let latest = cells[at..at + take].iter().max_by_key(|one| one.order).expect("a run has cells");
            let (last, order) = (latest.last, latest.order);
            let merged = Constant {
                stores,
                last,
                order,
                root: cells[at].root,
                low,
                high: cells[at + take - 1].high,
                value,
                pointer: cells[at].pointer,
            };
            cells.splice(at..at + take, [merged]);
            changed = true;
            at += 1;
        }
        if !changed {
            return cells;
        }
    }
}

/// `function` with each run of stores of constants to adjacent cells of one
/// object made stores of the widest legal integer; whether any was.
pub fn merged_constants(
    context: &Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    size: bool,
) -> bool {
    let legal = layout.largest_legal_integer();
    let costs = outer.target().size_costs();
    let mut plan: Vec<Constant> = Vec::new();
    {
        let unit = Unit::within(context, layout, function, outer);
        // How many stores of the function write each constant: a value stored
        // more than once is held in a register, which a store reads without an
        // immediate.
        let mut stored: IndexMap<(u128, u32), usize> = IndexMap::default();
        let mut spans: Vec<Vec<Constant>> = Vec::new();
        for block in function.layout() {
            let mut open: Vec<Constant> = Vec::new();
            for (order, inst) in function.iter_block(*block).enumerate() {
                if let Some(one) = constant(&unit, inst, order) {
                    *stored.entry((one.value, (one.high - one.low) as u32)).or_default() += 1;
                    open.push(one);
                } else if !memory::speculatable(unit.context, callees, function, inst)
                    || matches!(function.instruction(inst).opcode, Opcode::Store { .. })
                {
                    spans.push(std::mem::take(&mut open));
                }
            }
            spans.push(open);
        }
        for span in spans {
            // One object and address space at a time, by address.
            let mut groups: Vec<Vec<Constant>> = Vec::new();
            for one in span {
                let space = unit.space(one.pointer);
                match groups.iter_mut().find(|group| group[0].root == one.root && unit.space(group[0].pointer) == space)
                {
                    Some(group) => group.push(one),
                    None => groups.push(vec![one]),
                }
            }
            for mut group in groups {
                group.sort_by_key(|one| one.low);
                // Two stores to one byte keep their order: leave the object
                // alone.
                if group.windows(2).any(|two| two[1].low < two[0].high) {
                    continue;
                }
                plan.extend(
                    constants_merged(&unit, group, legal, &stored, &costs, size)
                        .into_iter()
                        .filter(|one| one.stores.len() > 1),
                );
            }
        }
    }
    for wide in &plan {
        let bits = (8 * (wide.high - wide.low)) as u32;
        let value = crate::counting::constant(context, &llrm_analysis::num::Num::from(wide.value), bits);
        let void = context.types.void();
        let last = wide.last;
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![value, wide.pointer],
            Flags::default(),
            None,
        );
        function.insert(store, Position::Before(last)).expect("a placed store");
        for one in &wide.stores {
            function.erase(*one).expect("a store has no result");
        }
    }
    !plan.is_empty()
}

#[cfg(test)]
mod tests {
    use llrm_analysis::testing::{DOS, layout};
    use llrm_mir::passes::Outer;

    use super::merged_copies;
    use crate::testing::{parsed, printed, results};

    /// `text` with its copies merged: the printed form, and whether any were.
    /// The function computes the same on `inputs` before and after.
    fn merge(
        text: &str,
        inputs: &[&[i128]],
    ) -> (String, bool) {
        let before = parsed(&format!("{DOS}{text}"));
        let mut module = before.clone();
        let (layout, outer) =
            (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
        let callees = llrm_mir::memory::callees(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let changed = merged_copies(context, &layout, &callees, function, &outer, None);
        let after = printed(&module);
        assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
        (after, changed)
    }

    const INPUTS: &[&[i128]] = &[&[0], &[1]];

    /// Two word fields copied one by one from `@b` to `@a`.
    fn two_words(order: [usize; 2]) -> String {
        let steps = order
            .iter()
            .map(|at| {
                format!(
                    "  %s{at} = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 {at}\n  %d{at} = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 {at}\n  %v{at} = load i16, ptr %s{at}\n  store i16 %v{at}, ptr %d{at}\n"
                )
            })
            .collect::<String>();
        format!(
            "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {{\n{steps}  %r0 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 0\n  %r1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %a0 = load i16, ptr %r0\n  %a1 = load i16, ptr %r1\n  %t = shl i16 %a1, 8\n  %u = add i16 %a0, %t\n  %w = add i16 %u, %x\n  ret i16 %w\n}}\n"
        )
    }

    /// `a[0] = b[0]; a[1] = b[1]` is a dword load and a dword store.
    /// 24 word copies from `@b` to `@a`, with `size` the pass's mode: the
    /// printed form. Priced in code bytes on the DOS target.
    fn forty_eight_bytes(size: bool) -> String {
        let steps = (0..24)
            .map(|at| {
                format!(
                    "  %s{at} = getelementptr inbounds [24 x i16], ptr @b, i16 0, i16 {at}\n  %d{at} = getelementptr inbounds [24 x i16], ptr @a, i16 0, i16 {at}\n  %v{at} = load i16, ptr %s{at}\n  store i16 %v{at}, ptr %d{at}\n"
                )
            })
            .collect::<String>();
        let text = format!(
            "@a = global [24 x i16] zeroinitializer\n@b = global [24 x i16] zeroinitializer\n\ndefine i16 @f(i16 %q) {{\n{steps}  ret i16 %q\n}}\n"
        );
        let before = parsed(&format!("{DOS}{text}"));
        let mut module = before.clone();
        let machine = std::rc::Rc::new(llrm_x86_m16::Dos::default());
        let (layout, outer) = (layout(&module), Outer::of(&module, Some(machine.clone())));
        let callees = llrm_mir::memory::callees(&module);
        let bytes = llrm_mir::target::Machine::size_costs(&*machine);
        let mut declared = llrm_mir::passes::Declared::default();
        let (context, function) = module.function_mut("f").expect("@f");
        let runs = size.then_some((&mut declared, &bytes));
        merged_copies(context, &layout, &callees, function, &outer, runs);
        printed(&module)
    }

    /// 96 bytes of word copies were 24 dword loads and stores (copy_runs -Os
    /// m16: 501 bytes against main's 338); tuned for size they are one
    /// `llvm.memcpy`, which isel makes `rep movsd`. Tuned for speed they stay
    /// the pin's dword copies.
    #[test]
    fn test_a_long_run_of_copies_is_a_memcpy_tuned_for_size() {
        let size = forty_eight_bytes(true);
        // The declaration is added once the pass has run: the callee prints
        // unnamed.
        assert!(size.contains("(ptr %d0, ptr %s0, i16 48, i1 false)"), "{size}");
        assert_eq!(size.matches("load").count(), 0, "{size}");
        let speed = forty_eight_bytes(false);
        assert!(!speed.contains("i16 48"), "{speed}");
        assert_eq!(speed.matches("load i32").count(), 12, "{speed}");
    }

    #[test]
    fn test_two_adjacent_word_copies_are_one_dword_copy() {
        let (after, changed) = merge(&two_words([0, 1]), INPUTS);
        assert!(changed, "{after}");
        assert!(after.contains("load i32") && after.contains("store i32"), "{after}");
        assert_eq!(after.matches("store").count(), 1, "{after}");
    }

    /// The copies are found by address, whichever is written first.
    #[test]
    fn test_the_copies_are_merged_whichever_is_written_first() {
        let (after, changed) = merge(&two_words([1, 0]), INPUTS);
        assert!(changed && after.contains("store i32"), "{after}");
    }

    /// A call between the copies may read or write either cell: nothing is
    /// merged.
    #[test]
    fn test_a_call_between_the_copies_keeps_them_apart() {
        let text = "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\ndefine void @g() {\n  store i16 5, ptr @b\n  ret void\n}\n\ndefine i16 @f(i16 %x) {\n  %s1 = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 1\n  %d1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %v0 = load i16, ptr @b\n  store i16 %v0, ptr @a\n  call void @g()\n  %v1 = load i16, ptr %s1\n  store i16 %v1, ptr %d1\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed && !after.contains("load i32"), "{after}");
    }

    /// Cells of one object copied within it (`a[1] = a[0]`) may overlap what is
    /// read: nothing is merged.
    #[test]
    fn test_copies_within_one_object_are_kept() {
        let text = "@a = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {\n  %p1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %p2 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 2\n  %p3 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 3\n  %v0 = load i16, ptr @a\n  store i16 %v0, ptr %p2\n  %v1 = load i16, ptr %p1\n  store i16 %v1, ptr %p3\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed, "{after}");
    }

    /// Cells that are not next to each other are not one copy.
    #[test]
    fn test_copies_with_a_gap_between_the_cells_are_kept() {
        let text = "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {\n  %s2 = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 2\n  %d2 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 2\n  %v0 = load i16, ptr @b\n  store i16 %v0, ptr @a\n  %v2 = load i16, ptr %s2\n  store i16 %v2, ptr %d2\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed, "{after}");
    }

    /// Four byte copies are one dword copy (the widest legal integer), not two
    /// words.
    #[test]
    fn test_four_byte_copies_are_one_dword_copy() {
        let steps: String = (0..4)
            .map(|at| {
                format!(
                    "  %s{at} = getelementptr inbounds [8 x i8], ptr @b, i16 0, i16 {at}\n  %d{at} = getelementptr inbounds [8 x i8], ptr @a, i16 0, i16 {at}\n  %v{at} = load i8, ptr %s{at}\n  store i8 %v{at}, ptr %d{at}\n"
                )
            })
            .collect();
        let text = format!(
            "@a = global [8 x i8] zeroinitializer\n@b = global [8 x i8] [i8 1, i8 2, i8 3, i8 4, i8 5, i8 6, i8 7, i8 8]\n\ndefine i8 @f(i8 %x) {{\n{steps}  %p = getelementptr inbounds [8 x i8], ptr @a, i16 0, i16 3\n  %r = load i8, ptr %p\n  %w = add i8 %r, %x\n  ret i8 %w\n}}\n"
        );
        let (after, changed) = merge(&text, &[&[0], &[3]]);
        assert!(changed && after.contains("load i32") && !after.contains("store i8"), "{after}");
        assert_eq!(after.matches("store i32").count(), 1, "{after}");
    }
}

#[cfg(test)]
mod constant_tests {
    use llrm_analysis::testing::{DOS, layout};
    use llrm_mir::passes::Outer;

    use super::merged_constants;
    use crate::testing::{parsed, printed, results};

    fn merge(
        body: &str,
        size: bool,
    ) -> (String, bool) {
        let text = format!(
            "@s = global [8 x i8] zeroinitializer\n\ndefine i16 @f(i16 %x, i16 %q) {{\n  %p1 = getelementptr inbounds [8 x i8], ptr @s, i16 0, i16 1\n  %p2 = getelementptr inbounds [8 x i8], ptr @s, i16 0, i16 2\n  %p3 = getelementptr inbounds [8 x i8], ptr @s, i16 0, i16 3\n  %p4 = getelementptr inbounds [8 x i8], ptr @s, i16 0, i16 4\n{body}  %r = getelementptr inbounds [8 x i8], ptr @s, i16 0, i16 %q\n  %v = load i8, ptr %r\n  %w = zext i8 %v to i16\n  ret i16 %w\n}}\n"
        );
        let before = parsed(&format!("{DOS}{text}"));
        let mut module = before.clone();
        let (layout, outer) =
            (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
        let callees = llrm_mir::memory::callees(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let changed = merged_constants(context, &layout, &callees, function, &outer, size);
        let after = printed(&module);
        let inputs: &[&[i128]] = &[&[0, 0], &[0, 1], &[0, 2], &[0, 3], &[0, 4], &[0, 5]];
        assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
        (after, changed)
    }

    /// A byte pair and a word beside it are a dword: the store made from the
    /// pair is a candidate again.
    #[test]
    fn test_a_byte_pair_and_a_word_beside_it_are_a_dword() {
        let (after, changed) = merge("  store i8 1, ptr @s\n  store i8 2, ptr %p1\n  store i16 3, ptr %p2\n", false);
        assert!(
            changed && after.contains("store i32 197121, ptr @s") && after.matches("store").count() == 1,
            "{after}"
        );
    }

    /// A packed `{ a:8, b:32 }`: `a` is the byte at 0 and `b` the four at 1.
    /// The bytes are five, not a legal integer, and `b += 5` reads the four and
    /// writes them again: no store is merged with another, none is made wider
    /// across the read.
    #[test]
    fn test_packed_bitfield_stores_with_a_read_modify_write_between_are_kept() {
        let body = "  store i8 -56, ptr @s\n  store i32 7, ptr %p1\n  %b = load i32, ptr %p1\n  %c = add i32 %b, 5\n  store i32 %c, ptr %p1\n";
        let (after, changed) = merge(body, false);
        assert!(!changed, "{after}");
        assert_eq!(after.matches("store").count(), 3, "{after}");
    }

    /// The same two stores with only a partial overwrite after them: `b`'s low
    /// byte is stored again, so the dword store at 1 and the byte at 1 overlap:
    /// the object is left alone.
    #[test]
    fn test_a_partial_overwrite_of_a_merged_cell_leaves_the_object_alone() {
        let body = "  store i8 1, ptr @s\n  store i8 2, ptr %p1\n  store i8 9, ptr %p1\n";
        let (after, changed) = merge(body, false);
        assert!(!changed, "{after}");
    }

    /// A dword whose first byte is not a multiple of four (M3).
    #[test]
    fn test_a_pair_that_would_start_off_its_alignment_is_kept() {
        let (after, changed) = merge("  store i8 18, ptr %p1\n  store i8 52, ptr %p2\n", false);
        assert!(!changed, "{after}");
    }
}
