//! What a function nothing outside the program reaches leaves different: the registers its body, and the calls in it, write.
//!
//! Such a function (`callgraph::direct_only`, in no cycle of calls) keeps its convention, and each direct call of it clobbers
//! what it wrote and did not save, no more: a caller keeps a value in any other register across the call, the scratch ones
//! among them. (LLVM's no-CSR mode also drops the saves of such a function, which hands the registers it then writes to
//! every caller: on nbody_fixed that cost more than the pushes.) The fact is made when
//! the function is machined, once, from its final LIR, and asked for by every caller; the functions are machined callees
//! first so it is there when asked. LLVM's `RegUsageInfoCollector` and GCC's `-fipa-ra` do the same one function at a time;
//! this one asks the call graph for the order, so no caller meets a callee whose fact is not yet made, and leaves a function
//! in a cycle with its convention.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use iced_x86::Register;
use llrm_mir::callgraph::{CallGraph, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::module::Module;

use crate::model::ir::{self, Loc};
use crate::model::lir::LirBody;

/// What a function leaves different for its caller.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Written {
    /// The registers it wrote and did not save and restore.
    pub whole: BTreeSet<Register>,
    /// The registers it saved by their low half alone (a 16-bit push of a 32-bit register) and wrote: their upper half is lost.
    pub high: BTreeSet<Register>,
}

/// The functions of one module that take part in interprocedural register use, and what each was found to write.
#[derive(Default)]
pub struct CalleeFacts {
    /// What each function of the module does to memory, worked out once for every function selected from it.
    callees: Option<std::rc::Rc<llrm_mir::memory::Callees>>,
    eligible: BTreeSet<String>,
    written: RefCell<BTreeMap<String, Written>>,
}

impl CalleeFacts {
    /// The facts of a module nothing is known of yet: no function takes part.
    pub fn none() -> Self {
        Self::default()
    }

    /// The functions of `module` that may take part, and the order to machine them in: callees before their callers.
    pub fn of(module: &Module) -> (Self, Vec<GlobalId>) {
        let graph = CallGraph::new(module);
        let private = direct_only(module);
        let eligible = private.iter().filter(|&&id| !graph.recursive(id)).filter_map(|&id| module.global(id).name.clone()).collect();
        (Self { callees: Some(std::rc::Rc::new(llrm_mir::memory::callees(module))), eligible, written: RefCell::default() }, graph.bottom_up())
    }

    /// What each function of the module does to memory, where the facts were made of it (`of`).
    pub fn callees(&self) -> Option<std::rc::Rc<llrm_mir::memory::Callees>> {
        self.callees.clone()
    }

    /// Whether `name` saves nothing for its caller and tells its callers what it wrote.
    pub fn takes_part(&self, name: &str) -> bool {
        self.eligible.contains(name)
    }

    /// What `name` was found to write, where it takes part and has been machined.
    pub fn written(&self, name: &str) -> Option<Written> {
        self.written.borrow().get(name).cloned()
    }

    /// Records what the finished `body` of `name` leaves different: what it writes, but for the registers its prologue saves
    /// and its epilogue restores (the frame's `saved`, by whole register).
    pub fn record(&self, name: &str, body: &LirBody, frame: &llrm_target::FrameRegisters) {
        if self.takes_part(name) {
            let mut whole = written_by(body);
            // The frame's own registers are its prologue's and epilogue's.
            for own in [frame.pointer, frame.stack] {
                whole.remove(&ir::root(own));
            }
            let mut high = BTreeSet::new();
            for (kept, pushed) in &frame.saved {
                if whole.remove(&ir::root(*kept)) && kept != pushed {
                    high.insert(*kept);
                }
            }
            llrm_support::debug!("facts", "{name} writes {:?}, the upper half of {:?}", whole, high);
            self.written.borrow_mut().insert(name.to_owned(), Written { whole, high });
        }
    }
}

/// The registers `body` writes, by their roots (a write of AX is a write of EAX): its destinations, the registers it
/// pins values in and takes results from, and what its calls disturb.
pub fn written_by(body: &LirBody) -> BTreeSet<Register> {
    let mut found = BTreeSet::new();
    for one in body.insns() {
        if let Some(what) = &one.what {
            for place in &what.dests {
                if let Loc::Reg(ir::Reg { register, .. }) = place {
                    found.insert(ir::root(*register));
                }
            }
        }
        // A call disturbs what its callee's convention (or, where it takes part, its own fact) says: what its contract
        // claims of an unseen routine is a bound for the caller's allocator, not a thing that routine does.
        match &one.call {
            Some(call) => found.extend(call.disturbs.iter().copied().map(ir::root)),
            None => found.extend(one.clobbers.iter().copied().map(ir::root)),
        }
        found.extend(one.requires.iter().chain(&one.delivers).map(|(_, register)| ir::root(*register)));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> Module {
        let layout = llrm_x86_m16::layout().datalayout;
        llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\n{text}")).expect("parses")
    }

    const LEAF: &str = "define internal i16 @leaf(i16 %a) {\nb0:\n  ret i16 %a\n}\n";

    fn takes_part(text: &str, name: &str) -> bool {
        CalleeFacts::of(&parsed(text)).0.takes_part(name)
    }

    /// An internal function only called directly, in no cycle, takes part; what a caller outside the module's sight, a pointer or
    /// an interrupt may reach does not, and neither does one that calls itself, nor one in a cycle of calls.
    #[test]
    fn what_may_be_reached_another_way_takes_no_part() {
        let caller = "define i16 @f(i16 %x) {\nb0:\n  %r = call i16 @leaf(i16 %x)\n  ret i16 %r\n}\n";
        assert!(takes_part(&format!("{LEAF}{caller}"), "leaf"));
        assert!(!takes_part(&format!("{}{caller}", LEAF.replace("internal ", "")), "leaf"), "exported");
        assert!(!takes_part(&format!("@table = global ptr @leaf\n{LEAF}{caller}"), "leaf"), "its address is stored");
        assert!(!takes_part(&format!("declare void @take(ptr)\n{LEAF}{caller}define void @g() {{\nb0:\n  call void @take(ptr @leaf)\n  ret void\n}}\n"), "leaf"), "its address is passed");
        assert!(!takes_part("define internal x86_intrcc void @isr() {\nb0:\n  ret void\n}\ndefine void @f() {\nb0:\n  call x86_intrcc void @isr()\n  ret void\n}\n", "isr"), "an interrupt handler");
        let itself = "define internal i16 @down(i16 %n) {\nb0:\n  %r = call i16 @down(i16 %n)\n  ret i16 %r\n}\ndefine i16 @f(i16 %x) {\nb0:\n  %r = call i16 @down(i16 %x)\n  ret i16 %r\n}\n";
        assert!(!takes_part(itself, "down"), "it calls itself");
        let cycle = "define internal i16 @ping(i16 %n) {\nb0:\n  %r = call i16 @pong(i16 %n)\n  ret i16 %r\n}\ndefine internal i16 @pong(i16 %n) {\nb0:\n  %r = call i16 @ping(i16 %n)\n  ret i16 %r\n}\ndefine i16 @f(i16 %x) {\nb0:\n  %r = call i16 @ping(i16 %x)\n  ret i16 %r\n}\n";
        assert!(!takes_part(cycle, "ping") && !takes_part(cycle, "pong"), "a cycle of calls");
    }

    /// Callees come before their callers, which is when a caller finds the fact of what it calls.
    #[test]
    fn callees_are_machined_before_their_callers() {
        let module = parsed(&format!("define i16 @f(i16 %x) {{\nb0:\n  %r = call i16 @leaf(i16 %x)\n  ret i16 %r\n}}\n{LEAF}"));
        let order: Vec<_> = CalleeFacts::of(&module).1.into_iter().filter_map(|id| module.global(id).name.clone()).collect();
        assert_eq!(order, ["leaf", "f"]);
    }
}
