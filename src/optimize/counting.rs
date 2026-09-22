//! What a counted-loop rewrite places before the loop: its seeds, skip test and exit values.
//!
//! Port of `qbopt/optimize/counting.py`. Built only from
//! `induction::CountedLoop`, so every pass that replaces a loop's counter
//! enters and leaves the loop the same way.

use std::collections::BTreeMap;

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use crate::analysis::consts::{Known, masked};
use crate::analysis::induction::{self, AffineOperand, ControlReplacement, CountedLoop};
use crate::analysis::occurrence::PhiOccurrence;
use crate::model::mir::{self, Arg, Const, Held, Kind, MirBody, Op, OrderedMap, Phi, Value};

/// Preheader values, constructed after the symbolic proof is complete.
///
/// An operand the body proves constant is that constant, and `x+0`, `x-0`
/// and `x*1` are `x`: no pass spells a zero start as a separate case.
pub(crate) struct Seeds<'a> {
    pub serial: u32,
    pub variable: u32,
    pub at: i64,
    pub width: u32,
    pub ops: Vec<Op>,
    pub facts: &'a IndexMap<Value, Known>,
}

impl Seeds<'_> {
    fn _known(&self, arg: &Arg) -> Arg {
        let fact = match arg {
            Arg::Held(held) => self.facts.get(&held.value).map(|fact| (fact, held.width)),
            _ => None,
        };
        match fact {
            Some((fact, width)) if fact.width >= width => Arg::Const(Const::new(masked(&fact.n, width), width)),
            _ => arg.clone(),
        }
    }

    pub(crate) fn computed(&mut self, kind: Kind, args: Vec<Arg>) -> AffineOperand {
        let known = args.iter().map(|arg| self._known(arg)).collect::<Vec<_>>();
        let zero = |arg: &Arg| matches!(arg, Arg::Const(constant) if masked(&constant.n, self.width) == BigInt::from(0_u8));
        let one = |arg: &Arg| matches!(arg, Arg::Const(constant) if constant.n == BigInt::from(1_u8));
        let kept = match (kind, known.as_slice()) {
            (Kind::Add | Kind::Sub, [_, right]) if zero(right) => Some(&args[0]),
            (Kind::Add, [left, _]) if zero(left) => Some(&args[1]),
            (Kind::Mul, [_, right]) if one(right) => Some(&args[0]),
            (Kind::Mul, [left, _]) if one(left) => Some(&args[1]),
            _ => None,
        };
        if let Some(kept) = kept {
            return AffineOperand::from_arg(kept).expect("a seed operand is held or constant");
        }
        let value = Value { id: self.serial, at: self.at, flags: false, variable: self.variable, version: 0 };
        self.serial += 1;
        self.variable += 1;
        self.ops.push(mir::computed(self.at, kind, value, args, self.width));
        AffineOperand::Held(Held { value, width: self.width })
    }

    /// `arg` as a value, for a phi to name.
    pub(crate) fn held(&mut self, arg: AffineOperand) -> Held {
        if let AffineOperand::Held(held) = arg {
            return held;
        }
        let value = Value { id: self.serial, at: self.at, flags: false, variable: self.variable, version: 0 };
        self.serial += 1;
        self.variable += 1;
        self.ops.push(mir::computed(self.at, Kind::Copy, value, vec![arg.as_arg()], self.width));
        Held { value, width: self.width }
    }
}

/// The preheader compare and branch that leave a counted loop before its first trip.
///
/// `body` is the snapshot `proof`'s occurrences index; Python holds the ops.
pub(crate) fn skip_guard(body: &MirBody, proof: &CountedLoop, at: i64, flags: Value) -> (Op, Op) {
    let ((left, right), test) = induction::skipped(proof);
    let args = vec![left.as_arg(), right.as_arg()];
    let compare = Op {
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

/// Exit phis of a replaced counter, reading its exit value after a trip and its start after none.
///
/// Keyed by the phi's occurrence in `body`, Python's `id(phi)`.
pub(crate) fn leaving(
    body: &MirBody,
    replacement: &ControlReplacement<'_>,
    seeds: &mut Seeds<'_>,
) -> BTreeMap<PhiOccurrence, Phi> {
    let proof = replacement.counted;
    if replacement.exits.is_empty() {
        return BTreeMap::new();
    }
    let exit = induction::exit_value(proof, &mut |kind, args| seeds.computed(kind, args));
    let value = seeds.held(exit);
    let phi_at = |at: PhiOccurrence| &body.blocks[at.block_index()].phis[at.phi_index()];
    let start = *phi_at(proof.phi).incoming.get(&proof.preheader).expect("KeyError");
    replacement
        .exits
        .iter()
        .map(|&at| {
            let phi = phi_at(at);
            let mut incoming = phi.incoming.keys().map(|&key| (key, value.value)).collect::<OrderedMap<_, _>>();
            incoming.insert(proof.preheader, start);
            (at, Phi { incoming, ..phi.clone() })
        })
        .collect()
}
