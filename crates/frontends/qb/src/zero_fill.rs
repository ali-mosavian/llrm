//! Entry zeroing where procedures frame themselves. The frontend stores zero at
//! entry to each local that needs it. Here those locals are laid out as one
//! block of the frame, and the block's stores become one fill.

use std::collections::BTreeSet;

use llrm_core::hir::model::{self, Number, Operand, Storage, TypeKind};

/// The local place a leading entry store of zero writes, if it is one.
fn zeroed_place(
    instruction: &model::Instruction,
    places: &[model::Place],
) -> Option<i64> {
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

/// `program` with each self-framed procedure's zeroed locals laid out as
/// one block just below BP, aggregates first; two or more zeroed aggregates
/// are one object, a place over them all, so one fill clears them. Scalars
/// stay objects of their own, which promotion makes values. A procedure
/// that keeps the runtime's frame
/// (`framed_by_runtime`) is zeroed by B$ENRA, so its stores are dropped.
pub(super) fn laid_out(
    program: &model::Program,
    framed_by_runtime: impl Fn(&model::Module, &model::Function) -> bool,
) -> model::Program {
    let mut program = program.clone();
    if program.frames != model::Frames::Own {
        return program;
    }
    let originals: Vec<model::Module> = program.modules.clone();
    for (module, original) in program.modules.iter_mut().zip(&originals) {
        let types: llrm_support::hash::HashMap<i64, &model::Type> =
            original.types.iter().map(|one| (one.id, one)).collect();
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
            if let Some(span) = relayout(&mut function.places, &zeroed, &types) {
                let id = function.places.iter().map(|one| one.id).max().unwrap_or(0) + 1;
                function.places.push(span.into_place(id));
            }
        }
    }
    program
}

/// The bytes, from `low` up to BP, of two or more zeroed aggregates, and
/// a type of one of them.
struct Span {
    low: i64,
    r#type: i64,
}

impl Span {
    fn into_place(
        self,
        id: i64,
    ) -> model::Place {
        model::Place {
            extent: Some(-self.low),
            ..model::Place::new(id, "$zeroed", self.r#type, Storage::Local, self.low)
        }
    }
}

/// Groups of local places that share bytes move together: zeroed
/// aggregates first, just below BP, then zeroed scalars, then the rest,
/// each group word-aligned. The zeroed aggregates' span, where two or more.
fn relayout(
    places: &mut [model::Place],
    zeroed: &BTreeSet<i64>,
    types: &llrm_support::hash::HashMap<i64, &model::Type>,
) -> Option<Span> {
    let extent = |one: &model::Place| one.extent.unwrap_or(types[&one.r#type].width);
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
    let scalar = |members: &[usize]| {
        matches!(
            members,
            [one] if matches!(types[&places[*one].r#type].kind, TypeKind::Integer | TypeKind::Float | TypeKind::Pointer | TypeKind::Boolean)
        )
    };
    groups.sort_by_key(|group| match (group.3, scalar(&group.2)) {
        (true, false) => 0,
        (true, true) => 1,
        (false, _) => 2,
    });
    let aggregates = groups.iter().filter(|group| group.3 && !scalar(&group.2)).count();
    let r#type = groups.first().map(|group| places[group.2[0]].r#type);
    let mut cursor = 0;
    let mut span = None;
    for (at, (low, high, members, _)) in groups.into_iter().enumerate() {
        let size = (high - low + 1) & !1;
        let delta = cursor - size - low;
        for index in members {
            places[index].offset += delta;
        }
        cursor -= size;
        if at + 1 == aggregates && aggregates > 1 {
            span = r#type.map(|r#type| Span { low: cursor, r#type });
        }
    }
    span
}
