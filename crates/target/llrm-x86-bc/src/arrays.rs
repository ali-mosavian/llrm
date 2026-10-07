//! Arrays: llrm-core's `raising_arrays`, DIM requests and what they leave
//! in the descriptor.
//!
//! `B$DDIM` and `B$RDIM` take `lo1, hi1, ..., loN, hiN, element size,
//! N + attributes << 8, descriptor` (rt/dynamic.asm) and pop them, so each
//! site pops what its own dimension count says. On `B$DDIM`'s normal
//! return the descriptor holds the dimension count at 8, the attributes at
//! 9, the element size at 12, and for each dimension, last pushed first,
//! its element count at 14 + 4d and its lower bound at 16 + 4d: the layout
//! all three shipped libraries share. Those are stored here, so what reads
//! them later reads known values.

use std::collections::HashMap;

use llrm_x86_bcmachine::model::ir::nodes::Node;
use llrm_x86_bcmachine::model::ir::{Imm, Loc, Operation};
use llrm_x86_bcmachine::objectfile::module::{Family, Space};
use llrm_mir::{BinaryOp, CastOp};

use crate::emit::{Emit, Emitter};
use crate::machine::{Facts, tracked};
use crate::sites::Recognizer;

pub const DIM: &str = "B$DDIM";
pub const REDIM: &str = "B$RDIM";

/// A routine whose sites each pop what their own pushes say.
pub fn sized(name: &str) -> bool {
    matches!(name, DIM | REDIM)
}

pub struct Dims;

impl Recognizer for Dims {
    fn node(&self, emitter: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let Node::Call(call) = node else { return None };
        if !sized(&call.name) || emitter.unit.procedures.contains_key(&call.name) {
            return None;
        }
        Some(dim(emitter, &call.name, call.insn.at))
    }
}

/// One DIM request: the call, then what its normal return leaves.
fn dim(emitter: &mut Emitter, name: &str, at: usize) -> Emit<()> {
    let depth = emitter.depth();
    let descriptor = emitter.stack_word(depth, 2)?;
    let shape = emitter.stack_word(depth - 2, 2)?;
    let shape = emitter.constant(shape).ok_or_else(|| format!("{name}'s dimension count is not a constant"))?;
    let (rank, attributes) = (shape & 0xff, (shape >> 8) & 0xff);
    if rank == 0 {
        return Err(format!("{name} of no dimensions"));
    }
    let words = 2 * rank + 3;
    let width = emitter.stack_word(depth - 4, 2)?;
    let width = emitter.constant(width);
    let mut bounds = Vec::new();
    for index in 0..rank {
        let lower = depth - 2 * (words - 1) + 4 * index;
        let (low, high) = (emitter.stack_word(lower, 2)?, emitter.stack_word(lower + 2, 2)?);
        bounds.push((emitter.constant(low), emitter.constant(high)));
    }
    emitter.runtime_call(name, at, Some(2 * words))?;
    let family = emitter.unit.facts.family();
    if name != DIM || !(0..=3).contains(&attributes) || !matches!(family, Family::Quickbasic | Family::Pds | Family::Vbdos) {
        return Ok(());
    }
    let mut fields = vec![(8, rank, 8), (9, attributes, 8)];
    fields.extend(width.map(|width| (12, width, 16)));
    for (dimension, bound) in bounds.iter().rev().enumerate() {
        if let &(Some(low), Some(high)) = bound {
            let at = 14 + 4 * dimension as i64;
            fields.extend([(at, i64::from((high - low + 1) as i16), 16), (at + 2, low, 16)]);
        }
    }
    let near = emitter.b.context.types.ptr(0);
    for (offset, value, bits) in fields {
        let offset = emitter.b.int(16, i128::from(offset));
        let address = emitter.binary(BinaryOp::Add, descriptor, offset);
        let pointer = emitter.cast(CastOp::IntToPtr, address, near);
        let value = emitter.b.int(bits, i128::from(value));
        emitter.b.store(value, pointer, false);
    }
    Ok(())
}

/// A DIM request of a DGROUP descriptor: its segment and first byte, its
/// dimension count and attributes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub segment: i64,
    pub start: i64,
    pub rank: i64,
    pub attributes: i64,
}

impl Request {
    /// The descriptor's bytes.
    pub fn end(&self) -> i64 {
        self.start + 14 + 4 * self.rank
    }
}

/// Each DIM request whose descriptor and dimension count are immediates,
/// pushed directly or through a register one block loaded.
pub fn requests(facts: &Facts) -> Vec<Request> {
    let mut out = Vec::new();
    for body in &facts.bodies {
        for block in &body.blocks {
            let mut held: HashMap<usize, Imm> = HashMap::new();
            let mut pushed: Vec<Option<Imm>> = Vec::new();
            for node in body.nodes_of(block) {
                let what = node.semantics();
                if let Node::Call(call) = &**node {
                    if let (true, [.., Some(shape), Some(descriptor)]) = (sized(&call.name), pushed.as_slice())
                        && let Some(address) = descriptor.address.filter(|one| one.space == Space::Segment && shape.address.is_none())
                    {
                        out.push(Request { segment: address.index, start: address.disp, rank: shape.value & 0xff, attributes: (shape.value >> 8) & 0xff });
                    }
                    held.clear();
                    pushed.clear();
                    continue;
                }
                match (what.op, what.sources.first()) {
                    (Operation::Push, Some(Loc::Imm(imm))) => pushed.push(Some(imm.clone())),
                    (Operation::Push, Some(Loc::Reg(reg))) => pushed.push(tracked(reg.register).and_then(|root| held.get(&root).cloned()).filter(|_| reg.width == 2)),
                    (Operation::Push, _) => pushed.push(None),
                    _ => {}
                }
                match node.effects().defs.as_ref() {
                    Some(defs) => {
                        for one in defs {
                            if let Some(root) = tracked(*one) {
                                held.remove(&root);
                            }
                        }
                    }
                    None => held.clear(),
                }
                if let (Operation::Move, [Loc::Reg(reg)], [Loc::Imm(imm)]) = (what.op, what.dests.as_slice(), what.sources.as_slice())
                    && reg.width == 2
                    && let Some(root) = tracked(reg.register)
                {
                    held.insert(root, imm.clone());
                }
            }
        }
    }
    out
}

/// Whether `callee`, a function's name, allocates an array.
pub fn allocates(callee: &str) -> bool {
    callee.strip_prefix(crate::RUNTIME).is_some_and(sized)
}

