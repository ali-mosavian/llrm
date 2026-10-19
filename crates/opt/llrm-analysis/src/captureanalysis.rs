//! `CaptureAnalysis` of `AliasAnalysis.h` at d1106deb71cc: how an object may be
//! captured before an instruction, with the two providers
//! `BasicAliasAnalysis.cpp` has.
//!
//! # How it works
//! An object that is not a function-local identified one keeps its provenance:
//! anything may hold it. For a local one, [`SimpleCaptureAnalysis`] says what
//! `PointerMayBeCaptured` finds in the whole function, whatever the context
//! instruction. [`EarliestEscapeAnalysis`] finds once the instruction that
//! dominates every capture (`FindEarliestCapture`) and answers "none" for a
//! context instruction that cannot be reached from it, so a load before the
//! escape is not read by the escape. The answers are kept per object for the
//! life of the provider: a batch drops them with it.
//!
//! A new provider implements [`CaptureAnalysis`]; `AAQueryInfo` holds one and
//! `BasicAA` asks it.
//!
//! DEVIATE A1: [`EarliestEscapeAnalysis`] takes no `LoopInfo` or `CycleInfo`
//! (see [`crate::reachability`]); `isNotInCycle` is the successor walk.
//!
//! # Example
//! `%slot` is read before the call that captures it, so a context before the
//! call sees no capture; one at the call sees it:
//!
//! ```mir
//! define void @f() {
//! b:
//!   %slot = alloca i32
//!   %v = load i32, ptr %slot
//!   call void @keep(ptr %slot)
//!   ret void
//! }
//! ```
//!
//! From unit test `a_load_before_the_escape_is_not_captured_before` in
//! `captureanalysis_tests.rs`.
//!
//! # Provides
//! [`CaptureAnalysis`], [`SimpleCaptureAnalysis`], [`EarliestEscapeAnalysis`].
//!
//! # See also
//! LLVM `AliasAnalysis.h`, `BasicAliasAnalysis.cpp`,
//! [`crate::capturetracking`], [`crate::reachability`].

use std::rc::Rc;

use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{InstId, Operand, ValueId};

use crate::basicaa::is_identified_function_local;
#[cfg(test)]
thread_local! {
    /// `FindEarliestCapture` runs, for the test that each object is searched
    /// once however often it is asked about.
    pub static SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

use crate::capturetracking::{
    CaptureComponents, CaptureResult, DEFAULT_MAX_USES_TO_EXPLORE, find_earliest_capture, pointer_may_be_captured,
};
use crate::memory::Unit;
use crate::reachability::{is_potentially_reachable, is_potentially_reachable_from_many};

/// `CaptureAnalysis`: how `object` may be captured before `inst`, provenance
/// only. With `or_at` the captures by `inst` count. With no `inst` a capture
/// anywhere counts.
pub trait CaptureAnalysis: std::fmt::Debug {
    fn captures_before(
        &mut self,
        unit: &Unit,
        object: Operand,
        inst: Option<InstId>,
        or_at: bool,
        return_captures: bool,
    ) -> CaptureComponents;
}

/// The local object `object` names, if it is an identified function-local one.
fn local(
    unit: &Unit,
    object: Operand,
) -> Option<ValueId> {
    match object {
        Operand::Value(value) if is_identified_function_local(unit, object) => Some(value),
        _ => None,
    }
}

/// `SimpleCaptureAnalysis`: whether an object is captured in the function at
/// all, not where.
#[derive(Debug, Default)]
pub struct SimpleCaptureAnalysis {
    /// `IsCapturedCache`.
    cache: IdMap<ValueId, CaptureResult>,
}

impl CaptureAnalysis for SimpleCaptureAnalysis {
    fn captures_before(
        &mut self,
        unit: &Unit,
        object: Operand,
        _: Option<InstId>,
        _: bool,
        return_captures: bool,
    ) -> CaptureComponents {
        let Some(value) = local(unit, object) else { return CaptureComponents::PROVENANCE };
        let found = *self
            .cache
            .get_or_insert_with(
                value,
                || pointer_may_be_captured(
                    unit.function,
                    unit.globals,
                    unit.context,
                    value,
                    CaptureComponents::PROVENANCE,
                    |cc| cc.intersect(CaptureComponents::PROVENANCE) == CaptureComponents::PROVENANCE,
                    DEFAULT_MAX_USES_TO_EXPLORE,
                ),
            );
        if return_captures { found.with_ret } else { found.without_ret }
    }
}

/// `EarliestEscapeAnalysis`: the earliest instruction that captures each local
/// object, found once.
#[derive(Debug)]
pub struct EarliestEscapeAnalysis {
    /// The function's dominator tree, given or made when first needed.
    tree: Option<Rc<DomTree>>,
    /// `EarliestEscapes`: the instruction before which the object does not
    /// escape (none if it never does) and what any use may capture.
    escapes: IdMap<ValueId, (Option<InstId>, CaptureResult)>,
    /// `Inst2Obj`: the objects an instruction is the earliest escape of.
    by_instruction: IdMap<InstId, Vec<ValueId>>,
}

impl Default for EarliestEscapeAnalysis {
    fn default() -> Self {
        Self { tree: None, escapes: IdMap::new(), by_instruction: IdMap::new() }
    }
}

impl EarliestEscapeAnalysis {
    /// The analysis over `tree`, the dominator tree of the function asked
    /// about.
    pub fn with_tree(tree: Rc<DomTree>) -> Self {
        Self { tree: Some(tree), ..Self::default() }
    }

    fn tree(
        &mut self,
        unit: &Unit,
    ) -> Rc<DomTree> {
        Rc::clone(self.tree.get_or_insert_with(|| Rc::new(DomTree::of(unit.function))))
    }

    /// `removeInstruction`: forget the objects `inst` was the earliest escape
    /// of.
    pub fn remove_instruction(
        &mut self,
        inst: InstId,
    ) {
        for object in self.by_instruction.remove(&inst).unwrap_or_default() {
            self.escapes.remove(&object);
        }
    }

    /// `isNotInCycle`.
    fn not_in_cycle(
        &mut self,
        unit: &Unit,
        inst: InstId,
    ) -> bool {
        let tree = self.tree(unit);
        let block = unit.function.parent(inst).expect("placed");
        let successors: Vec<_> = unit.function.successors(block).iter().copied().collect();
        successors.is_empty() || !is_potentially_reachable_from_many(unit.function, successors, block, Some(&tree))
    }
}

impl CaptureAnalysis for EarliestEscapeAnalysis {
    fn captures_before(
        &mut self,
        unit: &Unit,
        object: Operand,
        inst: Option<InstId>,
        or_at: bool,
        return_captures: bool,
    ) -> CaptureComponents {
        let Some(value) = local(unit, object) else { return CaptureComponents::PROVENANCE };
        if !self.escapes.contains_key(&value) {
            #[cfg(test)]
            SEARCHES.with(|runs| runs.set(runs.get() + 1));
            let tree = self.tree(unit);
            let found = find_earliest_capture(
                unit.function,
                unit.globals,
                unit.context,
                &tree,
                value,
                CaptureComponents::PROVENANCE,
                DEFAULT_MAX_USES_TO_EXPLORE,
            );
            if let Some(earliest) = found.0 {
                self.by_instruction.get_or_insert_with(earliest, Vec::new).push(value);
            }
            self.escapes.insert(value, found);
        }
        let (earliest, result) = *self.escapes.get(&value).expect("searched above");
        if return_captures {
            assert!(inst.is_none(), "a context instruction is not supported with ReturnCaptures");
            return result.with_ret;
        }
        let not_captured_before = match (earliest, inst) {
            // No capturing instruction.
            (None, _) => true,
            // No context instruction means any use is capturing.
            (_, None) => false,
            (Some(capture), Some(at)) if capture == at => !or_at && self.not_in_cycle(unit, at),
            (Some(capture), Some(at)) => !is_potentially_reachable(unit.function, capture, at, Some(&self.tree(unit))),
        };
        if not_captured_before { CaptureComponents::NONE } else { result.without_ret }
    }
}

#[cfg(test)]
#[path = "captureanalysis_tests.rs"]
mod tests;
