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

fn stepped() -> Vec<Inst> {
    let phys = |r| Reg::Phys(r);
    vec![
        Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(phys(reg::HL)), Operand::Imm(2)]),
        Inst::new(
            Kind::Op("ADD16"),
            vec![Operand::Def(phys(reg::HL)), Operand::Use(phys(reg::HL)), Operand::Use(phys(reg::DE))],
        ),
        Inst::new(Kind::Copy, vec![Operand::Def(phys(reg::DE)), Operand::Use(phys(reg::HL))]),
        Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(phys(reg::HL)), Operand::Imm(0)]),
        Inst::new(Kind::Op("RET"), vec![Operand::Use(phys(reg::DE))]),
    ]
}

/// `de` + 2 was `ld hl, #2; add hl, de; ex de, hl` (5 B, 29 T) when the sum
/// goes back into `de` and nothing reads `hl` or the carry after.
#[test]
fn a_pair_stepped_by_two_through_hl_is_two_incs() {
    let asm = text(stepped());
    assert!(asm.contains("inc de\ninc de") && !asm.contains("add hl"), "{asm}");
}

/// A variant priced with the printer's peepholes changed which allocation won:
/// copy_runs -O2 came out 2 B and 600 T worse than before them, the price
/// differing by 0.09%. While a variant is priced the sequence stays whole.
#[test]
fn a_variant_is_priced_before_the_printers_peepholes() {
    llrm_z80::expand::PRICING.with(|one| one.set(true));
    let asm = text(stepped());
    llrm_z80::expand::PRICING.with(|one| one.set(false));
    assert!(asm.contains("add hl, de") && !asm.contains("inc de"), "{asm}");
}

fn reload_then_spill(between: Option<Inst>) -> String {
    let phys = |r| Reg::Phys(r);
    let mut insts = vec![Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(phys(reg::BC)), Operand::Frame(1)])];
    insts.extend(between);
    insts.push(Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(1), Operand::Use(phys(reg::BC))]));
    insts.push(Inst::new(Kind::Op("RET"), vec![Operand::Use(phys(reg::BC))]));
    let mut function = MFunction::new("f");
    function.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    let lines = finish(&function, &Assignment(Vec::new()), 0, &[(1, 2), (2, 2)], Default::default()).expect("prints");
    lines
        .iter()
        .filter_map(|line| if let Line::Inst(text) = line { Some(text.as_str()) } else { None })
        .collect::<Vec<_>>()
        .join("\n")
}

/// idiv_loop_os -Os printed `ld c,-10(ix); ld b,-9(ix); ld -10(ix),c;
/// ld -9(ix),b`: a slot stored back what it was just loaded into, 6 B of 19 T
/// a byte. The spill of an unchanged reload is gone.
#[test]
fn a_slot_is_not_stored_back_what_was_just_loaded_from_it() {
    let asm = reload_then_spill(None);
    assert!(asm.contains("ld c, -2(ix)") && !asm.contains("ld -2(ix), c"), "{asm}");
}

/// A value changed between the load and the store is stored.
#[test]
fn a_changed_value_is_stored_back() {
    let changed =
        Inst::new(Kind::Op("INC16"), vec![Operand::Def(Reg::Phys(reg::BC)), Operand::Use(Reg::Phys(reg::BC))]);
    let asm = reload_then_spill(Some(changed));
    assert!(asm.contains("ld -2(ix), c") && asm.contains("ld -1(ix), b"), "{asm}");
}
