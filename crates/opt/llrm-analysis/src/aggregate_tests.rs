//! `AAResults::addAAResult` against AliasAnalysis.cpp's rules: the first
//! answer that is not `MayAlias`, the meet of the mod/ref answers.

use std::cell::Cell;
use std::rc::Rc;

use llrm_mir::module::{InstId, Operand, ValueId};

use super::*;
use crate::aa::{IRMemLocation, LocationSize};

/// A provider that always gives the same answers and counts how often it is
/// asked.
struct Fixed {
    alias: AliasResult,
    mod_ref: ModRefInfo,
    /// What `get_mod_ref_info_mask` says of a location.
    mask: ModRefInfo,
    effects: MemoryEffects,
    asked: Rc<Cell<usize>>,
}

impl Fixed {
    fn new(
        alias: AliasResult,
        mod_ref: ModRefInfo,
    ) -> (Self, Rc<Cell<usize>>) {
        let asked = Rc::new(Cell::new(0));
        let effects = MemoryEffects::everywhere(mod_ref);
        (Self { alias, mod_ref, mask: mod_ref, effects, asked: Rc::clone(&asked) }, asked)
    }
}

impl AAResults for Fixed {
    fn alias(
        &mut self,
        _: &MemoryLocation,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: Option<InstId>,
    ) -> AliasResult {
        self.asked.set(self.asked.get() + 1);
        self.alias
    }

    fn get_mod_ref_info_mask(
        &mut self,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: bool,
    ) -> ModRefInfo {
        self.asked.set(self.asked.get() + 1);
        self.mask
    }

    fn get_arg_mod_ref_info(
        &mut self,
        _: InstId,
        _: u32,
    ) -> ModRefInfo {
        self.asked.set(self.asked.get() + 1);
        self.mod_ref
    }

    fn get_memory_effects(
        &mut self,
        _: InstId,
        _: &mut AAQueryInfo,
    ) -> MemoryEffects {
        self.asked.set(self.asked.get() + 1);
        self.effects
    }

    fn get_mod_ref_info(
        &mut self,
        _: InstId,
        _: Option<&MemoryLocation>,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        self.asked.set(self.asked.get() + 1);
        self.mod_ref
    }

    fn get_mod_ref_info_call(
        &mut self,
        _: InstId,
        _: InstId,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        self.asked.set(self.asked.get() + 1);
        self.mod_ref
    }

    fn call_captures_before(
        &mut self,
        _: InstId,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        self.asked.set(self.asked.get() + 1);
        self.mod_ref
    }
}

fn location(at: u32) -> MemoryLocation {
    MemoryLocation::new(Operand::Value(ValueId(at)), LocationSize::Precise(4))
}

fn aliased(providers: &[(AliasResult, ModRefInfo)]) -> (AliasResult, Vec<usize>) {
    let mut chain = AggregateAA::new();
    let counts: Vec<_> = providers
        .iter()
        .map(|&(alias, mod_ref)| {
            let (provider, asked) = Fixed::new(alias, mod_ref);
            chain.add_aa_result(provider);
            asked
        })
        .collect();
    let answer = chain.alias(&location(0), &location(1), &mut AAQueryInfo::default(), None);
    (answer, counts.iter().map(|asked| asked.get()).collect())
}

const ANY: ModRefInfo = ModRefInfo::ModRef;

#[test]
fn the_first_answer_that_is_not_may_alias_decides() {
    use AliasResult::{MayAlias, MustAlias, NoAlias};
    assert_eq!(aliased(&[(MayAlias, ANY), (NoAlias, ANY)]).0, NoAlias);
    // The first decides: a later contrary answer is not asked.
    assert_eq!(aliased(&[(NoAlias, ANY), (MustAlias, ANY)]), (NoAlias, vec![1, 0]));
    assert_eq!(aliased(&[(MustAlias, ANY), (NoAlias, ANY)]).0, MustAlias);
    assert_eq!(aliased(&[(MayAlias, ANY), (MayAlias, ANY)]), (MayAlias, vec![1, 1]));
}

#[test]
fn an_empty_aggregate_knows_nothing() {
    assert_eq!(aliased(&[]).0, AliasResult::MayAlias);
    let mut chain = AggregateAA::new();
    let aaqi = &mut AAQueryInfo::default();
    assert_eq!(chain.get_mod_ref_info(InstId(0), None, aaqi), ModRefInfo::ModRef);
    assert_eq!(chain.get_memory_effects(InstId(0), aaqi), MemoryEffects::unknown());
}

#[test]
fn mod_ref_answers_meet_and_stop_at_none() {
    let mut chain = AggregateAA::new();
    let (reads, _) = Fixed::new(AliasResult::MayAlias, ModRefInfo::Ref);
    let (writes, _) = Fixed::new(AliasResult::MayAlias, ModRefInfo::Mod);
    let (never, asked) = Fixed::new(AliasResult::MayAlias, ModRefInfo::NoModRef);
    let (last, after) = Fixed::new(AliasResult::MayAlias, ModRefInfo::ModRef);
    let aaqi = &mut AAQueryInfo::default();
    chain.add_aa_result(reads);
    assert_eq!(chain.get_mod_ref_info(InstId(0), None, aaqi), ModRefInfo::Ref);
    chain.add_aa_result(writes);
    assert_eq!(chain.get_mod_ref_info(InstId(0), None, aaqi), ModRefInfo::NoModRef, "Ref meets Mod");
    chain.add_aa_result(never);
    chain.add_aa_result(last);
    // `never` sits behind a provider that already left nothing: not asked.
    assert_eq!((asked.get(), after.get()), (0, 0));
}

#[test]
fn memory_effects_meet_by_location() {
    let mut chain = AggregateAA::new();
    let (reads, _) = Fixed::new(AliasResult::MayAlias, ModRefInfo::Ref);
    chain.add_aa_result(reads);
    let effects = chain.get_memory_effects(InstId(0), &mut AAQueryInfo::default());
    assert_eq!(effects.mod_ref(IRMemLocation::ArgMem), ModRefInfo::Ref);
    assert_eq!(effects.mod_ref(IRMemLocation::Other), ModRefInfo::Ref);
}

/// A call to a location the mask says is constant modified it: the meet of the
/// providers' answers stayed `ModRef`, so EarlyCSE kept the second load of a
/// constant table after a call (QCport `config.c`, +34 B over three modules).
#[test]
fn a_call_cannot_modify_what_the_mask_says_is_constant() {
    let (mut provider, _) = Fixed::new(AliasResult::MayAlias, ModRefInfo::ModRef);
    provider.mask = ModRefInfo::Ref;
    let mut chain = AggregateAA::new();
    chain.add_aa_result(provider);
    let loc = MemoryLocation::new(Operand::Value(ValueId(0)), LocationSize::Precise(2));
    let aaqi = &mut AAQueryInfo::default();
    assert_eq!(chain.get_mod_ref_info(InstId(0), Some(&loc), aaqi), ModRefInfo::Ref);
}
