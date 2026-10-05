//! Port of `qbopt/hir/escape.py`: which data objects a module lets a pointer
//! reach.
//!
//! A load or store names its place; anything else given the place --
//! ADDRESS, a by-reference call argument -- hands out its address. So does a
//! relocation in initialized data, and a symbol other modules can see.

use std::collections::BTreeSet;

use llrm_support::hash::IndexMap;

use crate::model;

const _NAMING: [model::Op; 6] = [model::Op::Load, model::Op::Store, model::Op::Copy, model::Op::CopyBytes, model::Op::LifetimeStart, model::Op::LifetimeEnd];

/// The frame places whose address `function` hands out. Every other local
/// and parameter is reached only by name: no pointer, far or near, can.
pub fn exposed_frame(function: &model::Function) -> BTreeSet<i64> {
    handed(function).filter(|place| _framed(place)).map(|place| place.id).collect()
}

fn _framed(place: &model::Place) -> bool {
    matches!(place.storage, model::Storage::Local | model::Storage::Parameter)
}

/// Each place an operation other than a naming one is given.
fn handed(function: &model::Function) -> impl Iterator<Item = &model::Place> {
    let places: IndexMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
    function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .filter(|one| !_NAMING.contains(&one.op))
                .map(|one| &one.operands)
                .chain([&block.terminator.operands])
        })
        .flatten()
        .filter_map(move |operand| {
            let place = match operand {
                model::Operand::PlaceRef(one) => one.place,
                model::Operand::ArrayElement(one) => one.place,
                model::Operand::ProjectedPlace(one) => one.place,
                _ => return None,
            };
            Some(places[&place])
        })
}
