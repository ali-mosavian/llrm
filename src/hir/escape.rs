//! Port of `qbopt/hir/escape.py`: which data objects a module lets a pointer
//! reach.
//!
//! A load or store names its place; anything else given the place --
//! ADDRESS, a by-reference call argument -- hands out its address. So does a
//! relocation in initialized data, and a symbol other modules can see.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use crate::hir::model;

const _NAMING: [model::Op; 3] = [model::Op::Load, model::Op::Store, model::Op::Copy];

/// Data symbols whose address may be held by something other than a direct
/// reference. Another module's symbol is, unless it is declared unaddressed.
pub fn escaped(module: &model::Module) -> BTreeSet<i64> {
    let mut out: BTreeSet<i64> =
        module.data.iter().filter(|one| one.linkage != model::DataLinkage::Internal && one.addressed).map(|one| one.id).collect();
    out.extend(module.data.iter().flat_map(|one| one.relocations.iter()).map(|relocation| relocation.target));
    for function in &module.functions {
        out.extend(handed(function).filter(|place| !_framed(place)).map(|place| place.symbol));
    }
    out
}

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
