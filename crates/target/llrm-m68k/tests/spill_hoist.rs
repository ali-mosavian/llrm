//! A reload between a compare and its branch stayed the 6 B `movem` and kept its slot out of an address register:
//! the `move` that would replace it sets the codes the branch reads. known_loads_walks had 10 such reloads in `f`.

use llrm_m68k::mfunc::{Block, Class, MFunction};
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::reg;
use llrm_m68k::spillhoist::hoist_spills;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

fn phys(register: llrm_lir::registers::RegId) -> Reg {
    Reg::Phys(register)
}

fn compare(left: llrm_lir::registers::RegId, right: llrm_lir::registers::RegId) -> Inst {
    let mut inst = Inst::new(Kind::Op("CMP8dd"), vec![Operand::Use(phys(left)), Operand::Use(phys(right))]);
    inst.sets_flags = true;
    inst
}

fn reload(into: llrm_lir::registers::RegId) -> Inst {
    let mut inst =
        Inst::new(Kind::Op("MOVM32mp_P"), vec![Operand::Def(phys(into)), Operand::Imm(0), Operand::Frame(3)]);
    inst.may_load = true;
    inst
}

fn branch() -> Inst {
    let mut inst = Inst::new(Kind::Op("Bne8"), vec![Operand::Block(1)]);
    inst.reads_flags = true;
    inst.terminator = true;
    inst
}

fn kinds(insts: &[Inst]) -> Vec<String> {
    insts.iter().map(|inst| format!("{:?}", inst.kind)).collect()
}

fn run(insts: Vec<Inst>) -> Vec<String> {
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32; 1];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    hoist_spills(&mut function, &Assignment(vec![reg::D0]));
    kinds(&function.blocks[0].insts)
}

#[test]
fn a_reload_between_a_compare_and_its_branch_goes_above_the_compare() {
    let order = run(vec![compare(reg::D3, reg::D5), reload(reg::D0), branch()]);
    assert_eq!(order, ["Op(\"MOVM32mp_P\")", "Op(\"CMP8dd\")", "Op(\"Bne8\")"]);
}

#[test]
fn a_reload_into_a_register_the_compare_reads_stays() {
    let order = run(vec![compare(reg::D0, reg::D5), reload(reg::D0), branch()]);
    assert_eq!(order, ["Op(\"CMP8dd\")", "Op(\"MOVM32mp_P\")", "Op(\"Bne8\")"]);
}
