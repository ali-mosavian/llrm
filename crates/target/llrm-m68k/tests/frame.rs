//! The m68k frame on the pin's `llc -mtriple=m68k-linux-gnu -O2` prologues and
//! epilogues (both CPUs agree).

use llrm_codegen::backend::frameobjects::Object;
use llrm_m68k::frame::{Base, FrameOp, Function, layout};
use llrm_m68k::reg;

fn local(size: i64) -> Object {
    Object { size, align: 4, offset: 0, callee_saved: false, dead: false }
}

/// `@locals`: a 40-byte alloca and a call. The pin: `suba.l #44, %sp`, the
/// alloca at `(4,%sp)` above the 4-byte outgoing area, `adda.l #44, %sp`.
#[test]
fn a_frame_without_a_pointer_adjusts_the_stack_pointer() {
    let function = Function { objects: vec![local(40)], max_call_frame: 4, adjusts_stack: true, ..Function::default() };

    let frame = layout(&function);

    assert_eq!(frame.stack_size, 44);
    assert_eq!(frame.address(0), (Base::Sp, 4));
    assert_eq!(frame.prologue(), [FrameOp::SubSp(44)]);
    assert_eq!(frame.epilogue(), [FrameOp::AddSp(44)]);
}

/// `@saves`: `d2` and `d3` live across calls. The pin: `suba.l #12, %sp`,
/// `movem.l %d2-%d3, (4,%sp)`, the first argument at `(16,%sp)`, the reload
/// before `adda.l #12, %sp`. The saved slots are inside the adjustment
/// (`CalleeSavedFrameSize` is 0 without push sequences); an earlier version
/// took them out of it and adjusted by 4.
#[test]
fn saved_registers_are_stored_by_one_movem_above_the_outgoing_area() {
    let function = Function {
        objects: vec![Object { size: 4, align: 4, offset: 0, callee_saved: false, dead: false }],
        fixed: 1,
        saved: vec![reg::D2, reg::D3],
        max_call_frame: 4,
        adjusts_stack: true,
        ..Function::default()
    };

    let frame = layout(&function);

    assert_eq!(frame.stack_size, 12);
    assert_eq!(frame.address(0), (Base::Sp, 16));
    let registers = vec![reg::D2, reg::D3];
    assert_eq!(
        frame.prologue(),
        [FrameOp::SubSp(12), FrameOp::Save { registers: registers.clone(), base: Base::Sp, offset: 4 },]
    );
    assert_eq!(frame.epilogue(), [FrameOp::Restore { registers, base: Base::Sp, offset: 4 }, FrameOp::AddSp(12)]);
}

/// `@fp` (`"frame-pointer"="all"`): the pin opens with `link.w %a6, #-48`, the
/// alloca is at `(-40,%a6)`, and the epilogue is `unlk %a6`.
#[test]
fn a_frame_pointer_frame_is_link_and_unlk() {
    let function = Function {
        objects: vec![local(40)],
        max_call_frame: 4,
        adjusts_stack: true,
        frame_pointer_all: true,
        ..Function::default()
    };

    let frame = layout(&function);

    assert_eq!(frame.stack_size, 52);
    assert_eq!(frame.address(1), (Base::Fp, -40));
    assert_eq!(frame.prologue(), [FrameOp::Link { bytes: 48 }]);
    assert_eq!(frame.epilogue(), [FrameOp::Unlink]);
}

/// `@big`: an 80000-byte alloca, `suba.l #80004, %sp`.
#[test]
fn a_large_frame_is_one_adjustment() {
    let function =
        Function { objects: vec![local(80000)], max_call_frame: 4, adjusts_stack: true, ..Function::default() };

    assert_eq!(layout(&function).prologue(), [FrameOp::SubSp(80004)]);
}
