//! `f(x != 0, x)`: `callpush::foldable` dropped the load of `x` into its push
//! though an `sne` between them read the load's codes; the `sne` then read
//! whatever ran before the load. A reader of the codes keeps the load.
//! Also pins what the audit assumes of the pin's descriptions: `ADDX`/`SUBX`
//! read the codes they set, so passes that stop at a setter must test the
//! reader first (a later change to the description fails here).

use llrm_m68k::callpush::plan;
use llrm_m68k::dagisel::{hooks::Cpu, local};
use llrm_m68k::mfunc::{Inst, Kind, Operand, Reg, op_inst};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::timing::Price;

fn phys(register: llrm_lir::registers::RegId) -> Reg {
    Reg::Phys(register)
}

fn folded(between: Vec<Inst>) -> Option<usize> {
    let sp = || Operand::Use(phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let mut load =
        Inst::new(Kind::Op("MOV32rp"), vec![Operand::Def(phys(reg::D0)), Operand::Imm(0), Operand::Use(phys(reg::A2))]);
    load.may_load = true;
    load.sets_flags = true;
    let mut insts = vec![Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(4)]), load];
    insts.extend(between);
    insts.push(Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(phys(reg::D0))]));
    let store = insts.len() - 1;
    insts.push(call);
    insts.push(Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(4), Operand::Imm(0)]));
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, Price::Rate, true, |area| {
        Ok(if area > 0 { (4, 8) } else { (0, 0) })
    })
    .expect("plans")
    .folded_load(store)
}

/// Premise: with nothing reading the load's codes the load does fold.
#[test]
fn a_load_with_no_reader_of_its_codes_folds_into_the_push() {
    assert_eq!(folded(vec![]), Some(1));
}

#[test]
fn a_load_whose_codes_are_read_stays() {
    assert_eq!(folded(vec![sne()]), None);
}

/// A `tst` between the load and the reader gives the reader its own codes
/// (wc_mixed 68000 -O1: the first form of the guard kept the load, +2 B).
#[test]
fn a_reader_of_a_later_setters_codes_leaves_the_load_folding() {
    let mut tst = Inst::new(Kind::Op("TST32d"), vec![Operand::Use(phys(reg::D3))]);
    tst.sets_flags = true;
    assert_eq!(folded(vec![tst, sne()]), Some(1));
}

fn sne() -> Inst {
    let mut sne = Inst::new(Kind::Op("SETd8eq"), vec![Operand::Def(phys(reg::D1))]);
    sne.reads_flags = true;
    sne
}

#[test]
fn addx_and_subx_read_and_set_the_codes() {
    for name in ["ADDX32dd", "SUBX32dd", "ADDX16dd", "SUBX16dd", "ADDX8dd", "SUBX8dd"] {
        let desc = local::desc(local::code_of(name).expect("known")).expect("described");
        let inst = op_inst(name, desc, vec![]);
        assert!(inst.sets_flags && inst.reads_flags, "{name}: sets {} reads {}", inst.sets_flags, inst.reads_flags);
    }
}

/// `f(p + 1)` then an `addx`: the `pea` leaves the codes alone, so dropping
/// the `addq` that made `p + 1` changes the `X` the `addx` reads. The scan
/// after the store stopped at the `addx` as at a plain setter.
fn pea_with(after: Inst) -> Option<(llrm_lir::registers::RegId, i64)> {
    let sp = || Operand::Use(phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let mut add = Inst::new(
        Kind::Op("ADDQ32di"),
        vec![Operand::Def(phys(reg::D0)), Operand::Use(phys(reg::A3)), Operand::Imm(1)],
    );
    add.sets_flags = true;
    let insts = vec![
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(4)]),
        add,
        Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(phys(reg::D0))]),
        after,
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(4), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = vec![0; insts.len()];
    plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, Price::Rate, true, |area| {
        Ok(if area > 0 { (4, 8) } else { (0, 0) })
    })
    .expect("plans")
    .address_push(2)
}

#[test]
fn an_address_made_by_an_add_becomes_a_pea_when_nothing_reads_the_codes() {
    let mut setter = Inst::new(Kind::Op("ADD32dd"), vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3))]);
    setter.sets_flags = true;
    assert!(pea_with(setter).is_some());
}

#[test]
fn an_addx_after_the_store_keeps_the_add_that_set_x() {
    let desc = local::desc(local::code_of("ADDX32dd").expect("known")).expect("described");
    let addx = op_inst(
        "ADDX32dd",
        desc,
        vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3)), Operand::Use(phys(reg::D4))],
    );
    assert_eq!(pea_with(addx), None);
}

/// `move.l %d2,%a0; lea (4,%a0),%a1; move.l %a1,%d2` becomes `addq.l #4,%d2`,
/// which sets `X`; the `lea` trio leaves it. The scan after the trio stopped
/// at an `addx` as at a plain setter, so the carry it read was lost.
fn after_a_round_trip(after: Inst) -> usize {
    use llrm_m68k::addrarith::refine_lea_round_trips;
    use llrm_m68k::mfunc::{Block, MFunction};
    let virt = Reg::Virt;
    let mut lea = Inst::new(Kind::Op("LEA32p"), vec![Operand::Def(virt(1)), Operand::Imm(4), Operand::Use(virt(0))]);
    lea.may_load = false;
    let insts = vec![
        Inst::new(Kind::Copy, vec![Operand::Def(virt(0)), Operand::Use(phys(reg::D2))]),
        lea,
        Inst::new(Kind::Copy, vec![Operand::Def(phys(reg::D2)), Operand::Use(virt(1))]),
        after,
    ];
    let mut function = MFunction::new("f");
    function.classes = vec![llrm_m68k::mfunc::Class::Ar32; 2];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    let assignment = Assignment(vec![reg::A0, reg::A1]);
    refine_lea_round_trips(&mut function, &assignment, Cpu::M68000, false);
    function.blocks[0].insts.len()
}

#[test]
fn a_round_trip_becomes_an_add_when_nothing_reads_the_codes() {
    let mut setter = Inst::new(Kind::Op("ADD32dd"), vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3))]);
    setter.sets_flags = true;
    assert_eq!(after_a_round_trip(setter), 2);
}

#[test]
fn an_addx_after_a_round_trip_keeps_the_lea() {
    let desc = local::desc(local::code_of("ADDX32dd").expect("known")).expect("described");
    let addx = op_inst(
        "ADDX32dd",
        desc,
        vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3)), Operand::Use(phys(reg::D4))],
    );
    assert_eq!(after_a_round_trip(addx), 4);
}

/// `move.l %a0,%d1; addq.l #4,%d1; (addx); move.l (%d1),%d0`: the add becomes
/// an address-register add that leaves the codes alone, so an `addx` after it
/// read an `X` the `addq` no longer sets.
fn after_a_pointer_add(after: Inst) -> usize {
    use llrm_m68k::addrarith::address_adds;
    use llrm_m68k::mfunc::{Block, Class, MFunction};
    let virt = Reg::Virt;
    let mut add = Inst::new(Kind::Op("ADDQ32di"), vec![Operand::Def(virt(2)), Operand::Use(virt(1)), Operand::Imm(4)]);
    add.sets_flags = true;
    let mut load =
        Inst::new(Kind::Op("MOV32dp"), vec![Operand::Def(phys(reg::D0)), Operand::Imm(0), Operand::Use(virt(2))]);
    load.may_load = true;
    let insts = vec![Inst::new(Kind::Copy, vec![Operand::Def(virt(1)), Operand::Use(virt(0))]), add, after, load];
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Ar32, Class::Xr32, Class::Xr32];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    address_adds(&mut function, Cpu::M68000, false, false);
    function.blocks[0].insts.len()
}

#[test]
fn a_pointer_add_becomes_an_address_add_when_nothing_reads_the_codes() {
    let mut setter = Inst::new(Kind::Op("ADD32dd"), vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3))]);
    setter.sets_flags = true;
    assert_eq!(after_a_pointer_add(setter), 3);
}

#[test]
fn an_addx_after_a_pointer_add_keeps_the_add() {
    let desc = local::desc(local::code_of("ADDX32dd").expect("known")).expect("described");
    let addx = op_inst(
        "ADDX32dd",
        desc,
        vec![Operand::Def(phys(reg::D4)), Operand::Use(phys(reg::D3)), Operand::Use(phys(reg::D4))],
    );
    assert_eq!(after_a_pointer_add(addx), 4);
}
