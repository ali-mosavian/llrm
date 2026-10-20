//! Machine-neutral profitability shared by the MIR transforms: what an
//! instruction, a block and a function cost under a target's prices, how often
//! each block runs, and what registers cost past the target's.
//!
//! # How it works
//! Everything here prices semantic work: opcodes, memory effects and CFG
//! frequency. A kind of instruction without a price makes the answer `None`,
//! unknown rather than cheap, and a transform that copies code leaves a
//! function it cannot price alone (`priced`).
//!
//! The machine's facts come in as parameters: each operation's price
//! (`OperationCosts`, from the target by `costs`), how many integer values fit
//! in registers and across a call (`registers`, `Room`), and that floating
//! values do not take that room.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   op["<b>operation(inst)</b><br/><small>one instruction's price#59;<br/>gep or call by its kind,<br/>pointer advance by #96;advance#96;<br/>(carries)</small>"]:::mid
//!   blk["<b>_block / static</b><br/><small>sum over a block / over<br/>the body, once</small>"]:::mid
//!   w["<b>weighted(freq)</b><br/><small>sum of block price *<br/>block frequency</small>"]:::mid
//!   sf["<b>spill_forecast</b><br/><small>what #96;spill#96; says does not<br/>fit in registers</small>"]:::mid
//!   pa["<b>pressure_adjusted</b><br/><small>== motion_price<br/>(the price a motion is<br/>judged by)</small>"]:::opt
//!   op --> blk
//!   blk --> w
//!   w -->|"+"| pa
//!   sf -->|"+"| pa
//! ```
//!
//! *How the price a motion is judged by is built*
//!
//! Frequencies come in two models, both per entry:
//! - `_frequencies`: `branchprob`'s estimate in `UNIT`ths of an execution
//!   (never below one, so a cold arm is near free but ordered), with each
//!   loop's proven trips (`proven_trips`, from ScalarEvolution's
//!   `getSmallConstantTripCount`) in place of its estimate;
//! - `_loop_products`: the trips of each loop around a block multiplied, with
//!   `UNKNOWN_TRIPS` (ten) for a loop ScalarEvolution does not count. `lsr` and
//!   `gvn` were tuned on it (#202, #203). `_loop_products_by_branch` weighs
//!   each block by the share of its loop's trips that reach it.
//!
//! `site` is `peelsize`'s view of a loop: how often it is entered and
//! whether it calls what may touch memory, from frequencies worked out once
//! per version of the function (`Frequencies`).
//!
//! Prices of what has no instruction: a `llvm.memset` call is a fill, its
//! bytes the cells, `fill + cells * fill_cell`; a `select` is a branch and a
//! move, and has no cheaper form here; `frem`, `landingpad` and other kinds
//! the target does not price are `None`. A load or store touches memory, so no
//! operation carries a folded memory operand.
//!
//! # Example
//! ```mir
//! define double @f(double %x) {
//! b0:
//!   %y = frem double %x, 3.000000e+00
//!   ret double %y
//! }
//! ```
//! Facts:
//! ```text
//! profit::static(@f, OperationCosts::default()) = None
//! ```
//! The body has an instruction no price covers, so the answer is `None`, not a
//! small number.
//!
//! From unit test `an_unpriced_instruction_makes_the_body_unpriced` in
//! `profit_tests.rs`.
//!
//! # Provides
//! `costs`, `registers`, `operation`, `static`, `priced`, `weighted`,
//! `spill_forecast`, `pressure_adjusted`, `motion_price`, `advance`, `site`,
//! `proven_trips`, the frequency models and `UNIT`, `UNKNOWN_TRIPS`.
//!
//! # Used by
//! Each of [`crate::lsr_pass`], [`crate::hoist`], [`crate::gvn`],
//! [`crate::loopunrollpass`], [`crate::looppeel`],
//! [`crate::loopidioms`], [`crate::window`], [`crate::gepoffset`],
//! [`crate::indvars`], [`crate::ipacp`], [`crate::inline`]
//! and [`crate::interprocedural`] asks it, before a rewrite that trades code
//! size or registers for speed, whether the rewrite pays. Not a pass: it runs
//! only inside those.
//!
//! # Needs
//! The target's `OperationCosts` and registers, [`crate::spill`] for the
//! forecast, `branchprob` and `induction` for the frequencies.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::effects::Declarations;
use llrm_analysis::memory;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_mir::blockfrequency::frequency::BlockFrequency;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::Outer;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexMap;

use crate::spill::{self, Room};

// Trips assumed of a loop, and cells of a fill, whose count is not a number.
pub const UNKNOWN_TRIPS: i64 = 10;

pub use llrm_mir::target::OperationCosts;

/// What `outer`'s target prices each operation at.
pub fn costs(outer: &Outer) -> OperationCosts {
    outer.target().costs()
}

/// How many integer values `outer`'s target holds in registers, and how
/// many across a call.
pub fn registers(outer: &Outer) -> Room {
    Room::of(outer)
}

fn floating(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> bool {
    function.operand_type(context, operand).is_some_and(|ty| matches!(context.types.get(ty), Type::Float(_)))
}

/// What advancing a pointer of type `ty` costs: `plain`, the price of the
/// advance as its caller makes it, or the carry into the selector where its
/// space's displacement has one (`DataLayout::carries`), less by a
/// `constant` displacement. The one place the carry's price is stated.
pub fn advance(
    context: &Context,
    layout: &DataLayout,
    ty: TypeId,
    plain: i64,
    constant: bool,
    costs: &OperationCosts,
) -> i64 {
    match context.types.get(ty) {
        Type::Pointer(space) if layout.carries(*space) && constant => costs.carry_step,
        Type::Pointer(space) if layout.carries(*space) => costs.carry,
        _ => plain,
    }
}

/// Whether the one reader of `value` is a floating operation in `block` that
/// takes its operand from memory, so that a load of it is that operation's own
/// memory form and not an instruction of its own. A reader in another block
/// reads a register the load made.
fn folds_into_users(
    function: &Function,
    value: ValueId,
    block: BlockId,
) -> bool {
    let users = function.users(value);
    // One reader: a load read twice is held in a register, not read twice.
    users.has_one_use()
        && users.iter().all(|user| {
            let one = function.instruction(user.user);
            function.parent(user.user) == Some(block)
                && match one.opcode {
                    Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv)
                    | Opcode::FCmp(_) => true,
                    // Widening a float loaded is the load's own form (`fld
                    // m32`).
                    Opcode::Cast(CastOp::FPExt) => {
                        one.result.is_some_and(|wide| folds_into_users(function, wide, block))
                    }
                    _ => false,
                }
        })
}

/// Target price for semantic work, or None when it cannot be priced.
pub fn operation(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    one: InstId,
    costs: &OperationCosts,
) -> Option<i64> {
    let instruction = function.instruction(one);
    let price = match &instruction.opcode {
        Opcode::Load { .. }
            if instruction.result.is_some_and(|value| floating(context, function, Operand::Value(value))) =>
        {
            let home = function.parent(one);
            if costs.folds_loads
                && instruction
                    .result
                    .is_some_and(|value| home.is_some_and(|home| folds_into_users(function, value, home)))
            {
                costs.float_load_folded
            } else {
                costs.float_load
            }
        }
        Opcode::Load { .. } => costs.load,
        Opcode::Store { .. } if floating(context, function, instruction.operands[0]) => costs.float_store,
        Opcode::Store { .. } => costs.store,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor)
        | Opcode::ICmp(_) => costs.add,
        Opcode::Cast(_) | Opcode::ExtractValue(_) | Opcode::InsertValue(_) | Opcode::Phi | Opcode::Freeze => {
            costs.r#move
        }
        Opcode::Binary(BinaryOp::Mul) => costs.multiply,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => costs.divide,
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => costs.shift,
        Opcode::GetElementPtr { .. } => {
            let constant = instruction.operands[1..].iter().all(|&index| matches!(index, Operand::Constant(_)));
            advance(context, layout, function.value(instruction.result?).ty, costs.address, constant, costs)
        }
        Opcode::Alloca { .. } => costs.address,
        // Lowered as a jump around a move, the compare priced on its own.
        Opcode::Select => costs.branch + costs.r#move,
        Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub) | Opcode::FNeg | Opcode::FCmp(_) => costs.float_add,
        Opcode::Binary(BinaryOp::FMul) => costs.float_multiply,
        Opcode::Binary(BinaryOp::FDiv) => costs.float_divide,
        Opcode::Call(_) | Opcode::Invoke(_)
            if callee(context, function, one).and_then(|id| callees.get(&id)).is_some_and(|summary| summary.memset) =>
        {
            let cells = llrm_mir::intrinsics::length(context, instruction.operands[2])
                .and_then(|count| i64::try_from(count).ok())
                .unwrap_or(UNKNOWN_TRIPS);
            return Some(costs.fill + cells * costs.fill_cell);
        }
        Opcode::Call(_) | Opcode::Invoke(_) => costs.call,
        Opcode::Ret | Opcode::Resume | Opcode::Unreachable => costs.return_,
        Opcode::Br => costs.branch,
        // Lowered as a compare and a jump for each case, then the jump for the
        // rest.
        Opcode::Switch => costs.branch + (instruction.operands.len() as i64 - 2) / 2 * (costs.add + costs.branch),
        _ => return None,
    };
    Some(price)
}

pub fn _block(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    block: i64,
    costs: &OperationCosts,
) -> Option<i64> {
    function.iter_block(cfg::block(block)).map(|one| operation(context, layout, function, callees, one, costs)).sum()
}

/// Semantic work present once in the body, independent of frequency.
pub fn r#static(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
) -> Option<i64> {
    function.layout().iter().map(|&block| _block(context, layout, function, callees, cfg::id(block), costs)).sum()
}

/// Where `loop_` stands in the unit's function: how often it is entered for
/// each entry of the function, in `UNIT`ths (what its outside predecessors
/// weigh), and whether it calls a function that may touch memory.
///
/// The frequencies are those of the whole function: `frequencies` holds them
/// for as many loops as ask, which a caller makes once per version of the
/// function.
pub fn site(
    unit: &memory::Unit,
    outer: &Outer,
    loop_: &llrm_analysis::graph::loops::Loop,
    frequencies: &Frequencies,
) -> llrm_analysis::peelsize::Site {
    let entries = match frequencies.0.get_or_init(|| {
        FREQUENCIES.with(|runs| runs.set(runs.get() + 1));
        let trips = proven_trips(unit);
        _frequencies(unit.context, unit.metadata, &outer.globals, unit.function, Some(&trips))
    }) {
        Some(frequency) => cfg::graph(unit.function)
            .iter()
            .filter(|block| !loop_.body.contains(&block.at) && block.succ.contains(&loop_.header))
            .map(|block| frequency.get(&block.at).copied().unwrap_or(UNIT))
            .sum::<i64>()
            .max(1),
        None => UNIT,
    };
    let callees = outer.callees();
    let writes = loop_
        .body
        .iter()
        .flat_map(|&at| {
            let block = cfg::block(at);
            unit.function.iter_block(block).map(move |inst| (block, inst))
        })
        .any(|(_, inst)| {
            unit.calls_out(inst)
                && !callee(unit.context, unit.function, inst)
                    .and_then(|id| callees.get(&id))
                    .is_some_and(|summary| summary.effects == llrm_mir::memory::Effects::NONE)
        });
    llrm_support::debug!(
        "peelsite",
        "callees of the loop at b{}: {:?}",
        loop_.header,
        unit.function
            .walk()
            .filter(|(block, _)| loop_.body.contains(&cfg::id(*block)))
            .filter(|(_, inst)| unit.calls_out(*inst))
            .map(|(_, inst)| callee(unit.context, unit.function, inst)
                .map(|id| (id, callees.get(&id).map(|one| one.effects))))
            .collect::<Vec<_>>()
    );
    llrm_analysis::peelsize::Site { entries, writes }
}

/// A function's block frequencies, worked out when the first loop asks (`site`)
/// and kept for the rest.
#[derive(Default)]
pub struct Frequencies(std::cell::OnceCell<Option<BTreeMap<i64, i64>>>);

thread_local! {
    static FREQUENCIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has worked out a function's frequencies for
/// `site`.
pub fn frequencies_worked() -> usize {
    FREQUENCIES.with(std::cell::Cell::get)
}

/// Whether the target prices every instruction here, which a copy's cost needs.
pub fn priced(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
) -> bool {
    r#static(context, layout, function, callees, costs).is_some()
}

/// Each loop's trips by latch (the runs of its latch), where ScalarEvolution
/// proves them (`getSmallConstantTripCount`): the `trips` that replaces the
/// conventional ten.
pub fn proven_trips(unit: &memory::Unit) -> IndexMap<i64, i64> {
    let mut trips = IndexMap::default();
    let loops = &unit.shape().loops;
    if loops.is_empty() {
        return trips;
    }
    let mut session = ScevSession::within(unit.context, unit.globals, unit.metadata, unit.function);
    for loop_ in loops {
        if let Some(count) = scev_trips(&mut session, unit, loop_) {
            trips.extend(loop_.latches.iter().map(|&latch| (latch, count)));
        }
    }
    trips
}

/// How often `loop_`'s latch runs, where ScalarEvolution proves it
/// (`getSmallConstantTripCount`): one exit, in the latch or in the header.
/// The count is the header's runs, one more than the latch's where the header
/// leaves.
pub fn scev_trips(
    session: &mut ScevSession,
    unit: &memory::Unit,
    loop_: &llrm_analysis::graph::loops::Loop,
) -> Option<i64> {
    let header = cfg::block(loop_.header);
    let count =
        session.ask_within(unit.context, unit.layout, unit.globals, unit.metadata, unit.function, |scev, cx| {
            let natural = cx.loops.get_loop_for(header).filter(|found| found.get_header() == header)?;
            let [exiting] = natural.get_exiting_blocks(cx.function)[..] else { return None };
            let runs = scev.get_small_constant_trip_count(cx, header);
            if loop_.latches.contains(&cfg::id(exiting)) {
                Some(runs)
            } else if exiting == header {
                Some(runs.saturating_sub(1))
            } else {
                None
            }
        })?;
    (count != 0).then_some(i64::from(count))
}

/// At most how often `loop_`'s latch runs, where ScalarEvolution proves a
/// bound (`getSmallConstantMaxTripCount`, the header's runs, which the
/// latch's never exceed): the loop may also leave by another exit.
pub fn scev_trip_bound(
    session: &mut ScevSession,
    unit: &memory::Unit,
    loop_: &llrm_analysis::graph::loops::Loop,
) -> Option<i64> {
    let header = cfg::block(loop_.header);
    let count =
        session.ask_within(unit.context, unit.layout, unit.globals, unit.metadata, unit.function, |scev, cx| {
            cx.loops.get_loop_for(header).filter(|found| found.get_header() == header)?;
            Some(scev.get_small_constant_max_trip_count(cx, header))
        })?;
    (count != 0).then_some(i64::from(count))
}

/// What one execution of the entry weighs in `_frequencies`: a block taken a
/// third of the time weighs a third of it.
pub const UNIT: i64 = 256;

/// Block frequencies as `branchprob` estimates them (the heuristics, a loop's
/// proven `trips` by latch), per entry, in `UNIT`ths of an execution and never
/// below one, so a cold arm is near free but still ordered. `None`
/// for conflicting proofs.
pub fn _frequencies(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    _frequencies_in(context, metadata, globals, function, &cfg::loops_of(function), trips)
}

/// `_frequencies`, `loops` being `function`'s.
pub fn _frequencies_in(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    _odds_in(context, metadata, globals, function, loops, trips).map(|(frequency, _)| frequency)
}

/// Each loop's proven trips by its header: none for loops whose latches
/// disagree.
pub fn counted_by_header(
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    let mut counted = BTreeMap::new();
    for loop_ in loops {
        let exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        if let Some(count) = exact.into_iter().next() {
            counted.insert(loop_.header, count);
        }
    }
    Some(counted)
}

/// `_frequencies_in`, and the odds of each edge they were worked out from:
/// `BranchProbabilityInfo`'s, a loop's proven trips setting its exit's
/// (`blockfreq::trip_adjusted`, profit's own: LLVM's probabilities know no
/// trip count), then `BlockFrequencyInfo` over them, each block's frequency in
/// `UNIT`s of the entry's. `None` for conflicting proofs.
fn _odds_in(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<(BTreeMap<i64, i64>, BTreeMap<(i64, i64), f64>)> {
    let counted = counted_by_header(loops, trips)?;
    let taken = _edge_odds(context, metadata, globals, function, loops, &counted);
    let frequencies = llrm_analysis::blockfreq::BlockFrequencies::of_probabilities(function, &taken);
    Some((
        function
            .layout()
            .iter()
            .map(|&block| {
                (cfg::id(block), ((frequencies.relative_to_entry(block) * UNIT as f64).round() as i64).max(1))
            })
            .collect(),
        taken,
    ))
}

/// Each edge's odds as `_odds_in` works the frequencies out from:
/// `BranchProbabilityInfo`'s, a proven loop's exit set by its trips
/// (`counted`, by header).
pub fn _edge_odds(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    counted: &BTreeMap<i64, i64>,
) -> BTreeMap<(i64, i64), f64> {
    // The layout only folds the integer constants of a loop's exit test
    // (`computeUnlikelySuccessors`).
    let layout = llrm_mir::datalayout::DataLayout::default();
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module { context, declarations: globals, metadata, layout: &layout },
        function,
    );
    let mut taken = BTreeMap::new();
    for &block in function.layout() {
        for to in function.successors(block) {
            taken.insert((cfg::id(block), cfg::id(to)), bpi.get_edge_probability_to(function, block, to).to_f64());
        }
    }
    llrm_analysis::blockfreq::trip_adjusted(function, loops, counted, &taken)
}

/// Block frequencies as a product of the trips of each loop around a block, ten
/// where none is proven: the model lsr's and gvn's prices were tuned on (#203,
/// #202), until they are retuned on `_frequencies`. `None` for conflicting
/// proofs.
pub fn _loop_products(
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    _loop_products_in(function, &cfg::loops_of(function), trips)
}

/// `_loop_products`, `loops` being `function`'s.
pub fn _loop_products_in(
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    let graph = cfg::graph(function);
    let mut frequency = graph.iter().map(|block| (block.at, 1_i64)).collect::<BTreeMap<_, _>>();
    let empty = IndexMap::default();
    let trips = trips.unwrap_or(&empty);
    for loop_ in loops {
        let exact = loop_.latches.iter().filter_map(|at| trips.get(at).copied()).collect::<BTreeSet<_>>();
        if exact.len() > 1 {
            return None;
        }
        let factor = exact.iter().next().copied().unwrap_or(UNKNOWN_TRIPS);
        for at in &loop_.body {
            if let Some(count) = frequency.get_mut(at) {
                *count = nested(*count, factor);
            }
        }
    }
    Some(frequency)
}

/// `_loop_products`, each block weighed by the share of its innermost loop's
/// trips that reach it, as `_frequencies` finds the share: the latch runs once
/// a trip, so a block behind a branch runs `odds[block] / odds[latch]` of them
/// and a block every trip passes through weighs the whole product, as before.
pub fn _loop_products_by_branch(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    _loop_products_by_branch_in(context, metadata, globals, function, &cfg::loops_of(function), trips)
}

/// `_loop_products_by_branch`, `loops` being `function`'s: asked of the
/// manager's, it is not derived three times over for each call.
pub fn _loop_products_by_branch_in(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<BTreeMap<i64, i64>> {
    _products_with_odds_in(context, metadata, globals, function, loops, trips).map(|(weight, _)| weight)
}

/// `_loop_products_by_branch_in`, and the odds of each edge it came from.
pub fn _products_with_odds_in(
    context: &Context,
    metadata: &[llrm_mir::module::MetadataNode],
    globals: &Declarations,
    function: &Function,
    loops: &[llrm_analysis::graph::loops::Loop],
    trips: Option<&IndexMap<i64, i64>>,
) -> Option<(BTreeMap<i64, i64>, BTreeMap<(i64, i64), f64>)> {
    let mut weight = _loop_products_in(function, loops, trips)?;
    let (odds, taken) = _odds_in(context, metadata, globals, function, loops, trips)?;
    for (at, count) in weight.iter_mut() {
        let Some(innermost) = loops.iter().filter(|one| one.body.contains(at)).min_by_key(|one| one.body.len()) else {
            continue;
        };
        let trip: i64 = innermost.latches.iter().map(|latch| odds.get(latch).copied().unwrap_or(UNIT)).sum();
        let here = odds.get(at).copied().unwrap_or(UNIT);
        if here < trip {
            *count = share_of(*count, here, trip);
        }
    }
    Some((weight, taken))
}

/// A block weight as the frequency type LLVM keeps it in: a saturating `u64`
/// ([`BlockFrequency`](llrm_mir::blockfrequency::frequency::BlockFrequency),
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/BlockFrequency.h#L27>).
/// DEVIATE: the maps keep `i64`, so a weight past `i64::MAX` is that.
fn frequency(count: i64) -> BlockFrequency {
    BlockFrequency(u64::try_from(count).unwrap_or(0))
}

fn weight(frequency: BlockFrequency) -> i64 {
    i64::try_from(frequency.get_frequency()).unwrap_or(i64::MAX)
}

/// `count` for a block one loop deeper, which runs `factor` trips: the product
/// of the trips of every loop around a block. Twenty nested loops of the
/// assumed ten trips pass `i64`'s end.
fn nested(
    count: i64,
    factor: i64,
) -> i64 {
    weight(frequency(count).saturating_mul(u64::try_from(factor).unwrap_or(0)))
}

/// `count` times the share `here / trip`, at least 1.
fn share_of(
    count: i64,
    here: i64,
    trip: i64,
) -> i64 {
    weight(frequency(count).scaled(u64::try_from(here).unwrap_or(0), u64::try_from(trip).unwrap_or(1))).max(1)
}

/// Profile-free expected work at `frequency`, a block's executions per entry
/// (`_frequencies`, or the older `_loop_products`).
pub fn weighted(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0 + 1, count.get().1)));
    let mut total: i64 = 0;
    for &block in function.layout() {
        let priced = _block(context, layout, function, callees, cfg::id(block), costs)?;
        total = total.saturating_add(frequency[&cfg::id(block)].saturating_mul(priced));
    }
    Some(total)
}

/// What fitting MIR within `room` spills, a call keeping what `across`
/// says, as the one spill model (`spill`) forecasts it.
pub fn spill_forecast(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<spill::Forecast<llrm_mir::dense::IdSet<ValueId>>> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0, count.get().1 + 1)));
    if !room.priced() {
        return Some(spill::Forecast { cost: 0, spilled: Default::default(), peak: 0 });
    }
    Some(spill::View::of(context, layout, function, room, across).forecast(costs, frequency))
}

/// `spill_forecast` of a function that `changes` made of the one `base` is of,
/// its liveness solved again for the values they name alone. `LLRM_CHECK_VIEW`
/// holds it to the forecast of the function on its own.
#[allow(clippy::too_many_arguments)]
pub fn spill_forecast_after(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    base: &std::rc::Rc<spill::Pressure>,
    changes: &[llrm_mir::module::Change],
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<spill::Forecast<llrm_mir::dense::IdSet<ValueId>>> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0, count.get().1 + 1)));
    if !room.priced() {
        return Some(spill::Forecast { cost: 0, spilled: Default::default(), peak: 0 });
    }
    let Some(pressure) =
        spill::Pressure::updated(std::rc::Rc::clone(base), changes, context, function, room.index_scales)
    else {
        return spill_forecast(context, layout, function, costs, room, across, frequency);
    };
    let found = spill::View::over(&pressure, context, layout, function, room, across).forecast(costs, frequency);
    if llrm_support::env_flag!("LLRM_CHECK_VIEW") {
        let whole = spill::View::of(context, layout, function, room, across).forecast(costs, frequency);
        let spilled =
            |forecast: &spill::Forecast<llrm_mir::dense::IdSet<ValueId>>| forecast.spilled.iter().collect::<Vec<_>>();
        assert!(
            found.cost == whole.cost && found.peak == whole.peak && spilled(&found) == spilled(&whole),
            "the forecast of a function from its base's liveness is not its own: cost {} vs {}, peak {} vs {}",
            found.cost,
            whole.cost,
            found.peak,
            whole.peak
        );
    }
    Some(found)
}

/// `spill_forecast` over the `pressure` of `function` itself.
pub fn spill_forecast_over(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    pressure: &spill::Pressure,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<spill::Forecast<llrm_mir::dense::IdSet<ValueId>>> {
    #[cfg(test)]
    PRICED.with(|count| count.set((count.get().0, count.get().1 + 1)));
    if !room.priced() {
        return Some(spill::Forecast { cost: 0, spilled: Default::default(), peak: 0 });
    }
    Some(spill::View::over(pressure, context, layout, function, room, across).forecast(costs, frequency))
}

/// Semantic work plus finite-capacity whole-range spill traffic.
#[allow(clippy::too_many_arguments)]
pub fn pressure_adjusted(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    callees: &Callees,
    costs: &OperationCosts,
    room: Room,
    across: &dyn Fn(InstId) -> i64,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    let work = weighted(context, layout, function, callees, costs, frequency)?;
    Some(work + spill_forecast(context, layout, function, costs, room, across, frequency)?.cost)
}

/// What `function` costs at `frequency` under `costs` and `room`: the one
/// price a motion is judged by, with the motion and without it.
pub fn motion_price(
    context: &Context,
    layout: &DataLayout,
    outer: &Outer,
    function: &Function,
    costs: &OperationCosts,
    room: Room,
    frequency: &BTreeMap<i64, i64>,
) -> Option<i64> {
    pressure_adjusted(
        context,
        layout,
        function,
        outer.callees(),
        costs,
        room,
        &|inst| spill::kept_across(outer, context, function, inst),
        frequency,
    )
}

#[cfg(test)]
thread_local! {
    /// (`weighted`, `spill_forecast`) calls, for a test that a price is the
    /// work and one forecast.
    pub(crate) static PRICED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

#[cfg(test)]
#[path = "profit_tests.rs"]
mod tests;
