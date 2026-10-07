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

use llrm_mir::callgraph::CallGraph;

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

#[derive(Debug)]
pub struct Usage {
    names: Vec<String>,
    frames: Vec<i64>,
    graph: CallGraph<usize>,
    /// Calls to routines the program does not define, by caller.
    outside: BTreeMap<usize, BTreeSet<String>>,
    bounds: BTreeMap<usize, Bound>,
}

/// The bytes a push or an `add sp` of `at` moves; a push of what has no width is a stack slot.
fn moved(what: &crate::model::ir::Semantics, slot: i64) -> i64 {
    match what.op {
        Operation::Push => what.sources.first().map_or(slot, |one| match one {
            Loc::Reg(register) => i64::from(register.width),
            Loc::Imm(immediate) => i64::from(immediate.width),
            _ => slot,
        }),
        _ => 0,
    }
}

/// Bytes `procedure` has on the stack at its deepest: not counting what it calls.
pub fn frame(procedure: &masm::Procedure, target: &dyn llrm_target::Target) -> i64 {
    let slot = target.stack_slot_bytes();
    let (enter, _) = masm::_frame_parts(procedure);
    let entry: i64 = enter
        .iter()
        .map(|one| match one.op {
            Operation::Push => moved(one, slot),
            // The saved BP and the locals.
            Operation::Nothing if one.name.as_deref() == Some("enter") => match one.sources.first() {
                Some(Loc::Imm(amount)) => slot + amount.value,
                _ => 0,
            },
            Operation::Binary if one.name.as_deref() == Some("sub") => match one.sources.get(1) {
                Some(Loc::Imm(amount)) => amount.value,
                _ => 0,
            },
            _ => 0,
        })
        .sum();
    target.return_address_bytes(procedure.far) + entry + procedure.entry + outgoing(procedure, slot)
}

/// What `procedure`'s call sites have pushed when they call, the deepest.
pub fn outgoing(procedure: &masm::Procedure, slot: i64) -> i64 {
    let mut peak = 0;
    for block in &procedure.body.blocks {
        let mut held = 0_i64;
        for one in &block.insns {
            let Some(what) = one.what.as_ref() else { continue };
            match what.op {
                Operation::Push => held += moved(what, slot),
                Operation::Pop => held -= slot,
                Operation::Call => {
                    peak = peak.max(held);
                    held = 0;
                }
                _ => {}
            }
        }
    }
    peak
}

/// Drops the stack check of each procedure that cannot take SP below the limit by more than the
/// runtime's red zone, given every other procedure checks:
///
/// - it is a leaf (no call, no inline code), so nothing below it is checked either way;
/// - only the module's own direct calls enter it (`entered_directly`: internal, address not
///   taken), so its caller is a checked procedure, or a leaf that calls nothing;
/// - its caller held SP at or above the limit after its own frame, then pushed at most the
///   module's deepest `outgoing` and the return address before this frame, so this frame and
///   that push stay within `red_zone` bytes below the limit.
pub fn elide_checks(procedures: &mut [masm::Procedure], entered_directly: &dyn Fn(&str) -> bool, target: &dyn llrm_target::Target) {
    let pushed = procedures.iter().map(|one| outgoing(one, target.stack_slot_bytes())).max().unwrap_or(0);
    for procedure in procedures.iter_mut() {
        let Some(check) = &procedure.stack_check else { continue };
        let leaf = procedure.callees.is_empty() && procedure.body.insns().iter().all(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Call));
        if leaf && !procedure.public && entered_directly(&procedure.name) && frame(procedure, target) + pushed <= check.red_zone {
            procedure.stack_check = None;
        }
    }
}

/// A stack is one segment, and DGROUP's data and heap share it: what a segment of
/// `segment_bytes` leaves it.
pub fn stack_limit(segment_bytes: Option<usize>) -> Option<i64> {
    segment_bytes.map(|bytes| bytes as i64 - 0x1000)
}

/// The bytes the object's stack segment adds to the `base` the runtime links,
/// so the deepest chain of frames fits. A chain that cannot fit a segment of `limit` bytes is an
/// error; none where the target has no segments.
pub fn stack_to_add(module: &Module, base: i64, reserve: i64, limit: Option<i64>, target: &dyn llrm_target::Target) -> Result<i64, String> {
    let need = Usage::of(std::slice::from_ref(module), target).deepest() + reserve;
    if let Some(limit) = limit.filter(|limit| need > *limit) {
        return Err(format!("the deepest chain of calls needs {need} bytes of stack, more than the {limit} a stack segment can hold"));
    }
    Ok((need - base).max(0))
}

impl Usage {
    pub fn of(modules: &[Module], target: &dyn llrm_target::Target) -> Self {
        let procedures: Vec<&masm::Procedure> = modules.iter().flat_map(|one| &one.procedures).collect();
        let names: Vec<String> = procedures.iter().map(|one| one.name.clone()).collect();
        let at: BTreeMap<&str, usize> = names.iter().enumerate().map(|(index, name)| (name.as_str(), index)).collect();
        let mut callees = BTreeMap::<usize, BTreeSet<usize>>::new();
        let mut outside = BTreeMap::<usize, BTreeSet<String>>::new();
        for (index, procedure) in procedures.iter().enumerate() {
            callees.entry(index).or_default();
            for callee in procedure.callees.values().filter(|one| one.code.is_empty()) {
                match at.get(callee.name.as_str()) {
                    Some(&to) => {
                        callees.entry(index).or_default().insert(to);
                    }
                    None => {
                        outside.entry(index).or_default().insert(callee.name.clone());
                    }
                }
            }
        }
        let graph = CallGraph::from_edges(callees);
        let frames = procedures.iter().map(|one| frame(one, target)).collect::<Vec<_>>();
        let mut usage = Self { names, frames, graph, outside, bounds: BTreeMap::new() };
        // Callees first: each bound is its frame and the deepest of its callees' memoized ones.
        for index in usage.graph.bottom_up() {
            let bound = usage.settle(index);
            usage.bounds.insert(index, bound);
        }
        usage
    }

    fn settle(&self, index: usize) -> Bound {
        if self.graph.recursive(index) {
            return Bound::Recursive;
        }
        let mut deepest = Bound::Bytes(0);
        for next in self.graph.callees_of(index) {
            deepest = match (deepest, self.bounds.get(&next).cloned().unwrap_or(Bound::Recursive)) {
                (Bound::Recursive, _) | (_, Bound::Recursive) => Bound::Recursive,
                (Bound::Bytes(one), Bound::Bytes(other)) => Bound::Bytes(one.max(other)),
                (Bound::Bytes(one) | Bound::AtLeast(one, _), Bound::AtLeast(other, named)) | (Bound::AtLeast(one, named), Bound::Bytes(other)) => Bound::AtLeast(one.max(other), named),
                (Bound::AtLeast(one, first), Bound::AtLeast(other, second)) => Bound::AtLeast(one.max(other), first.union(&second).cloned().collect()),
            };
        }
        if let Some(named) = self.outside.get(&index) {
            deepest = match deepest {
                Bound::Bytes(bytes) => Bound::AtLeast(bytes, named.clone()),
                Bound::AtLeast(bytes, more) => Bound::AtLeast(bytes, more.union(named).cloned().collect()),
                Bound::Recursive => Bound::Recursive,
            };
        }
        match deepest {
            Bound::Bytes(more) => Bound::Bytes(self.frames[index] + more),
            Bound::AtLeast(more, named) => Bound::AtLeast(self.frames[index] + more, named),
            Bound::Recursive => Bound::Recursive,
        }
    }

    /// What `name` can reach; a routine the program does not define, at least nothing and itself.
    pub fn bound(&self, name: &str) -> Bound {
        match self.names.iter().position(|one| one == name) {
            Some(index) => self.bounds.get(&index).cloned().unwrap_or(Bound::Recursive),
            None => Bound::AtLeast(0, BTreeSet::from([name.to_owned()])),
        }
    }

    /// The procedures nothing in the program calls.
    pub fn roots(&self) -> Vec<&str> {
        let called: BTreeSet<usize> = (0..self.names.len()).flat_map(|one| self.graph.callees_of(one)).collect();
        (0..self.names.len()).filter(|one| !called.contains(one)).map(|one| self.names[one].as_str()).collect()
    }

    /// The most any entry can reach: a bound the program's own procedures give,
    /// not counting what a routine it does not define adds. An entry that
    /// recurses has none and is left out.
    pub fn deepest(&self) -> i64 {
        self.roots()
            .into_iter()
            .filter_map(|name| match self.bound(name) {
                Bound::Bytes(bytes) | Bound::AtLeast(bytes, _) => Some(bytes),
                Bound::Recursive => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Each procedure's frame and bound, one to a line.
    pub fn report(&self) -> String {
        let mut text = String::from("stack usage, bytes: procedure, its frame, the most it can reach\n");
        let mut order: Vec<usize> = (0..self.names.len()).collect();
        order.sort_by_key(|&one| &self.names[one]);
        for index in order {
            let (name, frame) = (&self.names[index], self.frames[index]);
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
