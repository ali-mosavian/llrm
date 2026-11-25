//! A frame of one 32,767-byte local puts the incoming argument at
//! `(32,800,%sp)`. Where `%a6` is free the layout takes it as the frame
//! pointer; where the allocation used it, no register can name the argument,
//! and emission says so by name instead of stopping later in the encoder
//! ("operand 1 = 32792 does not fit its 16-bit field").

use llrm_m68k::expand::finish;
use llrm_m68k::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::{dagisel::hooks::Cpu, reg};

fn function(uses_a6: bool) -> MFunction {
    let mut function = MFunction::new("far");
    function.fixed.insert(-1, 0);
    function.locals.push((0, 32767, 1));
    let load =
        Inst::new(Kind::Op("MOV32rp"), vec![Operand::Def(Reg::Phys(reg::D0)), Operand::Imm(0), Operand::Frame(-1)]);
    let touch_a6 =
        Inst::new(Kind::Op("MOV32aa"), vec![Operand::Def(Reg::Phys(reg::A6)), Operand::Use(Reg::Phys(reg::A0))]);
    let ret = Inst::new(Kind::Op("RET"), vec![]);
    let mut insts = vec![load];
    insts.extend(uses_a6.then_some(touch_a6));
    insts.push(ret);
    function.blocks.push(Block { id: 0, insts, phis: Vec::new() });
    function
}

#[test]
fn an_argument_past_sp_reach_with_a6_allocated_is_named() {
    let error = finish(&function(true), &Assignment(Vec::new()), 0, &[], Cpu::M68000, false)
        .expect_err("no register names the argument");
    assert!(error.contains("an argument is past the reach of (d16,%sp) and %a6 is allocated"), "{error}");
}

#[test]
fn an_argument_past_sp_reach_with_a6_free_is_addressed_from_a6() {
    finish(&function(false), &Assignment(Vec::new()), 0, &[], Cpu::M68000, false).expect("a6 takes the frame");
}
