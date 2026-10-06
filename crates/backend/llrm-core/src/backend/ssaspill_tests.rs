//! Braun & Hack spilling on SSA, checked at its own input.

use std::collections::BTreeSet;

use crate::backend::frame::Frame;
use crate::backend::regalloc_input::{before_phase, Calls};
use crate::backend::ssaspill::Prices;
use crate::backend::{ssaspill, target};
use crate::model::ir::Loc;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

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
    let spilled = ssaspill::spilled(&body, &mut frame, &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).expect("spills");
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

/// Under pressure Belady evicted the value used furthest ahead, which was a loop
/// counter or accumulator redefined every iteration: a store each trip, where an
/// invariant already in memory costs only its reload (deedlines PLASMABLOBS spilled
/// its loop counter in a 1M-trip body, +7% executed instructions).
#[test]
fn test_a_value_defined_in_the_loop_is_not_stored_on_every_trip() {
    let (body, mut phases) = before_phase(Calls::C, "hotstore.ll", "_f", "486", "SsaSpill");
    let spilled = phases[0].transform(body).expect("spills");
    let frequency = crate::analysis::frequency::Frequency::of(&spilled);
    let stores = |hot: bool| -> usize {
        spilled
            .blocks
            .iter()
            .filter(|block| (frequency.block(block.at) >= 8.0) == hot)
            .flat_map(|block| &block.insns)
            .filter(|one| one.spill_store)
            .count()
    };
    assert!(stores(false) + stores(true) > 0, "premise: the body spills");
    assert_eq!(stores(true), 0, "a store per iteration");
}

/// `body` with an empty block on every edge, as splitkit and jump threading leave them.
fn with_empty_edge_blocks(body: &LirBody) -> LirBody {
    use crate::model::ir::{Operation, Semantics};
    use crate::model::lir::LirBlock;
    let mut next = body.blocks.iter().map(|block| block.at).max().unwrap_or(0) + 1;
    let mut made: Vec<LirBlock> = Vec::new();
    let mut blocks: Vec<LirBlock> = Vec::new();
    let mut landing: IndexMap<(i64, i64), i64> = IndexMap::default();
    for block in &body.blocks {
        let mut succ = Vec::new();
        for to in &block.succ {
            let at = *landing.entry((block.at, *to)).or_insert_with(|| {
                next += 1;
                made.push(LirBlock { succ: vec![*to], ..LirBlock::new(next - 1, Vec::new()) });
                next - 1
            });
            succ.push(at);
        }
        let insns = block
            .insns
            .iter()
            .map(|one| match &one.what {
                Some(what) if matches!(what.op, Operation::Jump | Operation::Branch) => match what.target.and_then(|to| landing.get(&(block.at, to))) {
                    Some(at) => {
                        let mut changed = (**one).clone();
                        changed.what = Some(Semantics { target: Some(*at), ..what.clone() });
                        std::sync::Arc::new(changed)
                    }
                    None => std::sync::Arc::clone(one),
                },
                _ => std::sync::Arc::clone(one),
            })
            .collect();
        blocks.push(LirBlock { succ, ..block.with_insns(insns) });
    }
    for block in &mut blocks {
        for phi in &mut block.phis {
            for (from, _) in &mut phi.incoming {
                if let Some(at) = landing.iter().find(|((_, to), _)| *to == block.at).and_then(|_| landing.get(&(*from, block.at))) {
                    *from = *at;
                }
            }
        }
    }
    blocks.extend(made);
    body.with_blocks(blocks)
}

/// Whether every reload of a frame slot follows a store of it on every path from the entry.
fn reloads_follow_stores(body: &LirBody) -> Result<(), String> {
    use crate::model::ir::Loc;
    let key = |one: &crate::model::lir::Insn, reads: bool| -> Option<String> {
        let what = one.what.as_ref()?;
        let place = if reads { what.sources.first()? } else { what.dests.first()? };
        matches!(place, Loc::Mem(_)).then(|| format!("{place:?}"))
    };
    let blocks: IndexMap<i64, &crate::model::lir::LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    let all: BTreeSet<String> = body.insns().iter().filter(|one| one.spill_store).filter_map(|one| key(one, false)).collect();
    let mut out: IndexMap<i64, BTreeSet<String>> = body.blocks.iter().map(|block| (block.at, all.clone())).collect();
    loop {
        let mut changed = false;
        for block in &body.blocks {
            let mut stored: BTreeSet<String> = if block.at == body.entry {
                BTreeSet::new()
            } else {
                preds.get(&block.at).map_or(BTreeSet::new(), |from| from.iter().map(|at| out[at].clone()).reduce(|a, b| a.intersection(&b).cloned().collect()).unwrap_or_default())
            };
            for one in &block.insns {
                if one.spill_store {
                    stored.extend(key(one, false));
                }
            }
            if out[&block.at] != stored {
                out.insert(block.at, stored);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for (at, block) in &blocks {
        let mut stored: BTreeSet<String> = if *at == body.entry {
            BTreeSet::new()
        } else {
            preds.get(at).map_or(BTreeSet::new(), |from| from.iter().map(|from| out[from].clone()).reduce(|a, b| a.intersection(&b).cloned().collect()).unwrap_or_default())
        };
        for one in &block.insns {
            if one.spill_reload {
                if let Some(slot) = key(one, true) {
                    if !stored.contains(&slot) {
                        return Err(format!("block {at:#x} reloads {slot} on a path that never stored it"));
                    }
                }
            }
            if one.spill_store {
                stored.extend(key(one, false));
            }
        }
    }
    Ok(())
}

/// A reload or store the edge fix-up put in an empty block was dropped (the code
/// only ran when the block had an instruction), so a reload could read a slot
/// nothing wrote; an empty block before a join was a hard error.
#[test]
fn test_spill_code_survives_empty_blocks() {
    let mut reloads = 0;
    for (fixture, name) in [("tilesum.ll", "_tile_sum"), ("matmul.ll", "_bench_matmul"), ("hotstore.ll", "_f"), ("trivialphi.ll", "_bench_shellsort")] {
        let (body, _) = before_phase(Calls::C, fixture, name, "486", "SsaSpill");
        let split = with_empty_edge_blocks(&body);
        let spilled = ssaspill::spilled(&split, &mut Frame::new(0), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).unwrap_or_else(|why| panic!("{fixture}: {why}"));
        reloads += spilled.insns().iter().filter(|one| one.spill_reload).count();
        reloads_follow_stores(&spilled).unwrap_or_else(|why| panic!("{fixture}: {why}"));
    }
    assert!(reloads > 0, "premise: the bodies spill");
}

/// The block SsaSpill puts on a critical edge carried no edge odds, so the branch
/// it hangs on read 50/50 and everything behind it changed frequency: a split
/// loop-closing edge made the loop's trips fewer, and later spill weights followed.
#[test]
fn test_a_bridge_keeps_every_blocks_frequency() {
    let (mut body, _) = before_phase(Calls::C, "phiwidth.ll", "_f", "486", "SsaSpill");
    let had: BTreeSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    // Where the bridge goes, then that edge stated as the likely one.
    let first = ssaspill::spilled(&body, &mut Frame::new(0), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).expect("spills");
    // phiwidth's loop-closing edge was bridged to reload a remade load; the add takes the load's cell now,
    // and the edge needs no code. Where a body does bridge an edge, the bridge's odds are stated too.
    if let Some(bridge) = first.blocks.iter().find(|block| !had.contains(&block.at)) {
        let from = first.blocks.iter().find(|block| block.succ.contains(&bridge.at)).expect("a bridge has a predecessor");
        assert!(from.succ.len() == 2, "premise: the bridge hangs on a branch");
        let to = bridge.succ[0];
        body.odds.taken.insert((from.at, to), (0.9 * crate::model::lir::BlockOdds::CERTAIN) as u32);
    }
    let spilled = ssaspill::spilled(&body, &mut Frame::new(0), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).expect("spills");
    let (before, after) = (crate::analysis::frequency::Frequency::of(&body), crate::analysis::frequency::Frequency::of(&spilled));
    for at in had {
        let (was, is) = (before.block(at), after.block(at));
        assert!((was - is).abs() <= 1e-6 * was.max(1.0), "block {at:#x} runs {is} times for {was}");
    }
}

/// A value read as its low word in a web whose slot is a dword: the fold took the
/// slot's width, so `add ax, bx` became `add ax, dword [slot]`, reading two bytes
/// too many and 32-bit-wide in a 16-bit operation. Greedy sizes it by the operand.
#[test]
fn test_a_folded_operand_is_as_wide_as_the_instruction_reads_it() {
    use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
    use crate::model::lir::Insn;
    let held = |value: u32| Loc::Held(Held { value, width: 2 });
    let what = Semantics { name: Some("add".to_owned()), dests: vec![held(3)], sources: vec![held(1), held(2)], ..Semantics::new(Operation::Binary) };
    let one = Insn::new(0, Some((0, 2)), Some(what), vec![3], vec![1, 2]);
    let slot = Mem::new(Some(crate::model::ir::Addr::new(crate::model::ir::Space::Frame, -4)), 4);
    let folded = ssaspill::folded_into(&one, 2, &slot).expect("an add folds its second source");
    let widths: Vec<u32> = folded.what.as_ref().unwrap().sources.iter().filter_map(|place| if let Loc::Mem(cell) = place { Some(cell.width) } else { None }).collect();
    assert_eq!(widths, vec![2], "the slot is 4 bytes, the add reads 2");
}

/// A value read 70 blocks after it is made, the blocks listed last first: the
/// next-use distances take 64 sweeps to settle and had not, so `FAR` read as
/// "dead", the value left the registers after its definition and was stored and
/// reloaded for nothing.
#[test]
fn test_a_value_is_dead_when_nothing_reads_it_not_when_its_distance_is_unsettled() {
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock};
    let held = |value: u32| Loc::Held(Held { value, width: 2 });
    let number = |value: i64| Loc::Imm(Imm { value, width: 2, address: None });
    let make = |name: &str, dest: u32, sources: Vec<Loc>, uses: Vec<u32>| {
        let what = Semantics { name: Some(name.to_owned()), dests: vec![held(dest)], sources, ..Semantics::new(if name == "mov" { Operation::Move } else { Operation::Binary }) };
        std::sync::Arc::new(Insn::new(0, Some((0, 2)), Some(what), vec![dest], uses))
    };
    let last = 70;
    let mut blocks: Vec<LirBlock> = (0..=last)
        .map(|at| {
            let insns = match at {
                0 => vec![make("mov", 0, vec![number(5)], vec![]), make("add", 1, vec![held(0), number(1)], vec![0])],
                _ if at == last => vec![make("add", 2, vec![held(1), number(1)], vec![1])],
                _ => Vec::new(),
            };
            LirBlock { succ: if at == last { Vec::new() } else { vec![at + 1] }, ..LirBlock::new(at, insns) }
        })
        .collect();
    blocks.reverse();
    let body = LirBody::new("chain", 0, blocks, IndexMap::default(), IndexMap::default());
    let spilled = ssaspill::spilled(&body, &mut Frame::new(0), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).expect("spills");
    assert!(spilled.insns().iter().all(|one| !one.spill_store && !one.spill_reload), "one value in a six-register machine was spilled");
}

/// The phase list has the spiller only when asked: `assemble` weighs the
/// allocator alone against the spiller in front of it.
#[test]
fn test_the_phase_list_has_the_spiller_only_when_asked() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let names = |spilling: bool| -> Vec<&'static str> {
        let frame = Some(Rc::new(RefCell::new(Frame::new(0))));
        let calls = IndexMap::default();
        let phases = crate::flow::machine(&IndexMap::default(), frame, None, Some(&calls), false, "386", &target::BUILT_IN, spilling);
        phases.expect("phases").iter().map(|phase| phase.class_name()).collect()
    };
    assert!(names(true).contains(&"SsaSpill"));
    assert!(!names(false).contains(&"SsaSpill"));
}

/// Seven invariant loads, each read once per trip of a 272-trip loop with more values live than
/// registers. Each was made again before its read (a load, then the add) and a store ran per trip:
/// 1905 executed remakes and 279 stores for 7 and 1 once the add takes the load's cell itself.
#[test]
fn test_an_invariant_load_is_read_in_place_not_remade_each_trip() {
    let (body, mut phases) = before_phase(Calls::C, "remadeload.ll", "_f", "486", "SsaSpill");
    let spilled = phases[0].transform(body).expect("spills");
    let done = crate::backend::executed::executed(&spilled).expect("a reducible body");
    assert!(done.stores > 0.0, "premise: the body spills");
    assert!(done.remats <= 272.0, "{} remakes executed", done.remats);
    assert!(done.stores < 272.0, "{} stores executed", done.stores);
}

/// The most values of one register file live at once in `body`, where `member` picks the file's values.
fn most_live(body: &LirBody, member: impl Fn(u32) -> bool) -> usize {
    let (_, live_out) = crate::backend::allocate::live(body);
    let mut most = 0;
    for block in &body.blocks {
        let mut live: BTreeSet<u32> = live_out[&block.at].iter().copied().filter(|value| member(*value)).collect();
        most = most.max(live.len());
        for one in block.insns.iter().rev() {
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied().filter(|value| member(*value)));
            most = most.max(live.len());
        }
    }
    most
}

/// Six far pointers read in one loop, three selector registers to hold them (the built-in machine's es, fs, gs).
/// The spiller left all six live at once for the allocator to juggle (`mov gs, [slot]` before most reads, and a
/// selector kept in a general register); the pressure of each register file now fits its size.
#[test]
fn test_selectors_live_at_once_fit_the_segment_registers() {
    let (body, mut phases) = before_phase(Calls::C, "selectorloop.ll", "_f", "486", "SsaSpill");
    let selectors = |body: &LirBody| {
        let classes = crate::backend::regclass::classes(body, &BTreeSet::new(), &target::BUILT_IN);
        most_live(body, |value| classes.get(&value).is_some_and(|class| class.iter().all(|one| target::BUILT_IN.selectors.contains(one))))
    };
    assert!(selectors(&body) > target::BUILT_IN.selectors.len(), "premise: more selectors live than registers");
    let spilled = phases[0].transform(body).expect("spills");
    let live = selectors(&spilled);
    assert!(live <= target::BUILT_IN.selectors.len(), "{live} selectors live at once");
}

/// QCport's `_draw_string` at --cpu 486: a word copied from the low half of a dword load was made again as
/// that dword load into a word (`mov dx, dword [bp+20]`), which no encoding has; llrm-c refused four modules.
#[test]
fn test_a_word_copied_from_a_dword_is_not_made_again_as_the_dword() {
    let (body, mut phases) = before_phase(Calls::C, "lowword.ll", "_draw_string", "486", "SsaSpill");
    let spilled = phases[0].transform(body).expect("spills");
    let wrong: Vec<String> = spilled
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| what.op == crate::model::ir::Operation::Move)
        .filter_map(|what| match (what.dests.as_slice(), what.sources.as_slice()) {
            ([Loc::Held(dest)], [Loc::Mem(cell)]) if dest.width != cell.width => Some(format!("{what:?}")),
            _ => None,
        })
        .collect();
    assert!(wrong.is_empty(), "loads of another width than their register: {wrong:?}");
}

/// Six far pointers read in a 32-trip loop. A selector the back edge reloaded went into a block of its own with a
/// jump, which ran each trip: 32 jumps executed. It is read where it is used instead (the same load, no jump).
#[test]
fn test_a_loop_s_back_edge_reload_does_not_cost_a_jump_per_trip() {
    let (body, _) = before_phase(Calls::C, "selectorloop.ll", "_f", "486", "SsaSpill");
    let spilled = ssaspill::spilled(&body, &mut Frame::new(0), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::code16(), Prices::clocks()).expect("spills");
    let done = crate::backend::executed::executed(&spilled).expect("a reducible body");
    assert!(done.jumps < 8.0, "{} jumps executed", done.jumps);
}

/// Six far pointers live in a loop against three segment registers: the points that do not fit are found, and SsaSpill
/// leaves none.
#[test]
fn test_ssaspill_leaves_no_point_the_classes_cannot_hold() {
    let (body, mut phases) = before_phase(Calls::C, "selectorloop.ll", "_f", "486", "SsaSpill");
    let found = |body: &LirBody| crate::backend::regclass::violations(body, &target::BUILT_IN, &ssaspill::untouchable(body));
    assert!(!found(&body).is_empty(), "premise: the body asks for more selectors than there are registers");
    let spilled = phases[0].transform(body).expect("spills");
    assert_eq!(found(&spilled), Vec::new());
}

/// Twelve phi copies on one edge were scored once each, before and after: PhiElimination turned SsaSpill's 39 crowded
/// points into 87 on PLASMABLOBS with no new pressure. A copy group is one parallel point.
#[test]
fn test_a_copy_group_is_one_point_not_one_per_copy() {
    let (body, _) = before_phase(Calls::C, "phiwide.ll", "_f", "486", "SsaSpill");
    let eliminated = crate::backend::phielim::eliminated(&body).expect("eliminates");
    let found = |body: &LirBody| crate::backend::regclass::violations(body, &target::BUILT_IN, &ssaspill::untouchable(body)).len();
    let grouped = found(&eliminated);
    let mut apart = eliminated.clone();
    for block in &mut apart.blocks {
        for one in block.insns.iter_mut().filter(|one| one.group.is_some()) {
            std::sync::Arc::make_mut(one).group = None;
        }
    }
    assert!(grouped < found(&apart), "{grouped} points with the copies grouped, {} apart", found(&apart));
}

/// Sixteen loop-carried values on six registers: every phi stayed a register phi and the loop's header held twelve
/// values live (PLASMABLOBS: 39 points that fit no register, #441). A phi the registers cannot hold lives in its slot;
/// what remains is the one register an edge's stores borrow to load an argument.
#[test]
fn test_phis_that_do_not_fit_live_in_memory() {
    let (body, mut phases) = before_phase(Calls::C, "phiwide.ll", "_f", "486", "SsaSpill");
    let spilled = phases[0].transform(body).expect("spills");
    let eliminated = crate::backend::phielim::eliminated(&spilled).expect("eliminates");
    let found = crate::backend::regclass::violations(&eliminated, &target::BUILT_IN, &ssaspill::untouchable(&eliminated));
    let most = found.iter().map(|one| if let crate::backend::regclass::Why::Crowded { live, registers } = one.why { live - registers } else { usize::MAX }).max().unwrap_or(0);
    assert!(most <= 1, "{most} values over the registers at the worst point");
}

/// A memory phi's argument that is dead where its web is defined shares the result's slot: it is stored where it is
/// made and the back edge carries no store. Without sharing every argument was stored on its edge, a store for each
/// of sixteen carried values each trip.
#[test]
fn test_memory_phi_arguments_share_their_results_slot() {
    let stores = |share: bool| {
        let (body, mut phases) = before_phase(Calls::C, "phiwide.ll", "_f", "486", "SsaSpill");
        let spilled = if share { phases[0].transform(body) } else { ssaspill::without_shared_slots(|| phases[0].transform(body)) }.expect("spills");
        let eliminated = crate::backend::phielim::eliminated(&spilled).expect("eliminates");
        crate::backend::executed::executed(&eliminated).map(|done| done.memory).expect("estimates")
    };
    let (shared, apart) = (stores(true), stores(false));
    assert!(shared < apart, "{shared} memory operands with shared slots, {apart} without");
}

/// TwoAddress leaves an empty instruction where it made a phi copy unnecessary; it split the edge's copy group, and
/// the points between the copies counted again: SPHEREMAPLASMA read 61 after PhiElimination and 91 after TwoAddress
/// with the same pressure.
#[test]
fn test_a_placeholder_between_a_groups_copies_does_not_end_the_group() {
    let (body, _) = before_phase(Calls::C, "phiwide.ll", "_f", "486", "SsaSpill");
    let eliminated = crate::backend::phielim::eliminated(&body).expect("eliminates");
    let found = |body: &LirBody| crate::backend::regclass::violations(body, &target::BUILT_IN, &ssaspill::untouchable(body)).len();
    let mut split = eliminated.clone();
    for block in &mut split.blocks {
        let Some(first) = block.insns.iter().position(|one| one.group.is_some()) else { continue };
        let blank = std::sync::Arc::new(crate::model::lir::Insn::new(block.insns[first].at, Some((block.insns[first].at, block.insns[first].at)), None, Vec::new(), Vec::new()));
        let mut insns = block.insns.clone();
        insns.insert(first + 1, blank);
        *block = block.with_insns(insns);
    }
    assert_eq!(found(&split), found(&eliminated));
}
