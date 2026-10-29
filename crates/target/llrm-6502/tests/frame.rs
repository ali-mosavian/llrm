//! The 6502 frame on hand-derived layouts. The soft stack grows down from `rs0`; the stack
//! arguments sit at offsets from `rs0` at entry, the locals below them, the outgoing call area at
//! the bottom; an object is at `offset + size` from `rs0` once the prologue has run.

use llrm_6502::dagisel::generated::ids::opcode as op;
use llrm_6502::emit::{MInst, Operand};
use llrm_6502::frame::{Function, layout};
use llrm_codegen::backend::frameobjects::Object;

fn object(
    size: i64,
    offset: i64,
) -> Object {
    Object { size, align: 1, offset, callee_saved: false, dead: false }
}

fn adjust(bytes: i64) -> MInst {
    MInst::new(op::ADJSP, vec![Operand::Imm(bytes)])
}

/// A 10-byte local and a 4-byte outgoing area: 14 bytes. The local is just above the outgoing area,
/// at `rs0 + 4`.
#[test]
fn a_local_sits_above_the_outgoing_area() {
    let function = Function { objects: vec![object(10, 0)], max_call_frame: 4, adjusts_stack: true, ..Function::default() };

    let frame = layout(&function);

    assert_eq!(frame.stack_size, 14);
    assert_eq!(frame.address(0), 4);
    assert_eq!(frame.prologue(), [adjust(-14)]);
    assert_eq!(frame.epilogue(), [adjust(14)]);
}

/// A word argument at entry offset 0 and two saved pairs (4 bytes): the frame is 4 bytes, the
/// argument is at `rs0 + 4` (above the frame), and the pairs fill `rs0 + 0` and `rs0 + 2`.
#[test]
fn saved_pairs_are_stored_below_the_arguments() {
    let function = Function {
        objects: vec![object(2, 0)],
        fixed: 1,
        saved: vec!["rs4".into(), "rs5".into()],
        ..Function::default()
    };

    let frame = layout(&function);

    assert_eq!(frame.stack_size, 4);
    assert_eq!(frame.address(0), 4);
    let prologue = frame.prologue();
    assert_eq!(prologue[0], adjust(-4));
    let mut slots: Vec<i64> = prologue[1..]
        .iter()
        .map(|inst| match inst.operands[2] {
            Operand::Imm(offset) => offset,
            _ => panic!("an offset"),
        })
        .collect();
    slots.sort();
    assert_eq!(slots, [0, 2]);
    let epilogue = frame.epilogue();
    assert_eq!((epilogue.len(), epilogue.last()), (3, Some(&adjust(4))));
    assert!(epilogue[..2].iter().all(|inst| inst.opcode == op::LDP16));
}

/// A reserved call frame needs nothing around a call; a frame with variable-sized objects moves
/// `rs0` by the bytes of the call.
#[test]
fn a_call_sequence_adjusts_rs0_only_when_the_frame_is_not_reserved() {
    let reserved = layout(&Function { max_call_frame: 4, adjusts_stack: true, ..Function::default() });
    assert!(reserved.call_sequence(true, 4).is_empty());

    let dynamic = layout(&Function { has_var_sized: true, adjusts_stack: true, ..Function::default() });
    assert_eq!(dynamic.call_sequence(true, 4), [adjust(-4)]);
    assert_eq!(dynamic.call_sequence(false, 4), [adjust(4)]);
}

/// A store to frame object 0 plus 1 is `rs0 + 4 + 1` once the 14-byte frame is laid out.
#[test]
fn a_frame_index_becomes_rs0_and_an_offset() {
    let function = Function { objects: vec![object(10, 0)], max_call_frame: 4, adjusts_stack: true, ..Function::default() };
    let frame = layout(&function);
    let store = MInst::new(op::STP8, vec![Operand::reg("rc2"), Operand::FrameIndex(0), Operand::Imm(1)]);

    let resolved = frame.eliminate(&store).unwrap();

    assert_eq!(resolved.operands, [Operand::reg("rc2"), Operand::reg("rs0"), Operand::Imm(5)]);
}

/// A stack argument past the 255 bytes a `(zp),Y` reaches is refused, not truncated.
#[test]
fn an_object_past_255_bytes_is_refused() {
    let function =
        Function { objects: vec![object(2, 0), object(300, 0)], fixed: 1, adjusts_stack: true, ..Function::default() };
    let frame = layout(&function);
    let load = MInst::new(op::LDP16, vec![Operand::reg("rs2"), Operand::FrameIndex(0), Operand::Imm(0)]);

    assert!(frame.eliminate(&load).unwrap_err().contains("past the 255"));
}
