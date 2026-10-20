//! Aggregates held as registers: `extractvalue`, `insertvalue`, and their loads
//! and stores, member by member at the offsets of the layout.

use llrm_mir::{Instruction, Type, TypeId};

use super::val::{AddrExpr, Val};
use super::{Selected, Selector, Src, refuse};

impl Selector<'_> {
    fn member<'v>(
        value: &'v Val,
        path: &[u32],
    ) -> Selected<&'v Val> {
        let mut at = value;
        for &index in path {
            at = match at {
                Val::Agg(members) => members.get(index as usize).ok_or("a member past the end")?,
                _ => return refuse("a member of a scalar"),
            };
        }
        Ok(at)
    }

    pub(super) fn extract_value(
        &mut self,
        instruction: &Instruction,
        path: &[u32],
    ) -> Selected<()> {
        let Src::Val(aggregate) = self.src(instruction.operands[0])? else { return refuse("a member of a constant") };
        let found = Self::member(&aggregate, path)?.clone();
        self.define(instruction.result, Src::Val(found))
    }

    pub(super) fn insert_value(
        &mut self,
        instruction: &Instruction,
        path: &[u32],
    ) -> Selected<()> {
        let Src::Val(mut aggregate) = self.src(instruction.operands[0])? else {
            return refuse("an insert into a constant");
        };
        let inserted = self.src(instruction.operands[1])?;
        let inserted = self.materialize(inserted)?;
        let mut at = &mut aggregate;
        for &index in path {
            at = match at {
                Val::Agg(members) => members.get_mut(index as usize).ok_or("a member past the end")?,
                _ => return refuse("a member of a scalar"),
            };
        }
        *at = inserted;
        self.define(instruction.result, Src::Val(aggregate))
    }

    /// The offsets of the members of an array or struct type, and their types.
    pub(crate) fn members(
        &self,
        ty: TypeId,
    ) -> Selected<Vec<(u64, TypeId)>> {
        let types = self.types();
        Ok(match types.get(ty) {
            Type::Array { element, count } => {
                let stride = self.layout.alloc_size(types, *element);
                (0..*count).map(|at| (at * stride, *element)).collect()
            }
            _ => {
                let fields = types.fields(ty).ok_or("an opaque struct")?.to_vec();
                let (_, offsets) = self.layout.struct_layout(types, ty);
                offsets.into_iter().zip(fields).collect()
            }
        })
    }

    /// Whether `ty` is an array or a struct.
    pub(crate) fn is_aggregate(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(
            self.ty(ty),
            Type::Array { .. } | Type::Struct { .. } | Type::Named(_)
        )
    }

    /// A value of type `ty` read from `address`.
    pub(crate) fn load_typed(
        &mut self,
        ty: TypeId,
        address: &AddrExpr,
    ) -> Selected<Val> {
        if !self.is_aggregate(ty) {
            return self.load_scalar(ty, address);
        }
        let mut out = Vec::new();
        for (offset, member) in self.members(ty)? {
            let at = AddrExpr { offset: address.offset + offset as i64, ..address.clone() };
            out.push(self.load_typed(member, &at)?);
        }
        Ok(Val::Agg(out))
    }

    /// `value`, of type `ty`, written to `address`.
    pub(crate) fn store_typed(
        &mut self,
        ty: TypeId,
        value: &Val,
        address: &AddrExpr,
    ) -> Selected<()> {
        if !self.is_aggregate(ty) {
            return self.store_scalar(ty, value.clone(), address);
        }
        let Val::Agg(values) = value else { return refuse("a scalar stored as an aggregate") };
        for ((offset, member), value) in self.members(ty)?.into_iter().zip(values) {
            let at = AddrExpr { offset: address.offset + offset as i64, ..address.clone() };
            self.store_typed(member, value, &at)?;
        }
        Ok(())
    }
}
