//! `f(x, 5)` where the register holding `x` is overwritten after the argument
//! stores and before the call (`add.l %d0, %d5` in place): the pushes waited
//! for the call, found `%d5` gone, and left the call area reserved (`lea
//! (-8,%sp)` and `addq.l #8,%sp` around the loop, 4 B on recdeep). The pushes
//! are made before the instruction that rewrites `%d5`, as late as it allows:
//! a stack-relative spill store between the last argument store and the call
//! keeps its offset (made at the last store it grew by 2 B, 12 seed cells).
//! A register stored last once indexed `last + 1..last` and panicked.

use llrm_m68k::callpush::plan;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{Inst, Kind, Operand, Reg};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;

fn phys(register: llrm_lir::registers::RegId) -> Reg {
    Reg::Phys(register)
}

#[test]
fn pushes_wait_for_the_last_store_not_the_call() {
    let sp = || Operand::Use(phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let insts = [
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(8)]),
        // x at offset 0, 5 at offset 4: not push order
        Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(phys(reg::D5))]),
        Inst::new(Kind::Op("MOV32pi"), vec![Operand::Imm(4), sp(), Operand::Imm(5)]),
        // overwrites %d5 before the call
        Inst::new(Kind::Op("ADD32dr"), vec![Operand::Def(phys(reg::D5)), Operand::Use(phys(reg::D0))]),
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(8), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    let found = plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, false, false, |area| {
        Ok(if area > 0 { (8, 16) } else { (0, 0) })
    })
    .expect("plans");
    assert_eq!(found.deferred_offset(1), Some(0), "x's push is not made");
    assert_eq!(found.deferred_offset(2), Some(4), "the constant's push is not made");
    assert!(found.flushes(3), "the pushes are not made before the rewrite");
    assert!(!found.flushes(2));
}

/// The last store is of a register too: `f(x, y)` stored in the order 0, 4.
#[test]
fn a_register_stored_last_is_pushed_before_the_call() {
    let sp = || Operand::Use(phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let insts = [
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(8)]),
        Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(phys(reg::D5))]),
        Inst::new(Kind::Op("MOV32pr"), vec![Operand::Imm(4), sp(), Operand::Use(phys(reg::D4))]),
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(8), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    let found = plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, false, false, |area| {
        Ok(if area > 0 { (8, 16) } else { (0, 0) })
    })
    .expect("plans");
    assert!(found.flushes(3), "the pushes are not made before the call");
}
