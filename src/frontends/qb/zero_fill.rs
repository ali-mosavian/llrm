//! QuickrBASIC's entry zeroing. The frontend stores zero at entry to each
//! local that needs it. Here those locals are laid out as one block of the
//! frame, and the block's stores become one fill.

use std::collections::BTreeSet;

use crate::hir::lower::Lowered;
use crate::hir::model::{self, Number, Operand, Storage};
use crate::model::ir::{Operation, Space};
use crate::model::mir::{Arg, Cell, Const, Held, Kind, MemRef, Op, OpCode, Value};

/// A run of zero stores this long or longer becomes a fill: from here the
/// fill's fixed setup is smaller code than the stores it replaces.
pub(super) const FILL_BYTES: i64 = 16;

/// The local place a leading entry store of zero writes, if it is one.
fn zeroed_place(instruction: &model::Instruction, places: &[model::Place]) -> Option<i64> {
    let [target, Operand::Constant(model::Constant { value: Number::Int(0), .. })] = instruction.operands.as_slice()
    else {
        return None;
    };
    let place = match target {
        Operand::PlaceRef(one) => one.place,
        Operand::ProjectedPlace(one) if one.indices.is_empty() => one.place,
        _ => return None,
    };
    let local = places.iter().any(|one| one.id == place && one.storage == Storage::Local);
    (instruction.op == model::Op::Store && local).then_some(place)
}

/// How many of the entry block's instructions are the frontend's zero
/// stores, and the places they write.
fn entry_zeroing(function: &model::Function) -> (usize, BTreeSet<i64>) {
    let Some(entry) = function.blocks.iter().find(|block| block.id == function.entry) else {
        return (0, BTreeSet::new());
    };
    let mut places = BTreeSet::new();
    let mut count = 0;
    for instruction in &entry.instructions {
        let Some(place) = zeroed_place(instruction, &function.places) else { break };
        places.insert(place);
        count += 1;
    }
    (count, places)
}

/// `program` with each QuickrBASIC procedure's zeroed locals laid out as
/// one block just below BP. A procedure that keeps the runtime's frame
/// (`framed_by_runtime`) is zeroed by B$ENRA, so its stores are dropped.
pub(super) fn laid_out(
    program: &model::Program,
    framed_by_runtime: impl Fn(&model::Module, &model::Function) -> bool,
) -> model::Program {
    let mut program = program.clone();
    if program.dialect != model::Dialect::Quickr {
        return program;
    }
    let originals: Vec<model::Module> = program.modules.clone();
    for (module, original) in program.modules.iter_mut().zip(&originals) {
        let widths: std::collections::HashMap<i64, i64> = original.types.iter().map(|one| (one.id, one.width)).collect();
        for (function, source) in module.functions.iter_mut().zip(&original.functions) {
            let (count, zeroed) = entry_zeroing(function);
            if count == 0 {
                continue;
            }
            if framed_by_runtime(original, source) {
                let entry = function.entry;
                let block = function.blocks.iter_mut().find(|block| block.id == entry).expect("an entry block");
                block.instructions.drain(..count);
                continue;
            }
            relayout(&mut function.places, &zeroed, &widths);
        }
    }
    program
}

/// Groups of local places that share bytes move together: zeroed groups
/// first, just below BP, then the rest, each group word-aligned.
fn relayout(places: &mut [model::Place], zeroed: &BTreeSet<i64>, widths: &std::collections::HashMap<i64, i64>) {
    let extent = |one: &model::Place| one.extent.unwrap_or(widths[&one.r#type]);
    let mut locals: Vec<usize> = (0..places.len()).filter(|&index| places[index].storage == Storage::Local).collect();
    locals.sort_by_key(|&index| (places[index].offset, places[index].id));
    // (low, high, members, zeroed)
    let mut groups: Vec<(i64, i64, Vec<usize>, bool)> = Vec::new();
    for index in locals {
        let (low, high) = (places[index].offset, places[index].offset + extent(&places[index]));
        let is_zeroed = zeroed.contains(&places[index].id);
        match groups.last_mut() {
            Some(group) if low < group.1 => {
                group.1 = group.1.max(high);
                group.2.push(index);
                group.3 |= is_zeroed;
            }
            _ => groups.push((low, high, vec![index], is_zeroed)),
        }
    }
    groups.sort_by_key(|group| !group.3);
    let mut cursor = 0;
    for (low, high, members, _) in groups {
        let size = (high - low + 1) & !1;
        let delta = cursor - size - low;
        for index in members {
            places[index].offset += delta;
        }
        cursor -= size;
    }
}

/// A frame store of constant zero, as its address and width.
fn zero_store(op: &Op) -> Option<(i64, i64)> {
    let [Arg::Const(value)] = op.args.as_slice() else { return None };
    let [cell] = op.stores.as_slice() else { return None };
    let addr = cell.addr?;
    let frame = cell.space == Some(Space::Frame) && addr.space == Space::Frame && cell.base.is_none();
    (op.kind == Kind::Store && frame && value.n == 0.into()).then_some((addr.disp, i64::from(cell.width)))
}

/// `body` with each run of at least FILL_BYTES contiguous leading zero
/// stores in its entry block made one word fill.
pub(super) fn filled(body: &Lowered) -> Lowered {
    let mut lowered = body.clone();
    let entry = lowered.body.entry;
    let Some(block) = lowered.body.blocks.iter_mut().find(|block| block.at == entry) else {
        return lowered;
    };
    let leading = block.ops.iter().take_while(|op| op.kind == Kind::Nothing || zero_store(op).is_some()).count();
    let mut stores: Vec<(i64, i64, usize)> = block.ops[..leading]
        .iter()
        .enumerate()
        .filter_map(|(index, op)| zero_store(op).map(|(disp, width)| (disp, width, index)))
        .collect();
    stores.sort();
    let mut runs: Vec<Vec<(i64, i64, usize)>> = Vec::new();
    for store in stores {
        match runs.last_mut() {
            Some(run) if run.last().is_some_and(|last| last.0 + last.1 == store.0) => run.push(store),
            _ => runs.push(vec![store]),
        }
    }
    let mut fresh = body
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().chain(&op.uses))
        .map(|value| (value.id, value.variable))
        .fold((0, 0), |most, (id, variable)| (most.0.max(id), most.1.max(variable)));
    let mut replaced: BTreeSet<usize> = BTreeSet::new();
    let mut made: Vec<Op> = Vec::new();
    for run in runs {
        let bytes: i64 = run.iter().map(|store| store.1).sum();
        // Whole words go to the fill; an odd last byte keeps its store.
        let words: Vec<_> = run.iter().take_while(|store| store.1 == 2).collect();
        let covered: i64 = words.iter().map(|store| store.1).sum();
        if bytes < FILL_BYTES || covered < FILL_BYTES {
            continue;
        }
        let first = &block.ops[words[0].2];
        let at = first.at;
        fresh = (fresh.0 + 1, fresh.1 + 1);
        let address = Held { value: Value { id: fresh.0, at, flags: false, variable: fresh.1, version: 1 }, width: 2 };
        let mut cell = first.stores[0].clone();
        cell.width = 2;
        let mut lea = Op::new(at, OpCode::Operation(Operation::Address), "lea", vec![address.value], vec![]);
        lea.kind = Kind::Address;
        lea.args = vec![Arg::Cell(Cell { r#ref: cell })];
        lea.results = vec![Arg::Held(address.clone())];
        let mut fill = Op::new(at, OpCode::Operation(Operation::Fill), Kind::Fill.as_str(), vec![], vec![address.value]);
        fill.kind = Kind::Fill;
        fill.args = vec![Arg::Const(Const::new(0, 2)), Arg::Const(Const::new(covered / 2, 2)), Arg::Held(address.clone())];
        // Every cell it writes, so alias analysis still sees each object.
        fill.stores = words.iter().flat_map(|store| block.ops[store.2].stores.clone()).collect::<Vec<MemRef>>();
        fill.source_backed = false;
        made.extend([lea, fill]);
        replaced.extend(words.iter().map(|store| store.2));
    }
    if made.is_empty() {
        return lowered;
    }
    let kept: Vec<Op> =
        block.ops.iter().enumerate().filter(|(index, _)| !replaced.contains(index)).map(|(_, op)| op.clone()).collect();
    block.ops = made.into_iter().chain(kept).collect();
    lowered
}
