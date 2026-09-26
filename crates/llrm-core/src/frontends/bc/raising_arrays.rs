//! Port of `qbopt/frontend/raising_arrays.py`: DIM requests and descriptor fields.

use std::rc::Rc;

use num_bigint::BigInt;

use super::{arrayfacts, raising_array_bounds, raising_fields};
use crate::analysis::consts;
use crate::model::mir::{Arg, ArrayRequest, Cell, Const, Held, Kind, MemRef, RaisedBody, Symbol, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;

pub fn annotated(body: RaisedBody, calls: &IndexMap<i64, String>, family: &str) -> RaisedBody {
    let sites: IndexMap<i64, &str> = calls
        .iter()
        .filter(|(_, name)| matches!(name.as_str(), "B$DDIM" | "B$RDIM"))
        .map(|(at, name)| (*at, name.as_str()))
        .collect();
    if sites.is_empty() {
        return raising_fields::named(arrayfacts::proven(body, 10000));
    }
    let known = consts::known(&Rc::new(body.body.clone()), None, None, None, None);
    let mut symbols: IndexMap<Value, Symbol> = IndexMap::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut arguments: Vec<Option<Arg>> = Vec::new();
        let mut ops = Vec::new();
        for op in &block.ops {
            let mut op = op.clone();
            if matches!(op.kind, Kind::Copy | Kind::Address) && op.args.len() == 1 && op.results.len() == 1 {
                let source = _argument(&op.args[0], &known, &symbols);
                if let (Some(Arg::Symbol(source)), Arg::Held(result)) = (source, &op.results[0]) {
                    if result.width == source.width {
                        symbols.insert(result.value, source);
                    }
                }
            }
            if op.kind == Kind::Arg && op.args.len() == 1 {
                arguments.push(_argument(&op.args[0], &known, &symbols));
            } else if sites.contains_key(&op.at) && op.kind == Kind::Call {
                let request = _request(&arguments, sites[&op.at] == "B$RDIM");
                let values = if sites[&op.at] == "B$DDIM" {
                    _descriptor_values(request.as_ref(), &arguments, family)
                } else {
                    Vec::new()
                };
                op.array = request;
                op.memory_values = values;
                arguments.clear();
            } else if !matches!(op.kind, Kind::Copy | Kind::Address | Kind::Xor) || !op.stores.is_empty() || op.barrier() {
                arguments.clear();
            }
            ops.push(op);
        }
        blocks.push(block.with_ops(ops));
    }
    raising_array_bounds::proven(_addresses(body.with_blocks(blocks), &symbols), 10000)
}

/// Normal-return facts for the numeric DDIM layout verified in the three shipped libraries.
pub fn _descriptor_values(
    request: Option<&ArrayRequest>,
    arguments: &[Option<Arg>],
    family: &str,
) -> Vec<(MemRef, Const)> {
    let Some((descriptor, element_width, rank, attributes)) = _shape(arguments) else {
        return Vec::new();
    };
    if !matches!(family, "qb45" | "pds71" | "vbdos") {
        return Vec::new();
    }
    if !(0..=3).contains(&attributes) {
        return Vec::new();
    }
    let start = descriptor.offset + descriptor.addend;
    if descriptor.space != Space::Segment || !(0 <= start && start <= 65536 - (14 + 4 * rank)) {
        return Vec::new();
    }
    // dynamic.asm consumes the stack backwards: last dimension comes first.
    let mut fields: Vec<(i64, i64, u32)> = vec![(8, rank, 1), (9, attributes, 1), (12, element_width, 2)];
    let bounds = request.map(|request| request.bounds.as_slice()).unwrap_or_default();
    for (dimension, (lower, upper)) in bounds.iter().rev().enumerate() {
        let dimension = dimension as i64;
        fields.extend([(14 + 4 * dimension, upper - lower + 1, 2), (16 + 4 * dimension, *lower, 2)]);
    }
    fields
        .into_iter()
        .map(|(offset, number, width)| {
            (
                MemRef::new(Some(Addr { index: descriptor.index, ..Addr::new(Space::Segment, start + offset) }), width),
                Const::new(number, width),
            )
        })
        .collect()
}

/// Resolve descriptor fields without moving their original relocation operands.
pub fn _addresses(body: RaisedBody, symbols: &IndexMap<Value, Symbol>) -> RaisedBody {
    let reference = |reference: &MemRef| -> MemRef {
        let Some(symbol) = reference.base.and_then(|base| symbols.get(&base)) else {
            return reference.clone();
        };
        if !matches!(symbol.space, Space::Segment | Space::Frame) || symbol.width != 2 {
            return reference.clone();
        }
        let Some(addr) = reference.addr else {
            return reference.clone();
        };
        if addr.space != Space::Literal || reference.segment.is_some() {
            return reference.clone();
        }
        let offset = symbol.offset + symbol.addend + addr.disp;
        let width = i64::from(reference.width);
        if symbol.space == Space::Segment && !(0 <= offset && offset <= 0x10000 - width)
            || symbol.space == Space::Frame && !(-0x8000 <= offset && offset <= 0x7FFF - width + 1)
        {
            return reference.clone();
        }
        MemRef { symbolic: Some(Symbol::new(symbol.space, symbol.index, offset, 2)), ..reference.clone() }
    };
    let argument = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(Cell { r#ref: reference(&cell.r#ref) }),
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
                    .map(|op| {
                        let mut made = op.clone();
                        made.loads = op.loads.iter().map(reference).collect();
                        made.stores = op.stores.iter().map(reference).collect();
                        made.args = op.args.iter().map(argument).collect();
                        made.results = op.results.iter().map(argument).collect();
                        made
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}

/// A `Const` or a `Symbol`, or nothing known.
pub fn _argument(
    arg: &Arg,
    known: &IndexMap<Value, consts::Known>,
    symbols: &IndexMap<Value, Symbol>,
) -> Option<Arg> {
    match arg {
        Arg::Const(one) => return (one.width == 2).then(|| arg.clone()),
        Arg::Symbol(one) => return (one.width == 2).then(|| arg.clone()),
        // A local dynamic-array descriptor is still a symbolic object even
        // though its address is relative to this invocation's frame.  The
        // frame-address raise deliberately removed BP from MIR; preserve the
        // same descriptor identity here without putting the register back.
        Arg::FrameAddress(one) if one.width == 2 => {
            return Some(Arg::Symbol(Symbol::new(Space::Frame, 0, one.offset, one.width)));
        }
        _ => {}
    }
    let Arg::Held(Held { value, width: 2 }) = arg else {
        return None;
    };
    if let Some(symbol) = symbols.get(value) {
        return Some(Arg::Symbol(*symbol));
    }
    let fact = known.get(value)?;
    if fact.width < 2 {
        return None;
    }
    let number = i64::try_from(&fact.n & BigInt::from(0xFFFF)).expect("16 bits");
    Some(Arg::Const(Const::new(if number < 0x8000 { number } else { number - 0x10000 }, 2)))
}

/// (descriptor, element width, dimension count, attributes).
pub fn _shape(arguments: &[Option<Arg>]) -> Option<(Symbol, i64, i64, i64)> {
    // runtime/rt/dynamic.asm: lo1, hi1, ..., loN, hiN, element size,
    // dimension count plus attributes, descriptor. ADIM does not allocate.
    if arguments.len() < 5 {
        return None;
    }
    let [width, dimensions, descriptor] = &arguments[arguments.len() - 3..] else {
        return None;
    };
    let (Some(Arg::Const(width)), Some(Arg::Const(dimensions)), Some(Arg::Symbol(descriptor))) =
        (width, dimensions, descriptor)
    else {
        return None;
    };
    let count = i64::try_from(&dimensions.n & BigInt::from(0xFF)).expect("eight bits");
    let width = i64::try_from(&width.n).ok()?;
    if count == 0 || width <= 0 || arguments.len() as i64 != 2 * count + 3 {
        return None;
    }
    let attributes = i64::try_from(&dimensions.n >> 8).ok()?;
    Some((*descriptor, width, count, attributes))
}

pub fn _request(arguments: &[Option<Arg>], replaces: bool) -> Option<ArrayRequest> {
    let (descriptor, width, count, _attributes) = _shape(arguments)?;
    let mut bounds = Vec::new();
    for index in 0..count as usize {
        let (Some(Arg::Const(lower)), Some(Arg::Const(upper))) = (&arguments[index * 2], &arguments[index * 2 + 1]) else {
            return None;
        };
        if upper.n < lower.n {
            return None;
        }
        bounds.push((i64::try_from(&lower.n).ok()?, i64::try_from(&upper.n).ok()?));
    }
    Some(ArrayRequest { descriptor, element_width: u32::try_from(width).ok()?, bounds, replaces })
}

#[cfg(test)]
#[path = "raising_arrays_tests.rs"]
mod tests;
