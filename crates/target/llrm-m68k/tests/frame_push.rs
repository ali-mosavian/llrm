//! The saved registers pushed with `-(%sp)` and popped with `(%sp)+`. The pin
//! opens the frame with `suba.l #n, %sp` and `movem`s them into slots, which
//! for one register is 24 bytes against 4.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

fn status(
    ir: &str,
    args: &[i64],
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("frame-push");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    // a run that ends on a signal is a failure, not a skipped check
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

fn asm(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

/// Population count of the argument: a loop that needs a register the callee
/// keeps (`%d2`) and nothing else on the frame.
const POPCOUNT: &str = "define i32 @f(i32 %x) {
entry:
  br label %loop
loop:
  %v = phi i32 [ %x, %entry ], [ %v1, %loop ]
  %c = phi i32 [ 0, %entry ], [ %c1, %loop ]
  %b = and i32 %v, 1
  %c1 = add i32 %c, %b
  %v1 = lshr i32 %v, 1
  %z = icmp eq i32 %v1, 0
  br i1 %z, label %done, label %loop
done:
  ret i32 %c1
}";

/// The same loop with a local the optimizer cannot remove: the frame has
/// something besides the saved register.
const WITH_LOCAL: &str = "define i32 @f(i32 %x) {
entry:
  %slot = alloca i32, align 4
  store volatile i32 5, ptr %slot, align 4
  br label %loop
loop:
  %v = phi i32 [ %x, %entry ], [ %v1, %loop ]
  %c = phi i32 [ 0, %entry ], [ %c1, %loop ]
  %b = and i32 %v, 1
  %c1 = add i32 %c, %b
  %v1 = lshr i32 %v, 1
  %z = icmp eq i32 %v1, 0
  br i1 %z, label %done, label %loop
done:
  %l = load volatile i32, ptr %slot, align 4
  %r = add i32 %c1, %l
  ret i32 %r
}";

/// One saved register, no other frame, was `suba.l #4,%sp; movem.l %d2,(0,%sp)`
/// and the mirror (24 bytes). It is `move.l %d2,-(%sp)` and `move.l
/// (%sp)+,%d2`.
#[test]
fn one_saved_register_is_pushed_and_popped() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = asm(POPCOUNT, cpu);
        assert!(text.contains("move.l\t%d2, -(%sp)") && text.contains("move.l\t(%sp)+, %d2"), "{text}");
        assert!(!text.contains("movem") && !text.contains("suba"), "{text}");
    }
    if let Some(code) = status(POPCOUNT, &[0xFF]) {
        assert_eq!(code, 8);
    }
}

/// A frame with a local keeps the pushed register on top: the local is opened
/// after the push and closed before the pop, and the local is still found.
#[test]
fn a_frame_with_a_local_pushes_first_and_still_finds_the_local() {
    let text = asm(WITH_LOCAL, Cpu::M68000);
    let push = text.find("-(%sp)").unwrap_or(usize::MAX);
    let open = text.find("suba.l").unwrap_or(0);
    assert!(push < open, "{text}");
    if let Some(code) = status(WITH_LOCAL, &[0xFF]) {
        assert_eq!(code, 13, "{text}");
    }
}

/// `movem.l %d2-%d3,-(%sp)` stores its mask reversed (bit 15 is `%d0`), and
/// `move.l %a2,-(%sp)` is not `move.l %d2,-(%sp)`.
#[test]
fn the_push_forms_encode_as_the_manual_gives_them() {
    let reg = |r| Operand::Reg(r);
    let movem = Inst { opcode: "MOVM32em", operands: vec![reg(reg::SP), Operand::Imm(0b1100)] };
    assert_eq!(encode(&movem).unwrap().bytes, [0x48, 0xE7, 0x30, 0x00]);
    let pop = Inst { opcode: "MOVM32mo", operands: vec![Operand::Imm(0b1100), reg(reg::SP)] };
    assert_eq!(encode(&pop).unwrap().bytes, [0x4C, 0xDF, 0x00, 0x0C]);
    let push_d = Inst { opcode: "MOV32ed", operands: vec![reg(reg::SP), reg(reg::D2)] };
    assert_eq!(encode(&push_d).unwrap().bytes, [0x2F, 0x02]);
    let push_a = Inst { opcode: "MOV32ea", operands: vec![reg(reg::SP), reg(reg::A2)] };
    assert_eq!(encode(&push_a).unwrap().bytes, [0x2F, 0x0A]);
    let pop_a = Inst { opcode: "MOV32ao", operands: vec![reg(reg::A2), reg(reg::SP)] };
    assert_eq!(encode(&pop_a).unwrap().bytes, [0x24, 0x5F]);
}

fn four_saved(frame_pointer: bool) -> llrm_m68k::frame::Layout {
    llrm_m68k::frame::layout(&llrm_m68k::frame::Function {
        saved: vec![reg::D2, reg::D3, reg::D4, reg::D5],
        frame_pointer_all: frame_pointer,
        ..Default::default()
    })
}

fn pushes(ops: &[llrm_m68k::frame::FrameOp]) -> bool {
    ops.iter().any(|op| matches!(
        op,
        llrm_m68k::frame::FrameOp::Push { .. } | llrm_m68k::frame::FrameOp::Pop { .. }
    ))
}

/// Four saved registers go through one `movem` push on each CPU when the tables
/// say it is no larger and no slower; a push priced dearer than the pin's by
/// more than its margin (the tables never do), in a byte or in a clock, leaves
/// the pin's `suba.l` and `movem` into slots. A rule that always pushed would
/// pass the first and fail the other two.
#[test]
fn the_pin_frame_is_kept_where_the_tables_price_the_push_dearer() {
    use llrm_m68k::expand::{frame_cost, frame_ops_priced};
    use llrm_m68k::frame::FrameOp;
    let layout = four_saved(false);
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let real = |ops: &[FrameOp]| frame_cost(ops, cpu);
        let (opening, closing) = frame_ops_priced(&layout, true, real).unwrap();
        assert!(pushes(&opening) && pushes(&closing), "{cpu:?}: {opening:?} {closing:?}");
        let dearer_clock =
            |ops: &[FrameOp]| frame_cost(ops, cpu).map(|(b, c)| (b, c + if pushes(ops) { 1000 } else { 0 }));
        let dearer_byte =
            |ops: &[FrameOp]| frame_cost(ops, cpu).map(|(b, c)| (b + if pushes(ops) { 1000 } else { 0 }, c));
        for (what, cost) in [("1000 clocks", &dearer_clock as &dyn Fn(&[FrameOp]) -> _), ("1000 bytes", &dearer_byte)] {
            let (opening, closing) = frame_ops_priced(&layout, true, cost).unwrap();
            assert!(!pushes(&opening) && !pushes(&closing), "{cpu:?}, {what} dearer: {opening:?} {closing:?}");
            assert_eq!((opening, closing), (layout.prologue(), layout.epilogue()));
        }
    }
}

/// A frame pointer keeps the pin's frame (`link`, `movem` into slots, `unlk`),
/// whatever the tables say.
#[test]
fn a_frame_with_a_frame_pointer_keeps_the_pins_frame() {
    use llrm_m68k::expand::{frame_cost, frame_ops_priced};
    let layout = four_saved(true);
    let (opening, closing) = frame_ops_priced(&layout, true, |ops| frame_cost(ops, Cpu::M68000)).unwrap();
    assert!(!pushes(&opening) && !pushes(&closing), "{opening:?} {closing:?}");
    assert_eq!((opening, closing), (layout.prologue(), layout.epilogue()));
}

/// `Options::PINNED` never pushes.
#[test]
fn the_pinned_options_keep_the_pins_frame() {
    use llrm_m68k::expand::{frame_cost, frame_ops_priced};
    let layout = four_saved(false);
    let (opening, closing) =
        frame_ops_priced(&layout, Options::PINNED.push_frame, |ops| frame_cost(ops, Cpu::M68000)).unwrap();
    assert_eq!((opening, closing), (layout.prologue(), layout.epilogue()));
}
