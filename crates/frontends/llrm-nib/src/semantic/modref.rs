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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Effects {
    reads: BTreeSet<u32>,
    writes: BTreeSet<u32>,
    /// It may touch any of them: foreign code, `asm`, an unknown callee.
    everything: bool,
}

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
/// runtime defines, which touch no module variable.
pub(super) fn check_lends(functions: &[hir::Function], lends: &[Lend], runtime: &BTreeSet<&str>) -> Result<(), Diagnostic> {
    if lends.is_empty() {
        return Ok(());
    }
    let summaries = summaries(functions, runtime);
    let unknown = Effects { everything: true, ..Effects::default() };
    for lend in lends {
        let effects = summaries.get(lend.callee.as_str()).unwrap_or(&unknown);
        let touched = match (effects.writes(lend.symbol), lend.mutable && effects.reads(lend.symbol)) {
            (true, _) => "write",
            (false, true) => "read",
            (false, false) => continue,
        };
        return Err(Diagnostic::new(lend.span, format!("{:?} is lent to {:?}, which may {touched} it", lend.name, lend.callee)));
    }
    Ok(())
}

/// Each function's effects, with those of everything it calls.
fn summaries<'a>(functions: &'a [hir::Function], runtime: &BTreeSet<&str>) -> BTreeMap<&'a str, Effects> {
    let mut callees: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut summaries: BTreeMap<&str, Effects> = BTreeMap::new();
    for function in functions {
        let (effects, called) = direct(function);
        callees.insert(&function.name, called.into_iter().filter(|one| !runtime.contains(one)).collect());
        summaries.insert(&function.name, effects);
    }
    let unknown = Effects { everything: true, ..Effects::default() };
    loop {
        let mut changed = false;
        for (caller, called) in &callees {
            let mut effects = summaries[caller].clone();
            for callee in called {
                effects.absorb(summaries.get(callee).unwrap_or(&unknown));
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
/// functions it calls by name.
fn direct(function: &hir::Function) -> (Effects, BTreeSet<&str>) {
    let module: BTreeMap<u32, u32> = function.places.iter().filter(|one| one.storage == "module").map(|one| (one.id, one.symbol)).collect();
    let mut effects = Effects::default();
    let mut called = BTreeSet::new();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        match (instruction.op, &instruction.callee) {
            ("call", Some(callee)) => {
                called.insert(callee.as_str());
            }
            ("call", None) => effects.everything = true,
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
    /// Records the module variables a call of `callee` is lent: what each
    /// borrowed argument borrows, and what each argument holds.
    pub(super) fn record_lends(&mut self, callee: &str, arguments: &[Expr], parameters: &[SignatureParameter]) {
        for (argument, parameter) in arguments.iter().zip(parameters) {
            let (reached, mutable) = match *parameter {
                SignatureParameter::Borrowed { mutable, .. } => (self.reach(argument), mutable),
                // What a value holds may be written through: taken as `&mut`.
                SignatureParameter::Scalar(type_name) => (self.value_roots(argument, ElementType::Scalar(type_name)), true),
                SignatureParameter::Owned { struct_id, .. } => (self.value_roots(argument, ElementType::Struct(struct_id)), true),
                SignatureParameter::Adapter { .. } => continue,
            };
            for root in reached.into_iter().filter(|root| root.life == borrows::Life::Module) {
                let borrows::BorrowKey::Place(place) = root.owner else {
                    continue;
                };
                let symbol = self.places.iter().find(|one| one.id == place).expect("a module place").symbol;
                self.lends.push(Lend { callee: callee.to_owned(), symbol, name: root.name, mutable, span: argument.span() });
            }
        }
    }
}
