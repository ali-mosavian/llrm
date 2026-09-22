//! Port of `tests/test_pairs.py`.
//!
//! Skipped, needing `mir.bodies`:
//! `test_raised_longs_leave_no_consumer_without_a_producer`.

use std::sync::Arc;

use super::*;
use crate::model::ir::nodes::{Data, Node, TableKind};
use crate::model::ir::{Effects, Imm, Reg, Semantics};
use crate::model::mir::{OpCode, Raising};
use crate::objectfile::module::{Addr, Space};

/// Python's `SimpleNamespace(semantics=...)` node.
fn node(semantics: Semantics) -> Arc<Node> {
    let mut data = Data::new(0, 0, TableKind::Jump, Vec::new(), Effects::no_effect());
    data.semantics = semantics;
    Arc::new(Node::Data(data))
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn raised(at: i64, op: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, what: Semantics) -> Op {
    let mut made = Op::new(at, OpCode::Operation(op), name, defines, uses);
    made.raising = Some(Box::new(Raising { node: Some(node(what)), covers: None, extra_covers: Vec::new() }));
    made
}

fn reg(register: Register) -> Loc {
    Loc::Reg(Reg { register, width: 2 })
}

#[test]
fn test_a_pair_needs_adjacent_addresses_and_a_known_pair() {
    let where_ = Addr::new(Space::Literal, 0x10);
    assert!(_adjacent(&MemRef::new(Some(where_), 2), &MemRef::new(Some(where_.plus(2)), 2)));
    // not two bytes apart
    assert!(!_adjacent(&MemRef::new(Some(where_), 2), &MemRef::new(Some(where_.plus(4)), 2)));
    // the wrong way round
    assert!(!_adjacent(&MemRef::new(Some(where_.plus(2)), 2), &MemRef::new(Some(where_), 2)));
    // a whole dword is not two halves
    assert!(!_adjacent(&MemRef::new(Some(where_), 4), &MemRef::new(Some(where_.plus(2)), 4)));
    // only BC's own two pairs, low half first
    let none = Origin::new();
    assert_eq!(_half_of(Register::EAX, &none), Some((0, 0)));
    assert_eq!(_half_of(Register::EDX, &none), Some((0, 1)));
    assert_eq!(_half_of(Register::ECX, &none), Some((1, 0)));
    assert_eq!(_half_of(Register::EBX, &none), Some((1, 1)));
    assert_eq!(_half_of(Register::ESI, &none), None);
}

/// `mov ax,es / cwd` is the shape and not the meaning.
#[test]
fn test_a_sign_extension_from_a_segment_register_is_not_a_long() {
    let extending = |source: Loc| -> (Op, Op) {
        let low = raised(
            0x100,
            Operation::Move,
            "mov",
            vec![Value::new(1, 0x100)],
            vec![],
            semantics(Operation::Move, "mov", vec![reg(Register::AX)], vec![source]),
        );
        let high = raised(
            0x103,
            Operation::Nothing,
            "cwd",
            vec![Value::new(2, 0x103)],
            vec![Value::new(1, 0x100)],
            semantics(Operation::Move, "cwd", vec![reg(Register::DX)], vec![]),
        );
        (low, high)
    };
    let origin: Origin =
        [(Value::new(1, 0x100), Register::EAX), (Value::new(2, 0x103), Register::EDX)].into_iter().collect();

    let from_register = extending(reg(Register::BX));
    assert!(
        _sign_extended(&from_register.0, &from_register.1, &origin).unwrap().is_some(),
        "an integer register widens"
    );

    let from_segment = extending(reg(Register::ES));
    assert!(
        _sign_extended(&from_segment.0, &from_segment.1, &origin).unwrap().is_none(),
        "a segment register is not a value"
    );
}

/// `neg ax / adc dx,0 / neg dx` -- the middle instruction is the negate.
#[test]
fn test_two_negates_without_the_borrow_are_not_one_long_negate() {
    let unary = |at: i64, name: &str, register: Register, value: u32| -> Op {
        let where_ = reg(register);
        raised(
            at,
            Operation::Unary,
            name,
            vec![Value::new(value, at)],
            vec![],
            semantics(Operation::Unary, name, vec![where_.clone()], vec![where_]),
        )
    };

    let low = unary(0x100, "neg", Register::AX, 1);
    let high = unary(0x106, "neg", Register::DX, 3);
    let origin: Origin = [
        (Value::new(1, 0x100), Register::EAX),
        (Value::new(2, 0x103), Register::EDX),
        (Value::new(3, 0x106), Register::EDX),
    ]
    .into_iter()
    .collect();

    let borrow = raised(
        0x103,
        Operation::Binary,
        "adc",
        vec![Value::new(2, 0x103)],
        vec![],
        semantics(
            Operation::Binary,
            "adc",
            vec![reg(Register::DX)],
            vec![reg(Register::DX), Loc::Imm(Imm { value: 0, width: 2, address: None })],
        ),
    );
    let ops = [low.clone(), borrow, high.clone()];
    assert!(_negate(&ops, 0, &origin).unwrap().is_some(), "the real idiom");

    let unrelated = unary(0x103, "not", Register::DX, 2);
    let ops = [low, unrelated, high];
    assert!(
        _negate(&ops, 0, &origin).unwrap().is_none(),
        "without the borrow folded in, these are two independent negates"
    );
}
