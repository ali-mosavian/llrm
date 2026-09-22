"""Which data objects a module lets a pointer reach.

A load or store names its place; anything else given the place -- ADDRESS, a
by-reference call argument -- hands out its address. So does a relocation
in initialized data, and a symbol other modules can see.
"""

from qbopt.hir import model

_NAMING = frozenset({model.Op.LOAD, model.Op.STORE, model.Op.COPY})
_PLACES = (model.PlaceRef, model.ArrayElement, model.ProjectedPlace)


def escaped(module: model.Module) -> frozenset[int]:
    """Data symbols whose address may be held by something other than a direct reference."""
    out = {one.id for one in module.data if one.linkage is not model.DataLinkage.INTERNAL}
    out |= {relocation.target for one in module.data for relocation in one.relocations}
    for function in module.functions:
        places = {one.id: one for one in function.places}
        for block in function.blocks:
            handed = [one.operands for one in block.instructions if one.op not in _NAMING]
            for operand in (one for operands in (*handed, block.terminator.operands) for one in operands):
                if not isinstance(operand, _PLACES):
                    continue
                place = places[operand.place]
                if place.storage not in (model.Storage.LOCAL, model.Storage.PARAMETER):
                    out.add(place.symbol)
    return frozenset(out)
