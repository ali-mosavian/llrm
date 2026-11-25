//! An argument that is a PC-relative symbol address was pushed as `lea (d,%pc),
//! %a0; move.l %a0, -(%sp)`, 6 B; `pea (d,%pc)` is 4 B and needs no register
//! (160 call sites in the QB modules, 2 B each).

use llrm_m68k::callpush::{Address, plan, symbol_push};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::print;
use llrm_m68k::mfunc::{Inst, Kind, Operand, Reg};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::timing::Price;

#[test]
fn a_pc_relative_address_argument_is_pushed_with_pea() {
    let sp = || Operand::Use(Reg::Phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let insts = [
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(4)]),
        Inst::new(Kind::Op("LEA32q"), vec![Operand::Def(Reg::Phys(reg::A0)), Operand::Symbol("s".to_owned(), 0)]),
        Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(Reg::Phys(reg::A0))]),
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(4), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let found = plan(&refs, &blocks, &Assignment::default(), cpu, Price::Size, true, true, |area| {
            Ok(if area > 0 { (4, 8) } else { (0, 0) })
        })
        .expect("plans");
        assert_eq!(found.address_push(2), Some(Address::Symbol(1)), "{cpu:?}");
    }
}

#[test]
fn pea_of_a_pc_relative_symbol_is_four_bytes() {
    let inst = symbol_push("s", 0);
    assert!(print(&inst).unwrap().starts_with("pea\t"));
    assert_eq!(encode(&inst).unwrap().bytes.len(), 4);
}
