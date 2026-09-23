//! Port of `qbopt/frontend/raising_returns.py`.

use std::sync::Arc;

use iced_x86::Register;

use crate::model::ir::nodes::Node;
use crate::model::ir::{self, Loc, Operation};

/// A return, reading the registers its caller reads after it.
///
/// BC's FUNCTION answers in AX or DX:AX.  `registers` is the procedure's
/// CodeView-established return class; a SUB and an implicit-INTEGER function
/// remain indistinguishable, but neither returns DX.  Missing debug evidence
/// keeps the conservative DX:AX default.  A headerless C object has no return
/// type, so both words of DX:AX, and SI/DI too, including when the original
/// leaf never used them.
pub fn returned(node: &Arc<Node>, header: bool, registers: Option<&[Register]>) -> Arc<Node> {
    let Node::Opaque(opaque) = &**node else {
        return node.clone();
    };
    if opaque.semantics.op != Operation::Return {
        return node.clone();
    }
    let registers: &[Register] = match registers.filter(|one| !one.is_empty()) {
        Some(one) => one,
        None if header => &[Register::AX, Register::DX],
        None => &[Register::AX, Register::DX, Register::SI, Register::DI],
    };
    let returned: Vec<ir::Reg> = registers.iter().map(|&register| ir::Reg { register, width: 2 }).collect();
    let mut made = opaque.clone();
    if let Some(uses) = &mut made.effects.uses {
        uses.extend(returned.iter().map(|one| ir::root(one.register)));
    }
    made.semantics.sources.extend(returned.into_iter().map(Loc::Reg));
    Arc::new(Node::Opaque(made))
}
