//! `f(dst + 5, src + 1, 1)` kept its three argument stores at `-Os`: a store of a
//! symbol's address was not an argument store, so the call stayed 22 B of
//! `move.l #k, (d,%sp)` where three pushes and the cleanup are 20 B
//! (memory_expansion, 900 calls, ~800 B of its +7.9 KB against gcc).

use llrm_m68k::callpush::plan;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{Inst, Kind, Operand, Reg};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::timing::Price;

#[test]
fn a_symbol_address_argument_is_pushed() {
    let sp = || Operand::Use(Reg::Phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let insts = [
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(12)]),
        Inst::new(Kind::Op("MOV32pi"), vec![Operand::Imm(8), sp(), Operand::Imm(1)]),
        Inst::new(Kind::Op("MOV32pi"), vec![Operand::Imm(4), sp(), Operand::Symbol("src".to_owned(), 1)]),
        Inst::new(Kind::Op("MOV32ji"), vec![sp(), Operand::Symbol("dst".to_owned(), 5)]),
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(12), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    let found = plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, Price::Size, true, |area| {
        Ok(if area > 0 { (4, 8) } else { (0, 0) })
    })
    .expect("plans");
    assert!((1..=3).all(|store| found.is_push(store)), "the stores stay stores");
}
