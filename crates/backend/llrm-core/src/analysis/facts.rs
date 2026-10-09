//! The derived facts of a `LirBody`, kept while their inputs are: llvm's `MachineFunctionAnalysisManager`, in the shape
//! of llrm-mir's `Analysis` (`NAME`, `run`, a result that is `PartialEq`).
//!
//! The manager is a field of the body, shared by every body made from it (`with_blocks`, a clone), so it lives as long
//! as the function's compile and no longer. A fact says what it read (`inputs`) and whether a body still has those
//! (`held_by`); an ask of a body that does has the kept result. `LLRM_CHECK_FACTS` works every kept result out again
//! and compares.

use std::any::{Any, TypeId};
use std::sync::{Arc, Mutex};

use crate::model::lir::LirBody;
use crate::support::hash::HashMap;

pub trait Fact: 'static {
    type Result: PartialEq + std::fmt::Debug + Send + Sync + 'static;
    /// What `run` read, as kept beside its result.
    type Inputs: Send + Sync + 'static;
    const NAME: &'static str;
    fn run(body: &LirBody) -> Self::Result;
    fn inputs(body: &LirBody) -> Self::Inputs;
    /// Whether `body` has the inputs `kept` was made from.
    fn held_by(
        kept: &Self::Inputs,
        body: &LirBody,
    ) -> bool;
}

struct Slot<F: Fact>(Option<(F::Inputs, Arc<F::Result>)>);

#[derive(Default)]
pub struct Facts {
    slots: Mutex<HashMap<TypeId, Box<dyn Any + Send>>>,
    runs: Mutex<HashMap<&'static str, usize>>,
}

fn check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_FACTS"))
}

impl Facts {
    /// `F` of `body`: the kept result where `body` has its inputs, else worked out.
    pub fn get<F: Fact>(
        &self,
        body: &LirBody,
    ) -> Arc<F::Result> {
        self.get_checked::<F>(body, check())
    }

    fn get_checked<F: Fact>(
        &self,
        body: &LirBody,
        checked: bool,
    ) -> Arc<F::Result> {
        let kept = {
            let mut slots = self.slots.lock().expect("facts");
            let slot = slots.entry(TypeId::of::<F>()).or_insert_with(|| Box::new(Slot::<F>(None)));
            let Slot(held) = slot.downcast_mut::<Slot<F>>().expect("a slot is of its fact");
            held.as_ref().filter(|(inputs, _)| F::held_by(inputs, body)).map(|(_, result)| Arc::clone(result))
        };
        if let Some(result) = kept {
            if checked {
                assert!(F::run(body) == *result, "{}: the kept {} differs from working it out", body.name, F::NAME);
            }
            return result;
        }
        *self.runs.lock().expect("facts").entry(F::NAME).or_default() += 1;
        let result = Arc::new(F::run(body));
        let mut slots = self.slots.lock().expect("facts");
        let slot = slots.entry(TypeId::of::<F>()).or_insert_with(|| Box::new(Slot::<F>(None)));
        slot.downcast_mut::<Slot<F>>().expect("a slot is of its fact").0 = Some((F::inputs(body), Arc::clone(&result)));
        result
    }

    /// A value of type `T` the manager holds for its users, made by `Default` on first use: for a fact that keeps
    /// several states (a ring of answers) where `get`'s one slot is not enough. `change` must not ask the manager.
    pub fn stash<T: Default + Send + 'static, R>(
        &self,
        change: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut slots = self.slots.lock().expect("facts");
        let slot = slots.entry(TypeId::of::<T>()).or_insert_with(|| Box::<T>::default());
        change(slot.downcast_mut::<T>().expect("a stash is of its type"))
    }

    /// Counts what a test asserts was not done twice.
    pub fn bump(
        &self,
        name: &'static str,
    ) {
        *self.runs.lock().expect("facts").entry(name).or_default() += 1;
    }

    pub fn counted(
        &self,
        name: &'static str,
    ) -> usize {
        self.runs.lock().expect("facts").get(name).copied().unwrap_or(0)
    }

    /// How many times `F` has been worked out, for a test that asking again does not.
    pub fn runs<F: Fact>(&self) -> usize {
        self.runs.lock().expect("facts").get(F::NAME).copied().unwrap_or(0)
    }
}

/// Facts compare as equal and clone shared: they are no part of what a body is.
#[derive(Clone, Default)]
pub struct Kept(pub Arc<Facts>);

impl std::fmt::Debug for Kept {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.write_str("Facts")
    }
}

impl PartialEq for Kept {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for Kept {}

#[cfg(test)]
mod tests {
    use super::{Fact, Facts};
    use crate::model::lir::LirBody;
    use crate::support::hash::IndexMap;

    /// A fact that claims its inputs held whatever the body: what a fact that forgot an input does.
    struct Forgetful;

    impl Fact for Forgetful {
        type Result = usize;
        type Inputs = ();
        const NAME: &'static str = "forgetful";
        fn run(body: &LirBody) -> usize {
            body.blocks.len()
        }
        fn inputs(_: &LirBody) {}
        fn held_by(
            _: &(),
            _: &LirBody,
        ) -> bool {
            true
        }
    }

    /// A kept result that the body no longer gives went unnoticed (a stale frequency made the allocator's costs wrong
    /// without a word): the check mode works every kept result out again.
    #[test]
    fn test_the_check_catches_a_kept_fact_the_body_no_longer_gives() {
        let none = LirBody::new("f", 1, Vec::new(), IndexMap::default(), IndexMap::default());
        let facts = Facts::default();
        assert_eq!(*facts.get_checked::<Forgetful>(&none, true), 0);
        let one = none.with_blocks(vec![crate::model::lir::LirBlock::new(1, Vec::new())]);
        assert_eq!(*facts.get_checked::<Forgetful>(&one, false), 0, "unchecked, the stale result stands");
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| facts.get_checked::<Forgetful>(&one, true)))
                .is_err(),
            "the stale result was not caught"
        );
    }
}
