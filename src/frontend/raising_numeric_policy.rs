//! Port of `qbopt/frontend/raising_numeric_policy.py`: separate BASIC's
//! explicit overflow observation from native integer arithmetic.

use iced_x86::Code;

use crate::frontend::raising_calls::_discarded;
use crate::model::floating::Exceptions;
use crate::model::ir::nodes::Node;
use crate::model::mir::{Kind, RaisedBody};

pub fn checkpoints(mut body: RaisedBody) -> RaisedBody {
    for block in &mut body.body_mut().blocks {
        for op in &mut block.ops {
            if op.kind == Kind::Fcheck {
                *op = _discarded(op);
            } else if let Some(floating) = &mut op.floating {
                floating.exceptions = Exceptions::Deferred;
            }
        }
    }
    body
}

pub fn native(mut body: RaisedBody) -> RaisedBody {
    for block in &mut body.body_mut().blocks {
        for op in &mut block.ops {
            if matches!(op.node().map(|node| &**node), Some(Node::Opaque(node)) if node.insn.insn.code() == Code::Into) {
                *op = _discarded(op);
            }
        }
    }
    body
}

#[cfg(test)]
#[path = "raising_numeric_policy_tests.rs"]
mod tests;
