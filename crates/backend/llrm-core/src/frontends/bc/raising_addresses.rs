//! Port of `qbopt/frontend/raising_addresses.py`: expose loaded
//! address-space identities and their memory dependencies.

use iced_x86::Register;

use super::raising_address_state;
use crate::abi::runtime;
use crate::model::mir::{self, Arg, Cell, Kind, MemRef, Op, RaisedBody, Symbol, Value};
use crate::objectfile::module::Space;
use crate::support::hash::IndexMap;

/// Give the segment register a value, so that reloading it is redundant.
///
/// One mechanism, not two. This used to run a block-local version -- a fresh
/// variable per load, dropped at every block boundary and every write -- and
/// reach for real SSA only where an `les` had already forced the question.
/// Two loads of one descriptor could then never be the same value, which is
/// the whole of what makes a reload removable, so a separate pass removed
/// them against the machine instead.
pub fn loaded(body: RaisedBody, contracts: Option<&IndexMap<i64, runtime::Contract>>) -> Result<RaisedBody, String> {
    Ok(_allocated(raising_address_state::raised(body, &_selector, contracts)?))
}

/// Attach a FAR access to the descriptor its selector was loaded from.
///
/// Bounds and object identity are separate facts.  An unchecked subscript
/// may be outside the allocation, but loading ES from descriptor D still
/// puts the access in D's heap segment; it cannot thereby become a write to
/// the caller's stack frame or to another array's descriptor.  The earlier
/// array raise already identified allocation requests, and this is the
/// first point where selector loads and their SSA values both exist.
fn _allocated(body: RaisedBody) -> RaisedBody {
    let selectors: IndexMap<(Space, i64, i64), Symbol> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|op| op.array.as_ref())
        .map(|request| {
            let descriptor = request.descriptor;
            ((descriptor.space, descriptor.index, descriptor.offset + descriptor.addend + 2), descriptor)
        })
        .collect();
    if selectors.is_empty() {
        return body;
    }

    let mut owners: IndexMap<Value, Symbol> = IndexMap::default();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if op.kind != Kind::Load || op.loads.len() != 1 {
            continue;
        }
        let reference = mir::symbolic_ref(&op.loads[0]);
        let Some(addr) = reference.addr else {
            continue;
        };
        if reference.base.is_some() || reference.segment.is_some() {
            continue;
        }
        let Some(owner) = selectors.get(&(addr.space, addr.index, addr.disp)) else {
            continue;
        };
        for result in &op.results {
            if let Arg::Held(result) = result {
                if result.width == 2 {
                    owners.insert(result.value, *owner);
                }
            }
        }
    }
    if owners.is_empty() {
        return body;
    }

    let allocated = |reference: &MemRef| -> MemRef {
        let owner = reference.segment.and_then(|segment| owners.get(&segment));
        match (owner, reference.addr) {
            (Some(owner), Some(addr)) if addr.space == Space::Far => {
                MemRef { allocation: Some(*owner), ..reference.clone() }
            }
            _ => reference.clone(),
        }
    };
    let argument = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(Cell { r#ref: allocated(&cell.r#ref) }),
        _ => arg.clone(),
    };
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            block.with_ops(
                block
                    .ops
                    .iter()
                    .map(|op| Op {
                        loads: op.loads.iter().map(allocated).collect(),
                        stores: op.stores.iter().map(allocated).collect(),
                        args: op.args.iter().map(argument).collect(),
                        results: op.results.iter().map(argument).collect(),
                        ..op.clone()
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}

fn _selector(op: &Op) -> bool {
    if op.kind != Kind::Load || op.barrier() || !op.defines.is_empty() || !op.stores.is_empty() {
        return false;
    }
    match (op.args.as_slice(), op.results.as_slice()) {
        ([Arg::Cell(cell)], [Arg::Opaque(result)]) if result.name == "es" => {
            let reference = &cell.r#ref;
            reference.width == 2
                && op.loads == [reference.clone()]
                && reference.addr.is_some_and(|addr| addr.segment != Register::ES)
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "raising_addresses_tests.rs"]
mod tests;
