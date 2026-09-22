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
/// reference.
pub fn escaped(module: &model::Module) -> BTreeSet<i64> {
    let mut out: BTreeSet<i64> =
        module.data.iter().filter(|one| one.linkage != model::DataLinkage::Internal).map(|one| one.id).collect();
    out.extend(module.data.iter().flat_map(|one| one.relocations.iter()).map(|relocation| relocation.target));
    for function in &module.functions {
        let places: IndexMap<i64, &model::Place> = function.places.iter().map(|one| (one.id, one)).collect();
        for block in &function.blocks {
            let handed = block
                .instructions
                .iter()
                .filter(|one| !_NAMING.contains(&one.op))
                .map(|one| &one.operands);
            for operand in handed.chain([&block.terminator.operands]).flatten() {
                let place = match operand {
                    model::Operand::PlaceRef(one) => one.place,
                    model::Operand::ArrayElement(one) => one.place,
                    model::Operand::ProjectedPlace(one) => one.place,
                    _ => continue,
                };
                let place = places[&place];
                if !matches!(place.storage, model::Storage::Local | model::Storage::Parameter) {
                    out.insert(place.symbol);
                }
            }
        }
    }
    out
}
