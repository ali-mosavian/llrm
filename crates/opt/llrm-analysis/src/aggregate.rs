//! `AAResults::addAAResult` of `AliasAnalysis.cpp` at d1106deb71cc: several
//! alias analyses asked as one.
//!
//! # How it works
//! An [`AggregateAA`] holds providers in the order they were added and is
//! itself an [`AAResults`]. A pair of locations is put to each in turn and the
//! first answer that is not `MayAlias` is the answer; none gives `MayAlias`.
//! What a call or instruction may do to memory is the meet of the providers'
//! answers (`ModRefInfo` and `MemoryEffects` intersect), and the walk stops at
//! the first answer that leaves nothing. A provider that knows little answers
//! the top of the lattice and costs the others nothing.
//!
//! A new analysis (type-based, scoped no-alias, globals) is a provider added to
//! the aggregate; nothing in a client or in another provider changes.
//!
//! DEVIATE G1: LLVM's `AAResults` also holds the `getModRefInfo(Instruction)`
//! dispatch by opcode and `callCapturesBefore`; here those are the trait's
//! methods, each provider answering them, and the aggregate meets the answers.
//! Exit: the dispatch moves into the aggregate when a second provider needs it.
//!
//! # Example
//! Two providers, the first unsure and the second sure the pointers are apart:
//! the aggregate says `NoAlias`. Asked of `[no, unsure]` it says `NoAlias` as
//! well (the first answer that is not `MayAlias`); asked of no providers,
//! `MayAlias`.
//!
//! ```text
//! aggregate [unsure, apart]   alias(a, b) = NoAlias
//! aggregate [apart, must]     alias(a, b) = NoAlias   (the first decides)
//! aggregate []                alias(a, b) = MayAlias
//! ```
//!
//! From unit test `the_first_answer_that_is_not_may_alias_decides` in
//! `aggregate_tests.rs`.
//!
//! # Provides
//! [`AggregateAA`].
//!
//! # See also
//! LLVM `AliasAnalysis.cpp`, [`crate::aa`], [`crate::basicaa`].

use llrm_mir::module::InstId;

use crate::aa::{AAQueryInfo, AAResults, AliasResult, MemoryEffects, MemoryLocation, ModRefInfo};

/// `AAResults` as a list of analyses (`addAAResult`). The providers are boxed
/// by default; a fixed set of concrete ones (`DefaultAA`) is held without a
/// box, which also leaves the aggregate without drop glue over its borrows.
pub struct AggregateAA<'a, R = Box<dyn AAResults + 'a>> {
    results: Vec<R>,
    /// The body the loads and stores asked of are read in.
    unit: Option<&'a crate::memory::Unit<'a>>,
}

impl<R> Default for AggregateAA<'_, R> {
    fn default() -> Self {
        Self { results: Vec::new(), unit: None }
    }
}

impl<'a> AggregateAA<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `self` answering `getModRefInfo(Load or Store, Loc)` itself, from its
    /// own `alias` (AliasAnalysis.cpp), for the instructions of `unit`.
    #[must_use]
    pub fn with_unit(
        self,
        unit: &'a crate::memory::Unit<'a>,
    ) -> Self {
        Self { unit: Some(unit), ..self }
    }

    /// `addAAResult`: asked after those already added.
    pub fn add_aa_result(
        &mut self,
        result: impl AAResults + 'a,
    ) {
        self.results.push(Box::new(result));
    }
}

impl<'a> AggregateAA<'a, DefaultAA<'a>> {
    /// The pin's default pipeline over `unit` (`buildDefaultAAPipeline`):
    /// BasicAA first, so that type-based answers are asked only of what it
    /// leaves MayAlias, then scoped no-alias, type-based and, where the module
    /// analysis is held, GlobalsAA.
    pub fn default_pipeline(unit: &'a crate::memory::Unit<'a>) -> Self {
        let mut stack = Self { results: Vec::with_capacity(4), unit: Some(unit) };
        stack.results.push(DefaultAA::Basic(crate::basicaa::BasicAA::new(unit)));
        stack.results.push(DefaultAA::ScopedNoAlias(crate::scopednoaliasaa::ScopedNoAliasAA::new(unit)));
        stack.results.push(DefaultAA::TypeBased(crate::typebasedaa::TypeBasedAA::new(unit)));
        if let Some(info) = unit.globals_mr {
            stack.results.push(DefaultAA::Globals(crate::globalsmodref::GlobalsAAResult::new(unit, info)));
        }
        stack
    }
}

/// The providers of the default pipeline, by value.
pub enum DefaultAA<'a> {
    Basic(crate::basicaa::BasicAA<'a>),
    ScopedNoAlias(crate::scopednoaliasaa::ScopedNoAliasAA<'a>),
    TypeBased(crate::typebasedaa::TypeBasedAA<'a>),
    Globals(crate::globalsmodref::GlobalsAAResult<'a>),
}

macro_rules! each {
    ($self:ident, $one:ident => $call:expr) => {
        match $self {
            DefaultAA::Basic($one) => $call,
            DefaultAA::ScopedNoAlias($one) => $call,
            DefaultAA::TypeBased($one) => $call,
            DefaultAA::Globals($one) => $call,
        }
    };
}

impl AAResults for DefaultAA<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ctx: Option<InstId>,
    ) -> AliasResult {
        each!(self, one => one.alias(a, b, aaqi, ctx))
    }

    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        each!(self, one => one.get_mod_ref_info_mask(loc, aaqi, ignore_locals))
    }

    fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo {
        each!(self, one => one.get_arg_mod_ref_info(call, arg))
    }

    fn get_memory_effects(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> MemoryEffects {
        each!(self, one => one.get_memory_effects(call, aaqi))
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        each!(self, one => one.get_mod_ref_info(inst, loc, aaqi))
    }

    fn get_mod_ref_info_with(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
        others: &mut dyn FnMut(&MemoryLocation, &MemoryLocation, &mut AAQueryInfo, Option<InstId>) -> AliasResult,
    ) -> ModRefInfo {
        each!(self, one => one.get_mod_ref_info_with(inst, loc, aaqi, others))
    }

    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        each!(self, one => one.get_mod_ref_info_call(inst, call, aaqi))
    }

    fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        each!(self, one => one.call_captures_before(inst, loc, aaqi))
    }
}

impl AAResults for Box<dyn AAResults + '_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ctx: Option<InstId>,
    ) -> AliasResult {
        (**self).alias(a, b, aaqi, ctx)
    }

    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        (**self).get_mod_ref_info_mask(loc, aaqi, ignore_locals)
    }

    fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo {
        (**self).get_arg_mod_ref_info(call, arg)
    }

    fn get_memory_effects(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> MemoryEffects {
        (**self).get_memory_effects(call, aaqi)
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        (**self).get_mod_ref_info(inst, loc, aaqi)
    }

    fn get_mod_ref_info_with(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
        others: &mut dyn FnMut(&MemoryLocation, &MemoryLocation, &mut AAQueryInfo, Option<InstId>) -> AliasResult,
    ) -> ModRefInfo {
        (**self).get_mod_ref_info_with(inst, loc, aaqi, others)
    }

    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        (**self).get_mod_ref_info_call(inst, call, aaqi)
    }

    fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        (**self).call_captures_before(inst, loc, aaqi)
    }
}

impl<R: AAResults> AAResults for AggregateAA<'_, R> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ctx: Option<InstId>,
    ) -> AliasResult {
        for result in &mut self.results {
            let answer = result.alias(a, b, aaqi, ctx);
            if answer != AliasResult::MayAlias {
                return answer;
            }
        }
        AliasResult::MayAlias
    }

    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        meet(&mut self.results, |result| result.get_mod_ref_info_mask(loc, aaqi, ignore_locals))
    }

    fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo {
        meet(&mut self.results, |result| result.get_arg_mod_ref_info(call, arg))
    }

    fn get_memory_effects(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> MemoryEffects {
        let mut effects = MemoryEffects::unknown();
        for result in &mut self.results {
            effects = effects.intersect(result.get_memory_effects(call, aaqi));
            if effects.does_not_access_memory() {
                break;
            }
        }
        effects
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        // `AAResults::getModRefInfo(Load, Loc)` and `(Store, Loc)`: the
        // aggregate's own `alias` decides, then a store to constant memory
        // modifies nothing.
        if let (Some(unit), Some(loc)) = (self.unit, loc) {
            use llrm_mir::opcode::Opcode;
            match unit.function.instruction(inst).opcode {
                Opcode::Load { .. } => {
                    if let Some(own) = MemoryLocation::get(unit, inst)
                        && self.alias(&own, loc, aaqi, Some(inst)) == AliasResult::NoAlias
                    {
                        return ModRefInfo::NoModRef;
                    }
                    return ModRefInfo::Ref;
                }
                Opcode::Store { .. } => {
                    if let Some(own) = MemoryLocation::get(unit, inst) {
                        if self.alias(&own, loc, aaqi, Some(inst)) == AliasResult::NoAlias {
                            return ModRefInfo::NoModRef;
                        }
                        if !self.get_mod_ref_info_mask(loc, aaqi, false).is_mod_set() {
                            return ModRefInfo::NoModRef;
                        }
                    }
                    return ModRefInfo::Mod;
                }
                _ => {}
            }
        }
        // Each provider's arguments are asked of the others as well: LLVM's
        // `AAQI.AAR.alias`, the aggregate.
        let mut answer = ModRefInfo::ModRef;
        for at in 0..self.results.len() {
            let (before, rest) = self.results.split_at_mut(at);
            let (this, after) = rest.split_first_mut().expect("a provider");
            let mut others = |a: &MemoryLocation, b: &MemoryLocation, aaqi: &mut AAQueryInfo, ctx: Option<InstId>| {
                for other in before.iter_mut().chain(after.iter_mut()) {
                    let found = other.alias(a, b, aaqi, ctx);
                    if found != AliasResult::MayAlias {
                        return found;
                    }
                }
                AliasResult::MayAlias
            };
            answer = answer.intersect(this.get_mod_ref_info_with(inst, loc, aaqi, &mut others));
            if answer.is_no_mod_ref() {
                break;
            }
        }
        // Apply the ModRef mask (`AAResults::getModRefInfo(CB, Loc)`): a call
        // cannot modify a constant location.
        if !answer.is_no_mod_ref()
            && let Some(loc) = loc
        {
            answer = answer.intersect(self.get_mod_ref_info_mask(loc, aaqi, false));
        }
        answer
    }

    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        meet(&mut self.results, |result| result.get_mod_ref_info_call(inst, call, aaqi))
    }

    fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        meet(&mut self.results, |result| result.call_captures_before(inst, loc, aaqi))
    }
}

/// The meet of the providers' answers from the top, stopping at the first that
/// leaves nothing.
fn meet<R: AAResults>(
    results: &mut [R],
    mut ask: impl FnMut(&mut R) -> ModRefInfo,
) -> ModRefInfo {
    let mut answer = ModRefInfo::ModRef;
    for result in results {
        answer = answer.intersect(ask(result));
        if answer.is_no_mod_ref() {
            break;
        }
    }
    answer
}

#[cfg(test)]
#[path = "aggregate_tests.rs"]
mod tests;
