//! How much stack a program may use, as gcc's `-fstack-usage` and
//! `-Wstack-usage` report it.
//!
//! # How it works
//! A procedure's frame is what a call to it leaves on the stack: the return
//! address, the entry's pushes and locals below BP (`push`, `enter`, `sub sp`),
//! and the most bytes its call sites have pushed for a call at once
//! (`outgoing`). Its bound is its frame plus the deepest bound of the
//! procedures it calls, computed bottom-up over the call graph.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   main["<b>main</b><br/><small>frame 12<br/>bound = 12 + max(30, 8) = 42</small>"]:::mid
//!   f["<b>f</b><br/><small>frame 20<br/>bound = 20 + 10 = 30</small>"]:::mid
//!   g["<b>g</b><br/><small>frame 8<br/>bound = 8, at least:<br/>calls extern</small>"]:::mid
//!   h["<b>h</b><br/><small>bound = 10</small>"]:::mid
//!   ext["<b>extern()</b>"]:::mid
//!   main --> f
//!   main --> g
//!   f --> h
//!   g --> ext
//! ```
//!
//! *Stack bound over a call graph*
//!
//! A bound is `Bytes(n)` (at most n), `AtLeast(n, names)` (a routine the
//! program does not define adds an unknown amount, so n is a lower bound and
//! the routines are named), or `Recursive` (a cycle of calls has none).
//! `deepest` is the largest bound over the procedures nothing calls. `report`
//! prints each frame and bound; `warnings` names each such root over
//! `-Wstack-usage=N`.
//!
//! Two other facts live here. `elide_checks` drops the stack check of a leaf
//! procedure that no outside code enters (not public, entered only by the
//! module's own direct calls) when its frame and the module's deepest push stay
//! within the runtime's red zone. `stack_to_add` is the bytes a frontend adds
//! to the base stack the runtime links, so the deepest chain fits, or an error
//! if it cannot fit a stack segment (`stack_limit`: the segment size less
//! 0x1000).
//!
//! # Example
//! A program whose `main` calls `f` and `g`, `f` calls `h` and the undefined
//! `ext`, and `g` calls `ext`:
//!
//! ```text
//! stack usage, bytes: procedure, its frame, the most it can reach
//! _f@3  6  >= 6 (and _ext@3)
//! _g@3  4  >= 4 (and _ext@3)
//! _h@3  4  4
//! _main  6  >= 10 (and _ext@3)
//! ```
//!
//! `_h@3` calls nothing, so its bound equals its frame. `ext` is not defined
//! in the program, so every procedure that reaches it has only a lower bound,
//! and the report names it.
//!
//! From `-fstack-usage` of a C program (`main` calls `f` and `g`; `f` calls
//! `h` and `ext`; `g` calls `ext`) at -O2 -m16.
//!
//! # Runs
//! `elide_checks` is the `stack checks` step of assembly, once per module.
//! `Usage::of` runs after every module is assembled, only with `-fstack-usage`
//! or `-Wstack-usage=N`. `stack_to_add` is called by the QB and Nib frontends.
//!
//! # See also
//! gcc's `-fstack-usage` and `-Wstack-usage`.

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

/// The bytes a push or an `add sp` of `at` moves; a push of what has no width
/// is a stack slot.
fn moved(
    what: &crate::model::ir::Semantics,
    slot: i64,
) -> i64 {
    match what.op {
        Operation::Push => what.sources.first().map_or(slot, |one| match one {
            Loc::Reg(register) => i64::from(register.width),
            Loc::Imm(immediate) => i64::from(immediate.width),
            _ => slot,
        }),
        _ => 0,
    }
}

/// Bytes `procedure` has on the stack at its deepest: not counting what it
/// calls.
pub fn frame(
    procedure: &masm::Procedure,
    target: &dyn llrm_target::Target,
) -> i64 {
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
pub fn outgoing(
    procedure: &masm::Procedure,
    slot: i64,
) -> i64 {
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

/// Drops the stack check of each procedure that cannot take SP below the limit
/// by more than the runtime's red zone, given every other procedure checks:
///
/// - it is a leaf (no call, no inline code), so nothing below it is checked
///   either way;
/// - only the module's own direct calls enter it (`entered_directly`: internal,
///   address not taken), so its caller is a checked procedure, or a leaf that
///   calls nothing;
/// - its caller held SP at or above the limit after its own frame, then pushed
///   at most the module's deepest `outgoing` and the return address before this
///   frame, so this frame and that push stay within `red_zone` bytes below the
///   limit.
pub fn elide_checks(
    procedures: &mut [masm::Procedure],
    entered_directly: &dyn Fn(&str) -> bool,
    target: &dyn llrm_target::Target,
) {
    let pushed = procedures.iter().map(|one| outgoing(one, target.stack_slot_bytes())).max().unwrap_or(0);
    for procedure in procedures.iter_mut() {
        let Some(check) = &procedure.stack_check else { continue };
        let leaf = procedure.callees.is_empty()
            && procedure.body.iter_insns().all(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Call));
        if leaf
            && !procedure.public
            && entered_directly(&procedure.name)
            && frame(procedure, target) + pushed <= check.red_zone
        {
            procedure.stack_check = None;
        }
    }
}

/// A stack is one segment, and DGROUP's data and heap share it: what a segment
/// of `segment_bytes` leaves it.
pub fn stack_limit(segment_bytes: Option<usize>) -> Option<i64> {
    segment_bytes.map(|bytes| bytes as i64 - 0x1000)
}

/// The bytes the object's stack segment adds to the `base` the runtime links,
/// so the deepest chain of frames fits. A chain that cannot fit a segment of
/// `limit` bytes is an error; none where the target has no segments.
pub fn stack_to_add(
    module: &Module,
    base: i64,
    reserve: i64,
    limit: Option<i64>,
    target: &dyn llrm_target::Target,
) -> Result<i64, String> {
    let need = Usage::of(std::slice::from_ref(module), target).deepest() + reserve;
    if let Some(limit) = limit.filter(|limit| need > *limit) {
        return Err(format!(
            "the deepest chain of calls needs {need} bytes of stack, more than the {limit} a stack segment can hold"
        ));
    }
    Ok((need - base).max(0))
}

impl Usage {
    pub fn of(
        modules: &[Module],
        target: &dyn llrm_target::Target,
    ) -> Self {
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
        // Callees first: each bound is its frame and the deepest of its
        // callees' memoized ones.
        for index in usage.graph.bottom_up() {
            let bound = usage.settle(index);
            usage.bounds.insert(index, bound);
        }
        usage
    }

    fn settle(
        &self,
        index: usize,
    ) -> Bound {
        if self.graph.recursive(index) {
            return Bound::Recursive;
        }
        let mut deepest = Bound::Bytes(0);
        for next in self.graph.callees_of(index) {
            deepest = match (deepest, self.bounds.get(&next).cloned().unwrap_or(Bound::Recursive)) {
                (Bound::Recursive, _) | (_, Bound::Recursive) => Bound::Recursive,
                (Bound::Bytes(one), Bound::Bytes(other)) => Bound::Bytes(one.max(other)),
                (Bound::Bytes(one) | Bound::AtLeast(one, _), Bound::AtLeast(other, named))
                | (Bound::AtLeast(one, named), Bound::Bytes(other)) => Bound::AtLeast(one.max(other), named),
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

    /// What `name` can reach; a routine the program does not define, at least
    /// nothing and itself.
    pub fn bound(
        &self,
        name: &str,
    ) -> Bound {
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
                Bound::AtLeast(bytes, named) => {
                    format!(">= {bytes} (and {})", named.into_iter().collect::<Vec<_>>().join(", "))
                }
                Bound::Recursive => "unbounded (recursion)".to_owned(),
            };
            text.push_str(&format!("{name}\t{frame}\t{bound}\n"));
        }
        text
    }

    /// What `-Wstack-usage=limit` says: each root that can reach more than
    /// `limit`.
    pub fn warnings(
        &self,
        limit: i64,
    ) -> Vec<String> {
        self.roots()
            .into_iter()
            .filter_map(|name| match self.bound(name) {
                Bound::Bytes(bytes) | Bound::AtLeast(bytes, _) if bytes > limit => {
                    Some(format!("warning: {name} can use {bytes} bytes of stack, over -Wstack-usage={limit}"))
                }
                Bound::Recursive => {
                    Some(format!("warning: {name} can recurse: its stack use has no bound, over -Wstack-usage={limit}"))
                }
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "stackusage_tests.rs"]
mod tests;
