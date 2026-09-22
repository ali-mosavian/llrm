//! Port of `qbopt/frontend/raising_calls.py`.

use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Held, Kind, Op, OpCode, OrderedMap};

pub fn _discarded(push: &Op) -> Op {
    let mut made = Op::new(push.at, OpCode::Operation(Operation::Nothing), "", Vec::new(), Vec::new());
    made.kind = Kind::Nothing;
    made.id = push.id;
    mir::raising_owned(made, &[push])
}

pub fn _capture(push: &Op, arg: &Arg, held: &Held) -> Op {
    let memory = matches!(arg, Arg::Cell(_));
    let uses = match arg {
        Arg::Held(one) => vec![one.value],
        Arg::Cell(cell) => [cell.r#ref.base, cell.r#ref.segment].into_iter().flatten().collect(),
        _ => Vec::new(),
    };
    let mut made = push.clone();
    made.kind = if memory { Kind::Load } else { Kind::Copy };
    made.op = Some(OpCode::Operation(Operation::Move));
    made.name = "mov".to_owned();
    made.defines = vec![held.value];
    made.uses = uses;
    made.args = vec![arg.clone()];
    made.results = vec![Arg::Held(*held)];
    made.loads = match arg {
        Arg::Cell(cell) => vec![cell.r#ref.clone()],
        _ => Vec::new(),
    };
    made.stores = Vec::new();
    made.raised = None;
    made.merges = OrderedMap::new();
    made.stack = None;
    made
}
