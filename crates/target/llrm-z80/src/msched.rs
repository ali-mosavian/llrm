//! POC for DEVIATE Z80S1: the pin's pre-RA `MachineScheduler`
//! (`llrm_codegen::backend::machinesched::scheduled`) run on the z80's function
//! after two-address conversion and before the allocator, as the pin orders
//! them. The function goes to LIR, the scheduler reorders each region, and
//! the instructions are put back in the new order. Off unless
//! `LLRM_Z80_MSCHED` is set.

use std::rc::Rc;

use llrm_codegen::backend::regpressure::Model;
use llrm_lir::Loc;
use llrm_lir::registers::Regs;
use llrm_spike_mf::tolir::to_lir;

use crate::mfunc::MFunction;
use crate::regalloc::Z80Registers;
use crate::REGISTER_INFO;

/// The bytes of general registers the allocator can hand out: `b c d e h l`
/// (`a` is the scratch of the ALU ops).
const BYTES: u32 = 6;

/// Reorders the regions of `function` by pressure. Blocks the converter does
/// not map one to one are left as they are.
pub fn schedule(function: &mut MFunction) -> Result<usize, String> {
    let bytes: u32 = std::env::var("LLRM_Z80_MSCHED_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(BYTES);
    let converted = to_lir::<Z80Registers>(function, Regs(&REGISTER_INFO))?;
    let mut body = converted.body.clone();
    body.registers = Some(Regs(&REGISTER_INFO));
    let scheduled = llrm_codegen::backend::machinesched::scheduled(&body, bytes as usize, |body, counted| {
        let mut widths: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
        for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
            if let Some(what) = &one.what {
                for place in what.dests.iter().chain(&what.sources) {
                    if let Loc::Held(held) = place {
                        let width = widths.entry(held.value).or_insert(held.width);
                        *width = (*width).max(held.width);
                    }
                }
            }
        }
        let mut model = Model::new(vec![bytes]);
        let one: Rc<[usize]> = Rc::from(vec![0]);
        for (value, width) in widths {
            if counted(value) {
                model.count(value, width, one.clone());
            }
        }
        model
    });
    let mut moved = 0;
    for (index, block) in scheduled.blocks.iter().enumerate() {
        let order: Vec<(usize, usize)> = block.insns.iter().map(|one| converted.origin[(one.at / 2) as usize]).collect();
        let originals = function.blocks[index].insts.len();
        let mut seen = vec![false; originals];
        let whole = order.len() == originals
            && order.iter().all(|&(b, i)| b == index && i < originals && !std::mem::replace(&mut seen[i], true));
        if std::env::var_os("LLRM_Z80_MSCHED_LOG").is_some() {
            let insns = converted.body.blocks[index].insns.to_vec();
            let regions = llrm_codegen::backend::machinesched::regions(&insns, body.regs(), body.bits);
            let none = insns.iter().filter(|one| one.what.is_none()).count();
            for one in insns.iter().take(14) {
                eprintln!(
                    "    at={} op={:?} name={:?} clob={} req={} del={} grp={:?} call={} fa={} sp={} rl={}",
                    one.at,
                    one.what.as_ref().map(|w| w.op),
                    one.what.as_ref().and_then(|w| w.name.clone()),
                    one.clobbers.len(),
                    one.requires.len(),
                    one.delivers.len(),
                    one.group,
                    one.call.is_some(),
                    one.frame_adjust,
                    one.spill_store,
                    one.spill_reload
                );
            }
            let no_effect = insns.iter().filter(|one| llrm_codegen::backend::liveness::effect(body.regs(), body.bits, one).is_none()).count();
            let pinned = insns.iter().filter(|one| !one.requires.is_empty() || !one.delivers.is_empty()).count();
            eprintln!("  insns={} effect_none={no_effect} pinned={pinned}", insns.len());
            eprintln!("  regions={:?} what_none={none}", regions.iter().map(|r| r.len()).collect::<Vec<_>>());
        }
        if std::env::var_os("LLRM_Z80_MSCHED_LOG").is_some() {
            let same = block.insns.iter().zip(converted.body.blocks[index].insns.iter()).all(|(a, b)| a.at == b.at);
            eprintln!("  block {index}: lir={} mf={originals} whole={whole} same_order={same}", block.insns.len());
        }
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
