//! Port of `qbopt/frontend/raising_defseg.py`: DEF SEG as the store it is.
//!
//! `DEF SEG = &HA000` is `mov ax,0A000h / push ax / call far B$DSEG`, and
//! B$DSEG is eleven bytes that move the pushed word into the runtime's own
//! `b$seg` -- the cell PEEK and POKE read their segment from. Left as a call,
//! the constant is invisible: every `mov es,[b$seg]` is an opaque load and the
//! POKE store addressed through it aliases every other reference in its loop,
//! which is 67% of qbdemo's run time.
//!
//! Its contract says `writes = NONE` on the argument that b$seg is not
//! anything the caller can name. The caller does name it -- that is the
//! `mov es,[b$seg]` at every POKE -- so making the write explicit here is also
//! the honest version of that claim.

use std::collections::{BTreeMap, BTreeSet};

use crate::abi::runtime;
use crate::analysis::ssa;
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, Op, OpCode, OrderedMap, Phi, RaisedBody, Value};
use crate::objectfile::module::{self, Addr, Module, SourceMap, Space};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;

const _CELL: &str = "b$seg";
const _NAME: &str = "B$DSEG";

/// `xor r,r`, which computes a constant and reads nothing.
///
/// BC writes one after a call whose result it is discarding, and while it
/// stands it reads a value only the call defines -- which is what stops the
/// call being replaced. raising_array_bounds already interprets the idiom
/// this way; this is the same fact where it unblocks something.
fn _self_xor(op: &Op) -> bool {
    op.kind == Kind::Xor
        && op.args.len() == 2
        && op.args[0] == op.args[1]
        && matches!(op.args[0], Arg::Held(_))
        && op.loads.is_empty()
        && op.stores.is_empty()
}

fn _zeroed(op: &Op) -> Op {
    let Arg::Held(held) = &op.args[0] else {
        unreachable!("a self xor holds its operand");
    };
    let mut made = op.clone();
    made.kind = Kind::Copy;
    made.op = Some(OpCode::Operation(Operation::Move));
    made.name = "mov".to_owned();
    made.args = vec![Arg::Const(Const::new(0, held.width))];
    made.uses = Vec::new();
    made.symbol = Some(false);
    mir::detached(made)
}

fn _pushed(op: &Op) -> Option<Value> {
    if op.kind != Kind::Arg || op.args.len() != 1 {
        return None;
    }
    match &op.args[0] {
        Arg::Held(held) => Some(held.value),
        _ => None,
    }
}

/// Where the object names b$seg with a 16-bit offset fixup.
///
/// The store needs a relocation of its own and the call's is a ptr32 to a
/// routine -- the wrong kind and the wrong target. Every PEEK and POKE in
/// the program already carries the right one.
fn _fixup(found: &Module) -> Option<i64> {
    let names = omf::externals(&found.records);
    let which = names.iter().position(|name| name == _CELL)? as i64;
    omf::fixups(&found.records)
        .into_iter()
        .find(|one| {
            one.seg == Some(found.seg) && one.target == "external" && one.index == which && one.loc == omf::LOC_OFF16
        })
        .map(|one| one.offset)
}

/// (what this call sets b$seg to, the value pushed), or None.
fn _candidate(
    op: &Op,
    block_ops: &[Op],
    found: &Module,
    contracts: &IndexMap<i64, runtime::Contract>,
    local: &BTreeSet<String>,
    expected: &runtime::Contract,
) -> Option<(Arg, Value)> {
    if op.kind != Kind::Call || found.calls.get(&op.at).map(String::as_str) != Some(_NAME) || local.contains(_NAME) {
        return None;
    }
    let contract = contracts.get(&op.at)?;
    if !contract.established
        || contract.cleanup != expected.cleanup
        || contract.control != runtime::Control::Returns
        || contract.enters_user_code
        || contract.raises_error
        || contract.error_handling
        || contract.clobbers != expected.clobbers
    {
        return None;
    }
    let place = block_ops.iter().position(|one| one == op).expect("the op is in its block");
    if place == 0 {
        return None;
    }
    let pushed = _pushed(&block_ops[place - 1])?;
    let made = block_ops[..place - 1].iter().find(|one| {
        one.kind == Kind::Copy
            && one.args.len() == 1
            && matches!(one.args[0], Arg::Const(_))
            && one.defines == [pushed]
    });
    // The literal where the push is one, and the pushed value otherwise.
    // What the rewrite is for is making the write to `b$seg` visible, and
    // `DEF SEG = <expression>` writes it exactly as `DEF SEG = &HA000` does;
    // requiring a constant left five of qbdemo's eleven as opaque calls,
    // each of which then ended every memory fact in its body.
    let value = match made {
        Some(made) => made.args[0].clone(),
        None => Arg::Held(Held { value: pushed, width: 2 }),
    };
    Some((value, pushed))
}

pub fn raised(
    body: RaisedBody,
    found: &Module,
    contracts: &IndexMap<i64, runtime::Contract>,
    source: &mut SourceMap,
) -> Result<RaisedBody, String> {
    let Some(field) = _fixup(found) else {
        return Ok(body);
    };
    let names = omf::externals(&found.records);
    let which = names.iter().position(|name| name == _CELL).expect("b$seg has a fixup") as i64;
    let reference = MemRef::new(Some(Addr { index: which, ..Addr::new(Space::External, 0) }), 2);
    let local = module::defines(&found.records, found.seg);
    let expected = runtime::contract(Some(_NAME));

    // Python keys this on `id(op)`: where the op sits.
    let mut wanted: IndexMap<(usize, usize), (Arg, Value)> = IndexMap::default();
    for (index, block) in body.blocks.iter().enumerate() {
        for (position, op) in block.ops.iter().enumerate() {
            if let Some(found_at) = _candidate(op, &block.ops, found, contracts, &local, &expected) {
                wanted.insert((index, position), found_at);
            }
        }
    }
    if wanted.is_empty() {
        return Ok(body);
    }

    // Only the discards standing in the way of one of these, not every
    // `xor r,r` in the body -- `xor ax,ax` is two bytes and `mov ax,0` is
    // three, so rewriting them wholesale trades size for nothing.
    let clobbered: BTreeSet<Value> =
        wanted.keys().flat_map(|(index, position)| body.blocks[*index].ops[*position].defines.iter().copied()).collect();
    // What the call leaves in ax is the word it was handed: rt/rtinit.asm is
    // `MOV AX,newseg` / `MOV [b$seg],AX`, so its one clobber is that value
    // and not an unknown. Anything reading it reads the pushed value, which
    // is still defined here -- so a reader is served by saying so rather than
    // by keeping the call, which is what blocked eight of qbdemo's eleven.
    let mut swap: BTreeMap<u32, Value> = BTreeMap::new();
    for (index, block) in body.blocks.iter().enumerate() {
        for (position, op) in block.ops.iter().enumerate() {
            if let Some((_, pushed)) = wanted.get(&(index, position)) {
                for one in op.defines.iter().filter(|one| !one.flags) {
                    swap.insert(one.id, *pushed);
                }
            }
        }
    }
    let blocks: Vec<mir::MirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            block.with_ops(
                block
                    .ops
                    .iter()
                    .map(|op| {
                        if _self_xor(op) && op.uses.iter().all(|one| clobbered.contains(one)) {
                            _zeroed(op)
                        } else {
                            op.clone()
                        }
                    })
                    .collect(),
            )
        })
        .collect();

    let mut read: BTreeSet<Value> = blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()).collect();
    read.extend(blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()));

    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let mut ops: Vec<Op> = Vec::new();
        for (position, op) in block.ops.iter().enumerate() {
            let value = wanted.get(&(index, position)).map(|got| got.0.clone());
            let Some(value) = value.filter(|_| !op.defines.iter().any(|one| one.flags && read.contains(one))) else {
                ops.push(op.clone());
                continue;
            };
            let gone = ops.pop().expect("the push, whose word the callee popped");
            // Both spans, as bytes: the push's and the call's own. With
            // node cleared nothing derives a length, and layout refuses a
            // body it cannot account for every byte of -- rightly, since
            // that is how it catches data BC put between instructions.
            if op.node().is_none() || gone.node().is_none() {
                ops.push(gone);
                ops.push(op.clone());
                continue;
            }
            let original = op;
            let mut changed = op.clone();
            changed.kind = Kind::Store;
            changed.op = Some(OpCode::Operation(Operation::Move));
            changed.name = "mov".to_owned();
            changed.uses = match &value {
                Arg::Held(held) => vec![held.value],
                _ => Vec::new(),
            };
            changed.args = vec![value];
            changed.results = vec![Arg::Cell(Cell { r#ref: reference.clone() })];
            changed.defines = Vec::new();
            changed.loads = Vec::new();
            changed.stores = vec![reference.clone()];
            changed.merges = OrderedMap::new();
            changed.raised = None;
            changed.symbol = Some(true);
            let op = mir::raising_owned(mir::detached(changed), &[&gone, original]);
            if let Some(id) = op.id {
                source.refs.insert(id, vec![field]);
            }
            ops.push(op);
        }
        out.push(block.with_ops(ops));
    }
    let body = body.with_blocks(out);
    if swap.is_empty() {
        return Ok(body);
    }

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut phis = Vec::new();
        for phi in &block.phis {
            let mut incoming = OrderedMap::new();
            for (at, one) in phi.incoming.iter() {
                incoming.insert(*at, ssa::provider(*one, &swap).map_err(|error| error.to_string())?);
            }
            phis.push(Phi { result: phi.result, incoming });
        }
        let mut ops = Vec::new();
        for op in &block.ops {
            ops.push(ssa::substituted(op, &swap).map_err(|error| error.to_string())?);
        }
        blocks.push(mir::MirBlock { phis, ..block.with_ops(ops) });
    }
    Ok(body.with_blocks(blocks))
}
