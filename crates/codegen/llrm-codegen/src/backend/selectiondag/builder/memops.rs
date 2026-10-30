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
use crate::backend::memcpycall::inline_widths;

/// `X86Subtarget::getMaxInlineSizeThreshold`.
const MAX_INLINE_SIZE: u64 = 128;

/// The bytes of a `rep stosb` fill in real mode: `lea di`, ES set, the byte,
/// the count, the fill, DI's save.
const FILL_BYTES: u64 = 3 + 4 + 2 + 3 + 2 + 2;

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
            Intrinsic::PtrDiff => {
                let (a, b) = (self.get_value(instruction.operands[0])?, self.get_value(instruction.operands[1])?);
                let vt = self.type_of(instruction.ty)?;
                let node = self.dag.get_node(super::super::x86isd::LLRM_PTRDIFF, vt, &[a, b]);
                self.set_value(instruction.result.ok_or(Refused("a pointer difference without a result"))?, node);
                Ok(())
            }
            Intrinsic::Window => {
                let pointer = self.get_value(instruction.operands[0])?;
                let node = self
                    .dag
                    .get_node(super::super::x86isd::LLRM_WINDOW, EVT::Simple(super::super::mvt::MVT::Far), &[pointer]);
                self.set_value(instruction.result.ok_or(Refused("a window without a result"))?, node);
                Ok(())
            }
            // an I/O port is a barrier on the chain: it reorders with nothing
            Intrinsic::PortIn | Intrinsic::PortOut => {
                let reading = matches!(intrinsic, Intrinsic::PortIn);
                let port = self.get_value(instruction.operands[0])?;
                let root = self.root();
                if reading {
                    let vt = self.type_of(instruction.ty)?;
                    let node =
                        self.dag.get_node_multi(super::super::x86isd::LLRM_PORT, &[vt, EVT::OTHER], &[root, port]);
                    self.dag.set_root(SDValue { node: node.node, res: 1 });
                    self.set_value(instruction.result.ok_or(Refused("a port read without a result"))?, node);
                } else {
                    let value = self.get_value(instruction.operands[1])?;
                    let node =
                        self.dag.get_node_multi(super::super::x86isd::LLRM_PORT, &[EVT::OTHER], &[root, port, value]);
                    self.dag.set_root(node);
                }
                Ok(())
            }
            Intrinsic::Assume | Intrinsic::Prefetch | Intrinsic::FakeUse => Ok(()),
            // `SelectionDAGBuilder.cpp#L7680`: the region information is
            // discarded; the scope is an undefined pointer.
            Intrinsic::InvariantEnd => Ok(()),
            Intrinsic::InvariantStart => {
                if let Some(result) = instruction.result {
                    let vt = self.type_of(instruction.ty)?;
                    let undef = self.dag.get_undef(vt);
                    self.set_value(result, undef);
                }
                Ok(())
            }
            // a hint the passes read: its value is its argument
            Intrinsic::Expect | Intrinsic::Restrict => {
                let value = self.get_value(instruction.operands[0])?;
                self.set_value(instruction.result.ok_or(Refused("a hint without a result"))?, value);
                Ok(())
            }
            Intrinsic::StackSave => {
                let vt = self.target.dag.pointer;
                let root = self.root();
                let node = self.dag.get_node_multi(ISD::StackSave, &[vt, EVT::OTHER], &[root]);
                self.dag.set_root(SDValue { node: node.node, res: 1 });
                self.set_value(instruction.result.ok_or(Refused("a stacksave without a result"))?, node);
                Ok(())
            }
            Intrinsic::StackRestore => {
                let saved = self.get_value(instruction.operands[0])?;
                let root = self.root();
                let node = self.dag.get_node(ISD::StackRestore, EVT::OTHER, &[root, saved]);
                self.dag.set_root(node);
                Ok(())
            }
            Intrinsic::Compare { .. }
            | Intrinsic::FMulAdd
            | Intrinsic::CopySign
            | Intrinsic::Fixed { .. }
            | Intrinsic::FixedMul { .. }
            | Intrinsic::ShiftSat { .. } => self.visit_math(instruction, intrinsic),
            Intrinsic::WithOverflow { .. }
            | Intrinsic::Saturating { .. }
            | Intrinsic::MinMax { .. }
            | Intrinsic::BitCount(_)
            | Intrinsic::ByteSwap
            | Intrinsic::PtrMask
            | Intrinsic::BitReverse
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
        // The SysV `va_list` is a four-field record over a register save area
        // (`LowerVASTART`), not the stack pointer the 32-bit one is; the entry
        // block's lowering of the arguments made the area.
        if self.target.dag.pointer == EVT::I64
            && matches!(intrinsic, Intrinsic::VaStart)
            && self.dag.var_args_save.is_none()
        {
            return Err(Refused("va_start at m64 outside the entry block"));
        }
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

    /// The pin's `hasOptSize`: clang puts `optsize` on every function of a
    /// `-Os` build, llrm's front ends do not, so the build's level stands for
    /// it.
    pub(super) fn has_opt_size(&self) -> bool {
        self.target.opt_size || self.function_has("optsize") || self.function_has("minsize")
    }

    /// The pin's `hasMinSize` (`-Oz`), as [`Self::has_opt_size`].
    pub(super) fn has_min_size(&self) -> bool {
        self.target.min_size || self.function_has("minsize")
    }

    pub(super) fn function_has(
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

    /// `X86TargetLowering::getOptimalMemOpType` without SSE or 64-bit mode:
    /// `i32` outside 64-bit mode, `-m16` included (a dword store is a prefix
    /// away).
    const WIDEST_MEM_OP: u64 = 4;

    /// `i64` for eight bytes or more where the pointer is 64 bits wide.
    fn widest_mem_op(
        &self,
        size: u64,
    ) -> u64 {
        if self.target.dag.pointer.size_in_bits() == 64 && size >= 8 { 8 } else { Self::WIDEST_MEM_OP }
    }

    /// `findOptimalMemOpLowering` for x86 without SSE ([`inline_widths`]).
    fn mem_ops(
        size: u64,
        widest: u64,
        limit: usize,
        volatile: bool,
    ) -> Option<Vec<u64>> {
        inline_widths(size, widest, limit, volatile)
    }

    /// The address space an access through `ptr` is in: a far pointer's
    /// offset half is read through `ES` after `LES` (llrm's space 259).
    fn space_of(
        &self,
        ptr: SDValue,
    ) -> u32 {
        if self.dag.value_type(ptr) == EVT::Simple(super::super::mvt::MVT::Far) { 259 } else { 0 }
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
        let slice = if self.volatile_memory { None } else { self.constant_slice(src) };
        let widest = self.widest_mem_op(size);
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
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
                    volatile: self.volatile_memory,
                    base: MemBase::Unknown,
                    ext: None,
                    space: self.space_of(dst),
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
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(src),
            };
            let value = self.dag.get_load(vt, chain, from, info);
            let to = self.dag.get_object_ptr_offset(dst, dst_off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(dst_align, dst_off) as u32,
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(dst),
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
        let widest = self.widest_mem_op(size);
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
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
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(src),
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
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(dst),
            };
            stores.push(self.dag.get_store(joined, value, to, info));
        }
        Ok(Some(self.dag.get_token_factor(&stores)))
    }

    /// DEVIATE SD223 (real mode): tuned for size, a constant memset whose
    /// stores take more bytes than a `rep stosb` fill is the fill, unless the
    /// fill's extra clocks outweigh the bytes it saves (`SizeTrade`). The pin
    /// counts stores (`MaxStoresPerMemsetOptSize`); in real mode each dword
    /// store carries a `66` prefix and the fill costs `FILL_BYTES`. Mirrors the
    /// old selector's `stored_bytes` rule. Owner w12. Exit: a store-cost
    /// hook on the target description.
    fn fill_is_smaller(
        &self,
        length: u64,
        opt_size: bool,
    ) -> bool {
        if !opt_size || self.target.dag.pointer.size_in_bits() != 16 {
            return false;
        }
        let dwords = length / 4;
        let words = u64::from(length % 4 >= 2);
        let bytes = u64::from(length % 2 == 1);
        let (each, setup) = if dwords > 1 { (4, 6) } else { (8, 0) };
        let stored = setup + dwords * each + words * 5 + bytes * 4;
        if stored <= FILL_BYTES {
            return false;
        }
        let Some(trade) = self.target.size_trade else {
            return true;
        };
        let stores = (dwords * (trade.store + trade.prefix) as u64 + (words + bytes) * trade.store as u64) as i64;
        let fill = trade.fill + length as i64 * trade.fill_cell;
        // Tuning constant fitted to sc.c, weapons, mdl and lru, not derived.
        (stored - FILL_BYTES) as i64 * trade.milliclocks_per_byte >= (fill - stores) * 1000
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
        let widest = self.widest_mem_op(size);
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
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
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(dst),
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

    /// A fill, copy or move through a huge pointer: the target's stepped
    /// routine (`huge_fill_routine`, `huge_copy_routine`), laid down inline at
    /// the call with its operands in the registers it reads, as the old
    /// selector does. A `memmove` is a copy only where a pass proved its
    /// direction.
    ///
    /// DEVIATE SD311: the pin has no huge pointers; this is `isel/huge.rs`'s
    /// routine called as a library helper with a register contract. Exit: none
    /// (m16 only). Owner sdag-c. Review 2026-12-10.
    fn huge_memory(
        &mut self,
        instruction: &Instruction,
        which: &Intrinsic,
    ) -> Result<(), Refused> {
        use super::super::libcall::{CallEnv, Place, lib_call};
        let operands = &instruction.operands;
        let far: EVT = super::super::mvt::MVT::Far.into();
        let word = self.target.dag.pointer;
        let dword = |this: &mut Self, value: SDValue| {
            let bits = this.dag.value_type(value).size_in_bits();
            match bits {
                32 => Some(value),
                8 | 16 => Some(this.dag.get_node(ISD::ZeroExtend, EVT::I32, &[value])),
                _ => None,
            }
        };
        let proved = |kind: &str| instruction.metadata.iter().any(|(one, _)| one == kind);
        let dst = self.get_value(operands[0])?;
        let count = self.get_value(operands[2])?;
        let count = dword(self, count).ok_or(Refused("a huge memory intrinsic of an odd length"))?;
        if self.dag.value_type(dst) != far {
            return Err(Refused("a huge memory intrinsic through a near destination"));
        }
        let [to_offset, to_selector] = self.split_value(dst, word);
        let (routine, values) = if matches!(which, Intrinsic::MemSet) {
            let value = self.get_value(operands[1])?;
            let pattern = dword(self, value).ok_or(Refused("a huge fill of an odd value"))?;
            let routine = self.arch.huge_fill_routine(1).map_err(|_| Refused("no huge fill routine"))?;
            (routine, vec![pattern, count, to_selector, to_offset])
        } else {
            let backward = match (which, proved(llrm_mir::intrinsics::FORWARD), proved(llrm_mir::intrinsics::BACKWARD))
            {
                (Intrinsic::MemCpy, ..) | (_, true, false) => false,
                (_, false, true) => true,
                _ => return Err(Refused("a huge memmove whose direction no pass proved")),
            };
            let src = self.get_value(operands[1])?;
            if self.dag.value_type(src) != far {
                return Err(Refused("a huge memory intrinsic through a near source"));
            }
            let [from_offset, from_selector] = self.split_value(src, word);
            let routine = self.arch.huge_copy_routine(backward).map_err(|_| Refused("no huge copy routine"))?;
            (routine, vec![count, from_selector, from_offset, to_selector, to_offset])
        };
        if routine.reads.len() != values.len() {
            return Err(Refused("a huge routine whose registers are not the operands"));
        }
        let args: Vec<(SDValue, Place)> =
            values.into_iter().zip(&routine.reads).map(|(value, reg)| (value, Place::Register(*reg))).collect();
        let callee = self.dag.get_target_external_symbol(word, routine.name);
        let root = self.memory_root();
        let env = CallEnv { arch: self.arch, pointer: word };
        let (chain, _) = lib_call(&mut self.dag, &env, root, callee, &args, 0, 0, &[]);
        self.dag.set_root(chain);
        Ok(())
    }

    fn visit_mem_intrinsic(
        &mut self,
        instruction: &Instruction,
        info: &CallInfo,
        which: &Intrinsic,
    ) -> Result<(), Refused> {
        let volatile = match instruction.operands.get(3) {
            Some(Operand::Constant(id)) => matches!(self.module.context.get(*id).kind, ConstantKind::Int(1)),
            _ => return Err(Refused("memory intrinsic with a variable volatile flag")),
        };
        // volatile accesses stay as many, as wide, and in the order written: no
        // overlapping tail, no constant source (`getMemcpyLoadsAndStores`)
        self.volatile_memory = volatile;
        let done = self.lower_mem_intrinsic(instruction, info, which);
        self.volatile_memory = false;
        done
    }

    fn lower_mem_intrinsic(
        &mut self,
        instruction: &Instruction,
        info: &CallInfo,
        which: &Intrinsic,
    ) -> Result<(), Refused> {
        let operands = &instruction.operands;
        let proved = |kind: &str| instruction.metadata.iter().any(|(one, _)| one == kind);
        let forward = proved(llrm_mir::intrinsics::FORWARD) && !proved(llrm_mir::intrinsics::BACKWARD);
        let backward = proved(llrm_mir::intrinsics::BACKWARD) && !proved(llrm_mir::intrinsics::FORWARD);
        // `rep stos`/`rep movs` and the inline moves step one segment: a huge
        // pointer's run can cross the end of it (and its count outgrow
        // `cx`), which the target's stepped routine handles.
        let huge = |this: &Self, operand: Operand| {
            matches!(
                this.function.operand_type(&this.module.context, operand).map(|ty| this.module.context.types.get(ty)),
                Some(llrm_mir::types::Type::Pointer(space)) if this.layout.carries(*space)
            )
        };
        if huge(self, operands[0]) || (!matches!(which, Intrinsic::MemSet) && huge(self, operands[1])) {
            return self.huge_memory(instruction, which);
        }
        let dst = self.get_value(operands[0])?;
        let other = self.get_value(operands[1])?;
        let size = self.get_value(operands[2])?;
        let dst_align = Self::argument_alignment(info, 0);
        let src_align = if matches!(which, Intrinsic::MemSet) { dst_align } else { Self::argument_alignment(info, 1) };
        let alignment = dst_align.min(src_align);
        let root = self.memory_root();

        let opt_size = self.has_opt_size();
        let constant = self.constant_size(size);
        if constant == Some(0) {
            return Ok(());
        }
        let chain = match which {
            Intrinsic::MemCpy | Intrinsic::MemMove => {
                let tuning = &self.target.tuning;
                let limit = match (which, opt_size) {
                    (Intrinsic::MemMove, true) => tuning.max_stores_per_memmove_opt_size,
                    (Intrinsic::MemMove, false) => tuning.max_stores_per_memmove,
                    (_, true) => tuning.max_stores_per_memcpy_opt_size,
                    (_, false) => tuning.max_stores_per_memcpy,
                };
                // DEVIATE SD225 (real mode): tuned for size, a copy of more
                // than `limit` pieces (dwords, then a word and
                // a byte: no overlap) is string moves; the pin
                // counts the overlapping last dword as one more
                // piece of the same size and keeps four pairs, 8 bytes
                // each here (15 bytes: 43 against 27). The old selector's rule.
                // Owner w12. Exit: a cost hook on the target.
                let strings = opt_size
                    && self.arch.string_registers().is_some()
                    && constant.is_some_and(|bytes| bytes / 4 + u64::from((bytes % 4).count_ones()) > limit as u64);
                let inline = match constant {
                    Some(_) if strings => None,
                    Some(bytes) if matches!(which, Intrinsic::MemCpy) => {
                        self.inline_memcpy(root, dst, other, bytes, limit, dst_align, src_align)?
                    }
                    Some(bytes) => self.inline_memmove(root, dst, other, bytes, limit, dst_align, src_align)?,
                    _ => None,
                };
                match (inline, which, constant) {
                    (Some(chain), _, _) => chain,
                    (None, Intrinsic::MemCpy, Some(bytes)) if self.has_min_size() => {
                        self.rep_movs(root, dst, other, bytes, 1, dst_align, src_align)?
                    }
                    (None, Intrinsic::MemCpy, Some(bytes)) if bytes <= MAX_INLINE_SIZE && alignment >= 4 => self
                        .rep_movs(root, dst, other, bytes, alignment.min(self.string_block()), dst_align, src_align)?,
                    // no `memcpy` to call for a copy the pin leaves to one
                    // (alignment under 4): dwords, as the old selector copies;
                    // x86 takes them unaligned
                    (None, Intrinsic::MemCpy, Some(bytes))
                        if !self.runtime_has("memcpy")
                            // the tail of a far copy is made from the far
                            // pointers moved on, which the string tail at size
                            // is not
                            && (opt_size || bytes % 4 == 0 || !(self.is_far(dst) || self.is_far(other))) =>
                    {
                        self.rep_movs(root, dst, other, bytes, 4, dst_align, src_align)?
                    }
                    _ if matches!(which, Intrinsic::MemCpy) && !self.runtime_has("memcpy") => {
                        self.rep_movs_dwords(root, dst, other, size)
                    }
                    // a move a pass proved forward, past the inline limit: `rep
                    // movs` ascends, as the move needs (DEVIATE: the pin calls
                    // `memmove`; at m16 the old selector inlines it, 2.2x on
                    // scroll.c)
                    _ if forward && matches!(which, Intrinsic::MemMove) && self.inline_string_move(dst, other) => {
                        self.rep_movs_dwords(root, dst, other, size)
                    }
                    // proved backward: from the last dword down
                    _ if backward && matches!(which, Intrinsic::MemMove) && self.inline_string_move(dst, other) => {
                        self.backward_rep_movs_dwords(root, dst, other, size)
                    }
                    _ if !self.runtime_has(if matches!(which, Intrinsic::MemCpy) { "memcpy" } else { "memmove" }) => {
                        // the old selector refuses it too: no instruction moves
                        // overlapping bytes, and a call of a routine the
                        // runtime lacks would not link
                        return Err(Refused("memmove with no memmove in the runtime"));
                    }
                    _ if matches!(which, Intrinsic::MemMove)
                        && self.dag.value_type(dst) == EVT::Simple(super::super::mvt::MVT::Far) =>
                    {
                        return Err(Refused("memmove of far pointers: the runtime's routine takes near ones"));
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
                let limit = if opt_size {
                    self.target.tuning.max_stores_per_memset_opt_size
                } else {
                    self.target.tuning.max_stores_per_memset
                };
                let inline = match constant {
                    Some(bytes) if !self.fill_is_smaller(bytes, opt_size) => {
                        self.inline_memset(root, dst, other, bytes, limit, alignment)?
                    }
                    _ => None,
                };
                match (inline, constant) {
                    (Some(chain), _) => chain,
                    (None, Some(bytes)) if self.has_min_size() || (bytes <= MAX_INLINE_SIZE && alignment >= 4) => {
                        self.rep_stos(root, dst, other, bytes, alignment)?
                    }
                    // no `memset` to call: dwords, as the old selector fills;
                    // x86 takes them unaligned. Only a size of whole dwords (a
                    // tail costs the bytes the dwords save, and a far one
                    // cannot be named) and not for size: `xor eax, eax` is a
                    // byte more than `xor al, al`.
                    (None, Some(bytes)) if !opt_size && !self.runtime_has("memset") && bytes % 4 == 0 => {
                        self.rep_stos(root, dst, other, bytes, 4)?
                    }
                    _ if !self.runtime_has("memset") => self.rep_stosb(root, dst, other, size),
                    _ => self.mem_libcall("memset", root, &[dst, other], size)?,
                }
            }
        };
        self.dag.set_root(chain);
        Ok(())
    }

    /// Whether the target's runtime has the library routine `name`: its
    /// `c.toml`'s `libcalls`, the fact the old selector and `memcpycall` read.
    fn runtime_has(
        &self,
        name: &str,
    ) -> bool {
        if let Some(names) = &self.target.libcalls {
            return names.iter().any(|(one, ..)| one == name);
        }
        self.arch
            .runtime("c")
            .and_then(|description| description.table().ok())
            .and_then(|table| table.get("libcalls").and_then(|calls| calls.as_array()).cloned())
            .is_some_and(|calls| calls.iter().any(|one| one.as_str() == Some(name)))
    }

    /// `rep movsd` of `size / 4` and then `rep movsb` of `size % 4`, the old
    /// selector's copy of a size not known at compile time. The second block
    /// goes on from where the first left `si`, `di` (the emitter carries them).
    fn rep_movs_dwords(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> SDValue {
        let pointer = self.target.dag.pointer;
        if self.is_far(dst) || self.is_far(src) {
            return self.rep_movsb(chain, dst, src, size);
        }
        let (cx, di, si) = self.string_registers();
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let two = self.dag.get_constant(2, EVT::I8);
        let dwords = self.dag.get_node(ISD::Srl, pointer, &[count, two]);
        let copy = self.dag.get_copy_to_phys_reg(chain, cx, dwords, None);
        let mut glue = SDValue { node: copy.node, res: 1 };
        let mut chain = SDValue { node: copy.node, res: 0 };
        for (reg, value) in [(di, dst), (si, src)] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let dword = self.dag.get_value_type_node(EVT::I32);
        let rep = self.dag.get_node_multi(x86isd::REP_MOVS, &[EVT::OTHER, EVT::GLUE], &[chain, dword, glue]);
        if self.constant_size(count).is_some_and(|bytes| bytes % 4 == 0) {
            return SDValue { node: rep.node, res: 0 };
        }
        let three = self.dag.get_constant(3, pointer);
        let bytes = self.dag.get_node(ISD::And, pointer, &[count, three]);
        let copy = self
            .dag
            .get_copy_to_phys_reg(
                SDValue { node: rep.node, res: 0 },
                cx,
                bytes,
                Some(SDValue { node: rep.node, res: 1 }),
            );
        let byte = self.dag.get_value_type_node(EVT::I8);
        let rep = self
            .dag
            .get_node_multi(
                x86isd::REP_MOVS,
                &[EVT::OTHER, EVT::GLUE],
                &[SDValue { node: copy.node, res: 0 }, byte, SDValue { node: copy.node, res: 1 }],
            );
        SDValue { node: rep.node, res: 0 }
    }

    /// `rep movsb` of a size not known at compile time: `cx` bytes from `si`
    /// to `di`, as the old selector copies when there is no `memcpy` to call.
    fn rep_movsb(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> SDValue {
        let pointer = self.target.dag.pointer;
        let (cx, di, si) = self.string_registers();
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let copy = self.dag.get_copy_to_phys_reg(chain, cx, count, None);
        let mut glue = SDValue { node: copy.node, res: 1 };
        let mut chain = SDValue { node: copy.node, res: 0 };
        for (reg, value) in [(di, dst), (si, src)] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let byte = self.dag.get_value_type_node(EVT::I8);
        let rep = self.dag.get_node_multi(x86isd::REP_MOVS, &[EVT::OTHER, EVT::GLUE], &[chain, byte, glue]);
        SDValue { node: rep.node, res: 0 }
    }

    /// Whether a move of proven direction is a string move here: near pointers
    /// at 16 bits (the old selector's inline copy), or any when the runtime has
    /// no `memmove` to call.
    fn inline_string_move(
        &self,
        dst: SDValue,
        src: SDValue,
    ) -> bool {
        let near_16 = self.target.dag.pointer == EVT::I16 && !self.is_far(dst) && !self.is_far(src);
        near_16 || !self.runtime_has("memmove")
    }

    /// The operands of a string move on a flat target, which names no string
    /// registers: count, source, destination.
    fn flat_string_names() -> [&'static str; 3] {
        ["cx", "si", "di"]
    }

    /// `std; rep movsd; cld` from the last dword down, then the `size % 4` low
    /// bytes (`add si, 3; add di, 3; mov cx, dx; rep movsb`): a move a pass
    /// proved backward, as the old selector writes it (`LLRM_ASM`, SD305, with
    /// `cx`, `si`, `di`, `dx` as operands). A size that is a multiple of four
    /// has no tail, and a flat target takes only that. At m16 the pointers are
    /// near, so DS is copied to ES first and `movsd` takes the operand-size
    /// prefix; a flat target needs neither (it names no string registers).
    fn backward_rep_movs_dwords(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> SDValue {
        use super::super::inline_asm::lower_asm;
        let pointer = self.target.dag.pointer;
        let flat = pointer != EVT::I16;
        if flat && pointer != EVT::I32 {
            return self.backward_rep_movsb(chain, dst, src, size);
        }
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let two = self.dag.get_constant(2, EVT::I8);
        let dwords = self.dag.get_node(ISD::Srl, pointer, &[count, two]);
        let three = self.dag.get_constant(3, pointer);
        let rest = self.dag.get_node(ISD::And, pointer, &[count, three]);
        let minus_four = self.dag.get_constant(mask_of(pointer) - 3, pointer);
        let last = |this: &mut Self, base: SDValue| {
            let end = this.dag.get_node(ISD::Add, pointer, &[base, count]);
            this.dag.get_node(ISD::Add, pointer, &[end, minus_four])
        };
        let (from, to) = (last(self, src), last(self, dst));
        let tail = self.constant_size(count).is_none_or(|bytes| bytes % 4 != 0);
        // A flat target names no string registers to carry a tail's count in.
        if flat && tail {
            return self.backward_rep_movsb(chain, dst, src, size);
        }
        let mut code = if flat { vec![0xFD, 0xF3, 0xA5] } else { vec![0x1E, 0x07, 0xFD, 0xF3, 0x66, 0xA5] };
        if tail {
            code.extend([0x83, 0xC6, 0x03, 0x83, 0xC7, 0x03, 0x89, 0xD1, 0xF3, 0xA4]);
        }
        code.push(0xFC);
        let strings = self.arch.string_registers();
        if strings.is_none() && !flat {
            return self.backward_rep_movsb(chain, dst, src, size);
        }
        let name_of = |register| self.arch.registers().name(register).expect("a described register");
        let mut names = match strings {
            Some(strings) => vec![name_of(strings.count), name_of(strings.source), name_of(strings.destination)],
            None => Self::flat_string_names().to_vec(),
        };
        let mut values = vec![dwords, from, to];
        if tail {
            names.push(name_of(strings.expect("a flat move has no tail").tail));
            values.push(rest);
        }
        let block = llrm_mir::intrinsics::AsmBlock {
            code,
            inputs: names.iter().map(|name| (*name).into()).collect(),
            outputs: Vec::new(),
            clobbers: names
                .iter()
                .copied()
                .chain(strings.map(|strings| name_of(strings.destination_segment)))
                .map(Into::into)
                .collect(),
            memory: true,
            fixups: Vec::new(),
        };
        let name = llrm_mir::intrinsics::asm_name(&block);
        let registers: Vec<_> = names
            .iter()
            .zip(values)
            .map(|(name, value)| (self.sized_register(self.arch.register_named(name), pointer), value))
            .collect();
        let (chain, _) = lower_asm(&mut self.dag, chain, &name, &[], &registers, &[]);
        chain
    }

    /// `std; rep movsb; cld` from the last byte of each side: a move a pass
    /// proved backward, where the runtime has no `memmove`. The pin calls
    /// `memmove`; this is the old selector's backward copy as one inline block
    /// (`LLRM_ASM`, SD305) with `cx`, `si` and `di` as its operands.
    fn backward_rep_movsb(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> SDValue {
        use super::super::inline_asm::lower_asm;
        let pointer = self.target.dag.pointer;
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let last = |this: &mut Self, base: SDValue| {
            let minus_one = this.dag.get_constant(mask_of(pointer), pointer);
            let end = this.dag.get_node(ISD::Add, pointer, &[base, count]);
            this.dag.get_node(ISD::Add, pointer, &[end, minus_one])
        };
        let (from, to) = (last(self, src), last(self, dst));
        let strings = self.arch.string_registers();
        let name_of = |register| self.arch.registers().name(register).expect("a described register");
        // A flat target names no string registers: `cx`, `si` and `di` are
        // its operands, and ES already holds what `pop es` loads.
        let operands = match strings {
            Some(strings) => {
                [(name_of(strings.count), count), (name_of(strings.source), from), (name_of(strings.destination), to)]
            }
            None => {
                let [counter, source, destination] = Self::flat_string_names();
                [(counter, count), (source, from), (destination, to)]
            }
        };
        let block = llrm_mir::intrinsics::AsmBlock {
            // `push ds; pop es`: both pointers are near, in DS; ES holds
            // whatever the last far access left
            code: vec![0x1E, 0x07, 0xFD, 0xF3, 0xA4, 0xFC],
            inputs: operands.iter().map(|(name, _)| (*name).into()).collect(),
            outputs: Vec::new(),
            clobbers: operands
                .iter()
                .map(|(name, _)| *name)
                .chain(strings.map(|strings| name_of(strings.destination_segment)))
                .map(Into::into)
                .collect(),
            memory: true,
            fixups: Vec::new(),
        };
        let name = llrm_mir::intrinsics::asm_name(&block);
        let registers =
            operands.map(|(name, value)| (self.sized_register(self.arch.register_named(name), pointer), value));
        let (chain, _) = lower_asm(&mut self.dag, chain, &name, &[], &registers, &[]);
        chain
    }

    /// `rep stosb` of a size not known at compile time.
    fn rep_stosb(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: SDValue,
    ) -> SDValue {
        let pointer = self.target.dag.pointer;
        let (cx, di, _) = self.string_registers();
        let al = self.arch.register_named("al");
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let copy = self.dag.get_copy_to_phys_reg(chain, al, value, None);
        let mut glue = SDValue { node: copy.node, res: 1 };
        let mut chain = SDValue { node: copy.node, res: 0 };
        for (reg, value) in [(cx, count), (di, dst)] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let byte = self.dag.get_value_type_node(EVT::I8);
        let rep = self.dag.get_node_multi(x86isd::REP_STOS, &[EVT::OTHER, EVT::GLUE], &[chain, byte, glue]);
        SDValue { node: rep.node, res: 0 }
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
        // The routine's symbol and calling convention are the target fact's
        // (`options.libcalls`), as `memcpycall` applies them to a `memcpy`.
        let Some((_, convention, routine_symbol)) =
            self.target.libcalls.as_ref().and_then(|calls| calls.iter().find(|(routine, ..)| routine == name)).cloned()
        else {
            return Err(Refused("library call whose symbol and convention are the target fact's"));
        };
        let types = &self.module.context.types;
        let pointer_type = types.ptr(0);
        let word = types.int(self.target.dag.pointer.size_in_bits());
        let symbol = self.dag.get_external_symbol(self.target.dag.pointer, &routine_symbol);
        let value_type =
            if name == "memset" { types.int(self.dag.value_type(first_two[1]).size_in_bits()) } else { pointer_type };
        let arg_types = [pointer_type, value_type, word];
        let first = self.arch.first_argument_offset(false);
        let plan: Convention = convention_for(
            self.module,
            self.layout,
            self.arch,
            first,
            convention,
            pointer_type,
            false,
            &arg_types,
            &[],
        )
        .map_err(|_| Refused("libcall the convention does not describe"))?;
        let target_symbol = self.dag.get_target_external_symbol(self.target.dag.pointer, &routine_symbol);
        let _ = symbol;
        let values = vec![first_two[0], first_two[1], size];
        self.dag.set_root(chain_in);
        let extends = [ISD::AnyExtend; 3];
        let _ = self.lower_call(target_symbol, values, &extends, &[], &plan, first, pointer_type, false)?;
        Ok(self.dag.get_root())
    }

    /// The count, destination and source registers of a string operation.
    fn string_registers(&self) -> (llrm_lir::registers::RegId, llrm_lir::registers::RegId, llrm_lir::registers::RegId) {
        (self.reg("ecx", "cx"), self.reg("edi", "di"), self.reg("esi", "si"))
    }

    fn reg(
        &self,
        wide: &str,
        narrow: &str,
    ) -> llrm_lir::registers::RegId {
        if self.target.dag.pointer == EVT::I64 {
            // `Subtarget.is64Bit() ? X86::RCX : X86::ECX`
            return self.arch.register_named(&format!("r{}", &wide[1..]));
        }
        self.arch.register_named(if self.target.dag.pointer == EVT::I16 { narrow } else { wide })
    }

    fn is_far(
        &self,
        pointer: SDValue,
    ) -> bool {
        self.dag.value_type(pointer) == EVT::Simple(super::super::mvt::MVT::Far)
    }

    /// `getMemcpy` with `AlwaysInline` (a byval argument's copy): the loads
    /// and stores while they fit the store limit, then X86's
    /// `emitConstantSizeRepmov` (`rep movs`, whatever the size or alignment),
    /// and the loads and stores whatever their number only where `rep movs`
    /// does not apply (a far pointer).
    pub(super) fn always_inline_memcpy(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: u64,
        align: u64,
    ) -> Result<Option<SDValue>, Refused> {
        // The pin's own limits, not the target's: m16's override is for the
        // load and store pairs of a `memcpy` (SD225); a byval argument is
        // three dword pushes where the table's 2 would make it `rep movs`
        // (tests/byval_copy_at_os.rs).
        let pin = llrm_mir::target::Tuning::PIN;
        let limit = if self.has_opt_size() { pin.max_stores_per_memcpy_opt_size } else { pin.max_stores_per_memcpy };
        if let Some(copy) = self.inline_memcpy(chain, dst, src, size, limit, align, align)? {
            return Ok(Some(copy));
        }
        if self.is_far(dst) || self.is_far(src) {
            return self.inline_memcpy(chain, dst, src, size, usize::MAX, align, align);
        }
        self.rep_movs(chain, dst, src, size, align.min(self.string_block()), align, align).map(Some)
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
        let block = if self.has_min_size() { 1 } else { alignment };
        let (count, left) = (size / block, size % block);
        let (cx, di, si) = self.string_registers();
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
        // DEVIATE SD224 (real mode): tuned for size, the bytes past the last
        // block are `movsw` and `movsb` glued after the move, not a load and a
        // store from addresses made again (the pin's tail): 1 byte each. The
        // old selector's tail. Owner w12. Exit: a cost hook on the target.
        if self.has_opt_size() && self.arch.string_registers().is_some() {
            let mut last = rep;
            let mut rest = left;
            for width in [2u64, 1] {
                if rest >= width {
                    let kind = self.dag.get_value_type_node(EVT::integer(width as u32 * 8));
                    last = self
                        .dag
                        .get_node_multi(
                            x86isd::REP_MOVS,
                            &[EVT::OTHER, EVT::GLUE],
                            &[SDValue { node: last.node, res: 0 }, kind, SDValue { node: last.node, res: 1 }],
                        );
                    rest -= width;
                }
            }
            return Ok(SDValue { node: last.node, res: 0 });
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

    /// The widest block a `rep movs` moves: a quadword where the pointer is one
    /// (`emitConstantSizeRepmovs` takes `i64` at 8-byte alignment on x86-64).
    fn string_block(&self) -> u64 {
        u64::from(self.target.dag.pointer.size_in_bits() / 8).max(4)
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
        let minsize = self.has_min_size();
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
            // `getOptimalRepType`: a dword at most on a target with 32-bit
            // registers, whatever its pointers; for size the pointer's width
            // (`mov eax, 0` is more bytes: fpbench.nib +3 at -Os).
            let block = alignment.min(if self.has_opt_size() { word } else { word.max(4) });
            let mut pattern = byte as u64 & 255;
            if block >= 2 {
                pattern |= pattern << 8;
            }
            if block >= 4 {
                pattern |= pattern << 16;
            }
            if block >= 8 {
                pattern |= pattern << 32;
            }
            (block, size / block, self.dag.get_constant(u128::from(pattern), EVT::integer(block as u32 * 8)))
        } else {
            (1, size, value)
        };
        let _ = &mut val;
        let name = match block {
            1 => "al",
            2 => "ax",
            _ => "eax",
        };
        // `RAX` for a quadword block: the dword's name widened (`rax`).
        let acc = self.arch.register_named(&if block == 8 { format!("r{}", &name[1..]) } else { name.to_owned() });
        let (cx, di, _) = self.string_registers();
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

/// All ones at a pointer's width.
fn mask_of(vt: EVT) -> u128 {
    super::super::fold::mask(vt.size_in_bits())
}
