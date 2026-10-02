//! Braun & Hack spilling on SSA, checked at its own input.

use std::collections::BTreeSet;

use crate::analysis::frequency::Frequency;
use crate::backend::frame::Frame;
use crate::backend::regalloc_input::{before_phase, Calls};
use crate::backend::{ssaspill, target};
use crate::model::ir::Loc;
use crate::model::lir::{BlockOdds, LirBody};

/// The widest a value or anything a phi joins it with is read or written, by operands alone.
fn joined_width(body: &LirBody, value: u32) -> u32 {
    let mut web: BTreeSet<u32> = BTreeSet::from([value]);
    loop {
        let before = web.len();
        for block in &body.blocks {
            for phi in &block.phis {
                let members: Vec<u32> = std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, one)| *one)).collect();
                if members.iter().any(|one| web.contains(one)) {
                    web.extend(members);
                }
            }
        }
        if web.len() == before {
            break;
        }
    }
    body.insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .filter_map(|place| if let Loc::Held(held) = place { Some(*held) } else { None })
        .filter(|held| web.contains(&held.value))
        .map(|held| held.width)
        .max()
        .unwrap_or(0)
}

/// A 32-bit accumulator carried around two loops is named only by phis where
/// it is spilled; its slot was sized at the default word and its reload lost
/// the high half (the loop corpus's conc3 pressure3 counterother outer case
/// reported -29181 for 36355).
#[test]
fn test_a_value_only_phis_name_is_spilled_at_its_full_width() {
    let (body, _) = before_phase(Calls::C, "phiwidth.ll", "_f", "486", "SsaSpill");
    let mut frame = Frame::new(0);
    let spilled = ssaspill::spilled(&body, &mut frame, &target::BUILT_IN).expect("spills");
    let stores: Vec<(u32, u32)> = spilled
        .insns()
        .iter()
        .filter(|one| one.spill_store)
        .filter_map(|one| match one.what.as_ref().map(|what| (&what.dests[..], &what.sources[..])) {
            Some(([Loc::Mem(cell)], [Loc::Held(held)])) => Some((held.value, cell.width)),
            _ => None,
        })
        .collect();
    let wide: Vec<(u32, u32)> = stores.iter().copied().filter(|(value, _)| joined_width(&body, *value) == 4).collect();
    assert!(!wide.is_empty(), "premise: a 32-bit value is spilled: {stores:?}");
    for (value, width) in wide {
        assert_eq!(width, 4, "value#{value} is stored in a {width}-byte slot");
    }
}

/// The bridges SSA destruction puts on critical edges carried no edge odds:
/// a latch with both edges bridged split 50/50, so a loop run 32 times read
/// as run twice, and the cost channel reported FRACLINE at 2280 executed
/// instructions for Greedy's 22355.
#[test]
fn test_assignment_keeps_block_frequencies_across_its_bridges() {
    let (body, mut phases) = before_phase(Calls::C, "fibswap.ll", "_fib", "486", "SsaSpill");
    let mut spilled = phases[0].transform(body).expect("spills");
    // FRACLINE's shape: the latch exits straight into a join, and isel stated both its odds.
    let latch = spilled.blocks.iter().find(|block| block.succ.contains(&block.at)).expect("premise: a self loop").at;
    let exit = *spilled.blocks.iter().find(|block| block.at == latch).expect("the latch").succ.iter().find(|to| **to != latch).expect("an exit");
    let forward = spilled.blocks.iter().find(|block| block.at == exit).expect("the exit").clone();
    assert!(forward.phis.is_empty() && forward.insns.len() == 1 && forward.succ.len() == 1, "premise: the exit only jumps on");
    let join = forward.succ[0];
    spilled.blocks.retain(|block| block.at != exit);
    for block in &mut spilled.blocks {
        if block.at == latch {
            block.succ = block.succ.iter().map(|to| if *to == exit { join } else { *to }).collect();
            block.insns = block.insns.iter().map(|one| match &one.what {
                Some(what) if what.target == Some(exit) => {
                    let mut made = (**one).clone();
                    made.what = Some(crate::model::ir::Semantics { target: Some(join), ..what.clone() });
                    std::sync::Arc::new(made)
                }
                _ => std::sync::Arc::clone(one),
            }).collect();
        }
        for phi in &mut block.phis {
            for (from, _) in &mut phi.incoming {
                if *from == exit {
                    *from = latch;
                }
            }
        }
    }
    spilled.odds.taken.insert((latch, latch), (0.96875 * BlockOdds::CERTAIN) as u32);
    spilled.odds.taken.insert((latch, join), (0.03125 * BlockOdds::CERTAIN) as u32);
    let (done, _) = ssaspill::assigned(&spilled, &target::BUILT_IN).expect("assigns");
    let had: BTreeSet<i64> = spilled.blocks.iter().map(|block| block.at).collect();
    let bridged = done.blocks.iter().find(|block| block.at == latch).expect("the latch").succ.iter().filter(|to| !had.contains(to)).count();
    assert_eq!(bridged, 2, "premise: both edges of the latch are bridged");
    let (before, after) = (Frequency::of(&spilled), Frequency::of(&done));
    for at in had {
        let (was, is) = (before.block(at), after.block(at));
        assert!((was - is).abs() <= 1e-6 * was.max(1.0), "block {at:#x} runs {is} times for {was}");
    }
}
