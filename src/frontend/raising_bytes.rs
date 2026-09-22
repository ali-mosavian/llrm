//! Port of `qbopt/frontend/raising_bytes.py`.
//!
//! Recognize BC's high-byte clearing as a whole-value mask.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::model::ir::{Loc, root};
use crate::model::mir::{self, Arg, Const, Held, Kind, Op, OrderedMap, RaisedBody, Value};

pub fn scalar(body: RaisedBody) -> RaisedBody {
    // Read by an operation, or by a phi something reads: every loop header
    // has a flags phi, and counting its unread incoming edges kept oimad's
    // `xor bh,bh` before an OUT as a register MIR cannot name.
    let mut observed: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()).collect();
    let phis: Vec<_> = body.blocks.iter().flat_map(|block| &block.phis).collect();
    loop {
        let grown: BTreeSet<Value> = phis
            .iter()
            .filter(|phi| observed.contains(&phi.result))
            .flat_map(|phi| phi.incoming.values().copied())
            .filter(|value| !observed.contains(value))
            .collect();
        if grown.is_empty() {
            break;
        }
        observed.extend(grown);
    }

    let raised = |op: &Op| -> Op {
        if op.kind != Kind::Xor || op.args.len() != 2 || op.args[0] != op.args[1] {
            return op.clone();
        }
        let register = match &op.args[0] {
            Arg::Opaque(opaque) => match opaque.machine_payload() {
                Some(Loc::Reg(reg))
                    if reg.width == 1
                        && [Register::AH, Register::BH, Register::CH, Register::DH].contains(&reg.register) =>
                {
                    reg.register
                }
                _ => return op.clone(),
            },
            _ => return op.clone(),
        };
        // Its flags are XOR's, not AND's, so the rewrite needs nobody to read them.
        if !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || op.defines.iter().any(|value| value.flags && observed.contains(value))
        {
            return op.clone();
        }
        let root = root(register);
        let inputs: Vec<Value> = op.uses.iter().copied().filter(|value| body.origin.get(value) == Some(&root)).collect();
        let outputs: Vec<Value> =
            op.defines.iter().copied().filter(|value| body.origin.get(value) == Some(&root)).collect();
        if inputs.len() != 1 || outputs.len() != 1 {
            return op.clone();
        }
        let (before, after) = (inputs[0], outputs[0]);
        let mut changed = op.clone();
        changed.kind = Kind::And;
        changed.name = "and".to_owned();
        changed.args = vec![Arg::Held(Held { value: before, width: 2 }), Arg::Const(Const::new(255, 2))];
        changed.results = vec![Arg::Held(Held { value: after, width: 2 })];
        changed.defines = vec![after];
        changed.uses = vec![before];
        changed.merges = [(before, after)].into_iter().collect::<OrderedMap<_, _>>();
        changed.raised = None;
        mir::detached(changed)
    };

    let blocks = body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(raised).collect())).collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "raising_bytes_tests.rs"]
mod tests;
