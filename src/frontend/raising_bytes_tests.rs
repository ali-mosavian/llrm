//! Port of `tests/test_raising_bytes.py`.
//!
//! Skipped, needing `mir.bodies` and `corpus.loaded` (NBODY's op at 0x464):
//! `test_nbody_high_byte_clear_defines_the_word_its_consumer_reads`,
//! `test_high_byte_clear_keeps_observable_flags`. The second is rebuilt below
//! on a synthetic `xor bh,bh`; its expected values come from the same inputs
//! run through Python.

use iced_x86::Register;

use super::*;
use crate::model::ir::{Operation, Reg};
use crate::model::mir::{MirBlock, MirBody, OpCode, Opaque};

#[test]
fn test_high_byte_clear_keeps_observable_flags_synthetic() {
    let (before, after) = (Value::new(1, 0x10), Value::new(2, 0x464));
    let flags = Value { flags: true, ..Value::new(3, 0x464) };
    let bh = Arg::Opaque(Opaque::new(Some(Loc::Reg(Reg { register: Register::BH, width: 1 }))));
    let mut xor = Op::new(0x464, OpCode::Operation(Operation::Binary), "xor", vec![after, flags], vec![before]);
    xor.kind = Kind::Xor;
    xor.args = vec![bh.clone(), bh];
    let op = mir::raising_occurrence(&xor, (0x464, 0x466), Vec::new(), None);
    let block = MirBlock::new(0, vec![], vec![op.clone()], vec![]);
    let mut body = RaisedBody::new(MirBody::new(0, vec![block.clone()]));
    body.origin = [(before, Register::EBX), (after, Register::EBX)].into_iter().collect();

    let done = scalar(body.clone()).blocks[0].ops[0].clone();
    assert_eq!(done.kind, Kind::And, "unread flags do not keep the clear");
    assert_eq!(done.args, [Arg::Held(Held { value: before, width: 2 }), Arg::Const(Const::new(255, 2))]);
    assert_eq!(done.merges, [(before, after)].into_iter().collect());

    let mut reader = op.clone();
    reader.kind = Kind::Nothing;
    reader.uses = vec![flags];
    reader.defines = vec![];
    reader.args = vec![];
    reader.results = vec![];
    let observed = body.with_blocks(vec![block.with_ops(vec![op.clone(), reader, op.clone()])]);
    assert_eq!(scalar(observed).blocks[0].ops[0], op);
}
