//! What each pass found nothing to do on, kept from one run of the pipeline
//! over a body to the next: a body goes through its pipeline again only where
//! something a pass read of it has changed.
//!
//! # How it works
//! A pass that changed nothing is noted as it runs: the parts of the module's
//! facts it read (`llrm_mir::passes::read`, noted by the analyses it asks for
//! and by what it asks of the proxy). When a run ends, each such pass that no
//! edit has reached since (`FunctionPass::reads`, `llrm_mir::footprint`) is
//! recorded for the body's history and its name: the point in the change log,
//! and a number for each part of what it could read outside the body (`slice`).
//! Asked to run again, the pass is skipped when the record holds: the body's
//! own attributes and every part the pass read are the same, and the edits
//! logged since reach nothing it reads. A pass that does change the body drops
//! its record.
//!
//! The body's own are its attributes and the module's metadata length. The
//! parts are, for each global the body names (a callee, a variable it takes the
//! address of, one a call's `!callees` lists): its declaration; what the callee
//! does to memory and the registers its call keeps; its summary; and, of the
//! module, what the globals analysis holds of those globals and what calling
//! back into the module does. A callee is taken by what is known of it, not by
//! its id: a call sent to a copy that is known to do the same is the same fact.
//!
//! An edit that a stage vouches for (`Settled::vouch`) is left out of what the
//! edits reach: a call sent to a copy of its callee whose attributes are the
//! same, an argument dropped that nothing else read. The facts' numbers check
//! the vouching: a copy that does something else is another number.
//!
//! ```text
//!   first run     fold gvn decide ...  idle   → records at mark 40, facts F
//!   stage edits   call g → call g.constprop.1   (vouched, g and its copy: F)
//!   second run    fold gvn decide ...  F same, nothing since reaches them → skipped
//!   a callee's memory effects change
//!   third run     F differs → each pass runs
//! ```
//!
//! `LLRM_CHECK_SETTLED=1` runs every pass that would be skipped and fails if
//! it changes the body.
//!
//! # Example
//! A body that calls `@g` is run through the pipeline, and a stage then sends
//! its call to `@g.copy`, a copy with the same attributes:
//!
//! ```mir
//! define i16 @f(i16 %x) {
//! b0:
//!   %a = call i16 @g(i16 %x)
//!   ret i16 %a
//! }
//! ```
//!
//! The second run starts no pass: the facts of the callee are the same and the
//! stage vouched for the edit. Where `@g.copy` writes memory that `@g` does
//! not, the facts differ and every pass runs again.
//!
//! From unit test
//! `test_a_body_run_again_after_a_call_is_sent_to_an_equal_copy_starts_no_pass`
//! in `pipeline_tests.rs`.
//!
//! # Provides
//! `Settled` (the records, held by the module's [`Context`]) and `slice`.
//!
//! # Needs
//! [`llrm_mir::footprint`], [`llrm_analysis::globalsaa`], and the module
//! analyses of the proxy a pass reads (`Outer`).
//!
//! # See also
//! LLVM's preserved analyses and gcc's `TODO_*` flags, which say the same of
//! what a pass needs to look at again; [`crate::pipeline`]'s per-run settled
//! test, which this extends across runs.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use llrm_analysis::manager::{GlobalsAA, Summaries};
use llrm_mir::context::{Context, GlobalId};
use llrm_mir::footprint::{self, Footprint};
use llrm_mir::module::{Change, Function, GlobalKind, Mark, MetadataOperand, Operand};
use llrm_mir::passes::{Outer, Unit, read};
use llrm_support::hash::HashMap;

/// A module's records: by the body's history and the pass.
#[derive(Default)]
pub struct Settled {
    found: RefCell<HashMap<u64, HashMap<&'static str, Record>>>,
    /// The stretches of a body's change log a stage vouched for, by history:
    /// first position and one past the last.
    sound: RefCell<HashMap<u64, Vec<(usize, usize)>>>,
    /// What the last asked stretch of a log reached, for the passes that ask of
    /// the same stretch one after the other: history, from, to, vouched
    /// stretches, reached.
    last: RefCell<Option<(u64, usize, usize, usize, Footprint)>>,
}

struct Record {
    mark: Mark,
    slice: Slice,
    /// The parts of the module's facts the pass read: the body's own are read
    /// by every pass.
    bits: u8,
}

/// What a body reads outside its instructions: of itself, and of the module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slice {
    own: u64,
    /// One number for each part of the module's facts
    /// (`llrm_mir::passes::read`), in the order of its bits; bit 3 is unused.
    parts: [u64; 5],
}

impl Slice {
    /// Whether the parts `bits` are the same in `other`.
    fn agrees(
        &self,
        other: &Self,
        bits: u8,
    ) -> bool {
        self.own == other.own && (0..5).all(|at| bits & (1 << at) == 0 || self.parts[at] == other.parts[at])
    }
}

impl Settled {
    /// Whether `pass` found nothing on `function` as it was, and nothing the
    /// facts or the edits since reach what it reads.
    pub fn holds(
        &self,
        pass: &'static str,
        function: &Function,
        slice: Slice,
        reads: Footprint,
    ) -> bool {
        let now = function.mark();
        let found = self.found.borrow();
        let Some(record) = found.get(&now.lineage()).and_then(|records| records.get(pass)) else {
            return false;
        };
        if !record.slice.agrees(&slice, record.bits) || !now.same_history(record.mark) {
            return false;
        }
        if now.position() == record.mark.position() {
            return true;
        }
        !self.reached(function, record.mark).meets(reads)
    }

    /// What the edits since `from` reach, the vouched ones left out.
    fn reached(
        &self,
        function: &Function,
        from: Mark,
    ) -> Footprint {
        let now = function.mark();
        let stretches = self.sound.borrow().get(&now.lineage()).map_or(0, Vec::len);
        let key = (now.lineage(), from.position(), now.position(), stretches);
        if let Some((a, b, c, d, found)) = *self.last.borrow()
            && (a, b, c, d) == key
        {
            return found;
        }
        let Some(changes) = function.changes_since(from) else { return Footprint::ALL };
        let sound = self.sound.borrow();
        let vouched = sound.get(&now.lineage());
        let reached: Vec<Change> = changes
            .iter()
            .enumerate()
            .filter(|&(at, _)| {
                let at = from.position() + at;
                !vouched.is_some_and(|ranges| ranges.iter().any(|&(from, to)| (from..to).contains(&at)))
            })
            .map(|(_, change)| change.clone())
            .collect();
        let found = footprint::of(function, &reached);
        *self.last.borrow_mut() = Some((key.0, key.1, key.2, key.3, found));
        found
    }

    /// Whether any pass has a record on `function`'s history.
    pub fn knows(
        &self,
        function: &Function,
    ) -> bool {
        self.found.borrow().contains_key(&function.mark().lineage())
    }

    /// The parts of the module's facts `pass` read when it found nothing.
    pub fn bits(
        &self,
        pass: &'static str,
        function: &Function,
    ) -> u8 {
        self.found
            .borrow()
            .get(&function.mark().lineage())
            .and_then(|records| records.get(pass))
            .map_or(0, |record| record.bits)
    }

    /// `pass` found nothing on a body as it was at `mark`, under facts `slice`.
    pub fn note(
        &self,
        pass: &'static str,
        mark: Mark,
        slice: Slice,
        bits: u8,
    ) {
        self.found.borrow_mut().entry(mark.lineage()).or_default().insert(pass, Record { mark, slice, bits });
    }

    /// `pass` changed `function`: what it found is no longer so.
    pub fn forget(
        &self,
        pass: &'static str,
        function: &Function,
    ) {
        if let Some(records) = self.found.borrow_mut().get_mut(&function.mark().lineage()) {
            records.remove(pass);
        }
    }

    /// The edits `function` had between `from` and now are vouched for.
    pub fn vouch(
        &self,
        function: &Function,
        from: Mark,
    ) {
        let now = function.mark();
        if now.same_history(from) && now.position() > from.position() {
            self.sound.borrow_mut().entry(now.lineage()).or_default().push((from.position(), now.position()));
        }
    }
}

/// The hasher of every digest here. SipHash was 1.8 M of a `d_faces` compile:
/// these are compared within a run, and nothing keyed guards them.
fn digest() -> llrm_mir::hash::Fx {
    llrm_mir::hash::Fx::default()
}

/// A number that is the same where a body reads the same outside itself: see
/// the module's text.
pub fn slice(
    unit: &Unit,
    held: &std::rc::Rc<Outer>,
) -> Slice {
    let outer = &**held;
    // What is read here is not what a pass reads.
    let saved = read::swap(0);
    let function = &*unit.function;
    let context = &*unit.context;
    let mut own = digest();
    // What a body says of its parameters for its callers (`initializes`) is a
    // result of its code, not a premise of it.
    let parameters: Vec<Vec<&llrm_mir::opcode::Attribute>> = function
        .parameter_attrs
        .iter()
        .map(|attrs| attrs.iter().filter(|one| !matches!(one, llrm_mir::opcode::Attribute::Initializes(_))).collect())
        .collect();
    let numbered = |value: &dyn Fn(&mut llrm_mir::hash::Fx)| {
        let mut hasher = digest();
        value(&mut hasher);
        hasher.finish()
    };
    let own_parts = [
        numbered(&|hasher| function.attrs.hash(hasher)),
        numbered(&|hasher| parameters.hash(hasher)),
        numbered(&|hasher| (&function.return_attrs, function.calling_convention, function.ty).hash(hasher)),
        numbered(&|hasher| unit.metadata.len().hash(hasher)),
    ];
    own_parts.hash(&mut own);
    let named = named(context, function, unit.metadata);
    let mut cache = FACTS.with(|cache| cache.take());
    if !cache.outer.as_ref().is_some_and(|then| std::rc::Rc::ptr_eq(then, held)) {
        cache = Facts { outer: Some(std::rc::Rc::clone(held)), ..Facts::default() };
    }
    unit.id.is_some_and(|id| named.contains(&id)).hash(&mut own);
    let summaries = outer.cached_ref::<Summaries>().and_then(|one| one.as_ref().ok());
    let (mut callees, mut known, mut declared) = (Vec::new(), Vec::new(), Vec::new());
    for &id in &named {
        let Some(global) = outer.globals.get(id.0 as usize) else { continue };
        if let Some(&(declaration, effects, summary)) = cache.each.get(&id) {
            declared.push(declaration);
            if let Some((effects, summary)) = effects.zip(summary) {
                callees.push(effects);
                known.push(summary);
            }
            continue;
        }
        let mut declaration = digest();
        let mut kept = (None, None);
        match &global.kind {
            GlobalKind::Function(callee) => {
                (
                    global.linkage,
                    &callee.attrs,
                    &callee.return_attrs,
                    callee.calling_convention,
                    // What a parameter says is told by its attributes, not by
                    // where it is.
                    callee.parameter_attrs.iter().filter(|attrs| !attrs.is_empty()).collect::<Vec<_>>(),
                )
                    .hash(&mut declaration);
                let mut effects = digest();
                // The pointer parameters it keeps a copy of are counted, not
                // placed: a parameter nothing reads is the one taken away.
                let pointers = |callee: &llrm_mir::module::Function| -> Vec<bool> {
                    match context.types.get(callee.ty) {
                        llrm_mir::types::Type::Function { parameters, .. } => parameters
                            .iter()
                            .map(|&one| matches!(context.types.get(one), llrm_mir::types::Type::Pointer(_)))
                            .collect(),
                        _ => Vec::new(),
                    }
                };
                let pointer = pointers(callee);
                outer
                    .callees()
                    .get(&id)
                    .map(|one| {
                        (
                            (
                                (one.effects.reads, one.effects.writes),
                                (one.accessible.reads, one.accessible.writes),
                                one.arguments_only,
                                one.returns,
                            ),
                            (one.memset, one.pattern, one.lifetime),
                            one.nocapture
                                .iter()
                                .enumerate()
                                .filter(|(at, kept)| !**kept && pointer.get(*at).copied().unwrap_or(true))
                                .count(),
                        )
                    })
                    .hash(&mut effects);
                outer.kept_across(Some(id)).hash(&mut effects);
                callees.push(effects.finish());
                let mut summary = digest();
                global.name.as_deref().and_then(|name| summaries?.get(name)).hash(&mut summary);
                known.push(summary.finish());
                kept = (Some(effects.finish()), Some(summary.finish()));
            }
            GlobalKind::Variable(_) => {
                (id, format!("{global:?}"), outer.sizes().get(&id)).hash(&mut declaration);
            }
        }
        declared.push(declaration.finish());
        cache.each.insert(id, (declaration.finish(), kept.0, kept.1));
    }
    let one_of = |mut numbers: Vec<u64>| {
        numbers.sort_unstable();
        numbers.dedup();
        let mut hasher = digest();
        numbers.hash(&mut hasher);
        hasher.finish()
    };
    let globals =
        outer.cached_ref::<GlobalsAA>().and_then(|one| one.as_ref().ok()).map(|globals| globals.read_by(&named));
    let hashed = |value: &dyn std::fmt::Debug| {
        let mut hasher = digest();
        format!("{value:?}").hash(&mut hasher);
        hasher.finish()
    };
    let slice =
        Slice { own: own.finish(), parts: [one_of(callees), one_of(known), hashed(&globals), 0, one_of(declared)] };
    FACTS.with(|held| held.replace(cache));
    read::swap(saved);
    slice
}

/// What the module's analyses say of each global, as of one proxy: made once
/// for the bodies that ask.
#[derive(Default)]
struct Facts {
    outer: Option<std::rc::Rc<Outer>>,
    /// Its declaration's number, what the callee does to memory and its
    /// summary.
    each: HashMap<GlobalId, (u64, Option<u64>, Option<u64>)>,
}

thread_local! {
    static FACTS: RefCell<Facts> = RefCell::new(Facts::default());
}

/// The globals `function` names: what it calls, what it takes the address of,
/// what its personality and the module's metadata name.
fn named(
    context: &Context,
    function: &Function,
    metadata: &[llrm_mir::module::MetadataNode],
) -> BTreeSet<GlobalId> {
    let mut out = BTreeSet::new();
    if let Some(one) = function.personality {
        llrm_analysis::globalsaa::embedded(context, one, &mut out);
    }
    for (_, inst) in function.walk() {
        for operand in &function.instruction(inst).operands {
            if let Operand::Constant(id) = operand {
                llrm_analysis::globalsaa::embedded(context, *id, &mut out);
            }
        }
    }
    for node in metadata {
        for operand in &node.operands {
            if let MetadataOperand::Constant(id) = operand {
                llrm_analysis::globalsaa::embedded(context, *id, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// The digests were `DefaultHasher`, SipHash13 over every attribute of a
    /// body at each slice. Does not compile for any other hasher type.
    #[test]
    fn test_digests_are_fx() {
        use std::hash::{Hash, Hasher};
        let mut hasher: llrm_mir::hash::Fx = super::digest();
        7u32.hash(&mut hasher);
        let mut by_hand = llrm_mir::hash::Fx::default();
        7u32.hash(&mut by_hand);
        assert_eq!(hasher.finish(), by_hand.finish());
    }
}
