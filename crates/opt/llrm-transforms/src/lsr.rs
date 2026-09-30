//! A loop's induction variables chosen once, by cost: LLVM's
//! LoopStrengthReduce and GCC's ivopts.
//!
//! Every read of a counter, or of a value affine in one, is one
//! `induction::Recurrence`, `pointer + start + step * trip`, with symbols
//! allowed (`induction::users`). A candidate is a recurrence the loop could
//! carry in a register. A use is realized from a candidate `r` as
//! `base + k * r + c`: an address the target's forms take, or the adds,
//! shifts and multiplies that compute it. The exit either keeps its compare
//! or tests one candidate for its last value, which is free where that is
//! zero.
//!
//! A set of candidates costs an add a trip each, its uses' cheapest
//! realizations, and a spill a trip for each register past the target's.
//! The cheapest set is found by adding, removing and swapping candidates
//! from the loop's own counters and from none, as GCC's
//! `find_optimal_iv_set`, and is carried out only where it beats the
//! counters the loop has. Prices, registers and address forms are the
//! target's; the counters left unread go to `dead`.
//!
//! It runs once, after the loop passes, on each loop innermost first, as
//! LLVM runs its LSR late: nothing after it chooses a loop's counters.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, CountedLoop, IvUse, Linear, Recurrence, UseKind, Users};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, liveness, memory};
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::target::{AddressForm, Machine, OperationCosts};
use num_traits::ToPrimitive;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::HashMap;
use num_bigint::BigInt;

use crate::counting::{self, Seeds};
use crate::expand::{self, Expander};
use crate::spill::{self, Room, Traffic};
use crate::{dead, profit, rotate};

pub struct Lsr;

impl FunctionPass for Lsr {
    fn name(&self) -> &'static str {
        "lsr"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if reduced(unit, analyses, &outer) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// What the target says a loop's choice may cost.
struct Target<'a> {
    machine: &'a dyn Machine,
    costs: OperationCosts,
    room: Room,
    forms: Vec<AddressForm>,
}

/// What may take a register in a loop: a value it has, or one a choice
/// adds -- a counter, an invariant, a symbolic step, a product, a value
/// built from a counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Resident {
    Value(ValueId),
    Counter(usize),
    Held(usize),
    Step(usize),
    Product(usize, i64, i64),
    Rebuilt(usize),
}

/// Each loop's counters chosen, innermost first; whether any changed.
pub fn reduced(unit: &mut Unit, analyses: &Analyses, outer: &Outer) -> bool {
    let target = Target { machine: outer.target(), costs: profit::costs(outer), room: profit::registers(outer), forms: outer.target().address_forms() };
    let mut done = BTreeSet::<i64>::new();
    let mut changed = false;
    loop {
        let facts = analyses.fresh().get::<Registers>(unit.context, unit.layout, unit.function);
        let plan = {
            let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer).with_registers(&facts);
            let mut loops = view.shape().loops.clone();
            loops.sort_by_key(|one| (one.body.len(), one.header));
            let Some(loop_) = loops.into_iter().find(|one| !done.contains(&one.header)) else { break };
            done.insert(loop_.header);
            _plan(&view, outer, &loop_, &target)
        };
        let Some(plan) = plan else { continue };
        if let Some(first) = _applied(unit, &plan) {
            done.insert(cfg::id(first));
        }
        // What the loop no longer reads is no use for the loop around it.
        dead::dead(unit.context, outer.callees(), unit.function);
        changed = true;
    }
    changed
}

/// A recurrence the loop could carry, and the phi that already does.
#[derive(Clone, Debug)]
struct Candidate {
    of: Recurrence,
    /// A pointer's type; an integer is its width's.
    pointer: Option<TypeId>,
    existing: Option<ValueId>,
}

/// An invariant a realization keeps in a register: a pointer plus a sum.
type Key = (Option<Operand>, Linear);

/// A register an address reads.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Reg {
    Iv(usize),
    /// A candidate scaled, once in a block.
    Product(usize, i64, i64),
    Held(usize),
}

/// A use read as `base + rest + k * candidate + constant`.
#[derive(Clone, Debug)]
struct Fit {
    k: BigInt,
    base: Option<Operand>,
    rest: Linear,
    constant: BigInt,
    /// The candidate's stepped value, read in the latch.
    next: bool,
    /// After the loop, the trip itself: the candidate's distance from its
    /// start, taken down where it counts down, shifted right by `shift` and
    /// times `inverse`, the inverse of its step's odd part modulo the bits
    /// left. `k` then multiplies the trip.
    trip: Option<(u32, bool, BigInt)>,
    /// An equality with an invariant, which takes the candidate itself and
    /// the invariant less the rest, times `k`, one or minus one.
    folded: Option<Linear>,
}

/// What realizing a use from one candidate costs each time it runs, and
/// what it holds.
#[derive(Clone, Debug)]
struct Price {
    cost: i64,
    /// Invariants held in registers, by their index in `Problem::keys`.
    held: Vec<usize>,
    /// The registers an address in the native form reads.
    address: Vec<Reg>,
    /// An address in a wider form.
    wide: bool,
    /// `k * candidate` computed in the loop, in `cost`: one serves every
    /// use of it in the block, whose index is the last.
    product: Option<(i64, i64, i64)>,
}

/// A use, where its realization goes and how often that runs.
#[derive(Clone, Debug)]
struct Site {
    one: IvUse,
    at: Place,
    frequency: i64,
    inside: bool,
    /// The counted exit's compare, which may test a candidate's last value instead.
    exit: bool,
    /// Its value, where it is read as the loop leaves on a known count.
    known: Option<Linear>,
}

/// Where a realization is placed.
#[derive(Clone, Copy, Debug)]
enum Place {
    Before(InstId),
    End(BlockId),
    /// After the phis of an exit block, for the exit phi it replaces.
    Exit(BlockId, InstId),
}

/// How the exit may be tested.
#[derive(Clone, Debug)]
struct Exit {
    proof: CountedLoop,
    trips: Linear,
    most: BigInt,
    /// A symbolic count needs a guard, and the loop entered at its body.
    guarded: bool,
}

/// One loop's choice, before anything changes.
struct Plan {
    loop_: Loop,
    preheader: BlockId,
    header: BlockId,
    latch: BlockId,
    candidates: Vec<Candidate>,
    /// Each use, its candidate and its fit.
    uses: Vec<(Site, usize, Fit)>,
    /// The candidate whose last value ends the loop, and that value.
    exit: Option<(Exit, usize)>,
}

/// Everything priced in one loop.
struct Problem<'a> {
    target: &'a Target<'a>,
    candidates: Vec<Candidate>,
    sites: Vec<Site>,
    /// Each site's fit and price from each candidate.
    fits: Vec<Vec<Option<(Fit, Price)>>>,
    exit: Option<Exit>,
    /// The exit's price from each candidate, testing its last value.
    exits: Vec<Option<(i64, Option<usize>)>>,
    keys: Vec<Key>,
    /// What the loop keeps in registers before each of its instructions,
    /// and across each call, by block, whatever is chosen.
    fixed: BTreeMap<i64, Vec<spill::Site>>,
    /// The spill traffic of what `fixed` keeps.
    traffic: BTreeMap<ValueId, Traffic>,
    /// Where each site's value is live, at the points of `fixed`.
    alive: Vec<BTreeMap<i64, Vec<bool>>>,
    latch: i64,
    header: i64,
    /// How often the preheader runs, where starts and invariants are built.
    entry: i64,
    /// Values the loop holds anyway: a key of one alone is no new register.
    live: BTreeSet<ValueId>,
    /// Frame objects: their addresses are the frame's register and a displacement.
    frames: BTreeSet<ValueId>,
}

fn _preheader(function: &Function, loop_: &Loop) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

fn _plan(view: &memory::Unit, outer: &Outer, loop_: &Loop, target: &Target) -> Option<Plan> {
    let function = view.function;
    let preheader = _preheader(function, loop_)?;
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let counters = induction::basics(view, loop_);
    let derived = induction::derived(view, loop_, Some(&counters));
    let users = induction::users(view, loop_, &counters, &derived);
    if users.counters.is_empty() || users.counters.iter().any(|&one| _latch_arm(function, one, cfg::block(latch)).is_none()) {
        return None;
    }
    let facts = view.registers();
    let frequencies = profit::_frequencies(function, Some(&profit::proven_trips(view, &facts)))?;
    let frequency = |block: BlockId| frequencies.get(&cfg::id(block)).copied().unwrap_or(1);
    let exit = _exit(view, loop_, &users);
    let nested = view.shape().loops.iter().filter(|one| one.header != loop_.header && loop_.body.contains(&one.header)).cloned().collect::<Vec<_>>();
    let mut sites = Vec::new();
    for one in &users.uses {
        let (at, inside) = _site(function, loop_, &nested, one)?;
        let block = match at {
            Place::Before(inst) => function.parent(inst)?,
            Place::End(block) | Place::Exit(block, _) => block,
        };
        let exit = exit.as_ref().is_some_and(|exit| exit.proof.compare == one.user);
        sites.push(Site { one: one.clone(), at, frequency: frequency(block), inside, exit, known: None });
    }
    // Exit phis that read one recurrence are realized once after the loop;
    // one reading several is realized on each way out.
    let mut exits = BTreeMap::<InstId, Vec<usize>>::new();
    for (index, site) in sites.iter().enumerate() {
        if let Place::Exit(_, phi) = site.at {
            exits.entry(phi).or_default().push(index);
        }
    }
    for (phi, indexes) in &exits {
        let operands = function.instruction(*phi).operands.chunks(2).count();
        let same = indexes.iter().all(|&index| sites[index].one.of == sites[indexes[0]].one.of);
        if !same || indexes.len() != operands {
            for &index in indexes {
                let from = match function.instruction(*phi).operands[sites[index].one.index + 1] {
                    Operand::Block(from) => from,
                    _ => return None,
                };
                sites[index].at = Place::End(from);
                sites[index].frequency = frequency(from);
                sites[index].inside = true;
            }
        } else {
            for &index in &indexes[1..] {
                sites[index].frequency = 0;
            }
        }
    }
    // Read as the loop leaves its header on a known count, a value is its
    // start plus its step each trip.
    if let Some(exit) = exit.as_ref().filter(|exit| !exit.proof.posttested && exit.trips.terms.is_empty()) {
        for site in &mut sites {
            let Place::Exit(block, phi) = site.at else { continue };
            let from_header = function.instruction(phi).operands.chunks(2).all(|pair| pair[1] == Operand::Block(cfg::block(loop_.header)));
            if block == cfg::block(exit.proof.exit) && from_header {
                let of = _normal(view, &site.one);
                site.known = of.step.product(&exit.trips.truncated(of.width())).map(|walked| of.start.plus(&walked));
            }
        }
    }
    let candidates = _candidates(view, target, &users, &sites, exit.as_ref());
    let web_values = users.values.keys().copied().collect::<BTreeSet<_>>();
    let live = _live_anyway(function, loop_, &users, exit.as_ref());
    let cells = spill::cells(function);
    let fixed = _fixed(view, outer, loop_, target.room, &cells, &web_values, &users, exit.as_ref(), &live);
    // The web's reads are the uses the choice replaces; each use adds its own back.
    let kept = |inst: InstId| !users.web.contains(&inst);
    let traffic = spill::traffic(function, &frequencies, &cells, &target.costs, &kept, &|value| spill::words(view.context, view.layout, function, value));
    let alive = _alive(function, loop_, &sites);
    let mut keys = Vec::new();
    let latch_block = cfg::block(latch);
    // The most backedges: the counted exit's, or what an in-bounds access allows.
    let most = exit.as_ref().map(|exit| exit.most.clone()).or_else(|| induction::inbounds_backedges(view, loop_));
    let fits = sites.iter().map(|site| candidates.iter().enumerate().map(|(index, one)| _priced(view, target, site, index, one, latch_block, most.as_ref(), &mut keys)).collect()).collect();
    let exits = candidates.iter().map(|one| exit.as_ref().and_then(|exit| _exit_price(target, exit, one, &mut keys))).collect();
    let problem = Problem {
        target,
        candidates,
        sites,
        fits,
        exit,
        exits,
        keys,
        fixed,
        traffic,
        alive,
        latch: frequency(cfg::block(latch)),
        header: frequency(cfg::block(loop_.header)),
        entry: frequency(preheader),
        live,
        frames: _frames(function),
    };
    let current = problem.candidates.iter().enumerate().filter(|(_, one)| one.existing.is_some()).map(|(index, _)| index).collect::<BTreeSet<_>>();
    let before = problem.total(&current);
    let chosen = problem.solved(&current);
    let after = chosen.as_ref().and_then(|chosen| problem.total(chosen));
    llrm_support::debug!(
        "lsr",
        "loop b{}: {} uses, {} candidates, exit {}; {:?} costs {:?}, {:?} costs {:?}",
        loop_.header,
        problem.sites.len(),
        problem.candidates.len(),
        problem.exit.as_ref().map_or("none".to_owned(), |exit| format!("{:?} trips", exit.trips)),
        current,
        before,
        chosen,
        after
    );
    if llrm_support::debug::enabled("lsr") {
        for (at, site) in problem.sites.iter().enumerate() {
            let op = function.instruction(site.one.user);
            llrm_support::debug!("lsr", "  site {at}: {:?} operand {} of {:?} {:?}, {:?} + {:?}*t, x{}", site.one.kind, site.one.index, op.opcode, op.result, site.one.of.start, site.one.of.step.known(), site.frequency);
        }
        for (index, one) in problem.candidates.iter().enumerate() {
            let alone = BTreeSet::from([index]);
            let sites = (0..problem.sites.len()).map(|at| problem.fits[at][index].as_ref().map(|(_, price)| price.cost)).collect::<Vec<_>>();
            llrm_support::debug!("lsr", "  {index}: {:?} + {:?}*t ptr {:?} existing {:?}: alone {:?}, exit {:?}, sites {:?}", one.of.start, one.of.step.known(), one.of.pointer, one.existing, problem.total(&alone), problem.exits[index], sites);
        }
    }
    let (chosen, after) = (chosen?, after?);
    if before.is_some_and(|before| after >= before) {
        return None;
    }
    let (uses, exit) = problem.assigned(&chosen)?;
    let kept = chosen.iter().copied().collect::<Vec<_>>();
    let renumber = |index: usize| kept.iter().position(|&one| one == index).expect("a chosen candidate");
    let Problem { candidates, exit: exit_proof, .. } = problem;
    let uses = uses.into_iter().map(|(site, index, fit)| (site, renumber(index), fit)).collect::<Vec<_>>();
    Some(Plan {
        loop_: loop_.clone(),
        preheader,
        header: cfg::block(loop_.header),
        latch: cfg::block(latch),
        candidates: kept.iter().map(|&index| candidates[index].clone()).collect(),
        uses,
        exit: exit.and_then(|index| Some((exit_proof?, renumber(index)))),
    })
}

/// Where a use's realization goes, and whether that is inside the loop:
/// before the user, at the end of the block a phi reads it from, out of
/// any loop inside this one, or after the phis of an exit.
fn _site(function: &Function, loop_: &Loop, nested: &[Loop], one: &IvUse) -> Option<(Place, bool)> {
    let op = function.instruction(one.user);
    let block = function.parent(one.user)?;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    if op.opcode == Opcode::Phi {
        let Operand::Block(from) = op.operands[one.index + 1] else { return None };
        if inside(block) {
            return Some((_hoisted(function, nested, Place::End(from)), true));
        }
        // An exit phi: every way into its block leaves the loop.
        if inside(from) && function.predecessors(block).iter().all(|&pred| inside(pred)) {
            return Some((Place::Exit(block, one.user), false));
        }
        return None;
    }
    // A reader after the loop that no exit phi carries is dominated by what
    // it reads, so reads it from the trip that left, as the header's
    // counters then are.
    Some((_hoisted(function, nested, Place::Before(one.user)), inside(block)))
}

/// `at`, moved out of every loop inside this one to its preheader.
fn _hoisted(function: &Function, nested: &[Loop], at: Place) -> Place {
    let block = match at {
        Place::Before(inst) => function.parent(inst).expect("a placed user"),
        Place::End(block) | Place::Exit(block, _) => block,
    };
    let outermost = nested.iter().filter(|one| one.body.contains(&cfg::id(block))).max_by_key(|one| one.body.len());
    match outermost.and_then(|one| _preheader(function, one)) {
        Some(preheader) => Place::End(preheader),
        None => at,
    }
}

/// The loop's counted exit, where its compare is read only by its branch.
fn _exit(view: &memory::Unit, loop_: &Loop, users: &Users) -> Option<Exit> {
    let function = view.function;
    // The count holds whenever the loop goes on: where another exit stops
    // the program, nothing after it reads what the counters were.
    let proofs = induction::counted_unless_stopped(view, loop_, None, true);
    let [proof] = &proofs[..] else { return None };
    let result = function.instruction(proof.compare).result?;
    if function.users(result).iter().any(|one| one.user != proof.branch) || users.web.contains(&proof.compare) {
        return None;
    }
    if proof.posttested && !proof.stepped {
        return None;
    }
    let trips = proof.trips_linear()?;
    let most = proof.most()?;
    let guarded = !trips.terms.is_empty();
    if guarded && (proof.posttested || rotate::_shape(function, loop_).is_none()) {
        return None;
    }
    Some(Exit { proof: proof.clone(), trips, most, guarded })
}

/// Values read inside the loop by something other than a recurrence or
/// the counted exit, or live after it.
fn _live_anyway(function: &Function, loop_: &Loop, users: &Users, exit: Option<&Exit>) -> BTreeSet<ValueId> {
    let mut live = BTreeSet::new();
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            if users.web.contains(&inst) || exit.is_some_and(|exit| exit.proof.compare == inst) {
                continue;
            }
            live.extend(function.instruction(inst).operands.iter().filter_map(|one| match one {
                Operand::Value(value) => Some(*value),
                _ => None,
            }));
        }
    }
    let found = liveness::live(function);
    for &at in &loop_.body {
        for succ in function.successors(cfg::block(at)) {
            if !loop_.body.contains(&cfg::id(succ)) {
                live.extend(found.live_in.get(&cfg::id(succ)).into_iter().flatten().copied());
            }
        }
    }
    live
}

/// The symbols of every recurrence the loop reads.
fn _symbols(users: &Users, exit: Option<&Exit>) -> BTreeSet<ValueId> {
    let mut symbols = BTreeSet::new();
    for of in users.values.values() {
        symbols.extend(of.start.terms.keys().chain(of.step.terms.keys()));
        if let Some(Operand::Value(pointer)) = of.pointer {
            symbols.insert(pointer);
        }
    }
    if let Some(exit) = exit {
        symbols.extend(exit.trips.terms.keys());
        if let induction::AffineOperand::Value(bound, _) = exit.proof.bound {
            symbols.insert(bound);
        }
    }
    symbols
}

/// What the loop keeps in registers besides its counters and the
/// invariants only its recurrences read, at each instruction.
#[allow(clippy::too_many_arguments)]
fn _fixed(
    view: &memory::Unit,
    outer: &Outer,
    loop_: &Loop,
    room: Room,
    cells: &BTreeMap<ValueId, ValueId>,
    web: &BTreeSet<ValueId>,
    users: &Users,
    exit: Option<&Exit>,
    live: &BTreeSet<ValueId>,
) -> BTreeMap<i64, Vec<spill::Site>> {
    let function = view.function;
    let symbols = _symbols(users, exit);
    let counted = |value: ValueId| spill::integer(view.context, function, value) && !web.contains(&value) && !(symbols.contains(&value) && !live.contains(&value));
    let found = liveness::live(function);
    let across = |inst: InstId| spill::kept_across(outer, view.context, function, inst);
    loop_.body.iter().map(|&at| (at, spill::sites(function, &found, cfg::block(at), room, &across, cells, &counted))).collect()
}

/// Where each site's value is live in the loop, before each instruction.
fn _alive(function: &Function, loop_: &Loop, sites: &[Site]) -> Vec<BTreeMap<i64, Vec<bool>>> {
    let found = liveness::live(function);
    sites
        .iter()
        .map(|site| {
            let own = |value: ValueId| value == site.one.value;
            loop_.body.iter().map(|&at| (at, liveness::live_points(function, &found, cfg::block(at)).into_iter().map(|(_, before, _)| before.iter().any(|&one| own(one))).collect())).collect()
        })
        .collect()
}

/// The candidates: each use's own recurrence, less its symbols and its
/// constant; the loop's counters; and each step counted to zero at the exit.
fn _candidates(view: &memory::Unit, target: &Target, users: &Users, sites: &[Site], exit: Option<&Exit>) -> Vec<Candidate> {
    let function = view.function;
    let pointer_type = |value: ValueId| {
        let ty = function.value(value).ty;
        matches!(view.context.types.get(ty), Type::Pointer(_)).then_some(ty)
    };
    let mut found = Vec::<Candidate>::new();
    let add = |found: &mut Vec<Candidate>, of: Recurrence, pointer: Option<TypeId>, existing: Option<ValueId>| {
        if of.step.is_zero() {
            return;
        }
        match found.iter_mut().find(|one| one.of == of) {
            Some(one) => one.existing = one.existing.or(existing),
            None => found.push(Candidate { of, pointer, existing }),
        }
    };
    for &counter in &users.counters {
        add(&mut found, users.values[&counter].clone(), pointer_type(counter), Some(counter));
    }
    let mut steps = BTreeSet::<Linear>::new();
    for site in sites {
        let of = _normal(view, &site.one);
        steps.insert(of.step.clone());
        let bare = Recurrence { pointer: None, start: of.start.clone(), step: of.step.clone() };
        // The start's symbols split every way between the counter and the
        // base, with its constant or without: LLVM's reassociation.
        let terms = of.start.terms.iter().collect::<Vec<_>>();
        if terms.len() <= _SPLIT_TERMS {
            for mask in 0..1_usize << terms.len() {
                let part = terms.iter().enumerate().filter(|(bit, _)| mask >> bit & 1 == 1).map(|(_, (value, factor))| (**value, (*factor).clone())).collect();
                let symbolic = Linear { constant: BigInt::from(0), terms: part, width: of.width() };
                add(&mut found, Recurrence { start: symbolic.clone(), ..bare.clone() }, None, None);
                add(&mut found, Recurrence { start: symbolic.plus(&Linear::constant(of.start.constant.clone(), of.width())), ..bare.clone() }, None, None);
            }
        } else {
            add(&mut found, Recurrence { start: Linear::constant(0, of.width()), ..bare.clone() }, None, None);
            add(&mut found, Recurrence { start: Linear::constant(of.start.constant.clone(), of.width()), ..bare.clone() }, None, None);
        }
        match of.pointer {
            Some(_) => {
                let pointer = pointer_type(site.one.value);
                if pointer.is_some() {
                    add(&mut found, of.clone(), pointer, None);
                    add(&mut found, Recurrence { start: of.start.symbolic(), ..of.clone() }, pointer, None);
                }
            }
            None => add(&mut found, bare, None, None),
        }
    }
    // An address form whose index is wider takes a counter of its width,
    // stepping by the address's step over one of its scales.
    for site in sites.iter().filter(|site| site.one.kind == UseKind::Address) {
        let of = _normal(view, &site.one);
        let Some(bytes) = of.step.known() else { continue };
        for form in target.forms.iter().filter(|form| form.index_width * 8 > i64::from(of.width())) {
            let width = u32::try_from(form.index_width * 8).expect("an index width");
            for &scale in &form.scales {
                if &bytes % scale != BigInt::from(0) {
                    continue;
                }
                let step = Linear::constant(&bytes / scale, width);
                steps.insert(step.clone());
                add(&mut found, Recurrence { pointer: None, start: Linear::constant(0, width), step }, None, None);
            }
        }
    }
    // Each step, counted to zero at the exit.
    if let Some(exit) = exit {
        for step in &steps {
            // A symbolic count is of its own width only.
            if step.width != exit.trips.width && !exit.trips.terms.is_empty() {
                continue;
            }
            let trips = exit.trips.truncated(step.width).times(&BigInt::from(-1));
            let Some(start) = trips.product(step) else { continue };
            add(&mut found, Recurrence { pointer: None, start, step: step.clone() }, None, None);
        }
    }
    found
}

/// The most symbols of a start split between counter and base, every way.
const _SPLIT_TERMS: usize = 3;

/// The function's frame objects, whose addresses need no register of their own.
fn _frames(function: &Function) -> BTreeSet<ValueId> {
    function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. })).filter_map(|(_, inst)| function.instruction(inst).result).collect()
}

/// What building `key` before the loop costs: its scaled terms, the adds
/// that join its parts, and an address off its pointer.
fn _built(target: &Target, key: &Key) -> i64 {
    let costs = &target.costs;
    let (pointer, sum) = key;
    let mut cost = sum.terms.values().map(|factor| _scaling(target, &BigInt::from(expand::signed(factor, sum.width).magnitude().clone()))).sum::<i64>();
    let parts = sum.terms.len() + usize::from(sum.constant != BigInt::from(0)) + usize::from(pointer.is_some() && !sum.is_zero());
    if sum.terms.is_empty() && pointer.is_none() {
        return 0;
    }
    cost += parts.saturating_sub(1) as i64 * costs.add;
    cost
}

/// A use's recurrence in its own width: an address's in its index's.
fn _normal(view: &memory::Unit, one: &IvUse) -> Recurrence {
    match one.of.pointer {
        Some(pointer) => match view.space(pointer) {
            Some(space) => one.of.truncated(view.layout.pointer(space).index_bits),
            None => one.of.clone(),
        },
        None => one.of.clone(),
    }
}

/// `site` as `base + rest + k * candidate + constant`.
fn _fit(view: &memory::Unit, site: &Site, candidate: &Candidate, most: Option<&BigInt>) -> Option<Fit> {
    let of = _normal(view, &site.one);
    // A wider counter indexes an address by its low bits.
    let narrowed;
    let candidate = if candidate.of.width() > of.width() && site.one.kind == UseKind::Address && candidate.of.pointer.is_none() {
        narrowed = Candidate { of: candidate.of.truncated(of.width()), ..candidate.clone() };
        &narrowed
    } else {
        candidate
    };
    if of.width() != candidate.of.width() {
        return None;
    }
    if let Some(known) = &site.known {
        let constant = known.known().unwrap_or_else(|| expand::signed(&known.constant, known.width));
        return Some(Fit { k: BigInt::from(0), base: of.pointer, rest: known.symbolic(), constant, next: false, trip: None, folded: None });
    }
    let Some(k) = of.step.over(&candidate.of.step) else { return _trip_fit(site, &of, candidate, most) };
    let (base, rest) = match (candidate.of.pointer, of.pointer) {
        (Some(one), Some(other)) if one == other && k == BigInt::from(1) => (None, of.start.minus(&candidate.of.start)),
        (Some(_), _) => return None,
        (None, pointer) => (pointer, of.start.minus(&candidate.of.start.times(&k))),
    };
    let mut constant = rest.known().unwrap_or_else(|| expand::signed(&rest.constant, rest.width));
    let mut rest = rest.symbolic();
    // What its reader cannot see need not be added: a multiple of the bits
    // it observes.
    let seen = BigInt::from(1) << site.one.demanded.min(rest.width);
    if base.is_none() && site.one.demanded < rest.width {
        rest.terms.retain(|_, factor| &*factor % &seen != BigInt::from(0));
        if &constant % &seen == BigInt::from(0) {
            constant = BigInt::from(0);
        }
    }
    Some(Fit { k, base, rest, constant, next: false, trip: None, folded: None })
}

/// After the loop, `site` from the trip the candidate has counted: its
/// distance from its start divided exactly by its step, which holds while
/// the loop's trips stay below the step's period.
fn _trip_fit(site: &Site, of: &Recurrence, candidate: &Candidate, most: Option<&BigInt>) -> Option<Fit> {
    let step = candidate.of.step.known()?;
    let k = of.step.known()?;
    if site.inside || candidate.of.pointer.is_some() || step == BigInt::from(0) {
        return None;
    }
    let magnitude = BigInt::from(step.magnitude().clone());
    let shift = u32::try_from(magnitude.trailing_zeros()?).ok()?;
    let bits = candidate.of.width().checked_sub(shift)?;
    if most? >= &(BigInt::from(1) << bits) {
        return None;
    }
    let inverse = _inverse(&(&magnitude >> shift), bits);
    let constant = of.start.known().unwrap_or_else(|| expand::signed(&of.start.constant, of.start.width));
    Some(Fit { k, base: of.pointer, rest: of.start.symbolic(), constant, next: false, trip: Some((shift, step < BigInt::from(0), inverse)), folded: None })
}

/// The inverse of odd `n` modulo `2^bits`, by Newton's iteration.
fn _inverse(n: &BigInt, bits: u32) -> BigInt {
    let modulus = BigInt::from(1) << bits;
    let mut x = n.clone();
    for _ in 0..7 {
        x = (&x * (BigInt::from(2) - n * &x)) % &modulus;
        if x < BigInt::from(0) {
            x += &modulus;
        }
    }
    x
}

/// What `phi`, a counter, takes from `latch`.
fn _latch_arm(function: &Function, phi: ValueId, latch: BlockId) -> Option<Operand> {
    let ValueDef::Instruction(inst) = function.value(phi).def else { return None };
    function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(latch)).map(|pair| pair[0])
}

/// What `k * r` costs a trip: nothing, a negation, a shift or a multiply.
fn _scaling(target: &Target, k: &BigInt) -> i64 {
    let costs = &target.costs;
    let magnitude = BigInt::from(k.magnitude().clone());
    if *k == BigInt::from(1) || *k == BigInt::from(0) {
        0
    } else if *k == BigInt::from(-1) {
        costs.add
    } else if (&magnitude & (&magnitude - 1)) == BigInt::from(0) {
        costs.shift + if *k < BigInt::from(0) { costs.add } else { 0 }
    } else {
        let multiply = magnitude.to_i64().map_or(costs.multiply, |factor| target.machine.multiply_by(factor));
        multiply + if *k < BigInt::from(0) { costs.add } else { 0 }
    }
}

/// `site`'s price from `candidate`.
/// `key`'s index in `keys`, added where new.
fn _interned(keys: &mut Vec<Key>, key: Key) -> usize {
    match keys.iter().position(|one| *one == key) {
        Some(index) => index,
        None => {
            keys.push(key);
            keys.len() - 1
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn _priced(view: &memory::Unit, target: &Target, site: &Site, index: usize, candidate: &Candidate, latch: BlockId, most: Option<&BigInt>, keys: &mut Vec<Key>) -> Option<(Fit, Price)> {
    let mut fit = _fit(view, site, candidate, most)?;
    // The candidate plus its step, in the latch, is the step itself: a new
    // one is placed before its first reader, a counter's must be what is read.
    let in_latch = match site.at {
        Place::Before(inst) => view.function.parent(inst) == Some(latch),
        Place::End(block) => block == latch,
        Place::Exit(..) => false,
    };
    let stepped = fit.k == BigInt::from(1) && fit.rest.is_zero() && candidate.of.step.known().is_some_and(|step| step == fit.constant) && candidate.of.pointer.is_none() && site.one.of.pointer.is_none();
    let own = match candidate.existing {
        Some(existing) => _latch_arm(view.function, existing, latch) == Some(Operand::Value(site.one.value)),
        None => true,
    };
    // A counter's own step is read where it stands.
    if (in_latch || candidate.existing.is_some()) && stepped && own && site.one.kind != UseKind::Address {
        fit.next = true;
        // A step's own flags answer its equality with zero.
        let op = view.function.instruction(site.one.user);
        let zero = matches!(op.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)) && view.int_constant(op.operands[1 - site.one.index]) == Some(0);
        let cost = if site.one.kind == UseKind::Compare && !zero { target.costs.add } else { 0 };
        let mut held = Vec::new();
        if site.one.kind == UseKind::Compare
            && let Operand::Value(other) = view.function.instruction(site.one.user).operands[1 - site.one.index]
        {
            let width = view.int_bits(Operand::Value(other)).unwrap_or(fit.rest.width);
            held.push(_interned(keys, (None, Linear::of(&induction::AffineOperand::Value(other, width), width))));
        }
        return Some((fit, Price { cost, held, address: Vec::new(), wide: false, product: None }));
    }
    let costs = &target.costs;
    // A constant pointer is a displacement; any other base a register.
    let frames = _frames(view.function);
    let pointer_base = |fit: &Fit| matches!(fit.base, Some(Operand::Value(value)) if !frames.contains(&value)) || !fit.rest.is_zero();
    let mut price = Price { cost: 0, held: Vec::new(), address: Vec::new(), wide: false, product: None };
    let block = cfg::id(match site.at {
        Place::Before(inst) => view.function.parent(inst)?,
        Place::End(block) | Place::Exit(block, _) => block,
    });
    let small = |k: &BigInt| k.to_string().parse::<i64>().unwrap_or(i64::MAX);
    match site.one.kind {
        UseKind::Address if candidate.of.pointer.is_some() => {
            price.address.push(Reg::Iv(index));
            if !fit.rest.is_zero() {
                let key = _interned(keys, (None, fit.rest.clone()));
                price.held.push(key);
                price.address.push(Reg::Held(key));
            }
        }
        UseKind::Address => {
            let native = target.forms.first()?;
            if pointer_base(&fit) {
                // A global is a displacement beside two registers; a frame object
                // takes BP for its own, and leaves one register for all it adds.
                let held = fit.base.filter(|base| matches!(base, Operand::Value(_)));
                let key = _interned(keys, (held, fit.rest.clone()));
                price.held.push(key);
                price.address.push(Reg::Held(key));
            }
            if fit.k == BigInt::from(1) {
                price.address.push(Reg::Iv(index));
                price.cost += native.use_cost;
            } else if fit.k != BigInt::from(0) {
                let scale = fit.k.to_string().parse::<i64>().unwrap_or(0);
                let wider = candidate.of.width() > fit.rest.width;
                let nonnegative = wider
                    || candidate.of.start.known().is_some_and(|start| start >= BigInt::from(0)) && candidate.of.step.known().is_some_and(|step| step > BigInt::from(0));
                // A form that scales takes the index at its own width: a
                // narrower one, zero-extended, must not go negative, and
                // pays its extension each time it changes.
                let width = i64::from(candidate.of.width()) / 8;
                let scaled = target
                    .forms
                    .iter()
                    .filter(|form| form.scales.contains(&scale) && (form.index_width == width || nonnegative && form.index_width > width))
                    .map(|form| form.use_cost + if form.index_width > width { form.extension_cost } else { 0 })
                    .min();
                let computed = _scaling(target, &fit.k) + native.use_cost;
                match scaled {
                    Some(cost) if cost < computed => {
                        price.cost += cost;
                        price.wide = true;
                        price.address.clear();
                    }
                    _ => {
                        price.cost += computed;
                        price.address.push(Reg::Product(index, small(&fit.k), block));
                        price.product = Some((small(&fit.k), _scaling(target, &fit.k), block));
                    }
                }
            }
        }
        UseKind::Compare if fit.trip.is_none() && fit.base.is_none() && candidate.of.pointer.is_none() && (fit.k == BigInt::from(1) || fit.k == BigInt::from(-1)) && {
            let op = view.function.instruction(site.one.user);
            matches!(op.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
        } =>
        {
            // `k * r + rest == w` is `r == k * (w - rest)`: the invariant absorbs the rest.
            let op = view.function.instruction(site.one.user);
            let other = op.operands[1 - site.one.index];
            let width = fit.rest.width;
            let invariant = match view.int_constant(other) {
                Some(bits) => Linear::constant(BigInt::from(bits), width),
                None => Linear::of(&induction::term(view, other)?, width),
            };
            let folded = invariant.minus(&fit.rest).minus(&Linear::constant(fit.constant.clone(), width)).times(&fit.k);
            if !folded.terms.is_empty() {
                price.held.push(_interned(keys, (None, folded.clone())));
            }
            price.cost += costs.add;
            fit.folded = Some(folded);
        }
        UseKind::Compare | UseKind::Basic => {
            if let Some((shift, _, inverse)) = &fit.trip {
                price.cost += costs.add + if *shift != 0 { costs.shift } else { 0 } + if *inverse != BigInt::from(1) { costs.multiply + costs.add } else { 0 };
            }
            price.cost += _scaling(target, &fit.k);
            if fit.trip.is_none() && _scaling(target, &fit.k) != 0 {
                price.product = Some((small(&fit.k), _scaling(target, &fit.k), block));
            }
            let pointer = candidate.of.pointer.is_some();
            let symbolic = !fit.rest.is_zero() || matches!(fit.base, Some(Operand::Value(_)));
            if symbolic {
                let whole = fit.rest.plus(&Linear::constant(fit.constant.clone(), fit.rest.width));
                price.held.push(_interned(keys, (if pointer { None } else { fit.base }, whole)));
                price.cost += costs.add;
            } else if fit.constant != BigInt::from(0) || fit.base.is_some() {
                price.cost += costs.add;
            }
            if site.one.kind == UseKind::Compare {
                price.cost += costs.add;
                let op = view.function.instruction(site.one.user);
                if let Operand::Value(other) = op.operands[1 - site.one.index] {
                    let width = view.int_bits(Operand::Value(other)).unwrap_or(fit.rest.width);
                    price.held.push(_interned(keys, (None, Linear::of(&induction::AffineOperand::Value(other, width), width))));
                }
            }
        }
    }
    Some((fit, price))
}

/// The exit's price testing `candidate` for its last value: a compare,
/// none where that is zero, and a register where it is symbolic.
fn _exit_price(target: &Target, exit: &Exit, candidate: &Candidate, keys: &mut Vec<Key>) -> Option<(i64, Option<usize>)> {
    let step = candidate.of.step.known()?;
    if candidate.of.width() < exit.trips.width || candidate.of.step.width != candidate.of.width() {
        return None;
    }
    // A symbolic count is of its own width only.
    if candidate.of.width() != exit.trips.width && !exit.trips.terms.is_empty() {
        return None;
    }
    let modulus = BigInt::from(1) << candidate.of.width();
    let period = &modulus / induction::gcd(BigInt::from(step.magnitude().clone()), modulus.clone());
    // Tested before a trip, the value must not come round in `most`; tested
    // after one, as a guarded loop entered at its body is, it may just return.
    let after = exit.guarded || exit.proof.posttested;
    if exit.most > period || (!after && exit.most == period) {
        return None;
    }
    let end = _end(exit, candidate);
    if end.is_zero() && candidate.of.pointer.is_none() {
        return Some((0, None));
    }
    let key = (!end.terms.is_empty() || candidate.of.pointer.is_some()).then(|| _interned(keys, (candidate.of.pointer, end)));
    Some((target.costs.add, key))
}

/// `candidate`'s value as the loop leaves: its start plus its step each trip.
fn _end(exit: &Exit, candidate: &Candidate) -> Linear {
    let trips = exit.trips.truncated(candidate.of.width());
    candidate.of.start.plus(&trips.product(&candidate.of.step).expect("a constant step"))
}

impl Problem<'_> {
    /// What holds invariant `key`: the loop's own value where it names one,
    /// else a new register.
    fn resident(&self, key: usize) -> Resident {
        let named = match &self.keys[key] {
            (None, sum) if sum.constant == BigInt::from(0) && sum.terms.len() == 1 => sum.terms.iter().find(|(_, factor)| **factor == BigInt::from(1)).map(|(value, _)| *value),
            (Some(Operand::Value(value)), sum) if sum.is_zero() => Some(*value),
            _ => None,
        };
        match named {
            Some(value) if self.free(&self.keys[key]) => Resident::Value(value),
            _ => Resident::Held(key),
        }
    }

    /// What keeping `one` in memory costs, read as `reads` says.
    fn spill_price(&self, one: Resident, reads: &BTreeMap<Resident, i64>) -> i64 {
        let read = reads.get(&one).copied().unwrap_or(0);
        let traffic = match one {
            Resident::Value(value) => {
                let kept = self.traffic.get(&value).copied().unwrap_or_default();
                Traffic { loads: kept.loads + read, ..kept }
            }
            // Stepped in place each trip; a new one's start stored on entry.
            Resident::Counter(at) => Traffic { stores: if self.candidates[at].existing.is_none() { self.entry } else { 0 }, updates: self.latch, loads: read, rebuild: None },
            Resident::Held(_) => Traffic { stores: self.entry, loads: read, ..Traffic::default() },
            Resident::Step(_) => Traffic { stores: self.entry, loads: self.latch, ..Traffic::default() },
            Resident::Product(..) => Traffic { stores: read, loads: read, ..Traffic::default() },
            Resident::Rebuilt(at) => Traffic { stores: self.sites[at].frequency, loads: self.sites[at].frequency, ..Traffic::default() },
        };
        traffic.price(&self.target.costs)
    }

    /// Each site's cheapest fit among `set`; the exit's candidate where
    /// testing its last value is cheaper than keeping its compare.
    #[allow(clippy::type_complexity)]
    fn assigned(&self, set: &BTreeSet<usize>) -> Option<(Vec<(Site, usize, Fit)>, Option<usize>)> {
        let mut uses = Vec::new();
        let mut exit = None;
        for (index, site) in self.sites.iter().enumerate() {
            let (kept, tested) = self.choice(set, index)?;
            if let Some((one, _)) = tested {
                exit = Some(one);
                continue;
            }
            let (one, fit, _) = kept.expect("a fit where nothing else ends the loop");
            uses.push((site.clone(), one, fit.clone()));
        }
        Some((uses, exit))
    }

    /// Site `index`'s cheapest fit among `set`, or for the exit's compare
    /// the candidate whose last value it tests where that is cheaper.
    #[allow(clippy::type_complexity)]
    fn choice(&self, set: &BTreeSet<usize>, index: usize) -> Option<(Option<(usize, &Fit, &Price)>, Option<(usize, (i64, Option<usize>))>)> {
        let kept = set.iter().filter_map(|&one| self.fits[index][one].as_ref().map(|(fit, price)| (one, fit, price))).min_by_key(|(one, _, price)| (price.cost, price.held.len(), *one));
        if !self.sites[index].exit {
            return kept.map(|kept| (Some(kept), None));
        }
        let tested = set.iter().filter_map(|&one| self.exits[one].clone().map(|price| (one, price))).min_by_key(|(one, (cost, key))| (*cost, key.is_some(), *one));
        match (kept, tested) {
            (Some(kept), Some(tested)) if (kept.2.cost, kept.2.held.len()) < (tested.1.0, usize::from(tested.1.1.is_some())) => Some((Some(kept), None)),
            (_, Some(tested)) => Some((None, Some(tested))),
            (Some(kept), None) => Some((Some(kept), None)),
            (None, None) => None,
        }
    }

    /// The cost of `set`, where every use has a fit in it.
    fn total(&self, set: &BTreeSet<usize>) -> Option<i64> {
        if set.is_empty() {
            return None;
        }
        let costs = &self.target.costs;
        let mut cost = set.len() as i64 * costs.add * self.latch;
        // A counter wider than the native index pays the operand-size prefix a step.
        let native = self.target.forms.first().map_or(i64::MAX, |form| form.index_width * 8);
        cost += set.iter().filter(|&&one| i64::from(self.candidates[one].of.width()) > native).count() as i64 * costs.prefix * self.latch;
        let mut held = BTreeSet::<usize>::new();
        let mut built = BTreeSet::<usize>::new();
        let mut products = BTreeSet::<(usize, i64, i64)>::new();
        // Where in its block each product is read last.
        let mut last = BTreeMap::<(usize, i64, i64), usize>::new();
        let mut address = BTreeSet::<Reg>::new();
        let mut pairs = BTreeSet::<(Reg, Reg)>::new();
        // Values built from a counter rather than being one.
        let mut rebuilt = Vec::<usize>::new();
        // How often each counter and invariant is read a trip.
        let mut reads = BTreeMap::<Resident, i64>::new();

        for (index, site) in self.sites.iter().enumerate() {
            match self.choice(set, index)? {
                (_, Some((one, (exit, key)))) => {
                    cost += exit * site.frequency;
                    *reads.entry(Resident::Counter(one)).or_default() += site.frequency;
                    if let Some(key) = key {
                        *reads.entry(self.resident(key)).or_default() += site.frequency;
                    }
                    held.extend(key);
                    built.extend(key);
                }
                (Some((one, fit, price)), None) => {
                    cost += price.cost * site.frequency;
                    if site.inside {
                        if fit.k != BigInt::from(0) {
                            *reads.entry(Resident::Counter(one)).or_default() += site.frequency;
                        }
                        for &key in &price.held {
                            *reads.entry(self.resident(key)).or_default() += site.frequency;
                        }
                    }
                    let product = fit.rest.is_zero() && fit.constant == BigInt::from(0) && fit.base.is_none();
                    if site.inside && site.one.kind == UseKind::Basic && !fit.next && !(product && fit.k == BigInt::from(1)) {
                        rebuilt.push(index);
                    }
                    if let Some((k, scaling, block)) = price.product
                        && site.inside
                        && !(product && rebuilt.last() == Some(&index))
                    {
                        if !products.insert((one, k, block)) {
                            cost -= scaling * site.frequency;
                        }
                        let reader = match site.at {
                            Place::Before(inst) => Some(inst),
                            _ => None,
                        };
                        let points = self.fixed.get(&block);
                        let at = points.and_then(|points| points.iter().position(|point| Some(point.inst) == reader)).unwrap_or_else(|| points.map_or(0, |points| points.len().saturating_sub(1)));
                        let slot = last.entry((one, k, block)).or_insert(at);
                        *slot = (*slot).max(at);
                        *reads.entry(Resident::Product(one, k, block)).or_default() += site.frequency;
                    }
                    built.extend(price.held.iter().copied());
                    if site.inside {
                        held.extend(price.held.iter().cloned());
                        address.extend(price.address.iter().cloned());
                        if let [one, other] = &price.address[..] {
                            pairs.insert((one.clone().min(other.clone()), one.clone().max(other.clone())));
                        }
                    }
                }
                (None, None) => return None,
            }
        }
        // Starts, symbolic steps and invariants, built once a way in.
        for &one in set {
            let candidate = &self.candidates[one];
            if candidate.existing.is_none() {
                cost += (_built(self.target, &(candidate.of.pointer, candidate.of.start.clone())) + _built(self.target, &(None, candidate.of.step.clone()))) * self.entry;
            }
        }
        cost += built.iter().map(|&key| _built(self.target, &self.keys[key])).sum::<i64>() * self.entry;
        // Counters, their symbolic steps and new invariants live throughout;
        // a product the loop computes, from its block's top to its last
        // reader; a value built from a counter, where it is live.
        let throughout = set
            .iter()
            .map(|&one| Resident::Counter(one))
            .chain(set.iter().filter(|&&one| self.candidates[one].of.step.known().is_none()).map(|&one| Resident::Step(one)))
            .chain(held.iter().filter(|&&key| !self.free(&self.keys[key])).map(|&key| Resident::Held(key)))
            .collect::<Vec<_>>();
        let points = self.fixed.iter().flat_map(|(&block, sites)| {
            let (throughout, last, rebuilt) = (&throughout, &last, &rebuilt);
            sites.iter().enumerate().flat_map(move |(index, site)| {
                let added = throughout
                    .iter()
                    .copied()
                    .chain(last.iter().filter(|(product, read)| product.2 == block && **read >= index).map(|(&(one, k, block), _)| Resident::Product(one, k, block)))
                    .chain(rebuilt.iter().filter(|&&at| self.alive[at].get(&block).is_some_and(|alive| alive[index])).map(|&at| Resident::Rebuilt(at)))
                    .collect::<Vec<_>>();
                std::iter::once(&site.before).chain(&site.across).map(move |point| spill::Point {
                    registers: point.registers,
                    residents: point.residents.iter().map(|&value| Resident::Value(value)).chain(added.iter().copied()).collect(),
                })
            })
        });
        if self.target.room.priced() {
            cost += spill::spilled(points, |one| self.spill_price(one, &reads));
        }
        if let Some(native) = self.target.forms.first()
            && let Some(limit) = native.address_registers()
        {
            let over = address.len() as i64 - limit;
            if over > 0 {
                cost += over * costs.r#move * self.header;
            }
            let partners = native.partners.unwrap_or(i64::MAX);
            let hub = address.iter().map(|reg| pairs.iter().filter(|(one, other)| one == reg || other == reg).count() as i64).max().unwrap_or(0);
            let unpaired = pairs.len() as i64 - hub.min(partners);
            if unpaired > 0 {
                cost += unpaired * costs.add * self.header;
            }
        }
        Some(cost)
    }

    /// Whether `key` is a value the loop holds anyway.
    fn free(&self, key: &Key) -> bool {
        match key {
            (None, sum) => sum.constant == BigInt::from(0) && sum.terms.len() == 1 && sum.terms.iter().all(|(value, factor)| *factor == BigInt::from(1) && self.live.contains(value)),
            (Some(Operand::Value(value)), sum) => sum.is_zero() && (self.live.contains(value) || self.frames.contains(value)),
            (Some(_), sum) => sum.is_zero(),
        }
    }

    /// The cheapest set found from `current` and from nothing.
    fn solved(&self, current: &BTreeSet<usize>) -> Option<BTreeSet<usize>> {
        let starts = [current.clone(), self.initial()?];
        starts.into_iter().filter_map(|start| self.improved(start)).min_by_key(|set| (self.total(set).unwrap_or(i64::MAX), set.len(), set.clone()))
    }

    /// Each site's cheapest candidate alone, added until every site fits.
    fn initial(&self) -> Option<BTreeSet<usize>> {
        let mut set = BTreeSet::new();
        for (index, _) in self.sites.iter().enumerate() {
            if set.iter().any(|&one: &usize| self.fits[index][one].is_some()) {
                continue;
            }
            let best = (0..self.candidates.len()).filter(|&one| self.fits[index][one].is_some()).min_by_key(|&one| {
                let mut with = set.clone();
                with.insert(one);
                (self.fits.iter().filter(|fits| with.iter().all(|&one| fits[one].is_none())).count(), self.fits[index][one].as_ref().map_or(i64::MAX, |(_, price)| price.cost), one)
            })?;
            set.insert(best);
        }
        if set.is_empty() {
            set.insert((0..self.candidates.len()).find(|&one| self.exits[one].is_some())?);
        }
        Some(set)
    }

    /// `set` improved by adding, removing or swapping one candidate while
    /// any lowers the cost.
    fn improved(&self, mut set: BTreeSet<usize>) -> Option<BTreeSet<usize>> {
        // Cheaper first, then fewer counters: a tie goes to fewer registers.
        let rank = |set: &BTreeSet<usize>| self.total(set).map(|total| (total, set.len()));
        let mut current = rank(&set).unwrap_or((i64::MAX, usize::MAX));
        loop {
            let mut best: Option<((i64, usize), BTreeSet<usize>)> = None;
            let mut consider = |with: BTreeSet<usize>| {
                if let Some(ranked) = rank(&with)
                    && ranked < current
                    && best.as_ref().is_none_or(|(least, one)| ranked < *least || (ranked == *least && with < *one))
                {
                    best = Some((ranked, with));
                }
            };
            for one in 0..self.candidates.len() {
                let mut with = set.clone();
                if !with.insert(one) {
                    with.remove(&one);
                }
                consider(with);
            }
            for &out in &set {
                for into in (0..self.candidates.len()).filter(|one| !set.contains(one)) {
                    let mut with = set.clone();
                    with.remove(&out);
                    with.insert(into);
                    consider(with);
                }
            }
            let Some((ranked, with)) = best else { break };
            current = ranked;
            set = with;
        }
        (current.0 != i64::MAX).then_some(set)
    }
}

/// `plan` carried out; the block a guarded loop is now entered at.
fn _applied(unit: &mut Unit, plan: &Plan) -> Option<BlockId> {
    let (context, function) = (&mut *unit.context, &mut *unit.function);
    let entering = function.terminator(plan.preheader).expect("a preheader's branch");
    let mut expander = Expander::new(entering);
    let back = function.terminator(plan.latch).expect("a latch's branch");
    // Each candidate: a header phi from its start, stepped before the latch's branch.
    let mut registers = Vec::new();
    let mut steps = Vec::new();
    let mut stepping = Vec::new();
    for one in &plan.candidates {
        if let Some(existing) = one.existing {
            registers.push(Operand::Value(existing));
            let ValueDef::Instruction(phi) = function.value(existing).def else { unreachable!("a counter is a phi") };
            let from_latch = function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(plan.latch)).map(|pair| pair[0]).expect("a latch arm");
            steps.push(from_latch);
            stepping.push(None);
            continue;
        }
        let ty = one.pointer.unwrap_or_else(|| context.types.int(one.of.width()));
        let start = expander.value(context, function, one.of.pointer, &one.of.start, ty);
        let step = expander.int(context, function, &one.of.step);
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("lsr.iv"));
        let first = function.block(plan.header).instructions()[0];
        function.insert(phi, Position::Before(first)).expect("a header");
        let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        let opcode = if one.of.pointer.is_some() { Opcode::GetElementPtr { source: context.types.int(8) } } else { Opcode::Binary(BinaryOp::Add) };
        let next = function.create_instruction(opcode, ty, vec![value, step], Flags::default(), Some("lsr.iv.next"));
        function.insert(next, Position::Before(back)).expect("a latch");
        stepping.push(Some(next));
        let next = Operand::Value(function.instruction(next).result.expect("a value"));
        function.set_operands(phi, vec![start, Operand::Block(plan.preheader), next, Operand::Block(plan.latch)]);
        registers.push(value);
        steps.push(next);
    }
    // Each use from its candidate. An exit phi takes the candidate as it
    // leaves, and what it carried is computed after the phis.
    let mut leaving = HashMap::<(BlockId, usize), Operand>::default();
    let mut products = HashMap::default();
    // Steps are placed first: an address after one reads the stepped value.
    let ordered = plan.uses.iter().filter(|(_, _, fit)| fit.next).chain(plan.uses.iter().filter(|(_, _, fit)| !fit.next));
    for (site, index, fit) in ordered {
        let candidate = &plan.candidates[*index];
        if let Place::Exit(_, phi) = site.at
            && function.is_erased(phi)
        {
            continue;
        }
        let (at, register) = match site.at {
            Place::Before(inst) => (Position::Before(inst), registers[*index]),
            Place::End(block) => (Position::Before(function.terminator(block).expect("a terminated block")), registers[*index]),
            Place::Exit(block, _) => {
                let register = match leaving.get(&(block, *index)) {
                    Some(&one) => one,
                    None => {
                        let preds = function.predecessors(block);
                        let ty = function.value(match registers[*index] {
                            Operand::Value(value) => value,
                            _ => unreachable!("a candidate is a value"),
                        })
                        .ty;
                        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
                        let first = function.block(block).instructions()[0];
                        function.insert(phi, Position::Before(first)).expect("an exit");
                        function.set_operands(phi, preds.iter().flat_map(|&pred| [registers[*index], Operand::Block(pred)]).collect());
                        let one = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
                        leaving.insert((block, *index), one);
                        one
                    }
                };
                let after = function.block(block).instructions().iter().copied().find(|&inst| function.instruction(inst).opcode != Opcode::Phi).expect("a terminator");
                (Position::Before(after), register)
            }
        };
        let ty = function.value(site.one.value).ty;
        let value = if fit.next {
            // The step goes before its first reader in the latch.
            if let (Some(step), Place::Before(reader)) = (stepping[*index], site.at) {
                let order = function.block(plan.latch).instructions();
                if order.iter().position(|&one| one == reader) < order.iter().position(|&one| one == step) {
                    function.move_to(step, Position::Before(reader)).expect("a placed reader");
                }
            }
            steps[*index]
        } else if fit.folded.is_some() {
            register
        } else if let Some(after) = _after_step(function, plan.latch, site, candidate, fit, steps[*index]) {
            _realized(context, function, &mut expander, &mut products, site, candidate, &after, steps[*index], ty, at)
        } else {
            _realized(context, function, &mut expander, &mut products, site, candidate, fit, register, ty, at)
        };
        match site.at {
            Place::Exit(_, phi) => {
                let result = function.instruction(phi).result.expect("a phi's value");
                function.replace_all_uses_with(result, value);
                function.set_operands(phi, Vec::new());
                function.erase(phi).expect("its uses were replaced");
            }
            _ => function.set_operand(site.one.user, site.one.index, value),
        }
        if let Some(folded) = &fit.folded {
            let other = expander.int(context, function, folded);
            function.set_operand(site.one.user, 1 - site.one.index, other);
        }
    }
    let (exit, index) = plan.exit.as_ref()?;
    let candidate = &plan.candidates[*index];
    let proof = &exit.proof;
    let end = _end(exit, candidate);
    let ty = function.value(match registers[*index] {
        Operand::Value(value) => value,
        _ => unreachable!("a candidate is a value"),
    })
    .ty;
    let end = expander.value(context, function, candidate.of.pointer, &end, ty);
    let tested = if proof.posttested { steps[*index] } else { registers[*index] };
    let branch = proof.branch;
    let continues = matches!(function.instruction(branch).operands[1], Operand::Block(block) if plan.loop_.body.contains(&cfg::id(block)));
    let predicate = if continues { IntPredicate::Ne } else { IntPredicate::Eq };
    let bit = context.types.int(1);
    let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![tested, end], Flags::default(), None);
    function.insert(test, Position::Before(branch)).expect("a placed branch");
    function.set_operand(branch, 0, Operand::Value(function.instruction(test).result.expect("a value")));
    if !exit.guarded {
        return None;
    }
    // A symbolic count holds only once the loop is entered: a guard skips
    // it where it runs no trip, and the loop is entered at its body.
    let guard = counting::skip_guard(&mut Seeds { context: &mut *context, function: &mut *function, at: entering, width: proof.width() }, proof).expect("a pre-tested loop");
    let shape = rotate::_shape(function, &plan.loop_).expect("a rotatable loop");
    rotate::_rotate(context, function, &shape, Some(Operand::Value(guard))).expect("a rotation");
    crate::cfg::merged(function);
    // The guard also reaches the exit: the loop leaves through its own block again.
    crate::loopsimplify::simplified(function);
    Some(shape.first)
}

/// An address placed after its candidate's step in the latch, as a fit
/// of the stepped value: a step less in its constant. Reading the counter
/// there would keep it live beside its successor.
fn _after_step(function: &Function, latch: BlockId, site: &Site, candidate: &Candidate, fit: &Fit, stepped: Operand) -> Option<Fit> {
    let Place::Before(reader) = site.at else { return None };
    let Operand::Value(stepped) = stepped else { return None };
    let ValueDef::Instruction(step) = function.value(stepped).def else { return None };
    let order = function.block(latch).instructions();
    let (step_at, reader_at) = (order.iter().position(|&one| one == step)?, order.iter().position(|&one| one == reader)?);
    if site.one.kind != UseKind::Address || fit.trip.is_some() || fit.k == BigInt::from(0) || step_at > reader_at {
        return None;
    }
    let by = candidate.of.step.known()?;
    Some(Fit { constant: &fit.constant - &fit.k * by, ..fit.clone() })
}

/// `site`'s value from `register`, the candidate's, placed at `at`.
#[allow(clippy::too_many_arguments)]
fn _realized(
    context: &mut Context,
    function: &mut Function,
    expander: &mut Expander,
    products: &mut HashMap<(Operand, BigInt, BlockId), Operand>,
    site: &Site,
    candidate: &Candidate,
    fit: &Fit,
    register: Operand,
    ty: TypeId,
    at: Position,
) -> Operand {
    let width = fit.rest.width;
    let int = context.types.int(width);
    let i8 = context.types.int(8);
    let magnitude = BigInt::from(fit.k.magnitude().clone());
    let constant = Linear::constant(fit.constant.clone(), width);
    let (register, pointer) = match &fit.trip {
        Some((shift, down, inverse)) => {
            let (shift, down) = (*shift, *down);
            let mut trip = if candidate.of.start.is_zero() && !down {
                // The distance from a start of zero is the counter.
                register
            } else {
                let start = expander.int(context, function, &candidate.of.start);
                let distance = if down { vec![start, register] } else { vec![register, start] };
                expand::placed(context, function, Opcode::Binary(BinaryOp::Sub), int, distance, at)
            };
            if shift != 0 {
                let by = counting::constant(context, &BigInt::from(shift), width);
                trip = expand::placed(context, function, Opcode::Binary(BinaryOp::LShr), int, vec![trip, by], at);
            }
            if *inverse != BigInt::from(1) {
                let by = counting::constant(context, inverse, width);
                trip = expand::placed(context, function, Opcode::Binary(BinaryOp::Mul), int, vec![trip, by], at);
                if shift != 0 {
                    let mask = counting::constant(context, &((BigInt::from(1) << (width - shift)) - 1), width);
                    trip = expand::placed(context, function, Opcode::Binary(BinaryOp::And), int, vec![trip, mask], at);
                }
            }
            (trip, None)
        }
        None => (register, candidate.of.pointer),
    };
    // A product in the loop is made once in its block, at its top.
    let shared = match (&fit.trip, site.at) {
        (None, Place::Before(inst)) => function.parent(inst),
        (None, Place::End(block)) => Some(block),
        _ => None,
    };
    let scaled = |context: &mut Context, function: &mut Function, products: &mut HashMap<(Operand, BigInt, BlockId), Operand>| -> Option<Operand> {
        if fit.k == BigInt::from(0) {
            return None;
        }
        if fit.k == BigInt::from(1) {
            return Some(register);
        }
        if let Some(&made) = shared.and_then(|block| products.get(&(register, fit.k.clone(), block))) {
            return Some(made);
        }
        // At the block's top, or after the register where the block steps it.
        let at = match shared {
            Some(block) => {
                let order = function.block(block).instructions();
                let defined = match register {
                    Operand::Value(value) => match function.value(value).def {
                        ValueDef::Instruction(def) => order.iter().position(|&one| one == def).filter(|_| function.instruction(def).opcode != Opcode::Phi),
                        _ => None,
                    },
                    _ => None,
                };
                let first = defined.map_or_else(|| order.iter().position(|&one| function.instruction(one).opcode != Opcode::Phi).expect("a terminator"), |def| def + 1);
                Position::Before(order[first])
            }
            None => at,
        };
        // An index is scaled at its register's width.
        let index = if fit.trip.is_none() { candidate.of.width() } else { width };
        let scaled = expand::scaled(context, function, register, &magnitude, index, at);
        let made = if fit.k < BigInt::from(0) {
            let zero = counting::constant(context, &BigInt::from(0), index);
            let wide = context.types.int(index);
            expand::placed(context, function, Opcode::Binary(BinaryOp::Sub), wide, vec![zero, scaled], at)
        } else {
            scaled
        };
        if let Some(block) = shared {
            products.insert((register, fit.k.clone(), block), made);
        }
        Some(made)
    };
    // A value no candidate steps, as one read after a loop of known count, is
    // its base and its sum.
    if fit.k == BigInt::from(0) && fit.trip.is_none() {
        let sum = fit.rest.plus(&constant);
        return match site.one.of.pointer {
            Some(_) => expander.value(context, function, fit.base, &sum, ty),
            None => expander.int(context, function, &sum),
        };
    }
    match (pointer, site.one.of.pointer) {
        // A pointer candidate: itself, offset by the rest and the constant.
        (Some(_), _) => {
            let mut value = register;
            if !fit.rest.is_zero() {
                let rest = expander.int(context, function, &fit.rest);
                value = expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![value, rest], at);
            }
            if fit.constant != BigInt::from(0) {
                let offset = counting::constant(context, &fit.constant, width);
                value = expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![value, offset], at);
            }
            value
        }
        // An address or pointer from an integer candidate: its base indexed
        // by the scaled candidate, then the constant, which the address form takes.
        (None, Some(_)) => {
            let base = expander.value(context, function, fit.base, &fit.rest, ty);
            let mut value = match scaled(context, function, products) {
                Some(index) => expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![base, index], at),
                None => base,
            };
            if fit.constant != BigInt::from(0) {
                let offset = counting::constant(context, &fit.constant, width);
                value = expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![value, offset], at);
            }
            value
        }
        (None, None) => {
            let invariant = fit.rest.plus(&constant);
            let scaled = scaled(context, function, products);
            let value = match (scaled, invariant.is_zero()) {
                (Some(scaled), true) => scaled,
                (None, _) => expander.int(context, function, &invariant),
                (Some(scaled), false) => {
                    let invariant = if invariant.terms.is_empty() { counting::constant(context, &constant.constant, width) } else { expander.int(context, function, &invariant) };
                    expand::placed(context, function, Opcode::Binary(BinaryOp::Add), int, vec![scaled, invariant], at)
                }
            };
            value
        }
    }
}

#[cfg(test)]
#[path = "lsr_tests.rs"]
mod tests;
