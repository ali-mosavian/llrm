//! POC for DEVIATE Z80S1: the pin's pre-RA `MachineScheduler`
//! (`llrm_codegen::backend::machinesched::scheduled`) run on the z80's function
//! after two-address conversion and before the allocator, as the pin orders
//! them. The function goes to LIR, the scheduler reorders each region, and
//! the instructions are put back in the new order. Off unless
//! `LLRM_Z80_MSCHED` is set.

use llrm_lir::registers::Regs;
use llrm_spike_mf::tolir::to_lir;

use crate::mfunc::MFunction;
use crate::regalloc::Z80Registers;
use crate::REGISTER_INFO;

/// Reorders the regions of `function` by pressure. Blocks the converter does
/// not map one to one are left as they are.
pub fn schedule(function: &mut MFunction) -> Result<usize, String> {
    let _lanes = llrm_codegen::backend::lanes::enter(Regs(&REGISTER_INFO));
    let mut converted = to_lir::<Z80Registers>(function, Regs(&REGISTER_INFO))?;
    if std::env::var_os("LLRM_Z80_MSCHED_LOG").is_some() {
        let with = converted.with_effects(function, Regs(&REGISTER_INFO));
        for (index, block) in with.blocks.iter().enumerate() {
            let insns = block.insns.to_vec();
            let regions = llrm_codegen::backend::machinesched::regions(&insns, Regs(&REGISTER_INFO), with.bits);
            let pinned = insns.iter().filter(|one| !one.requires.is_empty() || !one.delivers.is_empty()).count();
            eprintln!("  block {index}: insns={} pinned={pinned} regions={:?}", insns.len(), regions.iter().map(|r| r.len()).collect::<Vec<_>>());
        }
    }
    converted.schedule(function, Regs(&REGISTER_INFO));
    let mut moved = 0;
    for (index, block) in converted.body.blocks.iter().enumerate() {
        let order: Vec<(usize, usize)> = block.insns.iter().map(|one| converted.origin[(one.at / 2) as usize]).collect();
        let originals = function.blocks[index].insts.len();
        let mut seen = vec![false; originals];
        let whole = order.len() == originals
            && order.iter().all(|&(b, i)| b == index && i < originals && !std::mem::replace(&mut seen[i], true));
        if !whole {
            continue;
        }
        if order.iter().enumerate().any(|(at, &(_, i))| at != i) {
            let old = std::mem::take(&mut function.blocks[index].insts);
            function.blocks[index].insts = order.iter().map(|&(_, i)| old[i].clone()).collect();
            moved += 1;
        }
    }
    Ok(moved)
}
