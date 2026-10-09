//! A program's block locals through the whole pipeline: HIR with lifetime
//! markers to the frame it reserves.

use llrm_hir::model::{
    Block, Dialect, Function, Instruction, Module, Op, Operand, Place, Program, RuntimeProfile, Storage, Terminator,
    TerminatorKind, Type, TypeKind, Value,
};
use llrm_x86_m16::machine::BUILT_IN;

use super::{Options, compiled};
use crate::abi::machine::Machine;
use crate::backend::masm;

/// `F%(a)`: two arms, each with a local of its own, held in memory (volatile)
/// so promotion leaves them, and with a scope of lifetime markers when
/// `markers`.
fn program(markers: bool) -> Program {
    let mut integer = Type::new(1, "integer", TypeKind::Integer, 2);
    integer.signed = Some(true);
    let types = vec![Type::new(0, "void", TypeKind::Void, 0), integer, Type::new(2, "boolean", TypeKind::Boolean, 2)];
    let mut values: Vec<Value> = (1..=4).map(|id| Value { id, r#type: 1 }).collect();
    values[1].r#type = 2;
    let arm = |id: i64, place: i64, load: i64, results: i64| {
        let mut instructions = Vec::new();
        if markers {
            instructions.push(Instruction::new(
                id * 10 + 1,
                Op::LifetimeStart,
                vec![],
                vec![Operand::place_ref(place)],
            ));
        }
        instructions.push(Instruction::new(
            id * 10 + 2,
            Op::Store,
            vec![],
            vec![Operand::place_ref(place), Operand::value_ref(1)],
        ));
        instructions.push(Instruction::new(id * 10 + 3, Op::Load, vec![load], vec![Operand::place_ref(place)]));
        if markers {
            instructions.push(Instruction::new(id * 10 + 4, Op::LifetimeEnd, vec![], vec![Operand::place_ref(place)]));
        }
        Block::new(
            id,
            instructions,
            Terminator::new(TerminatorKind::Return, vec![Operand::value_ref(results)], Vec::new()),
        )
    };
    let test = Instruction::new(1, Op::Ne, vec![2], vec![Operand::value_ref(1), Operand::constant(1, 0)]);
    let entry =
        Block::new(1, vec![test], Terminator::new(TerminatorKind::Branch, vec![Operand::value_ref(2)], vec![2, 3]));
    let place = |id: i64, name: &str, offset: i64| {
        let mut place = Place::new(id, name, 1, Storage::Local, offset);
        place.volatile = true;
        place.extent = Some(2);
        place
    };
    let mut function = Function::new(
        1,
        "f",
        1,
        values,
        vec![place(1, "X", -2), place(2, "Y", -4)],
        vec![entry, arm(2, 1, 3, 3), arm(3, 2, 4, 4)],
        1,
    );
    function.parameters = vec![1];
    let mut program =
        Program::new(Dialect::Qb45, RuntimeProfile::Qb45, vec![Module::new(1, "m", types, vec![function])]);
    // A frame zeroed at entry writes every local outside its scope: no slot is
    // then free to share.
    program.zeroed_locals = false;
    program
}

/// The bytes `F%` reserves below BP.
fn frame(markers: bool) -> i64 {
    let machine = Machine { cpu: "486".to_owned(), ..BUILT_IN.clone() };
    let built = compiled(&program(markers), &Options::m16(machine)).expect("compiles");
    let text = masm::text(&built[0]).expect("prints");
    text.lines().find_map(|line| line.trim().strip_prefix("sub sp, ")?.parse().ok()).unwrap_or(0)
}

/// Block locals of sibling scopes took a slot each; their markers say they are
/// never live together, so the frame is one.
#[test]
fn test_sibling_scopes_share_their_locals_slot_through_the_whole_pipeline() {
    assert_eq!((frame(false), frame(true)), (4, 2));
}
