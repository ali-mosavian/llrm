//! Which module variables each function may read and write, closed over its
//! calls: a summary of the HIR it compiled to (section 8). A module
//! variable lent `&` to a call that may write it, or `&mut` to one that may
//! read it, would change under the borrow; each lend is checked against
//! the summary once every function is compiled.

use super::*;

/// A module variable lent to a call.
#[derive(Clone, Debug)]
pub(super) struct Lend {
    pub(super) callee: String,
    /// The variable's object, which every function's place for it names.
    pub(super) symbol: u32,
    pub(super) name: String,
    pub(super) mutable: bool,
    pub(super) span: Span,
    /// A borrow already held across the call, not passed to it.
    pub(super) across: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Effects {
    reads: BTreeSet<u32>,
    writes: BTreeSet<u32>,
    /// It may touch any of them: `asm` that touches memory.
    everything: bool,
}

/// What a call into code outside the module stands for. It names no module
/// variable, which is private to its object, but it may call back what it
/// can reach, the entries. The runtime calls back nothing.
const FOREIGN: &str = "$foreign";

impl Effects {
    fn absorb(&mut self, other: &Effects) {
        self.reads.extend(other.reads.iter().copied());
        self.writes.extend(other.writes.iter().copied());
        self.everything |= other.everything;
    }

    fn reads(&self, symbol: u32) -> bool {
        self.everything || self.reads.contains(&symbol)
    }

    fn writes(&self, symbol: u32) -> bool {
        self.everything || self.writes.contains(&symbol)
    }
}

/// Errs at the first lend whose callee may write the variable or, for a
/// `&mut` lend, read it. `runtime` names the routines the compiler's own
/// runtime defines, which touch no module variable; `entries` the functions
/// other objects may call: exported, or with their address taken.
pub(super) fn check_lends(functions: &[hir::Function], lends: &[Lend], runtime: &BTreeSet<&str>, entries: &BTreeSet<&str>) -> Result<(), Diagnostic> {
    if lends.is_empty() {
        return Ok(());
    }
    let summaries = summaries(functions, runtime, entries);
    // An interrupt may run during any call.
    let mut interrupts = Effects::default();
    for handler in functions.iter().filter(|one| one.abi.as_ref().is_some_and(|abi| abi.distance == "interrupt")) {
        interrupts.absorb(&summaries[handler.name.as_str()]);
    }
    for lend in lends {
        let mut effects = summaries.get(lend.callee.as_str()).unwrap_or(&summaries[FOREIGN]).clone();
        effects.absorb(&interrupts);
        let touched = match (effects.writes(lend.symbol), lend.mutable && effects.reads(lend.symbol)) {
            (true, _) => "write",
            (false, true) => "read",
            (false, false) => continue,
        };
        let how = if lend.across { "borrowed across a call to" } else { "lent to" };
        return Err(Diagnostic::new(lend.span, format!("{:?} is {how} {:?}, which may {touched} it", lend.name, lend.callee)));
    }
    Ok(())
}

/// Each function's effects, with those of everything it calls.
fn summaries<'a>(functions: &'a [hir::Function], runtime: &BTreeSet<&str>, entries: &BTreeSet<&'a str>) -> BTreeMap<&'a str, Effects> {
    let defined: BTreeSet<&str> = functions.iter().map(|one| one.name.as_str()).collect();
    let mut callees: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut summaries: BTreeMap<&str, Effects> = BTreeMap::new();
    for function in functions {
        let (effects, called) = direct(function);
        let called = called.into_iter().filter(|one| !runtime.contains(one)).map(|one| if defined.contains(one) { one } else { FOREIGN });
        callees.insert(&function.name, called.collect());
        summaries.insert(&function.name, effects);
    }
    summaries.insert(FOREIGN, Effects::default());
    callees.insert(FOREIGN, entries.clone());
    loop {
        let mut changed = false;
        for (caller, called) in &callees {
            let mut effects = summaries[caller].clone();
            for callee in called {
                effects.absorb(&summaries[callee]);
            }
            if effects != summaries[caller] {
                summaries.insert(caller, effects);
                changed = true;
            }
        }
        if !changed {
            return summaries;
        }
    }
}

/// What `function` itself reads and writes of module variables, and the
/// functions it calls; a call through a pointer calls foreign code.
fn direct(function: &hir::Function) -> (Effects, BTreeSet<&str>) {
    let module: BTreeMap<u32, u32> = function.places.iter().filter(|one| one.storage == "module").map(|one| (one.id, one.symbol)).collect();
    let mut effects = Effects::default();
    let mut called = BTreeSet::new();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        match (instruction.op, &instruction.callee) {
            ("call", Some(callee)) => {
                called.insert(callee.as_str());
            }
            ("call", None) => {
                called.insert(FOREIGN);
            }
            _ => {}
        }
        if instruction.asm.as_ref().is_some_and(|asm| asm.memory) {
            effects.everything = true;
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            let Some(symbol) = place_of(operand).and_then(|place| module.get(&place)) else {
                continue;
            };
            match (instruction.op, index) {
                ("load", _) => {
                    effects.reads.insert(*symbol);
                }
                ("store", 0) => {
                    effects.writes.insert(*symbol);
                }
                // Its address escapes, or an operation both reads and writes it.
                _ => {
                    effects.reads.insert(*symbol);
                    effects.writes.insert(*symbol);
                }
            }
        }
    }
    (effects, called)
}

fn place_of(operand: &hir::Operand) -> Option<u32> {
    match operand {
        hir::Operand::Place(place) | hir::Operand::ArrayElement(place, _) | hir::Operand::ProjectedPlace { place, .. } => Some(*place),
        _ => None,
    }
}

impl FunctionCompiler<'_> {
    /// Records the module variables a call of `callee` is lent, and those
    /// the bindings in scope hold borrowed across it.
    pub(super) fn record_lends(&mut self, callee: &str, arguments: &[Expr], lent: &[borrows::Lent], span: Span) {
        let passed = arguments.iter().zip(lent).flat_map(|(argument, one)| one.roots.iter().map(move |root| (root.clone(), one.mutable, argument.span(), None)));
        let held = self.held_borrows().into_iter().map(|(root, mutable, holder)| (root, mutable, span, Some(holder)));
        let lends: Vec<_> = passed.chain(held).filter(|(root, ..)| root.life == borrows::Life::Module).collect();
        for (root, mutable, span, holder) in lends {
            let borrows::BorrowKey::Place(place) = root.owner else {
                continue;
            };
            let symbol = self.places.iter().find(|one| one.id == place).expect("a module place").symbol;
            let lend = Lend { callee: callee.to_owned(), symbol, name: root.name, mutable, span, across: holder.is_some() };
            match holder {
                Some(holder) => self.hold_across(lend, holder),
                None => self.lends.push(lend),
            }
        }
    }
}
