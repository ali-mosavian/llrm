//! A reload of a spill slot followed by a store of the same register back to
//! it (`movem.l (44,%sp),%d3; movem.l %d3,(44,%sp)`) reached the output in 38
//! of 300 generated programs: the two had different virtual registers sharing
//! one physical one, which the virtual-register check missed. The pin's
//! `RemoveDeadStores` compares physical registers.

use llrm_lir::registers::RegId;
use llrm_m68k::mfunc::Class;
use llrm_m68k::regalloc::M68kRegisters;
use llrm_spike_mf::mfunc::Reg;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::spill::remove_dead_slot_stores;
use llrm_spike_mf::{Assignment, Block, Inst, Kind, MFunction, Operand};

const SLOT: i32 = 5;

fn reader(reg: u32) -> Inst {
    Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Virt(reg))])
}

fn writer(reg: u32) -> Inst {
    Inst::new(Kind::Op("DEF"), vec![Operand::Def(Reg::Virt(reg))])
}

/// `v0` is reloaded from the slot and `v1` stored to it, both in d0; the
/// instructions after follow.
fn run(after: Vec<Inst>) -> usize {
    let d0 = llrm_m68k::abi::register("d0");
    let mut insts = vec![
        <M68kRegisters as Target>::spill_load(Class::Dr32, Reg::Virt(0), SLOT).unwrap(),
        <M68kRegisters as Target>::spill_store(Class::Dr32, Reg::Virt(1), SLOT).unwrap(),
    ];
    insts.extend(after);
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32; 3];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    let assignment = Assignment(vec![d0; 3]);
    let removed =
        remove_dead_slot_stores::<M68kRegisters>(&mut function, &assignment, &llrm_m68k::REGISTER_INFO, &[SLOT]);
    assert_eq!(function.blocks[0].insts.len(), 2 + 1 - removed);
    removed
}

#[test]
fn a_store_of_the_reloaded_register_goes_and_the_load_stays_when_it_is_read() {
    assert_eq!(run(vec![reader(1)]), 1);
}

#[test]
fn the_load_goes_too_when_the_register_is_not_read_again() {
    assert_eq!(run(vec![writer(2)]), 2);
}

#[test]
fn a_store_of_another_register_stays() {
    let d0 = llrm_m68k::abi::register("d0");
    let d1 = llrm_m68k::abi::register("d1");
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Dr32; 2];
    function
        .blocks
        .push(
            Block {
                id: 0,
                insts: vec![
                    <M68kRegisters as Target>::spill_load(Class::Dr32, Reg::Virt(0), SLOT).unwrap(),
                    <M68kRegisters as Target>::spill_store(Class::Dr32, Reg::Virt(1), SLOT).unwrap(),
                ],
                phis: vec![],
            },
        );
    let assignment = Assignment(vec![d0, d1]);
    let _: &[RegId] = &assignment.0;
    let removed =
        remove_dead_slot_stores::<M68kRegisters>(&mut function, &assignment, &llrm_m68k::REGISTER_INFO, &[SLOT]);
    assert_eq!(removed, 0);
}

/// Seed 296 at the 68020: `movem.l (N,%sp),%dK` straight into `movem.l
/// %dK,(N,%sp)`, a reload stored back to its own slot.
#[test]
fn a_generated_program_has_no_reload_stored_back_to_its_slot() {
    use llrm_m68k::driver::{Level, assembly, cpu_of};
    let text =
        assembly(include_str!("data/seed296-slot-pair.ll"), cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(lines.iter().any(|line| line.starts_with("movem.l\t(") && line.contains("%sp)")), "premise: it spills");
    let pairs = lines
        .windows(2)
        .filter(|w| {
            let load = w[0].strip_prefix("movem.l\t").and_then(|rest| rest.split_once(", "));
            let store = w[1].strip_prefix("movem.l\t").and_then(|rest| rest.split_once(", "));
            matches!(
                (load, store),
                (Some((slot, reg)), Some((reg2, slot2))) if slot == slot2 && reg == reg2
            )
        })
        .count();
    assert_eq!(pairs, 0, "reload stored straight back: {pairs}");
}
