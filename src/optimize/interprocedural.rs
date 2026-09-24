//! The whole-module step a frontend runs once every body has reached its own
//! fixed point: inline, carry constants across direct calls, drop dead pure
//! calls and the tails of terminal ones, and send each changed body back
//! through its pipeline until no body changes.
//!
//! A frontend hands in each procedure's facts and its pipeline; nothing here
//! names a language or a machine.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::analysis::interprocedural as facts;
use crate::model::mir::{Const, Kind, MemRef, MirBody};
use crate::optimize::inline;
use crate::support::hash::IndexMap;

/// What the whole-module step needs to know about one procedure.
pub(crate) struct Procedure<'a> {
    pub name: &'a str,
    /// call site -> callee name
    pub calls: &'a IndexMap<i64, String>,
    /// Formal entry cells, in the order a call's pushes bind them, last first.
    pub parameters: &'a [MemRef],
    /// Scalar actuals per call site known before MIR, where the frontend has them.
    pub constants: &'a IndexMap<i64, Vec<Option<Const>>>,
    /// call site -> the ARG operations that feed it
    pub arguments: &'a IndexMap<i64, BTreeSet<i64>>,
}

/// What the step proved about the module.
pub(crate) struct Module {
    /// Private procedures that cannot return.
    pub noreturn: BTreeSet<String>,
    /// Procedures a root still calls, roots included.
    pub reachable: BTreeSet<String>,
}

/// Run the whole-module step over `bodies`, keyed by procedure name.
///
/// `reoptimised(index, body, stage)` runs procedure `index`'s pipeline again
/// on a body `stage` changed; `spliced` sees a body straight after inlining,
/// before that.  Only `private` procedures have every caller in `procedures`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn optimized<E: From<String>>(
    procedures: &[Procedure<'_>],
    bodies: &mut IndexMap<String, Rc<MirBody>>,
    private: &BTreeSet<String>,
    roots: &BTreeSet<String>,
    call_cost: i64,
    reoptimised: &mut dyn FnMut(usize, &Rc<MirBody>, &str) -> Result<Rc<MirBody>, E>,
    spliced: &mut dyn FnMut(usize, &str, &MirBody) -> Result<(), E>,
) -> Result<Module, E> {
    let named_calls =
        procedures.iter().map(|one| (one.name.to_owned(), one.calls.clone())).collect::<IndexMap<_, _>>();
    let parameters =
        procedures.iter().map(|one| (one.name.to_owned(), one.parameters.to_vec())).collect::<IndexMap<_, _>>();
    let call_arguments =
        procedures.iter().map(|one| (one.name.to_owned(), one.arguments.clone())).collect::<IndexMap<_, _>>();
    let named = |bodies: &IndexMap<String, Rc<MirBody>>| -> IndexMap<String, (Rc<MirBody>, IndexMap<i64, String>)> {
        procedures.iter().map(|one| (one.name.to_owned(), (bodies[one.name].clone(), one.calls.clone()))).collect()
    };
    fn borrowed(
        owned: &IndexMap<String, (Rc<MirBody>, IndexMap<i64, String>)>,
    ) -> IndexMap<String, (&MirBody, &IndexMap<i64, String>)> {
        owned.iter().map(|(name, (body, calls))| (name.clone(), (&**body, calls))).collect()
    }

    // Inline only after each independent body has reached its local fixed
    // point; the splice's result goes straight back through the pipeline.
    let pure = facts::pure_procedures(&borrowed(&named(bodies)));
    let mut inline_round = 0;
    loop {
        let counts = inline::call_counts(bodies, &named_calls);
        let available = inline::candidates(bodies, &parameters, &counts, private, &pure, call_cost);
        let mut changed = false;
        for (index, procedure) in procedures.iter().enumerate() {
            let before = bodies[procedure.name].clone();
            let constant = inline::constant_sites(
                bodies,
                &parameters,
                procedure.calls,
                procedure.constants,
                private,
                &pure,
                call_cost,
            );
            let after = inline::expanded(&before, procedure.calls, procedure.arguments, &available, Some(&constant))?;
            if Rc::ptr_eq(&after, &before) {
                continue;
            }
            let stage = format!("inline{inline_round}");
            spliced(index, &stage, &after)?;
            let optimised = reoptimised(index, &after, &format!("{stage}."))?;
            bodies.insert(procedure.name.to_owned(), optimised);
            changed = true;
            inline_round += 1;
        }
        if !changed {
            break;
        }
    }

    let mut propagated =
        procedures.iter().map(|one| (one.name.to_owned(), BTreeSet::new())).collect::<IndexMap<_, _>>();
    let mut return_round = 0;

    // Materialize every newly constant result, retaining seen calls.
    let propagate_constant_returns = |bodies: &mut IndexMap<String, Rc<MirBody>>,
                                          propagated: &mut IndexMap<String, BTreeSet<i64>>,
                                          return_round: &mut i64,
                                          reoptimised: &mut dyn FnMut(usize, &Rc<MirBody>, &str) -> Result<Rc<MirBody>, E>|
     -> Result<(), E> {
        loop {
            let returns = facts::constant_returns(bodies);
            let mut changed = false;
            for (index, procedure) in procedures.iter().enumerate() {
                let before = bodies[procedure.name].clone();
                let (after, done) =
                    facts::propagate_returns(&before, procedure.calls, &returns, &propagated[procedure.name])?;
                propagated.insert(procedure.name.to_owned(), done);
                if Rc::ptr_eq(&after, &before) {
                    continue;
                }
                let optimised = reoptimised(index, &after, &format!("ipa{return_round}."))?;
                bodies.insert(procedure.name.to_owned(), optimised);
                changed = true;
            }
            if !changed {
                return Ok(());
            }
            *return_round += 1;
        }
    };

    // A return fact may make the actual of a different direct call
    // constant.  Alternate that current-MIR proof with return propagation
    // until neither side discovers a new fact.
    propagate_constant_returns(bodies, &mut propagated, &mut return_round, reoptimised)?;
    let mut argument_round = 0;
    loop {
        let constants =
            facts::current_parameter_constants(bodies, &named_calls, &call_arguments, &parameters, private);
        let mut changed = false;
        for (index, procedure) in procedures.iter().enumerate() {
            let Some(constants_for_body) = constants.get(procedure.name) else {
                continue;
            };
            let before = bodies[procedure.name].clone();
            let after = facts::specialize_parameters(&before, procedure.parameters, constants_for_body);
            if Rc::ptr_eq(&after, &before) {
                continue;
            }
            let optimised = reoptimised(index, &after, &format!("ipa-args{argument_round}."))?;
            bodies.insert(procedure.name.to_owned(), optimised);
            changed = true;
        }
        if changed {
            argument_round += 1;
            propagate_constant_returns(bodies, &mut propagated, &mut return_round, reoptimised)?;
        }

        // A single current-MIR constant may be worth cloning even where
        // another caller keeps the private body dynamic.
        let counts = inline::call_counts(bodies, &named_calls);
        let available = inline::candidates(bodies, &parameters, &counts, private, &pure, call_cost);
        let mut inlined = false;
        for (index, procedure) in procedures.iter().enumerate() {
            let before = bodies[procedure.name].clone();
            let current = facts::current_call_constants(&before, procedure.calls, procedure.arguments, &parameters);
            let constant =
                inline::constant_sites(bodies, &parameters, procedure.calls, &current, private, &pure, call_cost);
            let after = inline::expanded(&before, procedure.calls, procedure.arguments, &available, Some(&constant))?;
            if Rc::ptr_eq(&after, &before) {
                continue;
            }
            let optimised = reoptimised(index, &after, &format!("ipa-inline{argument_round}."))?;
            bodies.insert(procedure.name.to_owned(), optimised);
            inlined = true;
        }
        if inlined {
            propagate_constant_returns(bodies, &mut propagated, &mut return_round, reoptimised)?;
        }
        if !changed && !inlined {
            break;
        }
    }
    let readonly = facts::readonly_procedures(&borrowed(&named(bodies)));
    for (index, procedure) in procedures.iter().enumerate() {
        let before = bodies[procedure.name].clone();
        let after = facts::remove_dead_pure_calls(&before, procedure.calls, &readonly, procedure.arguments)?;
        if !Rc::ptr_eq(&after, &before) {
            let optimised = reoptimised(index, &after, "ipa-pure.")?;
            bodies.insert(procedure.name.to_owned(), optimised);
        }
    }
    // A direct private body whose every path stops makes the tail of every
    // call site unreachable: keep the physical call, remove only the code
    // that would require it to return, and repeat.
    let noreturn = loop {
        let noreturn = facts::noreturn_procedures(&borrowed(&named(bodies)), private);
        let mut changed = false;
        for (index, procedure) in procedures.iter().enumerate() {
            let before = bodies[procedure.name].clone();
            let after = facts::terminal_calls(&before, procedure.calls, &noreturn);
            if Rc::ptr_eq(&after, &before) {
                continue;
            }
            let optimised = reoptimised(index, &after, "ipa-noreturn.")?;
            bodies.insert(procedure.name.to_owned(), optimised);
            changed = true;
        }
        if !changed {
            break noreturn;
        }
    };
    Ok(Module { noreturn, reachable: reachable(procedures, bodies, roots) })
}

/// Procedures a surviving direct call reaches from `roots`; all of them
/// when there are no roots.
fn reachable(
    procedures: &[Procedure<'_>],
    bodies: &IndexMap<String, Rc<MirBody>>,
    roots: &BTreeSet<String>,
) -> BTreeSet<String> {
    let defined = procedures.iter().map(|one| one.name.to_owned()).collect::<BTreeSet<_>>();
    if roots.is_empty() {
        return defined;
    }
    let by_name = procedures.iter().map(|one| (one.name, one)).collect::<IndexMap<_, _>>();
    let mut reached = BTreeSet::new();
    let mut pending = roots.intersection(&defined).cloned().collect::<Vec<_>>();
    while let Some(name) = pending.pop() {
        if reached.contains(&name) {
            continue;
        }
        reached.insert(name.clone());
        let procedure = by_name[name.as_str()];
        let sites = bodies[&name]
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| op.kind == Kind::Call)
            .map(|op| op.at)
            .collect::<BTreeSet<_>>();
        pending.extend(
            procedure
                .calls
                .iter()
                .filter(|(at, target)| sites.contains(at) && defined.contains(*target) && !reached.contains(*target))
                .map(|(_, target)| target.clone()),
        );
    }
    reached
}
