//! `ScopedNoAliasAA.cpp` at d1106deb71cc: two accesses are apart when, in some
//! scope domain, every scope one is in is named `!noalias` by the other.
//!
//! # How it works
//! `!alias.scope` lists the scopes an access is in and `!noalias` those it
//! promises not to alias. A scope is a node `!{self, domain, name}`. Accesses
//! `A` and `B` do not alias if, for some domain, all of `A`'s scopes in it are
//! among `B`'s `!noalias` ones, or the other way round. An access with either
//! list missing says nothing. A call or fence carries the lists on itself.
//! [`ScopedNoAliasAA`] is an [`AAResults`]; what it does not decide is the top
//! of the lattice, for [`crate::aggregate::AggregateAA`] to meet with the
//! others.
//!
//! # Example
//! A load in scope `!1` and a store with `!noalias !{!1}`: apart.
//!
//! ```text
//! %a = load i32, ptr %p, !alias.scope !2        !2 = !{!1}
//! store i32 0, ptr %q, !noalias !2              !1 = distinct !{!1, !0}
//! ```
//!
//! # Provides
//! [`ScopedNoAliasAA`], [`may_alias_in_scopes`].
//!
//! # Needs
//! [`crate::aa`] (`AAResults`, `AAInfo`).
//!
//! # See also
//! LLVM `ScopedNoAliasAA.cpp`, `inlinemetadata` and the inliner's
//! `AddAliasScopeMetadata`, which make these lists.

use std::collections::BTreeSet;

use llrm_mir::module::{InstId, MetadataId, MetadataNode, MetadataOperand};
use llrm_mir::opcode::{AtomicKind, Opcode};

use crate::aa::{AAQueryInfo, AAResults, AliasResult, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::memory::Unit;

fn node(
    metadata: &[MetadataNode],
    id: MetadataId,
) -> Option<&MetadataNode> {
    metadata.get(id.0 as usize)
}

/// `AliasScopeNode::getDomain`: the second operand, if it is a node.
fn domain(
    metadata: &[MetadataNode],
    scope: MetadataId,
) -> Option<MetadataId> {
    match node(metadata, scope)?.operands.get(1)? {
        MetadataOperand::Node(domain) => Some(*domain),
        _ => None,
    }
}

/// `collectMDInDomain`.
fn in_domain(
    metadata: &[MetadataNode],
    list: MetadataId,
    wanted: MetadataId,
) -> BTreeSet<MetadataId> {
    let Some(list) = node(metadata, list) else { return BTreeSet::new() };
    list.operands
        .iter()
        .filter_map(|one| match one {
            MetadataOperand::Node(one) if domain(metadata, *one) == Some(wanted) => Some(*one),
            _ => None,
        })
        .collect()
}

/// `ScopedNoAliasAAResult::mayAliasInScopes`: unless, for some domain of
/// `noalias`, its scopes there include all of `scopes`' there (and some).
pub fn may_alias_in_scopes(
    metadata: &[MetadataNode],
    scopes: Option<MetadataId>,
    noalias: Option<MetadataId>,
) -> bool {
    let (Some(scopes), Some(noalias)) = (scopes, noalias) else { return true };
    let Some(list) = node(metadata, noalias) else { return true };
    let domains: BTreeSet<MetadataId> = list
        .operands
        .iter()
        .filter_map(|one| match one {
            MetadataOperand::Node(one) => domain(metadata, *one),
            _ => None,
        })
        .collect();
    for wanted in domains {
        let ours = in_domain(metadata, scopes, wanted);
        if ours.is_empty() {
            continue;
        }
        // To not alias, all of `ours` must be in `noalias`.
        if ours.is_subset(&in_domain(metadata, noalias, wanted)) {
            return false;
        }
    }
    true
}

fn attached(
    unit: &Unit,
    inst: InstId,
    kind: &str,
) -> Option<MetadataId> {
    unit.function.instruction(inst).metadata.iter().find(|(one, _)| one == kind).map(|&(_, node)| node)
}

/// Whether `inst` carries its scope lists on itself: a call or a fence.
fn carries_lists(opcode: &Opcode) -> bool {
    match opcode {
        Opcode::Call(_) | Opcode::Invoke(_) => true,
        Opcode::Atomic(info) => matches!(info.kind, AtomicKind::Fence),
        _ => false,
    }
}

/// `ScopedNoAliasAAResult`.
pub struct ScopedNoAliasAA<'a> {
    unit: &'a Unit<'a>,
}

impl<'a> ScopedNoAliasAA<'a> {
    pub fn new(unit: &'a Unit<'a>) -> Self {
        Self { unit }
    }

    fn apart(
        &self,
        scopes: Option<MetadataId>,
        noalias: Option<MetadataId>,
        other_scopes: Option<MetadataId>,
        other_noalias: Option<MetadataId>,
    ) -> bool {
        !may_alias_in_scopes(self.unit.metadata, scopes, other_noalias)
            || !may_alias_in_scopes(self.unit.metadata, other_scopes, noalias)
    }
}

impl AAResults for ScopedNoAliasAA<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: Option<InstId>,
    ) -> AliasResult {
        if self.apart(a.aa_info.noalias_scope, a.aa_info.noalias, b.aa_info.noalias_scope, b.aa_info.noalias) {
            AliasResult::NoAlias
        } else {
            AliasResult::MayAlias
        }
    }

    fn get_mod_ref_info_mask(
        &mut self,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: bool,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_arg_mod_ref_info(
        &mut self,
        _: InstId,
        _: u32,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_memory_effects(
        &mut self,
        _: InstId,
        _: &mut AAQueryInfo,
    ) -> MemoryEffects {
        MemoryEffects::unknown()
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        let (Some(loc), true) = (loc, carries_lists(&self.unit.function.instruction(inst).opcode)) else {
            return ModRefInfo::ModRef;
        };
        let (scope, noalias) = (attached(self.unit, inst, "alias.scope"), attached(self.unit, inst, "noalias"));
        if self.apart(loc.aa_info.noalias_scope, loc.aa_info.noalias, scope, noalias) {
            ModRefInfo::NoModRef
        } else {
            ModRefInfo::ModRef
        }
    }

    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        let function = self.unit.function;
        if !carries_lists(&function.instruction(inst).opcode) || !carries_lists(&function.instruction(call).opcode) {
            return ModRefInfo::ModRef;
        }
        let first = (attached(self.unit, inst, "alias.scope"), attached(self.unit, inst, "noalias"));
        let second = (attached(self.unit, call, "alias.scope"), attached(self.unit, call, "noalias"));
        if self.apart(first.0, first.1, second.0, second.1) { ModRefInfo::NoModRef } else { ModRefInfo::ModRef }
    }

    fn call_captures_before(
        &mut self,
        _: InstId,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }
}

#[cfg(test)]
#[path = "scopednoaliasaa_tests.rs"]
mod tests;
