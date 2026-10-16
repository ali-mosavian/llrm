//! Golden text of every `Operation`, and of the operand forms the old dump
//! printed ambiguously (a branch with no target, a call with no callee, a
//! spill slot as `[[bp-0x4]]`, a cell placed as `@unplaced`).

use llrm_omf::module::{Addr, Space};

use crate::registers::RegId;
use crate::text::{Syntax, render};
use crate::{AddressRef, Held, Imm, Loc, Mem, Operation, Reg, Semantics};

fn value(
    number: u32,
    width: u32,
) -> Loc {
    Loc::Held(Held { value: number, width })
}

fn register(
    register: RegId,
    width: u32,
) -> Loc {
    Loc::Reg(Reg { register, width })
}

fn imm(
    value: i64,
    width: u32,
) -> Loc {
    Loc::Imm(Imm { value, width, address: None })
}

fn frame(
    disp: i64,
    width: u32,
) -> Loc {
    Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), width))
}

fn what(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn block(at: i64) -> String {
    format!("B{}", at / 4)
}

fn text(one: &Semantics) -> String {
    render(
        one,
        &Syntax {
            block: &block,
            frame: &|_| None,
            callee: Some(("fib", false)),
            arguments: &["v12@ax".to_owned()],
            otherwise: Some("B4".to_owned()),
        },
    )
    .to_string()
}

/// The instruction and its text for each operation. No wildcard: a new
/// `Operation` does not build until it has a line here.
fn golden(op: Operation) -> (Semantics, &'static str) {
    let ax = || register(RegId::AX, 2);
    let st = |index| Loc::st(index);
    match op {
        Operation::Move => (what(op, "mov", vec![value(5, 4)], vec![value(3, 4)]), "v5 = mov.i32 v3"),
        Operation::Exchange => (
            what(op, "xchg", vec![ax(), register(RegId::BX, 2)], vec![ax(), register(RegId::BX, 2)]),
            "ax, bx = xchg.i16 ax, bx",
        ),
        Operation::Address => (
            what(
                op,
                "lea",
                vec![value(4, 2)],
                vec![Loc::Address(AddressRef::new(Some(Addr::new(Space::Segment, 16))))],
            ),
            "v4 = lea.i16 &seg0+0x10",
        ),
        Operation::Binary => (what(op, "sub", vec![value(11, 4)], vec![value(2, 4), imm(2, 4)]), "v11 = sub.i32 v2, 2"),
        Operation::Multiply => {
            (what(op, "imul", vec![value(7, 2)], vec![value(5, 2), value(6, 2)]), "v7 = imul.i16 v5, v6")
        }
        Operation::Divide => (
            what(
                op,
                "idiv",
                vec![register(RegId::AX, 2), register(RegId::DX, 2)],
                vec![register(RegId::DX, 2), ax(), value(3, 2)],
            ),
            "ax, dx = idiv.i16 dx, ax, v3",
        ),
        Operation::Compare => (what(op, "cmp", vec![], vec![value(46, 2), imm(5, 2)]), "cmp.i16 v46, 5"),
        Operation::Unary => (what(op, "neg", vec![ax()], vec![ax()]), "ax = neg.i16 ax"),
        Operation::Funnel => (
            what(op, "shld", vec![value(9, 4)], vec![value(9, 4), value(8, 4), imm(3, 1)]),
            "v9 = shld.i32 v9, v8, 3:i8",
        ),
        Operation::Extend => (what(op, "movsx", vec![value(11, 4)], vec![value(10, 1)]), "v11 = movsx.i32 v10:i8"),
        Operation::Push => (what(op, "push", vec![], vec![value(3, 2)]), "push.i16 v3"),
        Operation::Pop => (what(op, "pop", vec![value(4, 2)], vec![]), "v4 = pop.i16"),
        Operation::Leave => (what(op, "leave", vec![], vec![]), "leave"),
        Operation::Fill => (what(op, "rep stosw", vec![], vec![register(RegId::CX, 2), ax()]), "rep stosw.i16 cx, ax"),
        Operation::Copy => (what(op, "rep movsw", vec![], vec![register(RegId::CX, 2)]), "rep movsw.i16 cx"),
        Operation::Jump => (Semantics { target: Some(20), ..what(op, "jmp", vec![], vec![]) }, "jmp B5"),
        Operation::Branch => (Semantics { target: Some(20), ..what(op, "jl", vec![], vec![]) }, "jl B5 else B4"),
        Operation::Escape => (what(op, "int", vec![], vec![imm(33, 1)]), "int 33"),
        Operation::Call => (what(op, "call", vec![value(13, 4)], vec![]), "v13 = call fib(v12@ax)"),
        Operation::Return => (what(op, "ret", vec![], vec![]), "ret v12@ax"),
        Operation::Nothing => (Semantics::new(op), "nothing"),
        Operation::Restore => (what(op, "restore", vec![], vec![]), "restore"),
        Operation::Data => (Semantics::new(op), "data"),
        Operation::FloatLoad => (what(op, "fld", vec![st(0)], vec![frame(-8, 8)]), "st0 = fld.f64 [bp-8]"),
        Operation::FloatStore => (what(op, "fstp", vec![frame(-8, 8)], vec![st(0)]), "fstp.f64 [bp-8], st0"),
        Operation::FloatArith => (what(op, "fmul", vec![st(0)], vec![st(0), st(1)]), "st0 = fmul.f80 st0, st1"),
        Operation::FloatArithPop => (what(op, "faddp", vec![st(1)], vec![st(1), st(0)]), "st1 = faddp.f80 st1, st0"),
        Operation::FloatUnary => (what(op, "fchs", vec![st(0)], vec![st(0)]), "st0 = fchs.f80 st0"),
        Operation::Barrier => (Semantics::new(op), "barrier"),
    }
}

/// Every operation prints exactly its golden text. One test, one line each, so
/// the failure names the operation.
#[test]
fn every_operation_prints_its_golden_text() {
    for op in Operation::ALL {
        let (one, expected) = golden(op);
        assert_eq!(text(&one), expected, "{op}");
    }
}

/// `golden` is exhaustive over the enum, so a new variant stops the build; this
/// checks `Operation::ALL`, which drives the test above, lists each one.
#[test]
fn the_list_of_operations_names_each_one_once() {
    let names: std::collections::BTreeSet<_> = Operation::ALL.iter().map(|op| op.as_str()).collect();
    assert_eq!(names.len(), Operation::ALL.len());
}

fn cell(
    configure: impl FnOnce(&mut Mem),
    width: u32,
    addr: Option<Addr>,
) -> String {
    let mut one = Mem::new(addr, width);
    configure(&mut one);
    text(&what(Operation::Move, "mov", vec![], vec![Loc::Mem(one)]))
}

/// A spill slot printed `[[bp-0x4]]` with no width; it is `[bp-4]` now, the
/// width on the opcode.
#[test]
fn a_frame_cell_names_its_displacement_in_decimal_and_its_width() {
    assert_eq!(cell(|_| {}, 2, Some(Addr::new(Space::Frame, -4))), "load.i16 [bp-4]");
    assert_eq!(cell(|_| {}, 2, Some(Addr::new(Space::Frame, 6))), "load.i16 [bp+6]");
}

/// A cell on a value printed `[None v3@unplaced]`; unplaced is no register.
#[test]
fn a_value_base_is_named_with_its_register_only_where_placed() {
    let base = Held { value: 3, width: 2 };
    assert_eq!(cell(|one| one.base = Some(base), 2, None), "load.i16 [v3]");
    assert_eq!(
        cell(
            |one| {
                one.base = Some(base);
                one.through = RegId::BX;
            },
            2,
            None
        ),
        "load.i16 [v3@bx]"
    );
}

#[test]
fn a_base_an_index_a_scale_and_a_displacement_all_print() {
    let (base, index) = (Held { value: 3, width: 2 }, Held { value: 4, width: 2 });
    let configure = |one: &mut Mem| {
        one.base = Some(base);
        one.index = Some(index);
        one.scale = 4;
        one.offset = 8;
    };
    assert_eq!(cell(configure, 2, None), "load.i16 [v3+v4*4+8]");
}

#[test]
fn a_far_cell_names_its_segment_register() {
    let far = Addr { segment: iced_x86::Register::ES, base: iced_x86::Register::BX, ..Addr::new(Space::Far, 2) };
    assert_eq!(cell(|_| {}, 2, Some(far)), "load.i16 [es:bx+0x2]");
    let selector = Held { value: 7, width: 2 };
    assert_eq!(cell(|one| one.selector = Some(selector), 2, None), "load.i16 [v7:0]");
}

#[test]
fn a_symbol_cell_names_its_segment_and_offset_in_hex() {
    assert_eq!(cell(|_| {}, 4, Some(Addr::new(Space::Segment, 16))), "load.i32 [seg0+0x10]");
    assert_eq!(cell(|_| {}, 1, Some(Addr { index: 3, ..Addr::new(Space::External, 0) })), "load.i8 [ext3]");
}

/// `0x0012  jl` named no target and `call` no callee.
#[test]
fn a_branch_names_both_ways_and_a_switch_its_runs() {
    let branch = Semantics { target: Some(20), ..what(Operation::Branch, "jl", vec![], vec![]) };
    assert_eq!(text(&branch), "jl B5 else B4");
    let table = Semantics {
        indirect: true,
        table: vec![20, 20, 20, 24, 28],
        ..what(Operation::Jump, "jmp", vec![], vec![value(3, 2)])
    };
    assert_eq!(text(&table), "switch v3 [0..2->B5, 3->B6, 4->B7]");
}

#[test]
fn a_mask_prints_in_hex_at_its_width_and_other_immediates_in_decimal() {
    let mask = what(Operation::Binary, "and", vec![value(2, 2)], vec![value(1, 2), imm(-256, 2)]);
    assert_eq!(text(&mask), "v2 = and.i16 v1, 0xff00");
    let add = what(Operation::Binary, "add", vec![value(2, 2)], vec![value(1, 2), imm(-256, 2)]);
    assert_eq!(text(&add), "v2 = add.i16 v1, -256");
}

/// A frame cell the caller has named is its slot, not a displacement.
#[test]
fn a_named_slot_replaces_the_frame_displacement() {
    let slot = |addr: &Addr| Some(format!("slot{}", (-addr.disp - 2) / 2));
    let syntax = Syntax { block: &block, frame: &slot, callee: None, arguments: &[], otherwise: None };
    let load = what(Operation::Move, "mov", vec![value(3, 2)], vec![frame(-4, 2)]);
    assert_eq!(render(&load, &syntax).to_string(), "v3 = load.i16 [slot1]");
    let save = what(Operation::Move, "mov", vec![frame(-2, 2)], vec![value(3, 2)]);
    assert_eq!(render(&save, &syntax).to_string(), "store.i16 [slot0], v3");
}
