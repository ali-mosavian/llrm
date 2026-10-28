//! `TypeBasedAliasAnalysis.cpp` at d1106deb71cc: two accesses whose `!tbaa`
//! types are unrelated cannot alias.
//!
//! # How it works
//! A tag names a base type, an access type and an offset; a type node names
//! its parent, or its fields, up to a root. Two accesses may alias unless their
//! access types hang under one root and neither access may be to a subobject of
//! the other (`matchAccessTags`, `getLeastCommonType`,
//! `mayBeAccessToSubobjectOf`: the path from one base type, by offset, reaches
//! the other's base type). A type of another root, or an access with no tag,
//! says nothing. [`TypeBasedAA`] is an [`AAResults`]: `alias` answers `NoAlias`
//! for such a pair and `MayAlias` otherwise; a tag marked immutable makes its
//! location constant memory (`getModRefInfoMask`); a call that carries a
//! `!tbaa` tag of its own does not touch a location of an unrelated type
//! (`getModRefInfo(Call, Loc)`). Everything else is the top of the lattice, for
//! [`crate::aggregate::AggregateAA`] to meet with the other providers.
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

use llrm_mir::context::ConstantId;
use llrm_mir::module::{InstId, MetadataId, MetadataNode, MetadataOperand};
use llrm_mir::opcode::Opcode;

use crate::aa::{AAInfo, AAQueryInfo, AAResults, AliasResult, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::memory::Unit;

/// `TBAAStructTagNode` and `TBAAStructTypeNode` over the module's metadata.
/// Every accessor is `None` where LLVM's `cast<>` would assert: the answer
/// then is `MayAlias`.
struct Tags<'a> {
    metadata: &'a [MetadataNode],
    /// The value of an integer constant operand.
    offset: &'a dyn Fn(ConstantId) -> Option<u64>,
}

/// A tag: base type, access type, offset, and whether it has the size field.
struct Tag {
    base: MetadataId,
    access: MetadataId,
    offset: u64,
    new_format: bool,
}

impl Tags<'_> {
    fn operands(
        &self,
        node: MetadataId,
    ) -> Option<&[MetadataOperand]> {
        Some(&self.metadata.get(node.0 as usize)?.operands)
    }

    fn node_at(
        &self,
        node: MetadataId,
        index: usize,
    ) -> Option<MetadataId> {
        match self.operands(node)?.get(index)? {
            MetadataOperand::Node(found) => Some(*found),
            _ => None,
        }
    }

    fn constant_at(
        &self,
        node: MetadataId,
        index: usize,
    ) -> Option<u64> {
        match self.operands(node)?.get(index)? {
            MetadataOperand::Constant(id) => (self.offset)(*id),
            _ => None,
        }
    }

    /// `isNewFormatTypeNode`: the size-aware format, whose first operand is a node.
    fn type_is_new_format(
        &self,
        ty: MetadataId,
    ) -> bool {
        self.operands(ty).is_some_and(|ops| ops.len() >= 3 && matches!(ops[0], MetadataOperand::Node(_)))
    }

    fn tag(
        &self,
        tag: MetadataId,
    ) -> Option<Tag> {
        let (base, access) = (self.node_at(tag, 0)?, self.node_at(tag, 1)?);
        Some(Tag {
            base,
            access,
            offset: self.constant_at(tag, 2)?,
            new_format: self.operands(tag)?.len() >= 4 && self.type_is_new_format(access),
        })
    }

    /// `TBAANode::getParent`.
    fn parent(
        &self,
        ty: MetadataId,
    ) -> Option<MetadataId> {
        if self.type_is_new_format(ty) { self.node_at(ty, 0) } else { self.node_at(ty, 1) }
    }

    /// `getLeastCommonType`: the deepest type both paths to the root share;
    /// `None` for two roots, or a cycle.
    fn least_common_type(
        &self,
        a: MetadataId,
        b: MetadataId,
    ) -> Option<MetadataId> {
        if a == b {
            return Some(a);
        }
        let path = |from: MetadataId| -> Option<Vec<MetadataId>> {
            let mut path = vec![from];
            while let Some(next) = self.parent(*path.last()?) {
                if path.contains(&next) {
                    return None;
                }
                path.push(next);
            }
            Some(path)
        };
        let (path_a, path_b) = (path(a)?, path(b)?);
        path_a.iter().rev().zip(path_b.iter().rev()).take_while(|(x, y)| x == y).last().map(|(common, _)| *common)
    }

    /// `TBAAStructTypeNode::getNumFields`.
    fn field_count(
        &self,
        ty: MetadataId,
    ) -> usize {
        let new = self.type_is_new_format(ty);
        let (first, per) = if new { (3, 3) } else { (1, 2) };
        self.operands(ty).map_or(0, |ops| ops.len().saturating_sub(first) / per)
    }

    /// `TBAAStructTypeNode::getField`: the field type that holds `offset`,
    /// and `offset` made relative to it.
    fn field(
        &self,
        ty: MetadataId,
        offset: &mut u64,
    ) -> Option<MetadataId> {
        let new = self.type_is_new_format(ty);
        let count = self.operands(ty)?.len();
        let (first, per) = if new { (3, 3) } else { (1, 2) };
        if new {
            // New-format root and scalar type nodes have no fields.
            if count < 6 {
                return None;
            }
        } else {
            // Parent can be omitted for the root node.
            if count < 2 {
                return None;
            }
            // Fast path for a scalar type node and a struct type node with a
            // single field.
            if count <= 3 {
                let current = if count == 2 { 0 } else { self.constant_at(ty, 2)? };
                *offset = offset.wrapping_sub(current);
                return self.node_at(ty, 1);
            }
        }
        // Assume the offsets are in order. We return the previous field if the
        // current offset is bigger than the given offset.
        let mut chosen = None;
        for index in (first..count).step_by(per) {
            if self.constant_at(ty, index + 1)? > *offset {
                // The offset must match a field.
                chosen = Some(index.checked_sub(per).filter(|&before| before >= first)?);
                break;
            }
        }
        // Move along the last field.
        let index = chosen.unwrap_or(count - per);
        *offset = offset.wrapping_sub(self.constant_at(ty, index + 1)?);
        self.node_at(ty, index)
    }

    /// `hasField`: `field` is a field of `base`, or of one of its fields.
    fn has_field(
        &self,
        base: MetadataId,
        field: MetadataId,
        depth: usize,
    ) -> bool {
        if depth > self.metadata.len() {
            return true;
        }
        let new = self.type_is_new_format(base);
        let (first, per) = if new { (3, 3) } else { (1, 2) };
        (0..self.field_count(base)).filter_map(|i| self.node_at(base, first + i * per)).any(|ty| {
            ty == field || self.has_field(ty, field, depth + 1)
        })
    }

    /// `mayBeAccessToSubobjectOf`: `None` where it returns false, else whether
    /// the accesses may alias (`MayAlias`).
    fn may_be_access_to_subobject_of(
        &self,
        base_tag: &Tag,
        subobject_tag: &Tag,
        common_type: MetadataId,
    ) -> Option<bool> {
        // If the base object is of the least common type, then this may be an
        // access to its subobject.
        if base_tag.access == base_tag.base && base_tag.access == common_type {
            return Some(true);
        }
        // If the access to the base object is through a field of the
        // subobject's type, then this may be an access to that field. Start
        // from the base type, follow the edge with the correct offset in the
        // type DAG and adjust the offset until we reach the field type or the
        // access type.
        let new_format = base_tag.new_format;
        let mut base_type = Some(base_tag.base);
        let mut offset_in_base = base_tag.offset;
        for _ in 0..=self.metadata.len() {
            // In the old format there is no distinction between fields and
            // parent types, so we consider all nodes up to the root.
            let Some(current) = base_type else { break };
            if current == subobject_tag.base {
                let may_alias = offset_in_base == subobject_tag.offset
                    || current == base_tag.access
                    || subobject_tag.base == subobject_tag.access;
                return Some(may_alias);
            }
            // With new-format nodes we stop at the access type.
            if new_format && current == base_tag.access {
                break;
            }
            // Follow the edge with the correct offset. Offset will be
            // adjusted to be relative to the field type.
            base_type = self.field(current, &mut offset_in_base);
        }
        // If the base object has a direct or indirect field of the
        // subobject's type, then this may be an access to that field.
        if new_format && base_type.is_some_and(|current| self.has_field(current, subobject_tag.base, 0)) {
            return Some(true);
        }
        None
    }

    /// `matchAccessTags(A, B)`: whether accesses tagged `a` and `b` may alias.
    fn match_access_tags(
        &self,
        a: MetadataId,
        b: MetadataId,
    ) -> Option<bool> {
        let (tag_a, tag_b) = (self.tag(a)?, self.tag(b)?);
        // If the final access types have different roots, they're part of
        // different potentially unrelated type systems: be conservative.
        let Some(common_type) = self.least_common_type(tag_a.access, tag_b.access) else { return Some(true) };
        // If one of the accessed objects may be a subobject of the other, then
        // such accesses may alias.
        if let Some(may_alias) = self.may_be_access_to_subobject_of(&tag_a, &tag_b, common_type) {
            return Some(may_alias);
        }
        if let Some(may_alias) = self.may_be_access_to_subobject_of(&tag_b, &tag_a, common_type) {
            return Some(may_alias);
        }
        // Otherwise, we've proved there's no alias.
        Some(false)
    }
}

/// `matchAccessTags(A, B)`: whether accesses tagged `a` and `b` may alias.
/// `offset` reads an integer constant operand of a tag.
pub fn aliases(
    metadata: &[MetadataNode],
    offset: &dyn Fn(ConstantId) -> Option<u64>,
    a: Option<MetadataId>,
    b: Option<MetadataId>,
) -> bool {
    // Accesses with no TBAA information may alias with any other accesses.
    let (Some(a), Some(b)) = (a, b) else { return true };
    if a == b {
        return true;
    }
    Tags { metadata, offset }.match_access_tags(a, b).unwrap_or(true)
}

/// `TypeBasedAAResult`.
pub struct TypeBasedAA<'a> {
    unit: &'a Unit<'a>,
}

impl<'a> TypeBasedAA<'a> {
    pub fn new(unit: &'a Unit<'a>) -> Self {
        Self { unit }
    }

    /// `matchAccessTags` over the unit's metadata and constants.
    fn aliases(
        &self,
        a: Option<MetadataId>,
        b: Option<MetadataId>,
    ) -> bool {
        let offset = |id: ConstantId| {
            self.unit.int_constant(llrm_mir::module::Operand::Constant(id)).and_then(|bits| u64::try_from(bits).ok())
        };
        aliases(self.unit.metadata, &offset, a, b)
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
        if self.aliases(a.aa_info.tbaa, b.aa_info.tbaa) {
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
            (Some(location), Some(call)) if !self.aliases(Some(location), Some(call)) => {
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
