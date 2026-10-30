//! Copies a function for the constants its callers pass and redirects those
//! calls to the copy, so the copy's loops and branches fold on values the
//! original cannot assume: GCC's `ipa-cp` cloning.
//!
//! # How it works
//! Each call whose callee is defined and copyable, and has at least one
//! actual that the caller's own folding proved constant
//! (`interprocedural::current_call_constants`), is grouped with the others
//! that pass the same constants. A group is a clone candidate when
//!
//! - some site is hot: a site in a function other than `main`, or one whose
//!   block runs 1.5 times per call or more;
//! - the constants save work: `time_saved` runs an optimistic constant
//!   propagation over the callee with its parameters bound, and prices the
//!   instructions that fold or are only reached by a decided branch, at their
//!   block frequency (held to ten trips);
//! - the benefit pays for the size: `benefit * 1000 >= 500 * size`
//!   (`ipa-cp-eval-threshold`), where `benefit` is `saved / 5` plus the loop
//!   bonus (`ipa-cp-loop-hint-bonus`, 64, earned where a loop's exit tests a
//!   known value against a varying one), scaled by the sites' summed frequency,
//!   and `size` is the callee's operations; and the unit's growth stays within
//!   `ipa-cp-unit-growth` (10% of the unit, or of 16000 operations if larger).
//!   A function in a cycle with others, not one calling itself, loses
//!   `ipa-cp-recursion-penalty` (40%) of the benefit.
//!
//! A callee that is private, not addressed, and called only from the group
//! needs no growth test: every call goes to the copy and the original dies.
//!
//! A caller whose calls go to a copy that says what the callee said (the same
//! attributes) is vouched for (`Changed::unsound` names the others): its passes
//! need not look at the new name ([`crate::settled`]).
//!
//! The copy is an internal function `<origin>.constprop.N` with each known
//! parameter replaced by its constant. A later call with the same constants
//! (`Cloning::contexts`) goes to the copy already made. A function gets at
//! most eight clones (`ipa-cp-max-recursive-depth`), which also bounds the
//! recursion: a clone that calls itself with a new constant (`place(q, row + 1,
//! n)` in the clone for `row`) is a candidate for the next clone, so GCC's -O3
//! unrolls `queens` into eight, one a row. The copy is an ordinary function:
//! the inliner takes a clone with one caller into it.
//!
//! GCC propagates lattices over the whole call graph and prices a value with
//! each function's time and size summaries under the context
//! (`good_cloning_opportunity_p`). Here a site's constants are those the
//! caller's own folding proved, and `saved` is the clocks `time_saved` finds
//! the callee no longer spends. The thresholds are GCC's.
//!
//! # Example
//! With `Threshold::cp_clone` on. Before:
//!
//! ```mir
//! define i16 @g(i16 %k, i16 %x) {
//! ; ... a loop that runs %k trips
//! }
//!
//! define i16 @f(i16 %x) {
//! b0:
//!   %a = call i16 @g(i16 4, i16 %x)
//!   %b = call i16 @g(i16 5, i16 %x)
//!   %s = add i16 %a, %b
//!   ret i16 %s
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %x) memory(none) willreturn nounwind norecurse {
//! b0:
//!   %a = call i16 @g.constprop.1(i16 %x)
//!   %b = call i16 @g.constprop.2(i16 %x)
//!   %s = add i16 %a, %b
//!   ret i16 %s
//! }
//!
//! define internal i16 @g.constprop.1(i16 %x) memory(none) willreturn nounwind norecurse {
//! ; ...
//! head:
//!   ; ...
//!   %go = icmp slt i16 %i, 4
//! ; ...
//! }
//! ```
//!
//! `@g` stays for other callers; `@g.constprop.2` is the same with 5.
//!
//! From unit test
//! `test_a_function_called_with_two_constants_is_cloned_for_each_at_o3` in
//! `interprocedural_tests.rs`.
//!
//! # Runs
//! In [`crate::interprocedural`]'s constants loop, after constant arguments are
//! specialised, repeated until nothing changes; every changed body goes back
//! through the pipeline as `ipa-cp.`. Only where `Threshold::cp_clone` is set
//! (`-fipa-cp-clone`): -O3 and -Omax; without it cloning does nothing and -O2
//! gets only the constants every call agrees on. See
//! `docs/reference/levels.md`.
//!
//! # Needs
//! [`crate::inline`] (copyability, call counts, operation counts),
//! [`crate::profit`] (prices, block frequencies), [`llrm_analysis::consts`].
//!
//! # See also
//! GCC's
//! [`ipa-cp.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ipa-cp.cc).

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::interprocedural as facts;
use llrm_analysis::memory::Unit;
use llrm_analysis::{cfg, consts};
use llrm_mir::context::{Constant, ConstantId, ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::Callees;
use llrm_mir::module::Function;
use llrm_mir::module::{GlobalKind, GlobalValue, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use crate::inline;
use crate::profit::{self, OperationCosts};

/// `ipa-cp-eval-threshold` (params.opt:217).
const EVAL_THRESHOLD: i64 = 500;
/// `ipa-cp-max-recursive-depth` (params.opt:225), which
/// `ipa-cp-value-list-size` (:253) also makes the clones one function gets.
const MAX_CLONES: usize = 8;
/// Our clocks, at our block frequencies (held to ten trips), per GCC time unit:
/// `place` with `row` known saves 667 here and 245.8 in GCC's dump
/// (`-fdump-ipa-cp-details`), which also declines quicksort's clone (time 1)
/// that this estimate has at 60 clocks.
const TIME_SCALE: i64 = 5;
/// The most a block counts in a callee's time: the trips GCC's loop estimates
/// stop at.
const MAX_FREQUENCY: i64 = 10;
/// `ipa-cp-loop-hint-bonus` (params.opt:221): time units added where the known
/// actuals make a loop's bound known.
const LOOP_HINT_BONUS: i64 = 64;
/// `ipa-cp-recursion-penalty` (params.opt:237): the percent a recursive
/// function's benefit loses.
const RECURSION_PENALTY: i64 = 40;
/// `ipa-cp-unit-growth` (params.opt:245) and `ipa-cp-large-unit-insns` (:249).
const UNIT_GROWTH: i64 = 10;
const LARGE_UNIT: i64 = 16000;

/// What the clones made so far come to.
#[derive(Default)]
pub struct Cloning {
    /// The function each clone is of.
    origin: BTreeMap<GlobalId, GlobalId>,
    made: BTreeMap<GlobalId, usize>,
    /// The clone of a function for the constants it was called with: a value
    /// gets one.
    contexts: BTreeMap<Key, GlobalId>,
    grown: i64,
}

/// One kind of call: a callee and the constants it is called with.
type Key = (GlobalId, Vec<Option<ConstantId>>);

/// The functions this changed: the clones made, the callers redirected.
pub struct Changed {
    pub added: Vec<GlobalId>,
    pub edited: Vec<GlobalId>,
    /// The callers among `edited` that a call was sent to a copy that says
    /// other than the callee did: their edit is more than a new name.
    pub unsound: Vec<GlobalId>,
}

/// Every call of a constant context worth a clone, cloned.
pub fn cloned(
    module: &mut Module,
    layout: &DataLayout,
    procedures: &[GlobalId],
    private: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    clone: bool,
    state: &mut Cloning,
) -> Changed {
    let mut changed = Changed { added: Vec::new(), edited: Vec::new(), unsound: Vec::new() };
    // GCC: "Not considering %s for cloning; -fipa-cp-clone disabled." What -O2
    // does without it is the propagation of constants every call agrees on.
    if !clone {
        return changed;
    }
    let callees = llrm_mir::memory::callees(module);
    let recursive = inline::recursive(module);
    let addressed = llrm_mir::callgraph::addressed(module);
    let counts = inline::call_counts(module);
    let unit: i64 = procedures.iter().filter_map(|&id| module.global(id).function()).map(inline::operations).sum();
    // The calls, grouped by what they know.
    let mut groups: BTreeMap<Key, Vec<(GlobalId, InstId, i64)>> = BTreeMap::new();
    for &id in procedures {
        let Some(caller) = module.global(id).function() else { continue };
        let constants = facts::current_call_constants(&module.context, caller);
        let frequency =
            profit::_frequencies(&module.context, &module.metadata, &module.globals, caller, None).unwrap_or_default();
        for (block, at) in caller.walk() {
            let Some(name) = llrm_mir::memory::callee(&module.context, caller, at) else { continue };
            let known = constants.get(&at).map_or(&[][..], Vec::as_slice);
            if known.iter().all(Option::is_none) {
                continue;
            }
            groups
                .entry((name, known.to_vec()))
                .or_default()
                .push(
                    (
                        id,
                        at,
                        frequency.get(&llrm_analysis::cfg::id(block)).copied().unwrap_or(profit::UNIT),
                    ),
                );
        }
    }
    for ((name, known), sites) in groups {
        let Some(body) = module.global(name).function() else { continue };
        // DEVIATE: an `available_externally` body is read, not run: the
        // definition is elsewhere. A copy would become a definition of its own
        // (`isDefinitionExact` is false for it; GCC's `ipa-cp` versions only a
        // node whose definition it owns).
        if module.global(name).linkage == Linkage::AvailableExternally {
            continue;
        }
        if !inline::copyable(module, body)
            || body.parameters().len() != known.len()
            || matches!(
                module.context.types.get(body.ty),
                Type::Function { variadic: true, .. }
            )
        {
            continue;
        }
        // GCC clones only for a hot call (`ipcp_cloning_candidate_p`: "no hot
        // calls"): one in a function that runs once, `main`, is hot
        // only in a loop (`cgraph_edge::maybe_hot_p`: frequency 1.5 or more).
        let hot =
            sites.iter().any(|&(caller, _, frequency)| !runs_once(module, caller) || frequency * 2 >= profit::UNIT * 3);
        if !hot {
            llrm_support::debug!("ipa-cp", "{}: no hot calls", module.global(name).name.as_deref().unwrap_or("?"));
            continue;
        }
        let origin = state.origin.get(&name).copied().unwrap_or(name);
        if let Some(&existing) = state.contexts.get(&(origin, known.clone())) {
            if existing == name {
                // A call of the clone, which still passes the constants it was
                // made for.
                continue;
            }
            // The calls that still name the function with these constants go to
            // the clone already made for them.
            redirect(module, existing, &sites, &mut changed);
            continue;
        }
        if state.made.get(&origin).copied().unwrap_or(0) >= MAX_CLONES {
            continue;
        }
        let size = inline::operations(body);
        // Every call goes to the clone: the function it copies is dead, and the
        // unit does not grow.
        let replaces = private.contains(&name)
            && !addressed.contains(&name)
            && counts.get(&name).copied().unwrap_or(0) == sites.len() as i64;
        let (saved, loops) = time_saved(module, layout, body, &known, &callees, costs);
        let frequency: i64 = sites.iter().map(|site| site.2).sum();
        let mut benefit = (saved / TIME_SCALE + if loops { LOOP_HINT_BONUS } else { 0 }) * frequency / profit::UNIT;
        // GCC's `incorporate_penalties`: a function in a cycle with others, not
        // one that calls itself.
        let own = body.walk().any(|(_, inst)| llrm_mir::memory::callee(&module.context, body, inst) == Some(name));
        if recursive.contains(&name) && !own {
            benefit = benefit * (100 - RECURSION_PENALTY) / 100;
        }
        let cost = if replaces { 0 } else { size };
        let verdict = (saved > 0 || loops)
            && (replaces
                || (benefit * 1000 >= EVAL_THRESHOLD * size
                    && state.grown + size <= unit.max(LARGE_UNIT) * UNIT_GROWTH / 100 + 1));
        llrm_support::debug!(
            "ipa-cp",
            "{}: {} sites, {} of {} actuals known, saved {saved} clocks, loops {loops}, benefit {benefit}, size {size}: {}",
            module.global(name).name.as_deref().unwrap_or("?"),
            sites.len(),
            known.iter().flatten().count(),
            known.len(),
            if verdict { "clone" } else { "kept" }
        );
        if !verdict {
            continue;
        }
        // The copy: the constants for the parameters they are the actuals of,
        // the function otherwise as it is.
        let mut copy = body.clone();
        for (&parameter, constant) in body.parameters().iter().zip(&known) {
            if let Some(constant) = constant {
                copy.replace_all_uses_with(parameter, Operand::Constant(*constant));
            }
        }
        let mut number = state.made.get(&origin).copied().unwrap_or(0);
        let origin_name = module.global(origin).name.owned().unwrap_or_else(|| "f".to_owned());
        let called = loop {
            number += 1;
            let candidate = format!("{origin_name}.constprop.{number}");
            if module.named(&candidate).is_none() {
                break candidate;
            }
        };
        let global = module.global(name);
        let (space, unnamed) = (global.address_space, global.unnamed_addr);
        let id = GlobalId(module.globals.len() as u32);
        module
            .globals
            .push(
                GlobalValue {
                    name: called.into(),
                    linkage: Linkage::Internal,
                    unnamed_addr: unnamed,
                    address_space: space,
                    kind: GlobalKind::Function(Box::new(copy)),
                },
            );
        state.origin.insert(id, origin);
        *state.made.entry(origin).or_default() += 1;
        state.grown += cost;
        changed.added.push(id);
        state.contexts.insert((origin, known.clone()), id);
        redirect(module, id, &sites, &mut changed);
    }
    changed
}

/// GCC's `time_benefit`: the clocks a call no longer spends in `body` given its
/// known actuals: what they fold, and what only a branch they decide reaches,
/// each at its block's frequency (a loop that does not run saves its trips).
fn time_saved(
    module: &Module,
    layout: &DataLayout,
    body: &Function,
    known: &[Option<ConstantId>],
    callees: &Callees,
    costs: &OperationCosts,
) -> (i64, bool) {
    let unit = Unit::of(module, layout, body);
    let mut values = IndexMap::default();
    for (&parameter, constant) in body.parameters().iter().zip(known) {
        if let Some(number) = constant.and_then(|id| consts::_operand(&unit, Operand::Constant(id), &values, None)) {
            values.insert(parameter, number);
        }
    }
    // Optimistic constant propagation (Wegman and Zadeck): a value is not yet
    // seen, a constant, or varying; a block is reached when a reached
    // block's branch can go there; a phi is the meet of its reached edges. A
    // loop whose first test is decided never runs.
    let mut folded = BTreeSet::new();
    let mut reached = BTreeSet::new();
    let mut edges: BTreeSet<(llrm_mir::module::BlockId, llrm_mir::module::BlockId)> = BTreeSet::new();
    let mut varying: BTreeSet<llrm_mir::module::ValueId> = BTreeSet::new();
    reached.extend(body.entry());
    // Whether `operand` is a value of the body no block has made yet.
    let unseen = |operand: &Operand,
                  values: &IndexMap<llrm_mir::module::ValueId, consts::Known>,
                  varying: &BTreeSet<llrm_mir::module::ValueId>| {
        matches!(
            operand,
            Operand::Value(v) if !values.contains_key(v) && !varying.contains(v) && matches!(body.value(*v).def, llrm_mir::module::ValueDef::Instruction(made) if consts::_defined(&unit, made) == Some(*v))
        )
    };
    loop {
        let before = (reached.len(), edges.len(), values.len(), varying.len());
        for &block in body.layout() {
            if !reached.contains(&block) {
                continue;
            }
            for inst in body.iter_block(block) {
                let instruction = body.instruction(inst);
                let Some(defined) = consts::_defined(&unit, inst) else { continue };
                if varying.contains(&defined) || matches!(instruction.opcode, Opcode::Br | Opcode::Ret) {
                    continue;
                }
                if instruction.opcode == Opcode::Phi {
                    let mut meet: Option<consts::Known> = None;
                    let mut vary = false;
                    for (value, from) in crate::lcssa::arms(body, inst) {
                        if !edges.contains(&(from, block)) || unseen(&value, &values, &varying) {
                            continue;
                        }
                        match (consts::_operand(&unit, value, &values, None), &meet) {
                            (Some(one), None) => meet = Some(one),
                            (Some(one), Some(kept)) if one.n == kept.n => {}
                            _ => vary = true,
                        }
                    }
                    if vary {
                        values.swap_remove(&defined);
                        varying.insert(defined);
                        folded.remove(&inst);
                    } else if let Some(one) = meet {
                        values.insert(defined, one);
                        folded.insert(inst);
                    }
                    continue;
                }
                if let Some(number) = consts::_result(&unit, inst, &values, None) {
                    values.insert(defined, number);
                    folded.insert(inst);
                } else if !instruction.operands.iter().any(|one| unseen(one, &values, &varying)) {
                    values.swap_remove(&defined);
                    varying.insert(defined);
                    folded.remove(&inst);
                }
            }
        }
        for &block in body.layout() {
            if !reached.contains(&block) {
                continue;
            }
            let Some(end) = body.terminator(block) else { continue };
            let terminator = body.instruction(end);
            // A branch on a value not yet seen goes nowhere yet.
            if terminator.opcode == Opcode::Br
                && terminator.operands.len() == 3
                && unseen(&terminator.operands[0], &values, &varying)
            {
                continue;
            }
            let decided = (terminator.opcode == Opcode::Br && terminator.operands.len() == 3)
                .then(|| consts::_operand(&unit, terminator.operands[0], &values, None))
                .flatten();
            let successors: llrm_mir::blocklist::BlockList = match (decided, &terminator.operands[..]) {
                (Some(number), [_, Operand::Block(yes), Operand::Block(no)]) => {
                    std::iter::once(if number.n.sign() == num_bigint::Sign::NoSign { *no } else { *yes }).collect()
                }
                _ => body.successors(block),
            };
            for to in successors {
                edges.insert((block, to));
                reached.insert(to);
            }
        }
        if before == (reached.len(), edges.len(), values.len(), varying.len()) {
            break;
        }
    }
    let frequency =
        profit::_frequencies(&module.context, &module.metadata, &module.globals, body, None).unwrap_or_default();
    // A block runs about as often as GCC guesses a loop does (its time
    // estimates stop at ten trips), not as often as the nest multiplies
    // out.
    let weight =
        |block| frequency.get(&cfg::id(block)).copied().unwrap_or(profit::UNIT).min(MAX_FREQUENCY * profit::UNIT);
    let (mut unknown, mut kept) = (0, 0);
    for (block, inst) in body.walk() {
        if matches!(body.instruction(inst).opcode, Opcode::Phi) {
            continue;
        }
        let price = profit::operation(&module.context, layout, body, callees, inst, costs).unwrap_or(0) * weight(block);
        unknown += price;
        if reached.contains(&block) && !folded.contains(&inst) {
            kept += price;
        }
    }
    // `INLINE_HINT_loop_iterations`: a loop's exit tests a value the known
    // actuals make constant against one that varies.
    let hint = cfg::loops_of(body)
        .iter()
        .any(
            |one| one.body.iter().any(|&at| {
                let block = cfg::block(at);
                let Some(end) = body.terminator(block) else { return false };
                let terminator = body.instruction(end);
                let (Opcode::Br, Some(Operand::Value(condition))) = (&terminator.opcode, terminator.operands.first()) else { return false };
                let llrm_mir::module::ValueDef::Instruction(compare) = body.value(*condition).def else { return false };
                let compared = body.instruction(compare);
                matches!(compared.opcode, Opcode::ICmp(_)) && reached.contains(&block) && {
                    let constant = |operand: &Operand| matches!(
                        operand,
                        Operand::Value(v) if values.contains_key(v) && !matches!(body.value(*v).def, llrm_mir::module::ValueDef::Instruction(_)) || matches!(operand, Operand::Value(v) if values.contains_key(v) && body.parameters().contains(v))
                    );
                    let moving = |operand: &Operand| matches!(operand, Operand::Value(v) if varying.contains(v));
                    compared.operands.len() == 2 && ((constant(&compared.operands[0]) && moving(&compared.operands[1])) || (constant(&compared.operands[1]) && moving(&compared.operands[0])))
                }
            }),
        );
    ((unknown - kept) / profit::UNIT, hint)
}

/// Whether `id` is a function the program runs once: `main`.
fn runs_once(
    module: &Module,
    id: GlobalId,
) -> bool {
    module.global(id).name.as_deref().is_some_and(|name| name.trim_start_matches('_') == "main")
}

/// The calls in `sites` made to `clone`.
fn redirect(
    module: &mut Module,
    clone: GlobalId,
    sites: &[(GlobalId, InstId, i64)],
    changed: &mut Changed,
) {
    let same_said = match (
        module.global(clone).function(),
        sites.first().and_then(|&(caller, call, _)| {
            let function = module.global(caller).function()?;
            let Some(&Operand::Constant(old)) = function.instruction(call).operands.last() else { return None };
            match module.context.get(old).kind {
                ConstantKind::Global(callee) => module.global(callee).function(),
                _ => None,
            }
        }),
    ) {
        (Some(copy), Some(original)) => {
            copy.attrs == original.attrs
                && copy.return_attrs == original.return_attrs
                && copy.parameter_attrs == original.parameter_attrs
                && copy.calling_convention == original.calling_convention
        }
        _ => false,
    };
    for &(caller, call, _) in sites {
        let GlobalKind::Function(function) = &module.globals[caller.0 as usize].kind else { continue };
        let Some(&Operand::Constant(old)) = function.instruction(call).operands.last() else { continue };
        let ty = module.context.get(old).ty;
        let target = Operand::Constant(module.context.constant(Constant { ty, kind: ConstantKind::Global(clone) }));
        let GlobalKind::Function(function) = &mut module.globals[caller.0 as usize].kind else { continue };
        let last = function.instruction(call).operands.len() - 1;
        function.set_operand(call, last, target);
        if !changed.edited.contains(&caller) {
            changed.edited.push(caller);
        }
        if !same_said && !changed.unsound.contains(&caller) {
            changed.unsound.push(caller);
        }
    }
}
