//! Trying a rewrite on a copy of the body: the copy is cleaned by the
//! pipeline's own passes and replaces the body only if it is better.
//!
//! # How it works
//! `adopted(unit, outer, costs, candidate, reoptimize, pricing)` runs
//! `reoptimize` over `candidate`, then compares it with the body in `unit`. It
//! is kept only when it has fewer loops, no more operations and no larger
//! price; a body with an operation the target does not price is never better.
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
//! None: a library. [`crate::unswitch`] judges a specialised loop by work
//! alone, [`crate::loopfuse`] a fused one with the spills.
//!
//! # Provides
//! `adopted`, `Pricing`.
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

/// What a body is priced by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pricing {
    /// The work: operations weighted by how often they run.
    Work,
    /// The work and what the target's registers spill: a rewrite that holds
    /// more values live at once pays for them.
    WithSpills,
}

/// `candidate`, made over `unit`'s body, cleaned by `reoptimize` and put in
/// its place when it has fewer loops, no more operations and no larger price;
/// whether it was. `outer` is what the prices' analyses read of the module.
pub fn adopted(
    unit: &mut Unit,
    outer: &Outer,
    costs: &OperationCosts,
    mut candidate: Function,
    reoptimize: &mut dyn FnMut(&mut Unit),
    pricing: Pricing,
) -> bool {
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
    let count = |state: &Function| cfg::Shape::of(state).loops.len();
    let price = |state: &Function| {
        let within = memory::Unit::within(unit.context, unit.layout, state, outer);
        // A body of its own, which no manager has seen.
        let registers = llrm_analysis::consts::known(&within);
        let shape = cfg::Shape::of(state);
        let within = within.with_registers(&registers).with_shape(&shape);
        let trips = profit::proven_trips(&within, &registers);
        let frequency = profit::_frequencies(unit.context, unit.metadata, &outer.globals, state, Some(&trips))?;
        match pricing {
            Pricing::Work => profit::weighted(unit.context, unit.layout, state, outer.callees(), costs, &frequency),
            Pricing::WithSpills => profit::motion_price(
                unit.context,
                unit.layout,
                outer,
                state,
                costs,
                profit::registers(outer),
                &frequency,
            ),
        }
    };
    let worse = match (price(unit.function), price(&candidate)) {
        (Some(before), Some(after)) => after > before,
        _ => true,
    };
    if count(&candidate) >= count(unit.function) || size(&candidate) > size(unit.function) || worse {
        return false;
    }
    *unit.function = candidate;
    true
}
