//! `flow::machine_with` builds its phase list for m68k from the target's own
//! facts, as it does for arm64: what the convergence (step 5) runs the function
//! through once it is in SSA form. A failure names the fact m68k still lacks.

use llrm_codegen::backend::{classes::RegisterClasses, cpu::tuned_for, peep::Rules, target::Segments};
use llrm_m68k::target::M68k;
use llrm_target::Target;

#[test]
fn machine_with_builds_for_m68k_and_names_what_it_runs() {
    let profile = tuned_for(&M68k, "m68000", false).unwrap_or_else(|why| panic!("{why}"));
    let regs = llrm_lir::registers::Regs(M68k.registers());
    let segments = Segments::of(&M68k.machine(), regs, M68k.operand_bytes());
    let phases = llrm_codegen::flow::machine_with(
        &Default::default(),
        None,
        None,
        None,
        false,
        profile,
        &segments,
        &std::rc::Rc::new(RegisterClasses::of(&M68k)),
        &Rules::NONE,
        &M68k.frame_registers(),
        llrm_codegen::backend::shadow::Selector::Old,
        &M68k.omitted_phases().iter().map(|(name, _)| *name).collect::<Vec<_>>(),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let names: Vec<&str> = phases.iter().map(|phase| phase.class_name()).collect();
    for kept in ["OptimizePHIs", "DeadMachineInstructionElim", "MachineLICM", "MachineCSE", "MachineSink", "RegAlloc"] {
        assert!(names.contains(&kept), "{kept} is not built for m68k: {names:?}");
    }
}
