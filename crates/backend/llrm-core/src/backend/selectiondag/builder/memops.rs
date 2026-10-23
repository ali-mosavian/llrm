//! `llvm.memcpy` and `llvm.memset` (pin `SelectionDAG::getMemcpy`,
//! `getMemset` and `X86SelectionDAGInfo`): a constant size within the store
//! limit is a run of loads and stores, a constant size up to 128 bytes at
//! 4-byte alignment is `REP_MOVS`/`REP_STOS` (the bytes past the last block
//! stored as above), and the rest is a call of `memcpy` or `memset`.

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::opcode::{Attribute, CallInfo};

use super::super::dag::{MemBase, MemInfo, NodeData, SDValue};
use super::super::isd::ISD;
use super::super::mvt::EVT;
use super::super::x86isd;
use super::{Builder, Refused, convention_for};
use crate::backend::isel::Convention;

/// `X86Subtarget::getMaxInlineSizeThreshold`.
const MAX_INLINE_SIZE: u64 = 128;

impl Builder<'_> {
    pub(super) fn visit_intrinsic(
        &mut self,
        instruction: &Instruction,
        info: &CallInfo,
        intrinsic: &Intrinsic,
    ) -> Result<(), Refused> {
        match intrinsic {
            Intrinsic::MemCpy | Intrinsic::MemSet | Intrinsic::MemMove => {
                self.visit_mem_intrinsic(instruction, info, intrinsic)
            }
            Intrinsic::FunnelShift { left } => self.visit_funnel_shift(instruction, *left),
            Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd => {
                self.visit_lifetime(instruction, matches!(intrinsic, Intrinsic::LifetimeStart))
            }
            Intrinsic::VaStart | Intrinsic::VaEnd | Intrinsic::VaCopy => self.visit_va(instruction, intrinsic),
            Intrinsic::MinMax { .. }
            | Intrinsic::BitCount(_)
            | Intrinsic::ByteSwap
            | Intrinsic::Abs
            | Intrinsic::LRint
            | Intrinsic::Unary(_) => self.visit_math(instruction, intrinsic),
            _ => Err(Refused("intrinsic call")),
        }
    }

    /// `visitVAStart`, `visitVAEnd`, `visitVACopy`: the node on the root with
    /// a `SrcValue` per list.
    fn visit_va(
        &mut self,
        instruction: &Instruction,
        intrinsic: &Intrinsic,
    ) -> Result<(), Refused> {
        let root = self.root();
        let mut ops = vec![root];
        for operand in &instruction.operands[..if matches!(intrinsic, Intrinsic::VaCopy) { 2 } else { 1 }] {
            ops.push(self.get_value(*operand)?);
        }
        for operand in &instruction.operands[..ops.len() - 1] {
            let key = match operand {
                Operand::Value(value) => value.0,
                Operand::Constant(id) => 1 << 31 | id.index() as u32,
                _ => 0,
            };
            ops.push(self.dag.get_src_value(key));
        }
        let code = match intrinsic {
            Intrinsic::VaStart => ISD::VAStart,
            Intrinsic::VaEnd => ISD::VAEnd,
            _ => ISD::VACopy,
        };
        let node = self.dag.get_node(code, EVT::OTHER, &ops);
        self.dag.set_root(node);
        Ok(())
    }

    /// `fshl`/`fshr`: a rotate when both inputs are one value, else the funnel
    /// node.
    fn visit_funnel_shift(
        &mut self,
        instruction: &Instruction,
        left: bool,
    ) -> Result<(), Refused> {
        let x = self.get_value(instruction.operands[0])?;
        let y = self.get_value(instruction.operands[1])?;
        let z = self.get_value(instruction.operands[2])?;
        let vt = self.dag.value_type(x);
        let node = if x == y {
            self.dag.get_node(if left { ISD::Rotl } else { ISD::Rotr }, vt, &[x, z])
        } else {
            self.dag.get_node(if left { ISD::FShl } else { ISD::FShr }, vt, &[x, y, z])
        };
        self.set_value(instruction.result.ok_or(Refused("a funnel shift without a result"))?, node);
        Ok(())
    }

    /// `lifetime.start`/`end` of a static alloca: a chain node on the frame
    /// index; any other object is ignored.
    fn visit_lifetime(
        &mut self,
        instruction: &Instruction,
        start: bool,
    ) -> Result<(), Refused> {
        let Some(Operand::Value(object)) = instruction.operands.iter().rev().nth(1).copied() else { return Ok(()) };
        let Some(index) = self.info.static_allocas.get(&object).copied() else { return Ok(()) };
        let root = self.root();
        let frame = self.dag.get_target_frame_index(self.target.dag.pointer, index);
        let node =
            self.dag.get_node(if start { ISD::LifetimeStart } else { ISD::LifetimeEnd }, EVT::OTHER, &[root, frame]);
        self.dag.set_root(node);
        Ok(())
    }

    fn function_has(
        &self,
        name: &str,
    ) -> bool {
        llrm_mir::memory::has(&self.function.attrs, name)
    }

    /// The `align` attribute of argument `at`, or 1 (`valueOrOne`).
    fn argument_alignment(
        info: &CallInfo,
        at: usize,
    ) -> u64 {
        info.argument_attrs
            .get(at)
            .into_iter()
            .flatten()
            .find_map(|attr| match attr {
                Attribute::Int(name, value) if name == "align" => Some(*value),
                _ => None,
            })
            .unwrap_or(1)
    }

    fn constant_size(
        &self,
        size: SDValue,
    ) -> Option<u64> {
        self.dag.constant(size).map(|bits| bits as u64)
    }

    /// `findOptimalMemOpLowering` for x86 without SSE: the width in bytes of
    /// each memory operation covering `size` bytes, the widest being the
    /// pointer's; `None` past `limit` operations. A last operation wider
    /// than the bytes left overlaps the one before it.
    fn mem_ops(
        size: u64,
        widest: u64,
        limit: usize,
        volatile: bool,
    ) -> Option<Vec<u64>> {
        let (mut left, mut width) = (size, widest);
        let mut ops = Vec::new();
        while left > 0 {
            let mut taken = width;
            while taken > left {
                let narrower = width / 2;
                if !ops.is_empty() && !volatile && narrower < left {
                    // a misaligned access is fast on x86: overlap the previous
                    // bytes
                    taken = left;
                } else {
                    width = narrower;
                    taken = narrower;
                }
            }
            if ops.len() + 1 > limit {
                return None;
            }
            ops.push(width);
            left -= taken;
        }
        Some(ops)
    }

    fn ptr_plus(
        &mut self,
        base: SDValue,
        offset: u64,
    ) -> SDValue {
        let vt = self.dag.value_type(base);
        let offset = self.dag.get_constant(u128::from(offset), vt);
        self.dag.get_node(ISD::Add, vt, &[base, offset])
    }

    /// `getMemcpyLoadsAndStores`: loads from the source and stores to the
    /// destination, the chains joined as `load0:1, store0, load1:1, ...`.
    pub(super) fn inline_memcpy(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: u64,
        limit: usize,
        dst_align: u64,
        src_align: u64,
    ) -> Result<Option<SDValue>, Refused> {
        let slice = self.constant_slice(src);
        let widest = u64::from(self.target.dag.pointer.size_in_bits() / 8);
        let Some(ops) = Self::mem_ops(size, widest, limit, false) else { return Ok(None) };
        let (mut src_off, mut dst_off, mut left) = (0u64, 0u64, size);
        let mut chains = Vec::new();
        for width in ops {
            if width > left {
                src_off -= width - left;
                dst_off -= width - left;
            }
            let vt = EVT::integer(width as u32 * 8);
            // `getMemsetStringVal`: the bytes from the source's offset, zero
            // past the end, as an immediate stored on the root
            if let Some((bytes, base)) = &slice {
                let mut value = 0u128;
                for at in 0..width {
                    let byte = bytes.as_ref().and_then(|bytes| bytes.get((base + src_off + at) as usize)).copied();
                    value |= u128::from(byte.unwrap_or(0)) << (8 * at);
                }
                let value = self.dag.get_constant(value, vt);
                let to = self.dag.get_object_ptr_offset(dst, dst_off as i64);
                let info = MemInfo {
                    mem_vt: vt,
                    align: common_alignment(dst_align, dst_off) as u32,
                    volatile: false,
                    base: MemBase::Unknown,
                    ext: None, space: 0,
                };
                chains.push(self.dag.get_store(chain, value, to, info));
                src_off += width;
                dst_off += width;
                left = left.saturating_sub(width);
                continue;
            }
            let from = self.dag.get_object_ptr_offset(src, src_off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(src_align, src_off) as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None, space: 0,
            };
            let value = self.dag.get_load(vt, chain, from, info);
            let to = self.dag.get_object_ptr_offset(dst, dst_off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(dst_align, dst_off) as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None, space: 0,
            };
            let store = self.dag.get_store(chain, value, to, info);
            chains.push(SDValue { node: value.node, res: 1 });
            chains.push(store);
            src_off += width;
            dst_off += width;
            left = left.saturating_sub(width);
        }
        Ok(Some(self.dag.get_token_factor(&chains)))
    }

    /// `getMemmoveLoadsAndStores`: every load first, then the stores on the
    /// loads' joined chains (the regions may overlap).
    fn inline_memmove(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: u64,
        limit: usize,
        dst_align: u64,
        src_align: u64,
    ) -> Result<Option<SDValue>, Refused> {
        let widest = u64::from(self.target.dag.pointer.size_in_bits() / 8);
        let Some(ops) = Self::mem_ops(size, widest, limit, false) else { return Ok(None) };
        let (mut off, mut left) = (0u64, size);
        let mut loads = Vec::new();
        let mut placed = Vec::new();
        for width in &ops {
            if *width > left {
                off -= width - left;
            }
            let vt = EVT::integer(*width as u32 * 8);
            let from = self.dag.get_object_ptr_offset(src, off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(src_align, off) as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None, space: 0,
            };
            let value = self.dag.get_load(vt, chain, from, info);
            placed.push((value, off, vt));
            loads.push(SDValue { node: value.node, res: 1 });
            off += width;
            left = left.saturating_sub(*width);
        }
        let joined = self.dag.get_token_factor(&loads);
        let mut stores = Vec::new();
        for (value, at, vt) in placed {
            let to = self.dag.get_object_ptr_offset(dst, at as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(dst_align, at) as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None, space: 0,
            };
            stores.push(self.dag.get_store(joined, value, to, info));
        }
        Ok(Some(self.dag.get_token_factor(&stores)))
    }

    /// `getMemsetStores`: the fill value splatted to the widest operation,
    /// truncated for the narrower ones.
    fn inline_memset(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: u64,
        limit: usize,
        align: u64,
    ) -> Result<Option<SDValue>, Refused> {
        let widest = u64::from(self.target.dag.pointer.size_in_bits() / 8);
        let Some(ops) = Self::mem_ops(size, widest, limit, false) else { return Ok(None) };
        let largest = ops.iter().copied().max().expect("a store");
        let largest_vt = EVT::integer(largest as u32 * 8);
        let splat = self.memset_value(value, largest_vt);
        let (mut off, mut left) = (0u64, size);
        let mut chains = Vec::new();
        for width in ops {
            if width > left {
                off -= width - left;
            }
            let vt = EVT::integer(width as u32 * 8);
            let fill = if width < largest { self.dag.get_node(ISD::Truncate, vt, &[splat]) } else { splat };
            let to = self.dag.get_object_ptr_offset(dst, off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(align, off) as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None, space: 0,
            };
            chains.push(self.dag.get_store(chain, fill, to, info));
            off += width;
            left = left.saturating_sub(width);
        }
        Ok(Some(self.dag.get_token_factor(&chains)))
    }

    /// `getMemsetValue`.
    fn memset_value(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        if let Some(byte) = self.dag.constant(value) {
            let mut splat = byte & 0xff;
            for _ in 1..vt.size_in_bits() / 8 {
                splat = (splat << 8) | (byte & 0xff);
            }
            return self.dag.get_constant(splat, vt);
        }
        let extended = self.dag.get_node(ISD::ZeroExtend, vt, &[value]);
        if vt.size_in_bits() > 8 {
            let mut magic = 1u128;
            for _ in 1..vt.size_in_bits() / 8 {
                magic = (magic << 8) | 1;
            }
            let magic = self.dag.get_constant(magic, vt);
            return self.dag.get_node(ISD::Mul, vt, &[extended, magic]);
        }
        extended
    }

    /// `isMemSrcFromConstant` (`getConstantDataArrayInfo`): the bytes of a
    /// constant global's initializer and the source's offset into them; no
    /// bytes is `zeroinitializer`. An initializer with an address in it has no
    /// bytes (`ReadByteArrayFromGlobal` fails) and is read with loads.
    fn constant_slice(
        &self,
        src: SDValue,
    ) -> Option<(Option<Vec<u8>>, u64)> {
        let (base, delta) = if self.dag.opcode(src) == ISD::Add {
            (self.dag.operand(src, 0), self.dag.constant(self.dag.operand(src, 1))? as u64)
        } else {
            (src, 0)
        };
        let NodeData::Global { symbol, offset } = self.dag.node(base).data else { return None };
        let llrm_mir::module::GlobalKind::Variable(variable) = &self.module.globals[symbol as usize].kind else {
            return None;
        };
        let init = variable.initializer.filter(|_| variable.constant)?;
        let from = u64::try_from(delta as i64 + offset).ok()?;
        let types = &self.module.context.types;
        let size = self.layout.store_size(types, self.module.context.get(init).ty);
        (from <= size).then_some(())?;
        if matches!(self.module.context.get(init).kind, ConstantKind::Zero) {
            return Some((None, from));
        }
        let mut bytes = vec![0; size as usize];
        self.flatten(init, &mut bytes, 0)?;
        Some((Some(bytes), from))
    }

    /// `ReadDataFromGlobal`: the constant's bytes at `at`, where the layout
    /// puts them; `None` for anything but integers, floats and aggregates of
    /// them.
    fn flatten(
        &self,
        id: llrm_mir::context::ConstantId,
        bytes: &mut [u8],
        at: usize,
    ) -> Option<()> {
        use llrm_mir::types::{FloatKind, Type};
        let constant = self.module.context.get(id);
        let types = &self.module.context.types;
        match &constant.kind {
            ConstantKind::Int(bits) => {
                let width = types.int_bits(constant.ty)?;
                (width % 8 == 0).then_some(())?;
                let count = (width / 8) as usize;
                bytes.get_mut(at..at + count)?.copy_from_slice(&bits.to_le_bytes()[..count]);
            }
            ConstantKind::Float(bits) => match types.get(constant.ty) {
                Type::Float(kind @ (FloatKind::Float | FloatKind::Double)) => {
                    let count = kind.value_bytes() as usize;
                    bytes.get_mut(at..at + count)?.copy_from_slice(&bits.to_le_bytes()[..count]);
                }
                _ => return None,
            },
            ConstantKind::Zero | ConstantKind::Undef | ConstantKind::Poison => {}
            ConstantKind::Bytes(data) => bytes.get_mut(at..at + data.len())?.copy_from_slice(data),
            ConstantKind::Aggregate(members) => match types.get(constant.ty) {
                Type::Array { element, .. } => {
                    let stride = self.layout.alloc_size(types, *element) as usize;
                    for (index, member) in members.iter().enumerate() {
                        self.flatten(*member, bytes, at + index * stride)?;
                    }
                }
                _ => {
                    let (_, offsets) = self.layout.struct_layout(types, constant.ty);
                    for (member, offset) in members.iter().zip(offsets) {
                        self.flatten(*member, bytes, at + offset as usize)?;
                    }
                }
            },
            _ => return None,
        }
        Some(())
    }

    fn visit_mem_intrinsic(
        &mut self,
        instruction: &Instruction,
        info: &CallInfo,
        which: &Intrinsic,
    ) -> Result<(), Refused> {
        let operands = &instruction.operands;
        let volatile = match operands.get(3) {
            Some(Operand::Constant(id)) => matches!(self.module.context.get(*id).kind, ConstantKind::Int(1)),
            _ => return Err(Refused("memory intrinsic with a variable volatile flag")),
        };
        if volatile {
            return Err(Refused("volatile memory intrinsic"));
        }
        let dst = self.get_value(operands[0])?;
        let other = self.get_value(operands[1])?;
        let size = self.get_value(operands[2])?;
        let dst_align = Self::argument_alignment(info, 0);
        let src_align = if matches!(which, Intrinsic::MemSet) { dst_align } else { Self::argument_alignment(info, 1) };
        let alignment = dst_align.min(src_align);
        let root = self.memory_root();

        let opt_size = self.function_has("optsize") || self.function_has("minsize");
        let constant = self.constant_size(size);
        if constant == Some(0) {
            return Ok(());
        }
        let chain = match which {
            Intrinsic::MemCpy | Intrinsic::MemMove => {
                let limit = if opt_size { 4 } else { 8 };
                let inline = match constant {
                    Some(bytes) if matches!(which, Intrinsic::MemCpy) => {
                        self.inline_memcpy(root, dst, other, bytes, limit, dst_align, src_align)?
                    }
                    Some(bytes) => self.inline_memmove(root, dst, other, bytes, limit, dst_align, src_align)?,
                    _ => None,
                };
                match (inline, which, constant) {
                    (Some(chain), _, _) => chain,
                    (None, Intrinsic::MemCpy, Some(bytes)) if self.function_has("minsize") => {
                        self.rep_movs(root, dst, other, bytes, 1, dst_align, src_align)?
                    }
                    (None, Intrinsic::MemCpy, Some(bytes)) if bytes <= MAX_INLINE_SIZE && alignment >= 4 => {
                        self.rep_movs(root, dst, other, bytes, alignment.min(4), dst_align, src_align)?
                    }
                    _ => self.mem_libcall(
                        if matches!(which, Intrinsic::MemCpy) { "memcpy" } else { "memmove" },
                        root,
                        &[dst, other],
                        size,
                    )?,
                }
            }
            _ => {
                let limit = if opt_size { 8 } else { 16 };
                let inline = match constant {
                    Some(bytes) => self.inline_memset(root, dst, other, bytes, limit, alignment)?,
                    None => None,
                };
                match (inline, constant) {
                    (Some(chain), _) => chain,
                    (None, Some(bytes))
                        if self.function_has("minsize") || (bytes <= MAX_INLINE_SIZE && alignment >= 4) =>
                    {
                        self.rep_stos(root, dst, other, bytes, alignment)?
                    }
                    _ => self.mem_libcall("memset", root, &[dst, other], size)?,
                }
            }
        };
        self.dag.set_root(chain);
        Ok(())
    }

    /// `getMemcpy`'s libcall: `name(dst, src, size)` for a copy, `name(dst,
    /// value, size)` for a set; the result is read and dropped.
    fn mem_libcall(
        &mut self,
        name: &str,
        chain_in: SDValue,
        first_two: &[SDValue],
        size: SDValue,
    ) -> Result<SDValue, Refused> {
        let types = &self.module.context.types;
        let pointer_type = types.ptr(0);
        let word = types.int(self.target.dag.pointer.size_in_bits());
        let symbol = self.dag.get_external_symbol(self.target.dag.pointer, name);
        let value_type =
            if name == "memset" { types.int(self.dag.value_type(first_two[1]).size_in_bits()) } else { pointer_type };
        let arg_types = [pointer_type, value_type, word];
        let first = self.arch.first_argument_offset(false);
        let plan: Convention =
            convention_for(self.module, self.layout, self.arch, first, 0, pointer_type, false, &arg_types, &[])
                .map_err(|_| Refused("libcall the convention does not describe"))?;
        let target_symbol = self.dag.get_target_external_symbol(self.target.dag.pointer, name);
        let _ = symbol;
        let values = vec![first_two[0], first_two[1], size];
        self.dag.set_root(chain_in);
        let extends = [ISD::AnyExtend; 3];
        let _ = self.lower_call(target_symbol, values, &extends, &[], &plan, first, pointer_type)?;
        Ok(self.dag.get_root())
    }

    fn reg(
        &self,
        wide: &str,
        narrow: &str,
    ) -> llrm_lir::registers::RegId {
        self.arch.register_named(if self.target.dag.pointer == EVT::I16 { narrow } else { wide })
    }

    /// `emitRepmovs` with the block width of `emitConstantSizeRepmov`.
    fn rep_movs(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: u64,
        alignment: u64,
        dst_align: u64,
        src_align: u64,
    ) -> Result<SDValue, Refused> {
        let chain_in = chain;
        let pointer = self.target.dag.pointer;
        let block = if self.function_has("minsize") { 1 } else { alignment.min(u64::from(pointer.size_in_bits() / 8)) };
        let (count, left) = (size / block, size % block);
        let (cx, di, si) = (self.reg("ecx", "cx"), self.reg("edi", "di"), self.reg("esi", "si"));
        let count = self.dag.get_constant(u128::from(count), pointer);
        let copy = self.dag.get_copy_to_phys_reg(chain, cx, count, None);
        let copy = self
            .dag
            .get_copy_to_phys_reg(
                SDValue { node: copy.node, res: 0 },
                di,
                dst,
                Some(SDValue { node: copy.node, res: 1 }),
            );
        let copy = self
            .dag
            .get_copy_to_phys_reg(
                SDValue { node: copy.node, res: 0 },
                si,
                src,
                Some(SDValue { node: copy.node, res: 1 }),
            );
        let block_type = self.dag.get_value_type_node(EVT::integer(block as u32 * 8));
        let rep = self
            .dag
            .get_node_multi(
                x86isd::REP_MOVS,
                &[EVT::OTHER, EVT::GLUE],
                &[SDValue { node: copy.node, res: 0 }, block_type, SDValue { node: copy.node, res: 1 }],
            );
        if left == 0 {
            return Ok(rep);
        }
        // the bytes past the last block: a copy of their own, from the original
        // chain
        let rep = SDValue { node: rep.node, res: 0 };
        let offset = size - left;
        let _size = self.dag.get_constant(u128::from(left), pointer);
        let (from, to) = (self.ptr_plus(src, offset), self.ptr_plus(dst, offset));
        let tail =
            self.inline_memcpy(chain_in, to, from, left, 8, dst_align, src_align)?.ok_or(Refused("rep movs tail"))?;
        Ok(self.dag.get_token_factor(&[rep, tail]))
    }

    /// `emitConstantSizeRepstos`.
    fn rep_stos(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: u64,
        alignment: u64,
    ) -> Result<SDValue, Refused> {
        let pointer = self.target.dag.pointer;
        let minsize = self.function_has("minsize");
        let word = u64::from(pointer.size_in_bits() / 8);
        let chain_in = chain;
        let (block, count, mut val) = if minsize {
            match self.dag.constant(value) {
                Some(byte) if size & 31 == 0 && byte & 255 == 0 => {
                    (word, size / word, self.dag.get_constant(0, EVT::integer(word as u32 * 8)))
                }
                _ => (1, size, value),
            }
        } else if let Some(byte) = self.dag.constant(value) {
            let block = alignment.min(word);
            let mut pattern = byte as u64 & 255;
            if block >= 2 {
                pattern |= pattern << 8;
            }
            if block >= 4 {
                pattern |= pattern << 16;
            }
            (block, size / block, self.dag.get_constant(u128::from(pattern), EVT::integer(block as u32 * 8)))
        } else {
            (1, size, value)
        };
        let _ = &mut val;
        let acc = match block {
            1 => self.arch.register_named("al"),
            2 => self.arch.register_named("ax"),
            _ => self.arch.register_named("eax"),
        };
        let (cx, di) = (self.reg("ecx", "cx"), self.reg("edi", "di"));
        let count = self.dag.get_constant(u128::from(count), pointer);
        let copy = self.dag.get_copy_to_phys_reg(chain, acc, val, None);
        let copy = self
            .dag
            .get_copy_to_phys_reg(
                SDValue { node: copy.node, res: 0 },
                cx,
                count,
                Some(SDValue { node: copy.node, res: 1 }),
            );
        let copy = self
            .dag
            .get_copy_to_phys_reg(
                SDValue { node: copy.node, res: 0 },
                di,
                dst,
                Some(SDValue { node: copy.node, res: 1 }),
            );
        let block_type = self.dag.get_value_type_node(EVT::integer(block as u32 * 8));
        let rep = self
            .dag
            .get_node_multi(
                x86isd::REP_STOS,
                &[EVT::OTHER, EVT::GLUE],
                &[SDValue { node: copy.node, res: 0 }, block_type, SDValue { node: copy.node, res: 1 }],
            );
        let left = size % block;
        if left == 0 {
            return Ok(rep);
        }
        let offset = size - left;
        let to = self.ptr_plus(dst, offset);
        let _size = self.dag.get_constant(u128::from(left), pointer);
        let tail = self.inline_memset(chain_in, to, value, left, 8, alignment)?.ok_or(Refused("rep stos tail"))?;
        Ok(self.dag.get_token_factor(&[rep, tail]))
    }
}

/// `commonAlignment`: what is still known of an alignment at an offset.
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    if offset == 0 { align } else { align.min(1 << offset.trailing_zeros()) }
}
