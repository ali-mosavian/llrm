//! `M68kMachine::registers` (14) and `call_registers` (10) were constants
//! beside the allocator's file and `calling.toml`: a change to either left the
//! mid-end pricing spills and unrolls against a register file the allocator no
//! longer had. They are the allocator's candidates, and the ones the ABI keeps.

use llrm_m68k::abi;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::machine::M68kMachine;
use llrm_m68k::mfunc::Class;
use llrm_m68k::regalloc::M68kRegisters;
use llrm_mir::target::Machine;
use llrm_spike_mf::regalloc::Target;

#[test]
fn the_machine_counts_the_registers_the_allocator_can_give_and_the_abi_keeps() {
    let free = M68kRegisters::candidates(Class::Xr32);
    let kept: Vec<_> = abi::c().preserved.iter().map(|one| abi::register(&one.full)).collect();
    let across = free.iter().filter(|one| kept.contains(one)).count() as i64;
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let machine = M68kMachine { cpu };
        assert_eq!(machine.registers(), free.len() as i64, "{cpu:?}: d0-d7 and a0-a5, the frame pointer a6 excluded");
        assert_eq!(machine.call_registers(), across, "{cpu:?}: d2-d7 and a2-a5, the ones calling.toml preserves");
    }
}
