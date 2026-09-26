//! A value affine in a loop counter, written as the counter scaled plus one
//! invariant base: `counter * by + base`.
//!
//! `((x - k) + 640) * 2` becomes `x * 2 + base`, with `base` built from the
//! invariant terms. Every such value of one counter and scale then shares
//! `x * 2`, and hoist moves each `base` out of the loop. `induction` owns
//! the affine form; this only spells it.

use std::collections::BTreeMap;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::analysis::induction::{self, Derived};
use crate::analysis::loops::Loop;
use crate::analysis::occurrence::{OpOccurrence, operations};
use crate::analysis::regions::RegionLayout;
use crate::analysis::ssa;
use crate::model::mir::{Arg, Held, Kind, MirBody, Op, Value};
use crate::optimize::strength::{_made, starts_through};
use crate::optimize::transform;

pub(crate) struct Affine {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Affine {
    fn class_name(&self) -> &'static str {
        "Affine"
    }

    fn name(&self) -> &str {
        "affine"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        let layout = self.r#where.bounds.as_ref().map(|bounds| RegionLayout {
            shared_segments: None,
            landmarks: bounds.iter().map(|(key, marks)| (*key, marks.clone())).collect(),
        });
        canonical(&body, layout.as_ref())
    }
}

/// `body` with each loop-affine value built from its scaled counter and one
/// invariant base, where the loop lets that base move out.
pub(crate) fn canonical(
    body: &Rc<MirBody>,
    layout: Option<&RegionLayout>,
) -> Result<Rc<MirBody>, String> {
    let found = induction::of(body, layout).map_err(|error| format!("{error:?}"))?;
    let op_at = |at: OpOccurrence| &body.blocks[at.block_index()].ops[at.operation_index()];
    // A value is affine in every loop around it; its innermost loop is the
    // one whose trips it varies with.
    let mut innermost = BTreeMap::<OpOccurrence, (&Loop, &Derived)>::new();
    for (loop_, _, derived) in &found {
        for one in derived {
            let keep = innermost.get(&one.op).is_some_and(|(other, _)| other.body.len() <= loop_.body.len());
            if !keep {
                innermost.insert(one.op, (loop_, one));
            }
        }
    }
    let mut next_id = ssa::values(body).map(|one| one.id).max().unwrap_or(0) + 1;
    let mut next_variable = ssa::values(body).map(|one| one.variable).max().unwrap_or(0) + 1;
    let mut fresh = |at: i64| {
        let value = Value { id: next_id, at, flags: false, variable: next_variable, version: 1 };
        next_id += 1;
        next_variable += 1;
        value
    };
    let mut replacements = BTreeMap::<OpOccurrence, Vec<Op>>::new();
    for (at, (loop_, one)) in innermost {
        let op = op_at(at);
        let Some((counter, answer)) = _rewritable(body, loop_, one, op) else {
            continue;
        };
        let scaled = match &one.by {
            Arg::Const(by) if by.n == BigInt::from(1_u8) => counter,
            by => {
                let into = fresh(op.at);
                let mul = _made(Kind::Mul, "imul", into, vec![Arg::Held(counter), by.clone()], op.at, op);
                replacements.entry(at).or_default().push(mul);
                Held { value: into, width: answer.width }
            }
        };
        let base = fresh(op.at);
        let starts = starts_through(body, base, one, op.at, false, &mut || fresh(op.at));
        let sum = _made(
            Kind::Add,
            "add",
            answer.value,
            vec![Arg::Held(scaled), Arg::Held(Held { value: base, width: answer.width })],
            op.at,
            op,
        );
        let ops = replacements.entry(at).or_default();
        ops.extend(starts);
        ops.push(sum);
    }
    if replacements.is_empty() {
        return Ok(body.clone());
    }
    let mut changed = MirBody::clone(body);
    for block in &mut changed.blocks {
        block.ops.clear();
    }
    for (at, _, op) in operations(body) {
        let ops = replacements.remove(&at).unwrap_or_else(|| vec![op.clone()]);
        changed.blocks[at.block_index()].ops.extend(ops);
    }
    Ok(Rc::new(changed))
}

/// The counter and result `op` is rewritten through, when its affine form
/// has an invariant held term, the loop lets that term's base move out, and
/// `op` is not already `counter * by + base`.
///
/// An op whose flags are read (an overflow check) defines them and is left
/// alone; the ops it was built from stay, and so do their checks.
fn _rewritable(body: &MirBody, loop_: &Loop, one: &Derived, op: &Op) -> Option<(Held, Held)> {
    let [Arg::Held(answer)] = op.results.as_slice() else {
        return None;
    };
    if one.pointer.is_some()
        || op.defines != [answer.value]
        || one.of.start.width() != answer.width
        || !one.offsets.iter().all(|(offset, _)| matches!(offset, Arg::Held(_) | Arg::Const(_)))
        || !one.offsets.iter().any(|(offset, _)| matches!(offset, Arg::Held(_)))
    {
        return None;
    }
    let inside = body.blocks.iter().filter(|block| loop_.body.contains(&block.at));
    if transform::motion_blocked(inside.flat_map(|block| &block.ops)) {
        return None;
    }
    let counter = body
        .blocks
        .iter()
        .find(|block| block.at == loop_.header)?
        .phis
        .iter()
        .find(|phi| phi.result.id == one.of.value)?
        .result;
    let counter = Held { value: counter, width: answer.width };
    // Already `scaled + base`: one held offset, added to the scaled counter.
    let spelled = op.kind == Kind::Add
        && one.offsets.len() == 1
        && one.offsets[0].1 == BigInt::from(1_u8)
        && op.args.contains(&one.offsets[0].0);
    (!spelled).then_some((counter, *answer))
}
