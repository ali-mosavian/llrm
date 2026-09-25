//! What a counted-loop rewrite places before the loop: its seeds, skip test and exit values.
//!
//! Port of `qbopt/optimize/counting.py`. Built only from
//! `induction::CountedLoop`, so every pass that replaces a loop's counter
//! enters and leaves the loop the same way.

use std::collections::BTreeMap;

use crate::analysis::induction::{self, AffineOperand, ControlReplacement, CountedLoop};
use crate::analysis::occurrence::PhiOccurrence;
use crate::analysis::consts;
use crate::model::mir::{self, Arg, Const, Held, Kind, MirBody, Op, OrderedMap, Phi, Value};

/// Preheader values, placed after the symbolic proof is complete.
///
/// Arithmetic on constants is its constant, so a rewrite whose count and
/// start are known places nothing; `canonical.identities` folds the neutral
/// terms of the rest.
pub(crate) struct Seeds {
    pub serial: u32,
    pub variable: u32,
    pub at: i64,
    pub width: u32,
    pub ops: Vec<Op>,
}

impl Seeds {
    pub(crate) fn computed(&mut self, kind: Kind, args: Vec<Arg>) -> AffineOperand {
        let arith = consts::ARITH.iter().find(|(one, _)| *one == kind).map(|(_, arith)| *arith);
        if let (Some(arith), [Arg::Const(left), Arg::Const(right)]) = (arith, args.as_slice()) {
            return AffineOperand::Const(Const::new(consts::masked(&arith(&left.n, &right.n), self.width), self.width));
        }
        let value = self.fresh(self.at);
        self.ops.push(mir::computed(self.at, kind, value, args, self.width));
        AffineOperand::Held(Held { value, width: self.width })
    }

    /// A value no other names, defined at `at`.
    pub(crate) fn fresh(&mut self, at: i64) -> Value {
        let value = Value { id: self.serial, at, flags: false, variable: self.variable, version: 0 };
        self.serial += 1;
        self.variable += 1;
        value
    }

    /// `arg` as a value, for a phi to name.
    pub(crate) fn held(&mut self, arg: AffineOperand) -> Held {
        if let AffineOperand::Held(held) = arg {
            return held;
        }
        let value = self.fresh(self.at);
        self.ops.push(mir::computed(self.at, Kind::Copy, value, vec![arg.as_arg()], self.width));
        Held { value, width: self.width }
    }
}

/// The preheader compare and branch that leave a counted loop before its first trip.
///
/// `body` is the snapshot `proof`'s occurrences index; Python holds the ops.
pub(crate) fn skip_guard(body: &MirBody, proof: &CountedLoop, at: i64, flags: Value) -> (Op, Op) {
    let ((left, right), test) = induction::skipped(proof).expect("a pre-tested proof");
    let args = vec![left.as_arg(), right.as_arg()];
    // A subtract whatever the loop's own test was: `or i,i` names one operand.
    let compare = Op {
        kind: Kind::Sub,
        name: String::new(),
        results: Vec::new(),
        merges: OrderedMap::default(),
        at,
        defines: vec![flags],
        uses: args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Held(held) => Some(held.value),
                _ => None,
            })
            .collect(),
        source_backed: false,
        args,
        raised: None,
        absorbed: Vec::new(),
        symbol: Some(false),
        ..body.blocks[proof.compare.block_index()].ops[proof.compare.operation_index()].clone()
    };
    let branch = Op {
        at,
        name: String::new(),
        defines: Vec::new(),
        uses: vec![flags],
        source_backed: false,
        test: Some(test),
        target: Some(proof.exit),
        raised: None,
        absorbed: Vec::new(),
        symbol: Some(false),
        ..body.blocks[proof.branch.block_index()].ops[proof.branch.operation_index()].clone()
    };
    (compare, branch)
}

/// Exit phis of a replaced counter, reading its exit value after a trip and,
/// where a guard may skip the loop, its start after none.
///
/// Keyed by the phi's occurrence in `body`, Python's `id(phi)`.
pub(crate) fn leaving(
    body: &MirBody,
    replacement: &ControlReplacement<'_>,
    seeds: &mut Seeds,
    guarded: bool,
) -> BTreeMap<PhiOccurrence, Phi> {
    let proof = replacement.counted;
    if replacement.exits.is_empty() {
        return BTreeMap::new();
    }
    let exit = induction::exit_value(proof, &mut |kind, args| seeds.computed(kind, args));
    let value = seeds.held(exit.expect("a pre-tested proof"));
    let phi_at = |at: PhiOccurrence| &body.blocks[at.block_index()].phis[at.phi_index()];
    let preheader = proof.preheader.expect("control_replacement proved a preheader");
    let start = *phi_at(proof.phi).incoming.get(&preheader).expect("KeyError");
    replacement
        .exits
        .iter()
        .map(|&at| {
            let phi = phi_at(at);
            let mut incoming = phi.incoming.keys().map(|&key| (key, value.value)).collect::<OrderedMap<_, _>>();
            if guarded {
                incoming.insert(preheader, start);
            }
            (at, Phi { incoming, ..phi.clone() })
        })
        .collect()
}
