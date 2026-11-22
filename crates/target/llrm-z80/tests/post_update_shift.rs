//! The post-update fold with the `ld t, #2` of the step before the access.

use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_z80::reg;

fn phys(r: llrm_lir::registers::RegId) -> Reg {
    Reg::Phys(r)
}

/// `lo = (hl); hl += 2; hi = (hl)` with the `ld bc, #2` of the step ahead of
/// the first load. Folding removed the `ld` and the `add` and then wrote the
/// merged load over the slot the first load had, which by then held the second:
/// the `float` pair `vf[j]` loaded its low word twice and passed an address for
/// its high word.
#[test]
fn a_step_whose_constant_precedes_the_access_merges_into_that_access() {
    let mut sets = Inst::new(Kind::Op("CP16i"), vec![Operand::Def(phys(reg::HL)), Operand::Imm(0)]);
    sets.sets_flags = true;
    let insts = vec![
        Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(phys(reg::BC)), Operand::Imm(2)]),
        Inst::new(
            Kind::Op("LD16rm"),
            vec![Operand::Def(phys(reg::DE)), Operand::Use(phys(reg::HL)), Operand::Def(phys(reg::A))],
        ),
        Inst::new(
            Kind::Op("ADD16"),
            vec![Operand::Def(phys(reg::HL)), Operand::Use(phys(reg::HL)), Operand::Use(phys(reg::BC))],
        ),
        Inst::new(
            Kind::Op("LD16rm"),
            vec![Operand::Def(phys(reg::HL)), Operand::Use(phys(reg::HL)), Operand::Def(phys(reg::A))],
        ),
        sets,
        Inst::new(Kind::Op("RET"), vec![Operand::Use(phys(reg::HL)), Operand::Use(phys(reg::DE))]),
    ];
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    assert!(llrm_z80::post_update::fold(&mut function, &Assignment(Vec::new())));
    let kinds: Vec<_> = function.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds[0], Kind::Op("LD16rm_pi"), "{kinds:?}");
    assert_eq!(kinds[1], Kind::Op("LD16rm"), "{kinds:?}");
}
