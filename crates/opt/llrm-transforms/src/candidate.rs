//! Trying a rewrite on a copy of the body: the copy is cleaned by the
//! pipeline's own passes and replaces the body only if it is better.
//!
//! # How it works
//! `adopted(unit, outer, costs, candidate, reoptimize, pricing, gain)` runs
//! `reoptimize` over `candidate`, then compares it with the body in `unit`. By
//! `Gain::FewerLoops` it is kept only when it has fewer loops, no more
//! operations and no larger price; by `Gain::CheaperBy(percent)` when its price
//! is smaller by that percent. A body with an operation the target does not
//! price is never better.
//! The price is [`crate::profit::weighted`] (`Pricing::Work`) or
//! [`crate::profit::motion_price`], which adds what the registers spill
//! (`Pricing::WithSpills`), each block weighted by the trips `induction`
//! proves, as [`crate::profit::_frequencies`] has them.
//!
//! ```text
//!   body ---- rewrite ----> candidate ---- reoptimize ----> candidate'
//!     |                                                        |
//!     +----------- fewer loops, no more ops, price <= -------- +  keep candidate'
//! ```
//!
//! # Example
//! None: a library. [`crate::loopversion`] judges a specialised loop by work
//! alone.
//!
//! # Provides
//! `adopted`, `priced`, `priced_at`, `frequencies`, `Pricing`, `Gain`,
//! `Baseline`.
//!
//! # Needs
//! [`crate::profit`] and the target's operation costs.
//!
//! # See also
//! LLVM's `SimpleLoopUnswitch` cost check (`computeDomSubtreeCost`) and
//! gcc's `loop_unswitch` size limits are bounds before the copy; this is the
//! judgement after it.

use llrm_analysis::{cfg, memory, occurrence};
use llrm_mir::module::Function;
use llrm_mir::passes::{Outer, Unit};

use crate::profit::{self, OperationCosts};

/// What makes a candidate better than the body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gain {
    /// Fewer loops, no more operations and no larger price: a rewrite that
    /// joins or removes loops.
    FewerLoops,
    /// A price smaller by this percent, whatever the loops and operations: a
    /// rewrite that makes more of them, kept only for what it saves, and
    /// that by more than the price's own error.
    CheaperBy(i64),
}

/// What the candidate is judged against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Baseline {
    /// The body as it is.
    Kept,
    /// The body put through what the candidate is: where the candidate is
    /// judged after passes the body has not yet had, the body is too.
    Reoptimized,
}

/// What a body is priced by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pricing {
    /// The work: operations weighted by how often they run.
    Work,
    /// The work and what the target's registers spill: a rewrite that holds
    /// more values live at once pays for them.
    WithSpills,
}

/// How often each block of `state` runs per entry, as `adopted` prices it: the
/// trips `induction` proves in place of the conventional ten.
pub fn frequencies(
    unit: &Unit,
    outer: &Outer,
    state: &Function,
) -> Option<std::collections::BTreeMap<i64, i64>> {
    let within = memory::Unit::within(unit.context, unit.layout, state, outer);
    // A body of its own, which no manager has seen.
    let registers = llrm_analysis::consts::known(&within);
    let shape = cfg::Shape::of(state);
    let within = within.with_registers(&registers).with_shape(&shape);
    let trips = profit::proven_trips(&within);
    // The nodes the passes made on the copy are numbered after the module's.
    let metadata = [unit.metadata, unit.declared.nodes()].concat();
    profit::_frequencies(unit.context, &metadata, &outer.globals, state, Some(&trips))
}

/// What `state` costs at `frequency`, as `adopted` judges it: a body with an
/// operation the target does not price has none.
pub fn priced_at(
    unit: &Unit,
    outer: &Outer,
    costs: &OperationCosts,
    state: &Function,
    pricing: Pricing,
    frequency: &std::collections::BTreeMap<i64, i64>,
) -> Option<i64> {
    match pricing {
        Pricing::Work => profit::weighted(unit.context, unit.layout, state, outer.callees(), costs, frequency),
        Pricing::WithSpills => {
            profit::motion_price(unit.context, unit.layout, outer, state, costs, profit::registers(outer), frequency)
        }
    }
}

/// What `state` costs, as `adopted` judges it.
pub fn priced(
    unit: &Unit,
    outer: &Outer,
    costs: &OperationCosts,
    state: &Function,
    pricing: Pricing,
) -> Option<i64> {
    priced_at(unit, outer, costs, state, pricing, &frequencies(unit, outer, state)?)
}

/// `candidate`, made over `unit`'s body, cleaned by `reoptimize` and put in
/// its place when it is better by `gain`; whether it was. `outer` is what the
/// prices' analyses read of the module.
pub fn adopted(
    unit: &mut Unit,
    outer: &Outer,
    costs: &OperationCosts,
    mut candidate: Function,
    reoptimize: &mut dyn FnMut(&mut Unit),
    pricing: Pricing,
    gain: Gain,
    baseline: Baseline,
) -> bool {
    let mut kept = match baseline {
        Baseline::Kept => None,
        Baseline::Reoptimized => Some(unit.function.clone()),
    };
    if let Some(body) = kept.as_mut() {
        reoptimize(&mut Unit {
            context: &mut *unit.context,
            layout: unit.layout,
            function: body,
            id: None,
            metadata: unit.metadata,
            declared: &mut *unit.declared,
        });
    }
    reoptimize(&mut Unit {
        context: &mut *unit.context,
        layout: unit.layout,
        function: &mut candidate,
        // A copy: its calls to the original are not calls to itself.
        id: None,
        metadata: unit.metadata,
        declared: &mut *unit.declared,
    });

    let size = |state: &Function| occurrence::operations(state).count();
    let count = |state: &Function| cfg::loops_of(state).len();
    let price = |state: &Function| priced(unit, outer, costs, state, pricing);
    let body = kept.as_ref().unwrap_or(&*unit.function);
    let prices = (price(body), price(&candidate));
    llrm_support::debug!(
        "candidate",
        "price {:?} -> {:?}, loops {} -> {}",
        prices.0,
        prices.1,
        count(body),
        count(&candidate)
    );
    let worse = |kept: &dyn Fn(i64, i64) -> bool| match prices {
        (Some(before), Some(after)) => !kept(after, before),
        _ => true,
    };
    let refused = match gain {
        Gain::FewerLoops => {
            count(&candidate) >= count(body) || size(&candidate) > size(body) || worse(&|after, before| after <= before)
        }
        Gain::CheaperBy(percent) => worse(&|after, before| after * 100 < before * (100 - percent)),
    };
    if refused {
        return false;
    }
    *unit.function = candidate;
    true
}
