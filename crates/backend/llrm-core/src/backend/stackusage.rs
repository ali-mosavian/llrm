//! How much stack a program may use, as GCC's `-fstack-usage` and
//! `-Wstack-usage`: each procedure's own frame, and the most a call from it
//! can reach through the procedures it calls.
//!
//! A frame is what a call leaves on the stack: the return address, the
//! entry's pushes and the locals below BP, and the most a call's pushed
//! arguments hold at once. A routine the program does not define has a frame
//! this cannot know, so a bound through one is a lower bound, and a cycle of
//! calls has none.

use std::collections::{BTreeMap, BTreeSet};

use crate::backend::masm::{self, Module};
use crate::model::ir::{Loc, Operation};

/// What a procedure can reach.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Bound {
    /// At most this many bytes.
    Bytes(i64),
    /// At least this many, and what the named routines, which the program
    /// does not define, add.
    AtLeast(i64, BTreeSet<String>),
    /// It can call itself.
    Recursive,
}

#[derive(Debug, Default)]
pub struct Usage {
    frames: BTreeMap<String, i64>,
    calls: BTreeMap<String, BTreeSet<String>>,
}

/// The bytes a push or an `add sp` of `at` moves.
fn moved(what: &crate::model::ir::Semantics) -> i64 {
    match what.op {
        Operation::Push => what.sources.first().map_or(2, |one| match one {
            Loc::Reg(register) => i64::from(register.width),
            Loc::Imm(immediate) => i64::from(immediate.width),
            _ => 2,
        }),
        _ => 0,
    }
}

/// Bytes `procedure` has on the stack at its deepest: not counting what it calls.
fn frame(procedure: &masm::Procedure) -> i64 {
    let (enter, _) = masm::_frame_parts(procedure);
    let entry: i64 = enter
        .iter()
        .map(|one| match one.op {
            Operation::Push => moved(one),
            Operation::Binary if one.name.as_deref() == Some("sub") => match one.sources.get(1) {
                Some(Loc::Imm(amount)) => amount.value,
                _ => 0,
            },
            _ => 0,
        })
        .sum();
    // What its call sites have pushed when they call, the deepest.
    let mut peak = 0;
    for block in &procedure.body.blocks {
        let mut held = 0_i64;
        for one in &block.insns {
            let Some(what) = one.what.as_ref() else { continue };
            match what.op {
                Operation::Push => held += moved(what),
                Operation::Pop => held -= 2,
                Operation::Call => {
                    peak = peak.max(held);
                    held = 0;
                }
                _ => {}
            }
        }
    }
    (if procedure.far { 4 } else { 2 }) + entry + peak
}

impl Usage {
    pub fn of(modules: &[Module]) -> Self {
        let mut usage = Self::default();
        for procedure in modules.iter().flat_map(|one| &one.procedures) {
            usage.frames.insert(procedure.name.clone(), frame(procedure));
            usage.calls.insert(procedure.name.clone(), procedure.callees.values().filter(|one| one.code.is_empty()).map(|one| one.name.clone()).collect());
        }
        usage
    }

    /// What `name` can reach.
    pub fn bound(&self, name: &str) -> Bound {
        self.reach(name, &mut Vec::new())
    }

    fn reach(&self, name: &str, path: &mut Vec<String>) -> Bound {
        let Some(&own) = self.frames.get(name) else { return Bound::AtLeast(0, BTreeSet::from([name.to_owned()])) };
        if path.iter().any(|one| one == name) {
            return Bound::Recursive;
        }
        path.push(name.to_owned());
        let mut deepest = Bound::Bytes(0);
        for next in self.calls.get(name).into_iter().flatten() {
            deepest = match (deepest, self.reach(next, path)) {
                (Bound::Recursive, _) | (_, Bound::Recursive) => Bound::Recursive,
                (Bound::Bytes(one), Bound::Bytes(other)) => Bound::Bytes(one.max(other)),
                (Bound::Bytes(one) | Bound::AtLeast(one, _), Bound::AtLeast(other, named)) | (Bound::AtLeast(one, named), Bound::Bytes(other)) => Bound::AtLeast(one.max(other), named),
                (Bound::AtLeast(one, first), Bound::AtLeast(other, second)) => Bound::AtLeast(one.max(other), first.union(&second).cloned().collect()),
            };
        }
        path.pop();
        match deepest {
            Bound::Bytes(more) => Bound::Bytes(own + more),
            Bound::AtLeast(more, named) => Bound::AtLeast(own + more, named),
            Bound::Recursive => Bound::Recursive,
        }
    }

    /// The procedures nothing in the program calls.
    pub fn roots(&self) -> Vec<&str> {
        let called: BTreeSet<&str> = self.calls.values().flatten().map(String::as_str).collect();
        self.frames.keys().map(String::as_str).filter(|one| !called.contains(one)).collect()
    }

    /// Each procedure's frame and bound, one to a line.
    pub fn report(&self) -> String {
        let mut text = String::from("stack usage, bytes: procedure, its frame, the most it can reach\n");
        for (name, frame) in &self.frames {
            let bound = match self.bound(name) {
                Bound::Bytes(bytes) => bytes.to_string(),
                Bound::AtLeast(bytes, named) => format!(">= {bytes} (and {})", named.into_iter().collect::<Vec<_>>().join(", ")),
                Bound::Recursive => "unbounded (recursion)".to_owned(),
            };
            text.push_str(&format!("{name}\t{frame}\t{bound}\n"));
        }
        text
    }

    /// What `-Wstack-usage=limit` says: each root that can reach more than `limit`.
    pub fn warnings(&self, limit: i64) -> Vec<String> {
        self.roots()
            .into_iter()
            .filter_map(|name| match self.bound(name) {
                Bound::Bytes(bytes) | Bound::AtLeast(bytes, _) if bytes > limit => Some(format!("warning: {name} can use {bytes} bytes of stack, over -Wstack-usage={limit}")),
                Bound::Recursive => Some(format!("warning: {name} can recurse: its stack use has no bound, over -Wstack-usage={limit}")),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "stackusage_tests.rs"]
mod tests;
