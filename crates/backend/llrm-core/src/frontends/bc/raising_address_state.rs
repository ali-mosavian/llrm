//! Port of `qbopt/frontend/raising_address_state.py`: the segment register
//! as an SSA variable of its own.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::abi::runtime;
use crate::analysis::ssa;
use crate::model::mir::{Arg, Cell, Held, Kind, MemRef, MirBody, Op, OrderedMap, RaisedBody, Value};
use crate::support::hash::IndexMap;

pub fn raised(
    body: RaisedBody,
    selector: &dyn Fn(&Op) -> bool,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> Result<RaisedBody, String> {
    if !_selects(&body, selector) {
        return Ok(body);
    }
    let values: Vec<Value> = ssa::values(&body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
    let variable = values.iter().map(|value| value.variable).max().unwrap_or(0) + 1;
    let incoming = Value { id: serial, at: body.entry, flags: false, variable, version: 0 };
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut current = incoming;
        let mut operations = Vec::new();
        for op in &block.ops {
            let state = current;
            let reference = |reference: &MemRef| -> MemRef {
                if reference.addr.is_some_and(|addr| addr.segment == Register::ES) {
                    MemRef { segment: Some(state), ..reference.clone() }
                } else {
                    reference.clone()
                }
            };
            let argument = |arg: &Arg| match arg {
                Arg::Cell(cell) => Arg::Cell(Cell { r#ref: reference(&cell.r#ref) }),
                _ => arg.clone(),
            };

            let (loads, stores): (Vec<MemRef>, Vec<MemRef>) =
                (op.loads.iter().map(reference).collect(), op.stores.iter().map(reference).collect());
            let effect = op.node().map(|node| node.effects());
            let contract = if op.kind == Kind::Call { contracts.and_then(|contracts| contracts.get(&op.at)) } else { None };
            let (mut reads, writes) = match contract {
                Some(contract) if contract.established => {
                    // A call says what it touches where anything does. Without
                    // asking, every call minted a selector nothing wrote, and a
                    // routine established to leave ES alone still ended its
                    // caller's descriptor.
                    (
                        contract.inputs.as_ref().is_none_or(|inputs| inputs.contains(&runtime::Reg::Es)),
                        contract.clobbers.contains(&runtime::Reg::Es),
                    )
                }
                _ => {
                    let unknown = op.barrier() || op.kind == Kind::Call;
                    match effect {
                        None => (unknown, unknown),
                        Some(effect) => (
                            effect.uses.as_ref().is_none_or(|uses| uses.contains(&Register::ES)),
                            effect.defs.as_ref().is_none_or(|defs| defs.contains(&Register::ES)),
                        ),
                    }
                }
            };
            reads |= loads.iter().chain(&stores).any(|one| one.segment == Some(current));
            let selected = selector(op);
            let mut op = Op {
                args: op.args.iter().map(argument).collect(),
                results: op.results.iter().map(argument).collect(),
                loads,
                stores,
                ..op.clone()
            };
            if reads {
                let mut uses = Vec::new();
                for value in op.uses.iter().copied().chain([current]) {
                    if !uses.contains(&value) {
                        uses.push(value);
                    }
                }
                op.uses = uses;
            }
            if selected || writes {
                serial += 1;
                current = Value { id: serial, at: op.at, flags: false, variable, version: 1 };
                op.defines.push(current);
                op.results = op
                    .results
                    .iter()
                    .map(|result| match result {
                        Arg::Opaque(one) if one.name == "es" => Arg::Held(Held { value: current, width: 2 }),
                        _ => result.clone(),
                    })
                    .collect();
                if selected {
                    op.results = vec![Arg::Held(Held { value: current, width: 2 })];
                    op.merges = OrderedMap::new();
                }
            }
            operations.push(op);
        }
        blocks.push(block.with_ops(operations));
    }
    let constructed = ssa::constructed(&body.body.with_blocks(blocks), &BTreeSet::from([variable]))
        .map_err(|error| error.to_string())?;
    let built = _undefined(constructed, variable);
    let result = ssa::renumbered(&built, variable);
    let mut origin = body.origin.clone();
    for value in ssa::values(&result).filter(|value| value.variable == variable) {
        origin.insert(value, Register::ES);
    }
    Ok(RaisedBody { body: result, origin, pins: body.pins.clone() })
}

/// Whether anything here goes through a selector at all.
///
/// A body with no far access has none to name, and minting one anyway makes
/// every call define a value nothing reads -- which reads, correctly, as the
/// call disturbing one more thing than it does.
fn _selects(body: &MirBody, selector: &dyn Fn(&Op) -> bool) -> bool {
    body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
        selector(op)
            || op
                .loads
                .iter()
                .chain(&op.stores)
                .any(|one| one.addr.is_some_and(|addr| addr.segment == Register::ES))
    })
}

/// Forget the selector where no definition of it reaches.
///
/// Construction starts every block from one placeholder standing for "the
/// reaching definition here", and resolves it. Where no definition reaches
/// -- a body entered with ES already loaded, a resume entry -- what is left
/// names no definition: the reference has no selector, exactly as it had
/// none before there was a value at all. Left in, it is a value the
/// allocator must find a register for and the spiller a slot, though no
/// instruction anywhere writes it.
///
/// Asked of the body rather than of a version number, because construction
/// numbers a value it did not see defined the same as one it did.
fn _undefined(body: MirBody, variable: u32) -> MirBody {
    let defined: BTreeSet<Value> = body
        .blocks
        .iter()
        .flat_map(|block| {
            block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
        })
        .collect();

    let _entry = |value: Option<Value>| value.is_some_and(|value| value.variable == variable && !defined.contains(&value));
    let forget = |reference: &MemRef| {
        if _entry(reference.segment) { MemRef { segment: None, ..reference.clone() } } else { reference.clone() }
    };
    let argument = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(Cell { r#ref: forget(&cell.r#ref) }),
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
                        loads: op.loads.iter().map(forget).collect(),
                        stores: op.stores.iter().map(forget).collect(),
                        args: op.args.iter().map(argument).collect(),
                        results: op.results.iter().map(argument).collect(),
                        uses: op.uses.iter().copied().filter(|one| !_entry(Some(*one))).collect(),
                        ..op.clone()
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}
