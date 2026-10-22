//! `llvm.memcpy` and `llvm.memset` (pin `SelectionDAG::getMemcpy`,
//! `getMemset` and `X86SelectionDAGInfo`): a constant size within the store
//! limit is a run of loads and stores (not yet built here), a constant size
//! up to 128 bytes at 4-byte alignment is `REP_MOVS`/`REP_STOS`, and the rest
//! is a call of `memcpy` or `memset`.

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Instruction, Operand};
use llrm_mir::opcode::{Attribute, CallInfo};

use super::super::dag::SDValue;
use super::super::isd::ISD;
use super::super::mvt::EVT;
use super::super::x86isd;
use super::{Builder, Refused, convention_for};
use crate::backend::isel::Convention;

/// `X86Subtarget::getMaxInlineSizeThreshold`.
const MAX_INLINE_SIZE: u64 = 128;

impl Builder<'_> {
    pub(super) fn visit_intrinsic(&mut self, instruction: &Instruction, info: &CallInfo, intrinsic: &Intrinsic) -> Result<(), Refused> {
        match intrinsic {
            Intrinsic::MemCpy | Intrinsic::MemSet | Intrinsic::MemMove => self.visit_mem_intrinsic(instruction, info, intrinsic),
            _ => Err(Refused("intrinsic call")),
        }
    }

    fn function_has(&self, name: &str) -> bool {
        llrm_mir::memory::has(&self.function.attrs, name)
    }

    /// The `align` attribute of argument `at`, or 1 (`valueOrOne`).
    fn argument_alignment(info: &CallInfo, at: usize) -> u64 {
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

    fn constant_size(&self, size: SDValue) -> Option<u64> {
        self.dag.constant(size).map(|bits| bits as u64)
    }

    /// How many memory operations `findOptimalMemOpLowering` makes of `size`
    /// bytes where the widest is `widest` bytes (the pointer's: no vectors
    /// or `i64` here), or `None` past `limit`.
    fn mem_op_count(size: u64, widest: u64, limit: usize, volatile: bool) -> Option<usize> {
        let (mut left, mut width, mut count) = (size, widest, 0usize);
        while left > 0 {
            let mut taken = width;
            while taken > left {
                let narrower = width / 2;
                if count > 0 && !volatile && narrower < left {
                    // a misaligned access is fast on x86: overlap the previous bytes
                    taken = left;
                } else {
                    width = narrower;
                    taken = narrower;
                }
            }
            count += 1;
            if count > limit {
                return None;
            }
            left -= taken;
        }
        Some(count)
    }

    fn visit_mem_intrinsic(&mut self, instruction: &Instruction, info: &CallInfo, which: &Intrinsic) -> Result<(), Refused> {
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
        let pointer = self.target.dag.pointer;
        let widest = u64::from(pointer.size_in_bits() / 8);
        let opt_size = self.function_has("optsize") || self.function_has("minsize");
        let constant = self.constant_size(size);
        if constant == Some(0) {
            return Ok(());
        }
        let chain = match which {
            Intrinsic::MemCpy | Intrinsic::MemMove => {
                let limit = if opt_size { 4 } else { 8 };
                if let Some(size) = constant {
                    if Self::mem_op_count(size, widest, limit, volatile).is_some() {
                        return Err(Refused("inline memcpy"));
                    }
                }
                match (which, constant) {
                    (Intrinsic::MemCpy, Some(bytes)) if self.function_has("minsize") => self.rep_movs(root, dst, other, bytes, 1)?,
                    (Intrinsic::MemCpy, Some(bytes)) if bytes <= MAX_INLINE_SIZE && alignment >= 4 => self.rep_movs(root, dst, other, bytes, alignment.min(4))?,
                    _ => self.mem_libcall(if matches!(which, Intrinsic::MemCpy) { "memcpy" } else { "memmove" }, root, &[dst, other], size)?,
                }
            }
            _ => {
                let limit = if opt_size { 8 } else { 16 };
                if let Some(size) = constant {
                    if Self::mem_op_count(size, widest, limit, volatile).is_some() {
                        return Err(Refused("inline memset"));
                    }
                }
                match constant {
                    Some(bytes) if self.function_has("minsize") || (bytes <= MAX_INLINE_SIZE && alignment >= 4) => self.rep_stos(root, dst, other, bytes, alignment)?,
                    _ => {
                        self.mem_libcall("memset", root, &[dst, other], size)?
                    }
                }
            }
        };
        self.dag.set_root(chain);
        Ok(())
    }

    /// `getMemcpy`'s libcall: `name(dst, src, size)` for a copy, `name(dst,
    /// value, size)` for a set; the result is read and dropped.
    fn mem_libcall(&mut self, name: &str, chain_in: SDValue, first_two: &[SDValue], size: SDValue) -> Result<SDValue, Refused> {
        let types = &self.module.context.types;
        let pointer_type = types.ptr(0);
        let word = types.int(self.target.dag.pointer.size_in_bits());
        let symbol = self.dag.get_external_symbol(self.target.dag.pointer, name);
        let value_type = if name == "memset" { types.int(self.dag.value_type(first_two[1]).size_in_bits()) } else { pointer_type };
        let arg_types = [pointer_type, value_type, word];
        let first = self.arch.first_argument_offset(false);
        let plan: Convention = convention_for(self.module, self.layout, self.arch, first, 0, pointer_type, false, &arg_types, &[])
            .map_err(|_| Refused("libcall the convention does not describe"))?;
        let target_symbol = self.dag.get_target_external_symbol(self.target.dag.pointer, name);
        let _ = symbol;
        let values = vec![first_two[0], first_two[1], size];
        self.dag.set_root(chain_in);
        let extends = [ISD::AnyExtend; 3];
        let _ = self.lower_call(target_symbol, values, &extends, &plan, first, pointer_type)?;
        Ok(self.dag.get_root())
    }

    fn reg(&self, wide: &str, narrow: &str) -> llrm_lir::registers::RegId {
        self.arch.register_named(if self.target.dag.pointer == EVT::I16 { narrow } else { wide })
    }

    /// `emitRepmovs` with the block width of `emitConstantSizeRepmov`.
    fn rep_movs(&mut self, chain: SDValue, dst: SDValue, src: SDValue, size: u64, alignment: u64) -> Result<SDValue, Refused> {
        let pointer = self.target.dag.pointer;
        let block = if self.function_has("minsize") { 1 } else { alignment.min(u64::from(pointer.size_in_bits() / 8)) };
        let (count, left) = (size / block, size % block);
        if left != 0 {
            return Err(Refused("rep movs with a tail"));
        }
        let (cx, di, si) = (self.reg("ecx", "cx"), self.reg("edi", "di"), self.reg("esi", "si"));
        let count = self.dag.get_constant(u128::from(count), pointer);
        let copy = self.dag.get_copy_to_phys_reg(chain, cx, count, None);
        let copy = self.dag.get_copy_to_phys_reg(SDValue { node: copy.node, res: 0 }, di, dst, Some(SDValue { node: copy.node, res: 1 }));
        let copy = self.dag.get_copy_to_phys_reg(SDValue { node: copy.node, res: 0 }, si, src, Some(SDValue { node: copy.node, res: 1 }));
        let block_type = self.dag.get_value_type_node(EVT::integer(block as u32 * 8));
        Ok(self.dag.get_node_multi(x86isd::REP_MOVS, &[EVT::OTHER, EVT::GLUE], &[SDValue { node: copy.node, res: 0 }, block_type, SDValue { node: copy.node, res: 1 }]))
    }

    /// `emitConstantSizeRepstos`.
    fn rep_stos(&mut self, chain: SDValue, dst: SDValue, value: SDValue, size: u64, alignment: u64) -> Result<SDValue, Refused> {
        let pointer = self.target.dag.pointer;
        let minsize = self.function_has("minsize");
        let word = u64::from(pointer.size_in_bits() / 8);
        let (block, count, mut val) = if minsize {
            match self.dag.constant(value) {
                Some(byte) if size & 31 == 0 && byte & 255 == 0 => (word, size / word, self.dag.get_constant(0, EVT::integer(word as u32 * 8))),
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
            if size % block != 0 {
                return Err(Refused("rep stos with a tail"));
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
        let copy = self.dag.get_copy_to_phys_reg(SDValue { node: copy.node, res: 0 }, cx, count, Some(SDValue { node: copy.node, res: 1 }));
        let copy = self.dag.get_copy_to_phys_reg(SDValue { node: copy.node, res: 0 }, di, dst, Some(SDValue { node: copy.node, res: 1 }));
        let block_type = self.dag.get_value_type_node(EVT::integer(block as u32 * 8));
        Ok(self.dag.get_node_multi(x86isd::REP_STOS, &[EVT::OTHER, EVT::GLUE], &[SDValue { node: copy.node, res: 0 }, block_type, SDValue { node: copy.node, res: 1 }]))
    }
}
