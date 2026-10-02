//! Recolouring the dominance-order colours, checked at the assignment's input.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::frequency::Frequency;
use crate::backend::frame::Frame;
use crate::backend::regalloc_input::{before_phase, Calls};
use crate::backend::target;
use crate::backend::{ssaassign, ssacolour, ssarecolour, ssaspill};
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// `name` of `fixture` at SsaSpill's input, spilled.
fn spilled(fixture: &str, name: &str) -> LirBody {
    let (body, _) = before_phase(Calls::C, fixture, name, "486", "SsaSpill");
    ssaspill::spilled(&body, &mut Frame::new(0), &target::BUILT_IN).expect("spills")
}

/// How many copies the assignment placed in blocks that run at least 8 times.
fn hot_copies(body: &LirBody) -> usize {
    let frequency = Frequency::of(body);
    body.blocks.iter().filter(|block| frequency.block(block.at) >= 8.0).flat_map(|block| &block.insns).filter(|one| one.group.is_some_and(|group| group >= 1 << 40)).count()
}

/// tile_sum's index `((i + k) & 63) * 2` took the register of the value it was tied
/// to, which no address can use; each iteration then traded it with another value
/// (`push edi / mov di, cx / pop ecx`), 14832 executed instructions for 13809.
#[test]
fn test_a_tied_chain_ending_in_an_address_is_recoloured_to_an_index_register() {
    let body = spilled("tilesum.ll", "_tile_sum");
    let (plain, _) = ssaspill::assigned_with(&body, &target::BUILT_IN, false).expect("assigns");
    let (recoloured, _) = ssaspill::assigned_with(&body, &target::BUILT_IN, true).expect("assigns");
    let (before, after) = (hot_copies(&plain), hot_copies(&recoloured));
    assert!(before > 0, "premise: dominance order leaves a copy in the loop");
    assert!(after < before, "{after} copies in hot blocks after recolouring, {before} before");
}

/// A phi web split on purpose is joined again where nothing live beside it holds the register.
#[test]
fn test_a_split_phi_web_is_joined_where_it_can_be() {
    let body = spilled("fibswap.ll", "_fib");
    let floats = ssaspill::floating(&body);
    let colour = ssacolour::coloured(&body, &floats, &body.pins, &target::BUILT_IN);
    let interferes = ssarecolour::interference(&body);
    // The phi whose result and every argument share a register, and a register nothing beside any of them holds.
    let (result, args) = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .filter(|phi| phi.incoming.len() == 2)
        .map(|phi| (phi.result, phi.incoming.iter().map(|(_, value)| *value).collect::<Vec<_>>()))
        .find(|(result, args)| colour.get(result).is_some() && args.iter().all(|arg| colour.get(arg) == colour.get(result)) && !args.contains(result))
        .expect("premise: a phi web held in one register");
    let web: BTreeSet<u32> = args.iter().copied().chain([result]).collect();
    let taken: BTreeSet<Register> = web.iter().flat_map(|value| interferes.get(value).into_iter().flatten()).filter_map(|value| colour.get(value).copied()).collect();
    let spare = target::AVAILABLE.iter().map(|one| crate::backend::allocate::_whole(*one)).find(|register| !taken.contains(register) && Some(register) != colour.get(&result)).expect("a register beside which nothing lives");
    let mut split: IndexMap<u32, Register> = colour.clone();
    split.insert(args[0], spare);
    assert!(ssaassign::improper(&body, &split).is_none(), "premise: the split colouring is proper");
    let joined = ssarecolour::recoloured(&body, &split, &body.pins, &target::BUILT_IN);
    assert!(ssaassign::improper(&body, &joined).is_none());
    assert_eq!(joined.get(&args[0]), joined.get(&result), "the web is one register again");
}

/// The executions of tied instructions whose result is not in the register of the
/// operand it takes over, which twoaddr then copies into: each one is a `mov`.
fn tie_copies(body: &LirBody, colour: &IndexMap<u32, Register>) -> f64 {
    let frequency = Frequency::of(body);
    let (_, live_out) = crate::backend::allocate::live(body);
    let mut total = 0.0;
    for block in &body.blocks {
        let mut live = live_out[&block.at].clone();
        for one in block.insns.iter().rev() {
            if let (Some(taken), [made]) = (crate::backend::twoaddr::tie_source(one, &live), &one.defines[..]) {
                if !live.contains(&taken) && colour.get(&taken) != colour.get(made) {
                    total += frequency.block(block.at);
                }
            }
            for value in &one.defines {
                live.remove(value);
            }
            live.extend(one.uses.iter().copied());
        }
    }
    total
}

/// matmul's `acc + a[i] * b[j]` sums: the add's first source stays live while its
/// second dies, and twoaddr swaps the pair, but the colourer asked for the first
/// source's register, so each such add copied the dying operand into the result's:
/// 3010 executed instructions for Greedy's 2484.
#[test]
fn test_a_swapped_tie_gives_the_result_the_dying_operands_register() {
    let body = spilled("matmul.ll", "_bench_matmul");
    let floats = ssaspill::floating(&body);
    let colour = ssacolour::coloured(&body, &floats, &body.pins, &target::BUILT_IN);
    assert_eq!(tie_copies(&body, &colour), 0.0);
}
