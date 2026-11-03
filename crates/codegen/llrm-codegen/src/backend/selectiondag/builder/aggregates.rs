//! First-class aggregates (`ComputeValueVTs`): a struct or array value is its
//! scalar leaves, in memory order, as the results of one `MERGE_VALUES` node.
//! `insertvalue` and `extractvalue` replace and pick a run of them; a
//! constant, a `select`, a `load` and a `store` of an aggregate act on each.
//!
//! An aggregate that crosses a block (a phi, a value read in another block),
//! is passed to a call or returned is not built yet: the register export
//! (`FunctionLoweringInfo`) and the convention count one register per value.

use llrm_mir::context::{ConstantId, ConstantKind};
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::types::{Type, TypeId};

use super::super::dag::SDValue;
use super::super::isd::ISD;
use super::super::mvt::{EVT, MVT};
use super::{Builder, Refused};

/// More leaves than this are not flattened: a large array is memory, not
/// a value.
const MAX_LEAVES: usize = 32;

impl Builder<'_> {
    pub(super) fn is_aggregate(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(
            self.module.context.types.get(ty),
            Type::Struct { .. } | Type::Array { .. } | Type::Named(_)
        )
    }

    /// The scalar leaves of `ty` with their byte offsets (`ComputeValueVTs`).
    pub(super) fn leaves(
        &self,
        ty: TypeId,
    ) -> Result<Vec<(TypeId, u64)>, Refused> {
        let mut out = Vec::new();
        self.collect_leaves(ty, 0, &mut out)?;
        if out.is_empty() {
            return Err(Refused("an aggregate with no members"));
        }
        Ok(out)
    }

    fn collect_leaves(
        &self,
        ty: TypeId,
        base: u64,
        out: &mut Vec<(TypeId, u64)>,
    ) -> Result<(), Refused> {
        let types = &self.module.context.types;
        match types.get(ty) {
            Type::Struct { .. } | Type::Named(_) => {
                let fields = types.fields(ty).ok_or(Refused("an opaque struct"))?.to_vec();
                let (_, offsets) = self.layout.struct_layout(types, ty);
                for (field, offset) in fields.into_iter().zip(offsets) {
                    self.collect_leaves(field, base + offset, out)?;
                }
            }
            Type::Array { element, count } => {
                let size = self.layout.alloc_size(types, *element);
                for at in 0..*count {
                    if out.len() > MAX_LEAVES {
                        break;
                    }
                    self.collect_leaves(*element, base + at * size, out)?;
                }
            }
            Type::Vector { .. } => return Err(Refused("vector value")),
            _ => {
                self.type_of(ty)?;
                out.push((ty, base));
            }
        }
        if out.len() > MAX_LEAVES {
            return Err(Refused("an aggregate with many members"));
        }
        Ok(())
    }

    /// The run of leaves `indices` names in `ty`: where it starts and how long.
    fn leaf_range(
        &self,
        ty: TypeId,
        indices: &[u32],
    ) -> Result<(usize, usize), Refused> {
        let types = &self.module.context.types;
        let (mut start, mut ty) = (0, ty);
        for &index in indices {
            match types.get(ty) {
                Type::Struct { .. } | Type::Named(_) => {
                    let fields = types.fields(ty).ok_or(Refused("an opaque struct"))?;
                    let field = *fields.get(index as usize).ok_or(Refused("bad struct field"))?;
                    for before in &fields[..index as usize] {
                        start += self.leaves(*before)?.len();
                    }
                    ty = field;
                }
                Type::Array { element, count } => {
                    if u64::from(index) >= *count {
                        return Err(Refused("bad array index"));
                    }
                    start += index as usize * self.leaves(*element)?.len();
                    ty = *element;
                }
                _ => return Err(Refused("a member of a scalar")),
            }
        }
        Ok((start, self.leaves(ty)?.len()))
    }

    /// `MERGE_VALUES` of the leaves: one result each.
    pub(super) fn merge(
        &mut self,
        parts: &[SDValue],
    ) -> SDValue {
        if let [only] = parts {
            return *only;
        }
        let types: Vec<EVT> = parts.iter().map(|part| self.dag.value_type(*part)).collect();
        self.dag.get_node_multi(ISD::MergeValues, &types, parts)
    }

    /// The leaves of a value built as `merge` builds them.
    fn split(
        whole: SDValue,
        count: usize,
    ) -> Vec<SDValue> {
        (0..count).map(|at| SDValue { node: whole.node, res: whole.res + at as u8 }).collect()
    }

    /// `operand`'s leaves: itself for a scalar.
    pub(super) fn value_parts(
        &mut self,
        operand: Operand,
    ) -> Result<Vec<SDValue>, Refused> {
        let ty = self.function.operand_type(&self.module.context, operand).ok_or(Refused("block operand"))?;
        if !self.is_aggregate(ty) {
            return Ok(vec![self.get_value(operand)?]);
        }
        let count = self.leaves(ty)?.len();
        match operand {
            Operand::Constant(id) => self.constant_parts(ty, id),
            _ => {
                let whole = self.get_value(operand)?;
                if whole.res as usize + count > self.dag.node(whole).values.len() {
                    return Err(Refused("an aggregate that is not a node's results"));
                }
                Ok(Self::split(whole, count))
            }
        }
    }

    /// An aggregate constant's leaves.
    pub(super) fn constant_parts(
        &mut self,
        ty: TypeId,
        id: ConstantId,
    ) -> Result<Vec<SDValue>, Refused> {
        let kind = self.module.context.get(id).kind.clone();
        match kind {
            ConstantKind::Aggregate(members) => {
                let mut out = Vec::new();
                for member in members {
                    let member_ty = self.module.context.get(member).ty;
                    if self.is_aggregate(member_ty) {
                        out.extend(self.constant_parts(member_ty, member)?);
                    } else {
                        out.push(self.constant_value(member)?);
                    }
                }
                Ok(out)
            }
            ConstantKind::Bytes(bytes) => {
                Ok(bytes.iter().map(|byte| self.dag.get_constant(u128::from(*byte), EVT::I8)).collect())
            }
            ConstantKind::Zero | ConstantKind::Null => {
                let mut out = Vec::new();
                for (leaf, _) in self.leaves(ty)? {
                    let vt = self.type_of(leaf)?;
                    out.push(if vt == MVT::Far.into() {
                        let zero = self.dag.get_constant(0, EVT::I16);
                        self.dag.get_node(ISD::BuildPair, vt, &[zero, zero])
                    } else if vt.is_floating_point() {
                        self.dag.get_constant_fp(0, vt)
                    } else {
                        self.dag.get_constant(0, vt)
                    });
                }
                Ok(out)
            }
            ConstantKind::Undef | ConstantKind::Poison => {
                let mut out = Vec::new();
                for (leaf, _) in self.leaves(ty)? {
                    let vt = self.type_of(leaf)?;
                    out.push(self.dag.get_undef(vt));
                }
                Ok(out)
            }
            _ => Err(Refused("aggregate constant")),
        }
    }

    /// An aggregate constant as one value.
    pub(super) fn aggregate_constant(
        &mut self,
        ty: TypeId,
        id: ConstantId,
    ) -> Result<SDValue, Refused> {
        let parts = self.constant_parts(ty, id)?;
        Ok(self.merge(&parts))
    }

    pub(super) fn visit_insert_value(
        &mut self,
        instruction: &Instruction,
        indices: &[u32],
    ) -> Result<(), Refused> {
        let (start, count) = self.leaf_range(instruction.ty, indices)?;
        let mut parts = self.value_parts(instruction.operands[0])?;
        let new = self.value_parts(instruction.operands[1])?;
        if new.len() != count || start + count > parts.len() {
            return Err(Refused("insertvalue of a member of another size"));
        }
        parts.splice(start..start + count, new);
        let merged = self.merge(&parts);
        self.set_value(instruction.result.expect("an insertvalue has a result"), merged);
        Ok(())
    }

    pub(super) fn visit_extract_value(
        &mut self,
        instruction: &Instruction,
        indices: &[u32],
    ) -> Result<(), Refused> {
        let ty = self
            .function
            .operand_type(&self.module.context, instruction.operands[0])
            .ok_or(Refused("block operand"))?;
        let (start, count) = self.leaf_range(ty, indices)?;
        let parts = self.value_parts(instruction.operands[0])?;
        if start + count > parts.len() {
            return Err(Refused("extractvalue of an aggregate that is not a node's results"));
        }
        let picked = self.merge(&parts[start..start + count]);
        self.set_value(instruction.result.expect("an extractvalue has a result"), picked);
        Ok(())
    }

    /// `select` of aggregates: one select on the condition for each leaf.
    pub(super) fn visit_aggregate_select(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), Refused> {
        let cond = self.get_value(instruction.operands[0])?;
        let then = self.value_parts(instruction.operands[1])?;
        let other = self.value_parts(instruction.operands[2])?;
        if then.len() != other.len() {
            return Err(Refused("select of aggregates of different shapes"));
        }
        let mut parts = Vec::new();
        for (a, b) in then.into_iter().zip(other) {
            let vt = self.dag.value_type(a);
            parts.push(self.dag.get_node(ISD::Select, vt, &[cond, a, b]));
        }
        let merged = self.merge(&parts);
        self.set_value(instruction.result.expect("a select has a result"), merged);
        Ok(())
    }

    /// The alignment of the leaf at `offset` of an aggregate aligned `align`.
    fn leaf_align(
        align: u64,
        offset: u64,
    ) -> u64 {
        if offset == 0 { align } else { align.min(offset & offset.wrapping_neg()) }
    }

    /// `load` of an aggregate: a load for each leaf.
    pub(super) fn visit_aggregate_load(
        &mut self,
        instruction: &Instruction,
        align: Option<u64>,
        volatile: bool,
    ) -> Result<(), Refused> {
        let leaves = self.leaves(instruction.ty)?;
        let whole = align.unwrap_or_else(|| self.layout.align(&self.module.context.types, instruction.ty));
        let ptr = self.get_value(instruction.operands[0])?;
        let root = if volatile { self.root() } else { self.dag.get_root() };
        let (mut loads, mut chains) = (Vec::new(), Vec::new());
        for (leaf, offset) in leaves {
            let mut info =
                self.mem_info(leaf, Some(Self::leaf_align(whole, offset)), volatile, instruction.operands[0])?;
            if offset != 0 {
                info.base = super::super::dag::MemBase::Unknown;
            }
            let address = self.dag.get_object_ptr_offset(ptr, offset as i64);
            let load = self.dag.get_load(info.mem_vt, root, address, info);
            chains.push(SDValue { node: load.node, res: 1 });
            loads.push(load);
        }
        let token = self.dag.get_token_factor(&chains);
        if volatile {
            self.dag.set_root(token);
        } else {
            self.pending_loads.push(token);
        }
        let merged = self.merge(&loads);
        self.set_value(instruction.result.expect("a load has a result"), merged);
        Ok(())
    }

    /// `store` of an aggregate: a store for each leaf.
    pub(super) fn visit_aggregate_store(
        &mut self,
        instruction: &Instruction,
        align: Option<u64>,
        volatile: bool,
    ) -> Result<(), Refused> {
        let ty = self
            .function
            .operand_type(&self.module.context, instruction.operands[0])
            .ok_or(Refused("block operand"))?;
        let leaves = self.leaves(ty)?;
        let whole = align.unwrap_or_else(|| self.layout.align(&self.module.context.types, ty));
        let parts = self.value_parts(instruction.operands[0])?;
        let ptr = self.get_value(instruction.operands[1])?;
        let root = if volatile { self.root() } else { self.memory_root() };
        let mut stores = Vec::new();
        for ((leaf, offset), part) in leaves.into_iter().zip(parts) {
            let mut info =
                self.mem_info(leaf, Some(Self::leaf_align(whole, offset)), volatile, instruction.operands[1])?;
            if offset != 0 {
                info.base = super::super::dag::MemBase::Unknown;
            }
            let address = self.dag.get_object_ptr_offset(ptr, offset as i64);
            stores.push(self.dag.get_store(root, part, address, info));
        }
        let node = self.dag.get_token_factor(&stores);
        self.dag.set_root(node);
        Ok(())
    }

    /// The scalar leaves of `ty` and how many registers each takes when it
    /// crosses a block: the aggregate's registers are one chain, leaf after
    /// leaf.
    pub(super) fn aggregate_registers(
        &self,
        ty: TypeId,
    ) -> Result<Vec<(EVT, usize)>, Refused> {
        self.leaves(ty)?
            .into_iter()
            .map(|(leaf, _)| {
                let vt = self.type_of(leaf)?;
                Ok((vt, self.register_parts(vt)?.1))
            })
            .collect()
    }

    /// `getCopyFromRegs` of an aggregate: each leaf from its run of the chain
    /// that starts at `first`.
    pub(super) fn copy_aggregate_from_regs(
        &mut self,
        first: u32,
        ty: TypeId,
    ) -> Result<SDValue, Refused> {
        let layout = self.aggregate_registers(ty)?;
        let total = layout.iter().map(|(_, parts)| parts).sum();
        let chain = self.info.part_regs(first, total);
        let (mut at, mut leaves) = (0, Vec::new());
        for (vt, parts) in layout {
            // A leaf's type is not kept here: unmarked would let the pack take
            // a pointer's halves, so it is marked as one.
            leaves.push(self.copy_from_reg(chain[at], vt, true)?);
            at += parts;
        }
        Ok(self.merge(&leaves))
    }

    /// `CopyValueToVirtualRegister` of an aggregate: each leaf into its run of
    /// the chain that starts at `first`.
    pub(super) fn copy_aggregate_to_registers(
        &mut self,
        operand: Operand,
        first: u32,
    ) -> Result<(), Refused> {
        let ty = self.function.operand_type(&self.module.context, operand).ok_or(Refused("block operand"))?;
        let layout = self.aggregate_registers(ty)?;
        let total = layout.iter().map(|(_, parts)| parts).sum();
        let chain = self.info.part_regs(first, total);
        let parts = self.value_parts(operand)?;
        let mut at = 0;
        for ((_, count), part) in layout.into_iter().zip(parts) {
            self.copy_leaf_to_register(part, chain[at], ISD::AnyExtend)?;
            at += count;
        }
        Ok(())
    }
}
