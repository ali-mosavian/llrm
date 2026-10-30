//! Records which registers a private function writes, so its direct callers
//! keep values in the others across the call.
//!
//! # How it works
//! A function nothing outside the module reaches (`callgraph::direct_only`),
//! and in no cycle of calls, takes part. It keeps its convention, but each
//! direct call of it clobbers only what its body wrote and did not save, not
//! the whole convention. A caller may therefore keep a value in any other
//! register across the call, scratch ones included.
//!
//! The fact is made once per function, from its final LIR: `written_by`
//! collects the roots of every register the body writes (destinations,
//! registers it pins values in or takes results from, what its calls
//! disturb). `record` then takes away the frame's own registers and the ones
//! the prologue saves and the epilogue restores. A register saved by its low
//! half alone (a 16-bit push of a 32-bit one) and written loses its upper
//! half to the caller (`Written::high`); so does a register whose upper half
//! a callee in the body loses, unless this function saves it whole.
//!
//! ```text
//!  machined first                    machined after
//!    leaf  writes ax, dx  -record->  main: call leaf clobbers ax, dx
//!                                          cx, si, di stay live across it
//! ```
//!
//! `CalleeFacts::of` returns the facts and the order to machine the
//! functions in, callees before callers, so a caller finds its callee's fact
//! when it is selected (`isel.rs` reads `written`). A function that is
//! exported, has its address taken, is an interrupt handler or is in a cycle
//! takes no part and keeps the full convention.
//!
//! # Example
//! `bump` (`return x + 1`) is private and called twice from `main`. Its fact is
//! one register:
//!
//! ```text
//! bump   writes eax      callers: clobbers EAX
//! ```
//!
//! The two kinds of call in `main`, after selection (`; inserted` notes
//! dropped), the runtime's `N$BGRW` first, then `bump`:
//!
//! ```lir
//! call   requires v1@AX,v2@DX,v3@CX; delivers v4@EAX; clobbers EAX,ECX,EDX,EBX,ES,FS,GS
//! call   requires v6@AX; delivers v7@EAX; clobbers EAX
//! ```
//!
//! `main` keeps a value in `bx` across both calls of `bump`: its listing has
//! `mov bx, word ptr [si-4]` before the first and `cmp bx, 2` after the second.
//!
//! From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
//! -fno-inline-functions-called-once -fno-ipa-cp-clone
//! -march=i386` of `examples/pass-docs/calleefacts.nib`, stage
//! `FarIndirectCalls`.
//!
//! # Runs
//! Per function in `assemble.rs`: `record` after the function is machined.
//! With `-fallocation-search-all` (-Omax) a caller of a known callee is also
//! machined without the facts and the cheaper kept.
//!
//! # See also
//! LLVM's `RegUsageInfoCollector` and GCC's `-fipa-ra` do this one function
//! at a time; this ordering comes from the call graph.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use llrm_lir::registers::RegId;
use llrm_mir::callgraph::{CallGraph, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::module::Module;

use crate::model::ir::{self, Loc};
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// What a function leaves different for its caller.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Written {
    /// The registers it wrote and did not save and restore.
    pub whole: BTreeSet<RegId>,
    /// The registers it saved by their low half alone (a 16-bit push of a
    /// 32-bit register) and wrote: their upper half is lost.
    pub high: BTreeSet<RegId>,
}

/// The functions of one module that take part in interprocedural register use,
/// and what each was found to write.
#[derive(Default)]
pub struct CalleeFacts {
    /// What each function of the module does to memory, worked out once for
    /// every function selected from it.
    callees: Option<std::rc::Rc<llrm_mir::memory::Callees>>,
    eligible: BTreeSet<String>,
    written: RefCell<BTreeMap<String, Written>>,
    /// The functions that need the stack aligned at their entry: those holding
    /// a double or an extended real (their 8-byte slots) and every function
    /// that calls one of them directly.
    aligned: BTreeSet<String>,
    /// The functions no function of the module calls (itself included) and
    /// whose address is not taken.
    uncalled: BTreeSet<String>,
}

/// Whether some function of `module` has a value of type double or x87
/// extended.
fn holds_doubles(
    module: &Module,
    function: &llrm_mir::Function,
) -> bool {
    use llrm_mir::types::{FloatKind, Type};
    let wide = |ty| matches!(
        module.context.types.get(ty),
        Type::Float(FloatKind::Double | FloatKind::X86Fp80)
    );
    function.parameters().iter().any(|&one| wide(function.value(one).ty))
        || function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .any(|inst| wide(function.instruction(inst).ty))
}

impl CalleeFacts {
    /// The facts of a module nothing is known of yet: no function takes part.
    pub fn none() -> Self {
        Self::default()
    }

    /// The functions of `module` that may take part, and the order to machine
    /// them in: callees before their callers.
    pub fn of(module: &Module) -> (Self, Vec<GlobalId>) {
        let graph = CallGraph::new(module);
        let private = direct_only(module);
        let mut aligned: BTreeSet<GlobalId> = BTreeSet::new();
        for id in graph.bottom_up() {
            let llrm_mir::GlobalKind::Function(function) = &module.global(id).kind else { continue };
            if holds_doubles(module, function) || graph.callees_of(id).iter().any(|callee| aligned.contains(callee)) {
                aligned.insert(id);
            }
        }
        let eligible = private
            .iter()
            .filter(|&&id| !graph.recursive(id))
            .filter_map(|&id| module.global(id).name.owned())
            .collect();
        let addressed = llrm_mir::callgraph::addressed(module);
        let called: BTreeSet<GlobalId> = graph.bottom_up().into_iter().flat_map(|id| graph.callees_of(id)).collect();
        let uncalled = graph
            .bottom_up()
            .into_iter()
            .filter(|id| !called.contains(id) && !addressed.contains(id))
            .filter_map(|id| module.global(id).name.owned())
            .collect();
        (
            Self {
                uncalled,
                callees: Some(std::rc::Rc::new(llrm_mir::memory::callees(module))),
                eligible,
                written: RefCell::default(),
                aligned: aligned.iter().filter_map(|&id| module.global(id).name.owned()).collect(),
            },
            graph.bottom_up(),
        )
    }

    /// Whether nothing in the module calls `name` or takes its address.
    pub fn uncalled(
        &self,
        name: &str,
    ) -> bool {
        self.uncalled.contains(name)
    }

    /// The bytes the stack pointer is a multiple of when `name` is entered: the
    /// target's alignment where it needs the stack aligned (a quad on its stack
    /// is then 8-aligned, which a 486 charges 3 clocks for otherwise: Intel486
    /// data sheet 240440-002, section 10.1 and Table 2.1), else a slot, so code
    /// that holds no double pays nothing for it.
    pub fn stack_alignment(
        &self,
        arch: &dyn llrm_target::Target,
        name: &str,
    ) -> i64 {
        if self.aligned.contains(name) || arch.aligned_calls() {
            arch.stack_alignment()
        } else {
            arch.stack_slot_bytes()
        }
    }

    /// What each function of the module does to memory, where the facts were
    /// made of it (`of`).
    pub fn callees(&self) -> Option<std::rc::Rc<llrm_mir::memory::Callees>> {
        self.callees.clone()
    }

    /// Whether `name` saves nothing for its caller and tells its callers what
    /// it wrote.
    pub fn takes_part(
        &self,
        name: &str,
    ) -> bool {
        self.eligible.contains(name)
    }

    /// What `name` was found to write, where it takes part and has been
    /// machined.
    pub fn written(
        &self,
        name: &str,
    ) -> Option<Written> {
        self.written.borrow().get(name).cloned()
    }

    /// Records what the finished `body` of `name` leaves different: what it
    /// writes, but for the registers its prologue saves and its epilogue
    /// restores (the frame's `saved`, by whole register).
    pub fn record(
        &self,
        name: &str,
        body: &LirBody,
        frame: &llrm_target::FrameRegisters,
        calls: &IndexMap<i64, String>,
    ) {
        if self.takes_part(name) {
            let regs = body.regs();
            let mut whole = written_by(body);
            // A sibling call is a `jmp`, with no call fact of its own: the
            // callee's writes are this function's. One that records nothing
            // leaves this function to its convention.
            let mut tail_high = BTreeSet::new();
            for one in body.insns() {
                if one.what.as_ref().is_some_and(|what| what.is_sibling_call()) {
                    let Some(callee) = calls.get(&one.at).and_then(|callee| self.written(callee)) else {
                        self.written.borrow_mut().remove(name);
                        return;
                    };
                    whole.extend(callee.whole);
                    tail_high.extend(callee.high);
                }
            }
            // The frame's own registers are its prologue's and epilogue's.
            for own in [frame.pointer, frame.stack] {
                whole.remove(&regs.root(own));
            }
            let mut high = tail_high;
            let mut saved_whole = BTreeSet::new();
            for (kept, pushed) in &frame.saved {
                if kept == pushed {
                    saved_whole.insert(regs.root(*kept));
                }
                if whole.remove(&regs.root(*kept)) && kept != pushed {
                    high.insert(*kept);
                }
            }
            // The upper half a callee loses is lost to this function's caller
            // too, unless the whole register is saved here.
            for one in body.insns() {
                for lost in &one.clobbers_high {
                    let root = regs.root(*lost);
                    if !whole.contains(&root) && !saved_whole.contains(&root) {
                        high.insert(root);
                    }
                }
            }
            llrm_support::debug!("facts", "{name} writes {:?}, the upper half of {:?}", whole, high);
            self.written.borrow_mut().insert(name.to_owned(), Written { whole, high });
        }
    }
}

/// The registers `body` writes, by their roots (a write of AX is a write of
/// EAX): its destinations, the registers it pins values in and takes results
/// from, and what its calls disturb.
pub fn written_by(body: &LirBody) -> BTreeSet<RegId> {
    let regs = body.regs();
    let root = |register: RegId| regs.root(register);
    let mut found = BTreeSet::new();
    for one in body.insns() {
        if let Some(what) = &one.what {
            for place in &what.dests {
                if let Loc::Reg(ir::Reg { register, .. }) = place {
                    found.insert(root(*register));
                }
            }
        }
        // A call disturbs what its callee's convention (or, where it takes
        // part, its own fact) says: what its contract claims of an
        // unseen routine is a bound for the caller's allocator, not a thing
        // that routine does.
        match &one.call {
            Some(call) => found.extend(call.disturbs.iter().copied().map(root)),
            None => found.extend(one.clobbers.iter().copied().map(root)),
        }
        found.extend(one.requires.iter().chain(&one.delivers).map(|(_, register)| root(*register)));
    }
    found
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn parsed(text: &str) -> Module {
        let layout = llrm_x86_m16::layout().datalayout;
        llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\n{text}")).expect("parses")
    }

    const LEAF: &str = "define internal i16 @leaf(i16 %a) {\nb0:\n  ret i16 %a\n}\n";

    fn takes_part(
        text: &str,
        name: &str,
    ) -> bool {
        CalleeFacts::of(&parsed(text)).0.takes_part(name)
    }

    /// An internal function only called directly, in no cycle, takes part; what
    /// a caller outside the module's sight, a pointer or an interrupt may
    /// reach does not, and neither does one that calls itself, nor one in a
    /// cycle of calls.
    #[test]
    fn what_may_be_reached_another_way_takes_no_part() {
        let caller = "define i16 @f(i16 %x) {\nb0:\n  %r = call i16 @leaf(i16 %x)\n  ret i16 %r\n}\n";
        assert!(takes_part(&format!("{LEAF}{caller}"), "leaf"));
        assert!(!takes_part(&format!("{}{caller}", LEAF.replace("internal ", "")), "leaf"), "exported");
        assert!(!takes_part(&format!("@table = global ptr @leaf\n{LEAF}{caller}"), "leaf"), "its address is stored");
        assert!(
            !takes_part(
                &format!(
                    "declare void @take(ptr)\n{LEAF}{caller}define void @g() {{\nb0:\n  call void @take(ptr @leaf)\n  ret void\n}}\n"
                ),
                "leaf"
            ),
            "its address is passed"
        );
        assert!(
            !takes_part(
                "define internal x86_intrcc void @isr() {\nb0:\n  ret void\n}\ndefine void @f() {\nb0:\n  call x86_intrcc void @isr()\n  ret void\n}\n",
                "isr"
            ),
            "an interrupt handler"
        );
        let itself = "define internal i16 @down(i16 %n) {\nb0:\n  %r = call i16 @down(i16 %n)\n  ret i16 %r\n}\ndefine i16 @f(i16 %x) {\nb0:\n  %r = call i16 @down(i16 %x)\n  ret i16 %r\n}\n";
        assert!(!takes_part(itself, "down"), "it calls itself");
        let cycle = "define internal i16 @ping(i16 %n) {\nb0:\n  %r = call i16 @pong(i16 %n)\n  ret i16 %r\n}\ndefine internal i16 @pong(i16 %n) {\nb0:\n  %r = call i16 @ping(i16 %n)\n  ret i16 %r\n}\ndefine i16 @f(i16 %x) {\nb0:\n  %r = call i16 @ping(i16 %x)\n  ret i16 %r\n}\n";
        assert!(!takes_part(cycle, "ping") && !takes_part(cycle, "pong"), "a cycle of calls");
    }

    /// Callees come before their callers, which is when a caller finds the fact
    /// of what it calls.
    #[test]
    fn callees_are_machined_before_their_callers() {
        let module =
            parsed(&format!("define i16 @f(i16 %x) {{\nb0:\n  %r = call i16 @leaf(i16 %x)\n  ret i16 %r\n}}\n{LEAF}"));
        let order: Vec<_> =
            CalleeFacts::of(&module).1.into_iter().filter_map(|id| module.global(id).name.owned()).collect();
        assert_eq!(order, ["leaf", "f"]);
    }

    /// `octants` saves `si` and calls `plot`, whose callees lose the upper half
    /// of `edi`; its fact said it wrote only `esi`, so `rim` kept a long in
    /// `edi` across it and read garbage (qb's gcirc.c arcs lost 19 pixels).
    #[test]
    fn what_a_call_loses_of_the_upper_half_is_lost_to_the_callers_caller() {
        use crate::model::lir::{Insn, LirBlock};
        use crate::support::hash::IndexMap;

        let call = Arc::new(Insn {
            clobbers_high: BTreeSet::from([RegId::EDI]),
            ..Insn::new(
                1,
                Some((1, 1)),
                Some(crate::model::ir::Semantics::new(crate::model::ir::Operation::Call)),
                vec![],
                vec![],
            )
        });
        let body = LirBody::new(
            "leaf",
            1,
            vec![LirBlock::new(1, vec![call]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let frame = |saved: Vec<(RegId, RegId)>| llrm_target::FrameRegisters {
            pointer: RegId::BP,
            stack: RegId::SP,
            saved,
            slot: 2,
            align: 2,
            optional: false,
            enter: false,
            wrap_split_size: false,
            slot_scratch: None,
            free: false,
        };
        let facts = CalleeFacts::of(&parsed(LEAF)).0;
        facts.record(
            "leaf",
            &body,
            &frame(vec![(RegId::ESI, RegId::SI), (RegId::EDI, RegId::DI)]),
            &IndexMap::default(),
        );
        assert_eq!(facts.written("leaf").unwrap().high, BTreeSet::from([RegId::EDI]), "saved by the word");
        facts.record("leaf", &body, &frame(vec![(RegId::EDI, RegId::EDI)]), &IndexMap::default());
        assert!(facts.written("leaf").unwrap().high.is_empty(), "saved whole");
    }

    /// `heap.trim` ended in a sibling `jmp` to `heap.release`, which writes
    /// `ecx`; its fact said it wrote no `ecx`, so a caller kept a string
    /// length there across it and `line.append` gave "01\0" for "012".
    #[test]
    fn a_sibling_call_writes_what_its_callee_writes() {
        use crate::model::ir::{Operation, Semantics};
        use crate::model::lir::{Insn, LirBlock};

        let module = parsed(&format!(
            "define internal i16 @trim(i16 %a) {{\nb0:\n  ret i16 %a\n}}\n\
             define i16 @f(i16 %x) {{\nb0:\n  %r = call i16 @trim(i16 %x)\n  ret i16 %r\n}}\n"
        ));
        let facts = CalleeFacts::of(&module).0;
        facts
            .written
            .borrow_mut()
            .insert("release".to_owned(), Written { whole: BTreeSet::from([RegId::ECX]), high: BTreeSet::new() });
        let jump = Arc::new(Insn::new(
            7,
            Some((1, 1)),
            Some(Semantics { name: Some("jmp".into()), ..Semantics::new(Operation::Return) }),
            vec![],
            vec![],
        ));
        let body = LirBody::new(
            "trim",
            1,
            vec![LirBlock::new(1, vec![jump]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let frame = llrm_target::FrameRegisters {
            pointer: RegId::BP,
            stack: RegId::SP,
            saved: vec![],
            slot: 2,
            align: 2,
            optional: false,
            enter: false,
            wrap_split_size: false,
            slot_scratch: None,
            free: false,
        };
        let calls = IndexMap::from_iter([(7, "release".to_owned())]);
        facts.record("trim", &body, &frame, &calls);
        assert!(facts.written("trim").unwrap().whole.contains(&RegId::ECX));
        facts.record("trim", &body, &frame, &IndexMap::default());
        assert!(facts.written("trim").is_none(), "an unknown tail callee leaves it to the convention");
    }
}
