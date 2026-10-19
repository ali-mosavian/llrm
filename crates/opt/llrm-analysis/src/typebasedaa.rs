//! `TypeBasedAliasAnalysis.cpp` at d1106deb71cc: two accesses whose `!tbaa`
//! types are unrelated cannot alias.
//!
//! # How it works
//! A tag names an access type; a type node names its parent, up to a root.
//! Two accesses may alias unless their types hang under one root and neither
//! type is an ancestor of (or equal to) the other (`matchAccessTags`,
//! `getLeastCommonType`). A type of another root, or an access with no tag,
//! says nothing. [`TypeBasedAA`] is an [`AAResults`]: `alias` answers `NoAlias`
//! for such a pair and `MayAlias` otherwise; a tag marked immutable makes its
//! location constant memory (`getModRefInfoMask`); a call that carries a
//! `!tbaa` tag of its own does not touch a location of an unrelated type
//! (`getModRefInfo(Call, Loc)`). Everything else is the top of the lattice, for
//! [`crate::aggregate::AggregateAA`] to meet with the other providers.
//!
//! DEVIATE F1: only scalar tags are understood: base type equal to the access
//! type, offset 0, and a type node of a name, a parent and an offset of 0.
//! The C front end emits only these, as clang does for C. A struct-path tag
//! or a type node with fields answers `MayAlias`, and a test pins that.
//! Exit: a front end that emits struct-path tags.
//!
//! # Example
//! An `int` and a `float` under one root: apart. An `int` and the
//! `omnipotent char` that is its parent: may alias.
//!
//! ```text
//! !0 = !{!"Simple C/C++ TBAA"}
//! !1 = !{!"omnipotent char", !0, i64 0}
//! !2 = !{!"int4", !1, i64 0}      !3 = !{!"float4", !1, i64 0}
//! load int4  vs load float4   NoAlias
//! load int4  vs load char     MayAlias   (char is int4's ancestor)
//! ```
//!
//! From unit test `unrelated_scalar_types_are_apart_and_an_ancestor_is_not` in
//! `typebasedaa_tests.rs`.
//!
//! # Provides
//! [`TypeBasedAA`], [`aliases`].
//!
//! # Needs
//! [`crate::aa`] (`AAResults`, `AAInfo`), [`crate::aggregate`].
//!
//! # See also
//! LLVM `TypeBasedAliasAnalysis.cpp`, `llrm_mir::tbaa`.

use llrm_mir::module::{InstId, MetadataId, MetadataNode, MetadataOperand};
use llrm_mir::opcode::Opcode;

use crate::aa::{AAInfo, AAQueryInfo, AAResults, AliasResult, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::memory::Unit;

/// The type a scalar tag names, none for any other shape of tag.
fn scalar_access_type(
    metadata: &[MetadataNode],
    tag: MetadataId,
) -> Option<MetadataId> {
    let operands = &metadata.get(tag.0 as usize)?.operands;
    match operands.as_slice() {
        [MetadataOperand::Node(base), MetadataOperand::Node(access), MetadataOperand::Constant(_), ..]
            if base == access =>
        {
            Some(*access)
        }
        _ => None,
    }
}

/// The parent of a scalar type node: a name, a parent and an offset. None at
/// the root (a name alone), and `Err` for a node with fields.
fn parent(
    metadata: &[MetadataNode],
    ty: MetadataId,
) -> Result<Option<MetadataId>, ()> {
    let operands = &metadata.get(ty.0 as usize).ok_or(())?.operands;
    match operands.as_slice() {
        [MetadataOperand::String(_)] => Ok(None),
        [MetadataOperand::String(_), MetadataOperand::Node(parent)] => Ok(Some(*parent)),
        [MetadataOperand::String(_), MetadataOperand::Node(parent), MetadataOperand::Constant(_)] => Ok(Some(*parent)),
        _ => Err(()),
    }
}

/// Whether `ancestor` is `ty` or above it, and the root of `ty`. `Err` where
/// the chain leaves what is understood (a node with fields, a cycle).
fn climb(
    metadata: &[MetadataNode],
    ty: MetadataId,
    ancestor: MetadataId,
) -> Result<(bool, MetadataId), ()> {
    let (mut at, mut found) = (ty, false);
    // A chain is no longer than the module's metadata.
    for _ in 0..=metadata.len() {
        found |= at == ancestor;
        match parent(metadata, at)? {
            Some(next) => at = next,
            None => return Ok((found, at)),
        }
    }
    Err(())
}

/// `matchAccessTags(A, B)`: whether accesses tagged `a` and `b` may alias.
pub fn aliases(
    metadata: &[MetadataNode],
    a: Option<MetadataId>,
    b: Option<MetadataId>,
) -> bool {
    let (Some(a), Some(b)) = (a, b) else { return true };
    if a == b {
        return true;
    }
    let (Some(ty_a), Some(ty_b)) = (scalar_access_type(metadata, a), scalar_access_type(metadata, b)) else {
        return true;
    };
    let (Ok((a_has_b, root_a)), Ok((b_has_a, root_b))) = (climb(metadata, ty_a, ty_b), climb(metadata, ty_b, ty_a))
    else {
        return true;
    };
    // Types of different roots are unrelated type systems: be conservative.
    root_a != root_b || a_has_b || b_has_a
}

/// `TypeBasedAAResult`.
pub struct TypeBasedAA<'a> {
    unit: &'a Unit<'a>,
}

impl<'a> TypeBasedAA<'a> {
    pub fn new(unit: &'a Unit<'a>) -> Self {
        Self { unit }
    }

    /// `TBAAStructTagNode::isTypeImmutable`: the tag's fourth operand is 1.
    fn immutable(
        &self,
        tag: MetadataId,
    ) -> bool {
        match self.unit.metadata.get(tag.0 as usize).and_then(|node| node.operands.get(3)) {
            Some(MetadataOperand::Constant(id)) => {
                self.unit.int_constant(llrm_mir::module::Operand::Constant(*id)).is_some_and(|bits| bits & 1 == 1)
            }
            _ => false,
        }
    }
}

impl AAResults for TypeBasedAA<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: Option<InstId>,
    ) -> AliasResult {
        if aliases(self.unit.metadata, a.aa_info.tbaa, b.aa_info.tbaa) {
            AliasResult::MayAlias
        } else {
            AliasResult::NoAlias
        }
    }

    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: bool,
    ) -> ModRefInfo {
        // An immutable type is constant memory.
        match loc.aa_info.tbaa {
            Some(tag) if self.immutable(tag) => ModRefInfo::NoModRef,
            _ => ModRefInfo::ModRef,
        }
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
        let op = self.unit.function.instruction(inst);
        if !matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            return ModRefInfo::ModRef;
        }
        // A call with a tag of its own, against a location of an unrelated
        // type.
        let call = AAInfo::of(op).tbaa;
        match (loc.and_then(|loc| loc.aa_info.tbaa), call) {
            (Some(location), Some(call)) if !aliases(self.unit.metadata, Some(location), Some(call)) => {
                ModRefInfo::NoModRef
            }
            _ => ModRefInfo::ModRef,
        }
    }

    fn get_mod_ref_info_call(
        &mut self,
        _: InstId,
        _: InstId,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
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
#[path = "typebasedaa_tests.rs"]
mod tests;
