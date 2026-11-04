//! A pair stepped by a few bytes through `hl`.

use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::expand::{Line, finish};
use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_z80::reg;

fn text(insts: Vec<Inst>) -> String {
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    let lines = finish(&function, &Assignment(Vec::new()), 0, &[], Default::default()).expect("prints");
    lines
        .iter()
        .filter_map(|line| if let Line::Inst(text) = line { Some(text.as_str()) } else { None })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `de` + 2 was `ld hl, #2; add hl, de; ex de, hl` (5 B, 29 T) when the sum
/// goes back into `de` and nothing reads `hl` or the carry after.
#[test]
fn a_pair_stepped_by_two_through_hl_is_two_incs() {
    let phys = |r| Reg::Phys(r);
    let stepped = vec![
        Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(phys(reg::HL)), Operand::Imm(2)]),
        Inst::new(
            Kind::Op("ADD16"),
            vec![Operand::Def(phys(reg::HL)), Operand::Use(phys(reg::HL)), Operand::Use(phys(reg::DE))],
        ),
        Inst::new(Kind::Copy, vec![Operand::Def(phys(reg::DE)), Operand::Use(phys(reg::HL))]),
        Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(phys(reg::HL)), Operand::Imm(0)]),
        Inst::new(Kind::Op("RET"), vec![Operand::Use(phys(reg::DE))]),
    ];
    let asm = text(stepped);
    assert!(asm.contains("inc de\ninc de") && !asm.contains("add hl"), "{asm}");
}
