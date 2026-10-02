//! Braun & Hack spilling on SSA, checked at its own input.

use std::collections::BTreeSet;

use crate::backend::frame::Frame;
use crate::backend::regalloc_input::{before_phase, Calls};
use crate::backend::{ssaspill, target};
use crate::model::ir::Loc;
use crate::model::lir::LirBody;

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

/// A phi whose arguments are all one value stayed a value of its own: SsaSpill
/// held it apart from the value it renames, stored it to a slot on every
/// iteration and spilled the original around it (deedlines PLASMABLOBS' 160-trip
/// loop, +7.2% executed instructions).
#[test]
fn test_spilling_leaves_no_phi_that_names_one_value() {
    let trivial = |body: &LirBody| {
        body.blocks.iter().flat_map(|block| &block.phis).filter(|phi| phi.incoming.iter().all(|(_, value)| *value == phi.incoming[0].1)).count()
    };
    let (body, mut phases) = before_phase(Calls::C, "trivialphi.ll", "_bench_shellsort", "486", "SsaSpill");
    assert!(trivial(&body) > 0, "premise: the input has a phi of one value");
    let spilled = phases[0].transform(body).expect("spills");
    assert_eq!(trivial(&spilled), 0);
}
