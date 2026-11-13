//! `llvm.memcpy` and `llvm.memset` (pin `SelectionDAG::getMemcpy`,
//! `getMemset` and `X86SelectionDAGInfo`): a constant size within the store
//! limit is a run of loads and stores, a constant size up to 128 bytes at
//! 4-byte alignment is `REP_MOVS`/`REP_STOS` (the bytes past the last block
//! stored as above), and the rest is a call of `memcpy` or `memset`.

/// A `rep movsw` in 32-bit code is a byte longer than `rep movsb`: copies of at least this many bytes win 48 clocks for it.
const REP_WORD_MIN_BYTES: u64 = 32;

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Instruction, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, CallInfo, Opcode};

use super::super::dag::{MemBase, MemInfo, NodeData, SDNodeFlags, SDValue};
use super::super::isd::ISD;
use super::super::mvt::{EVT, MVT};
use super::super::target_calling::TargetCalling;
use super::LoweringNodes;
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
            // The expansion is x86's `rep movs` and `rep stos` and its
            // registers.
            Intrinsic::MemCpy | Intrinsic::MemSet | Intrinsic::MemMove
                if self.arch.full().is_none() && self.calling().is_none() =>
            {
                Err(Refused("a mem intrinsic for a target with no string instructions"))
            }
            Intrinsic::MemCpy | Intrinsic::MemSet | Intrinsic::MemMove => {
                self.visit_mem_intrinsic(instruction, info, intrinsic)
            }
            Intrinsic::MemSetPattern => self.visit_memset_pattern(instruction),
            Intrinsic::FunnelShift { left } => self.visit_funnel_shift(instruction, *left),
            Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd => {
                self.visit_lifetime(instruction, matches!(intrinsic, Intrinsic::LifetimeStart))
            }
            Intrinsic::VaStart | Intrinsic::VaEnd | Intrinsic::VaCopy => self.visit_va(instruction, intrinsic),
            Intrinsic::VarargAddress => {
                let index =
                    self.info.vararg_frame_index.ok_or(Refused("a vararg address in a function with no varargs"))?;
                let node = self.dag.get_frame_index(self.target.dag.pointer, index);
                self.set_value(instruction.result.ok_or(Refused("a vararg address without a result"))?, node);
                Ok(())
            }
            Intrinsic::PtrDiff => {
                let (a, b) = (self.get_value(instruction.operands[0])?, self.get_value(instruction.operands[1])?);
                let vt = self.type_of(instruction.ty)?;
                let node = self
                    .dag
                    .get_node(LoweringNodes::need(self.target.dag.nodes.ptrdiff, "pointer difference")?, vt, &[a, b]);
                self.set_value(instruction.result.ok_or(Refused("a pointer difference without a result"))?, node);
                Ok(())
            }
            Intrinsic::Window => {
                let pointer = self.get_value(instruction.operands[0])?;
                let node = self
                    .dag
                    .get_node(
                        LoweringNodes::need(self.target.dag.nodes.window, "pointer window")?,
                        EVT::Simple(super::super::mvt::MVT::Far),
                        &[pointer],
                    );
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
                    let node = self
                        .dag
                        .get_node_multi(
                            LoweringNodes::need(self.target.dag.nodes.port_io, "port io")?,
                            &[vt, EVT::OTHER],
                            &[root, port],
                        );
                    self.dag.set_root(SDValue { node: node.node, res: 1 });
                    self.set_value(instruction.result.ok_or(Refused("a port read without a result"))?, node);
                } else {
                    let value = self.get_value(instruction.operands[1])?;
                    let node = self
                        .dag
                        .get_node_multi(
                            LoweringNodes::need(self.target.dag.nodes.port_io, "port io")?,
                            &[EVT::OTHER],
                            &[root, port, value],
                        );
                    self.dag.set_root(node);
                }
                Ok(())
            }
            // `case Intrinsic::trap`: a `TRAP` node on the chain (selected
            // as `ud2`).
            Intrinsic::Trap => {
                let root = self.root();
                let trap = self.dag.get_node(ISD::Trap, EVT::OTHER, &[root]);
                self.dag.set_root(trap);
                Ok(())
            }
            // `case Intrinsic::debugtrap`: a `DEBUGTRAP` node (selected
            // as `int3`).
            Intrinsic::DebugTrap => {
                let root = self.root();
                let trap = self.dag.get_node(ISD::DebugTrap, EVT::OTHER, &[root]);
                self.dag.set_root(trap);
                Ok(())
            }
            // `case Intrinsic::ubsantrap`: `UBSAN_UD1` (a `ud1` with the kind
            // as its displacement) is not translated: a named refusal, not a
            // panic.
            Intrinsic::UbsanTrap => Err(Refused("llvm.ubsantrap: not translated")),
            // `case Intrinsic::experimental_noalias_scope_decl: return;`
            Intrinsic::Assume | Intrinsic::Prefetch | Intrinsic::FakeUse | Intrinsic::NoAliasScopeDecl => Ok(()),
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
            Intrinsic::Expect | Intrinsic::Restrict | Intrinsic::StartLoopIterations => {
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
            | Intrinsic::LLRint
            | Intrinsic::Unary(_) => self.visit_math(instruction, intrinsic),
            // the counter a phi carries, less what an iteration takes off
            Intrinsic::LoopDecrementReg => {
                let (left, right) =
                    (self.get_value(instruction.operands[0])?, self.get_value(instruction.operands[1])?);
                let vt = self.type_of(instruction.ty)?;
                let node = self.dag.get_node(ISD::Sub, vt, &[left, right]);
                self.set_value(instruction.result.ok_or(Refused("a decrement without a result"))?, node);
                Ok(())
            }
            _ => Err(Refused("intrinsic call")),
        }
    }

    /// `getSrcValue` of the IR pointer `operand`.
    fn src_value_of(
        &mut self,
        operand: Operand,
    ) -> SDValue {
        let key = match operand {
            Operand::Value(value) => value.0,
            Operand::Constant(id) => 1 << 31 | id.index() as u32,
            _ => 0,
        };
        self.dag.get_src_value(key)
    }

    /// `visitVAArg`: `VAARG` of the list at the root, the type's ABI
    /// alignment as the last operand; the chain it returns is the new root.
    pub(super) fn visit_va_arg(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), Refused> {
        let vt = self.type_of(instruction.ty)?;
        let mut align = self.layout.align(&self.module.context.types, instruction.ty);
        if let Some(cap) = self.target.va_arg_align_cap {
            align = align.min(u64::from(cap));
        }
        let root = self.root();
        let list = self.get_value(instruction.operands[0])?;
        let source = self.src_value_of(instruction.operands[0]);
        let align = self.dag.get_constant_with(align as u128, EVT::I32, true, false);
        let node = self.dag.get_node_multi(ISD::VAArg, &[vt, EVT::OTHER], &[root, list, source, align]);
        self.dag.set_root(SDValue { node: node.node, res: 1 });
        self.set_value(instruction.result.ok_or(Refused("a va_arg without a result"))?, node);
        Ok(())
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
        if let Some(area) = self.info.variadic_area.clone() {
            return self.visit_va_in_area(instruction, intrinsic, &area);
        }
        if self.target.dag.pointer == EVT::I64
            && matches!(intrinsic, Intrinsic::VaStart)
            && self.dag.var_args_save.is_none()
            && !self.win64_va
        {
            return Err(Refused("va_start outside the entry block"));
        }
        if let Some(buffer) = self.info.variadic_buffer {
            return self.visit_va_in_buffer(instruction, intrinsic, buffer);
        }
        let root = self.root();
        let mut ops = vec![root];
        for operand in &instruction.operands[..if matches!(intrinsic, Intrinsic::VaCopy) { 2 } else { 1 }] {
            ops.push(self.get_value(*operand)?);
        }
        for operand in &instruction.operands[..ops.len() - 1] {
            ops.push(self.src_value_of(*operand));
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

    /// `LowerVASTART` of a target whose `va_list` is words over the frame: each
    /// word stored to the list. `VAEND` is its chain, `VACOPY` a copy of the
    /// list's 8-byte words.
    fn visit_va_in_area(
        &mut self,
        instruction: &Instruction,
        intrinsic: &Intrinsic,
        area: &super::super::target_calling::VariadicArea,
    ) -> Result<(), Refused> {
        use super::super::target_calling::VaWord;
        let pointer = self.target.dag.pointer;
        let memory = |mem_vt: EVT| MemInfo {
            mem_vt,
            align: mem_vt.size_in_bits() / 8,
            volatile: false,
            base: MemBase::Unknown,
            ext: None,
            space: 0,
            ordering: None,
        };
        match intrinsic {
            Intrinsic::VaStart => {
                let list = self.get_value(instruction.operands[0])?;
                for &(offset, bytes, word) in &area.words {
                    let vt = if bytes == 8 { EVT::I64 } else { EVT::I32 };
                    let value = match word {
                        VaWord::Frame { index, offset } => {
                            let base = self.dag.get_frame_index(pointer, index);
                            if offset == 0 {
                                base
                            } else {
                                let step = self.dag.get_constant(offset as u64 as u128, pointer);
                                self.dag.get_node(ISD::Add, pointer, &[base, step])
                            }
                        }
                        VaWord::Constant(value) => {
                            self.dag.get_constant(value as u64 as u128 & ((1u128 << (bytes * 8)) - 1), vt)
                        }
                    };
                    let step = self.dag.get_constant(u128::from(offset), pointer);
                    let address = self.dag.get_node(ISD::Add, pointer, &[list, step]);
                    let root = self.root();
                    let store = self.dag.get_store(root, value, address, memory(vt));
                    self.dag.set_root(store);
                }
            }
            Intrinsic::VaCopy => {
                let destination = self.get_value(instruction.operands[0])?;
                let source = self.get_value(instruction.operands[1])?;
                for offset in (0..area.size).step_by(8) {
                    let step = self.dag.get_constant(u128::from(offset), pointer);
                    let from = self.dag.get_node(ISD::Add, pointer, &[source, step]);
                    let to = self.dag.get_node(ISD::Add, pointer, &[destination, step]);
                    let root = self.root();
                    let load = self.dag.get_load(EVT::I64, root, from, memory(EVT::I64));
                    let store = self.dag.get_store(SDValue { node: load.node, res: 1 }, load, to, memory(EVT::I64));
                    self.dag.set_root(store);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `LowerVASTART` of a target whose `va_list` is a pointer to the variadic
    /// buffer: the buffer's pointer, stored to the list. `VAEND` is
    /// expanded to its chain, `VACOPY` to a load and a store of the pointer
    /// (`LegalizeDAG`'s `ExpandNode`).
    fn visit_va_in_buffer(
        &mut self,
        instruction: &Instruction,
        intrinsic: &Intrinsic,
        buffer: u32,
    ) -> Result<(), Refused> {
        let pointer = self.target.dag.pointer;
        let info = MemInfo {
            mem_vt: pointer,
            align: pointer.size_in_bits() / 8,
            volatile: false,
            base: MemBase::Unknown,
            ext: None,
            space: 0,
            ordering: None,
        };
        match intrinsic {
            Intrinsic::VaStart => {
                let list = self.get_value(instruction.operands[0])?;
                let entry = self.dag.entry_node();
                let value = self.dag.get_copy_from_reg(entry, buffer, pointer);
                let root = self.root();
                let store = self.dag.get_store(root, value, list, info);
                self.dag.set_root(store);
            }
            Intrinsic::VaCopy => {
                let destination = self.get_value(instruction.operands[0])?;
                let source = self.get_value(instruction.operands[1])?;
                let root = self.root();
                let load = self.dag.get_load(pointer, root, source, info);
                let store = self.dag.get_store(SDValue { node: load.node, res: 1 }, load, destination, info);
                self.dag.set_root(store);
            }
            _ => {}
        }
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
        // `MFI.getObjectSize` of the frame object, which `visitLIFETIME_END`
        // bounds the dying stores by.
        if let ValueDef::Instruction(def) = self.function.value(object).def
            && let Opcode::Alloca { allocated, .. } = self.function.instruction(def).opcode
        {
            let count = match self.function.instruction(def).operands.first() {
                None => Some(1),
                Some(Operand::Constant(id)) => match self.module.context.get(*id).kind {
                    ConstantKind::Int(bits) => u64::try_from(bits).ok(),
                    _ => None,
                },
                Some(_) => None,
            };
            if let Some(count) = count {
                let bytes = self.layout.alloc_size(&self.module.context.types, allocated).saturating_mul(count);
                let at = usize::try_from(index).unwrap_or(0);
                if self.dag.frame_object_bytes.len() <= at {
                    self.dag.frame_object_bytes.resize(at + 1, 0);
                }
                self.dag.frame_object_bytes[at] = self.dag.frame_object_bytes[at].max(bytes);
            }
        }
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

    /// A constant copy is `rep movsb`: `hasMinSize`, or `-Os` on a target
    /// that says so (DEVIATE REP-MOVS-OS, `Tuning::rep_movs_at_size`).
    pub(super) fn rep_movsb_copy(&self) -> bool {
        self.has_min_size()
            || (self.has_opt_size() && self.target.tti.as_ref().is_some_and(|tti| tti.0.rep_movs_at_size()))
    }

    /// DEVIATE REP-MOVS-OS keeps `rep movs` for any size at `-Os`, but moves the widest block the copy is a whole
    /// number of and the pointers are aligned to (`emitConstantSizeRepmov`'s `AVT`, without a tail): the same
    /// bytes as `rep movsb`, a block of 4 bytes for the same 3 clocks. A block of 2 carries the operand-size
    /// prefix in 32-bit code, one byte that 1.5 clocks a byte copied pay back from `REP_WORD_MIN_BYTES`.
    fn exact_rep_block(
        &self,
        size: u64,
        alignment: u64,
    ) -> u64 {
        [4, 2]
            .into_iter()
            .filter(|block| *block <= self.string_block())
            .filter(|block| *block != 2 || self.string_block() < 4 || size >= REP_WORD_MIN_BYTES)
            .find(|block| alignment >= *block && size % *block == 0)
            .unwrap_or(1)
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

    /// A 16-byte piece of a volatile copy or fill is a volatile access: its
    /// loads and stores keep the mark (`x86_memop128`).
    fn mark_volatile_piece(
        &mut self,
        piece: SDValue,
    ) {
        if self.volatile_memory {
            self.dag.volatile_accesses.insert(piece.node);
        }
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
        // `findOptimalMemOpLowering` with no preferred type: the widest legal
        // integer. DEVIATE: a misaligned access is taken as fast
        // (wasm's `allowsMisalignedMemoryAccesses`). Exit: a hook on
        // the target when one that is not asks for it.
        if self.arch.full().is_none() && self.calling().is_some() {
            return u64::from(self.target.legal_ints.iter().copied().max().unwrap_or(32) / 8);
        }
        if self.target.dag.pointer.size_in_bits() == 64 && size >= 8 { 8 } else { Self::WIDEST_MEM_OP }
    }

    /// `getOptimalMemOpType` with SSE2: 16 bytes (`v4i32`, a `movups`/`movaps`
    /// pair; a zero is `xorps`) for a piece of 16 or more bytes, else
    /// [`Self::widest_mem_op`]. `integer_only` keeps the integer width: a
    /// fill that is no zero, a volatile move or a constant source (stored as
    /// immediates). A volatile copy or fill keeps its 16-byte pieces, each a
    /// volatile access (`MachineMemOperand`). x86-64 has SSE2 in its baseline;
    /// the 32-bit targets follow with their CPU's feature set.
    fn widest_mem_op_sse(
        &self,
        size: u64,
        integer_only: bool,
    ) -> u64 {
        let sse2 = self.target.dag.pointer.size_in_bits() == 64 && self.target.target_nodes.is_none();
        if sse2 && !integer_only && size >= 16 { 16 } else { self.widest_mem_op(size) }
    }

    /// `Tuning::x87_zero_fill_at_size` of this target's cost model.
    fn x87_zero_fill(&self) -> bool {
        self.target.tti.as_ref().is_some_and(|tti| tti.0.x87_zero_fill_at_size())
    }

    /// The alignment of a frame object a copy of `size` bytes fills, when the
    /// copy is expanded inline: the ABI alignment of the first piece's type if
    /// that is more, up to the natural stack alignment
    /// (`getMemcpyLoadsAndStores`' `DstAlignCanChange`: "Give the stack frame
    /// object a larger alignment if needed"). A copy by library call keeps
    /// `align`.
    pub(super) fn copy_destination_alignment(
        &self,
        size: u64,
        align: u64,
        opt_size: bool,
    ) -> u64 {
        let tuning = &self.target.tuning;
        let limit = if opt_size { tuning.max_stores_per_memcpy_opt_size } else { tuning.max_stores_per_memcpy };
        let Some(first) = Self::mem_ops(size, self.widest_mem_op_sse(size, false), limit, self.volatile_memory)
            .and_then(|ops| ops.first().copied())
        else {
            return align;
        };
        let natural = self.target.stack_align.unwrap_or_else(|| self.arch.stack_alignment().max(1) as u64);
        let mut raised = first;
        while raised > align && raised > natural {
            raised /= 2;
        }
        align.max(raised)
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
        let widest = self.widest_mem_op_sse(size, slice.is_some());
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
        let (mut src_off, mut dst_off, mut left) = (0u64, 0u64, size);
        let mut chains = Vec::new();
        let mut order = chain;
        for width in ops {
            if width > left {
                src_off -= width - left;
                dst_off -= width - left;
            }
            if width == 16 {
                let align = common_alignment(src_align, src_off).min(common_alignment(dst_align, dst_off));
                let align = self.dag.get_constant(u128::from(align), EVT::I32);
                let from = self.dag.get_object_ptr_offset(src, src_off as i64);
                let to = self.dag.get_object_ptr_offset(dst, dst_off as i64);
                let copy = self
                    .dag
                    .get_node(
                        LoweringNodes::need(self.target.dag.nodes.copy128, "128-bit copy")?,
                        EVT::OTHER,
                        &[order, to, from, align],
                    );
                self.mark_volatile_piece(copy);
                if self.target.ordered_copies {
                    order = copy;
                }
                chains.push(copy);
                src_off += width;
                dst_off += width;
                left = left.saturating_sub(width);
                continue;
            }
            let vt = EVT::integer(width as u32 * 8);
            // `getMemsetStringVal`: the bytes from the source's offset, zero
            // past the end, as an immediate stored on the root
            if let Some((bytes, base)) = &slice {
                let mut value = 0u128;
                for at in 0..width {
                    let byte = bytes.as_ref().and_then(|bytes| bytes.get((base + src_off + at) as usize)).copied();
                    // the first byte is the low bits on a little-endian target
                    let place = if self.dag.config().big_endian { width - at - 1 } else { at };
                    value |= u128::from(byte.unwrap_or(0)) << (8 * place);
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
                    ordering: None,
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
                ordering: None,
            };
            let value = self.dag.get_load(vt, order, from, info);
            let to = self.dag.get_object_ptr_offset(dst, dst_off as i64);
            let info = MemInfo {
                mem_vt: vt,
                align: common_alignment(dst_align, dst_off) as u32,
                volatile: self.volatile_memory,
                base: MemBase::Unknown,
                ext: None,
                space: self.space_of(dst),
                ordering: None,
            };
            let store = self.dag.get_store(order, value, to, info);
            if self.target.ordered_copies {
                order = store;
            }
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
        let widest = self.widest_mem_op_sse(size, self.volatile_memory);
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
        if ops.contains(&16) {
            // DEVIATE SD540 (see `LLRM_MOVE128`): the pin's `v4i32` pieces are
            // loaded and stored by one node
            let (mut off, mut left) = (0u64, size);
            let mut operands = vec![chain];
            for width in &ops {
                if *width > left {
                    off -= width - left;
                }
                let align = common_alignment(src_align, off).min(common_alignment(dst_align, off));
                operands.push(self.dag.get_object_ptr_offset(dst, off as i64));
                operands.push(self.dag.get_object_ptr_offset(src, off as i64));
                operands.push(self.dag.get_constant(u128::from(*width), EVT::I32));
                operands.push(self.dag.get_constant(u128::from(align), EVT::I32));
                off += width;
                left = left.saturating_sub(*width);
            }
            return Ok(Some(self.dag.get_node(
                LoweringNodes::need(self.target.dag.nodes.move128, "128-bit move")?,
                EVT::OTHER,
                &operands,
            )));
        }
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
                ordering: None,
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
                ordering: None,
            };
            stores.push(self.dag.get_store(joined, value, to, info));
        }
        Ok(Some(self.dag.get_token_factor(&stores)))
    }

    /// DEVIATE SD223 (real mode): a constant memset whose stores take more
    /// bytes than a `rep stosb` fill is the fill. The pin counts stores
    /// (`MaxStoresPerMemset`); in real mode each dword store carries a `66`
    /// prefix and the fill costs `FILL_BYTES`. Tuned for size the bytes
    /// decide (clocks never do); at speed the fill is taken where the bytes
    /// it saves, at the target's `milliclocks_per_byte`, pay for the clocks it
    /// adds. Mirrors the old selector's `stored_bytes` rule. Owner w12. Exit:
    /// a store-cost hook on the target description.
    fn fill_is_smaller(
        &self,
        length: u64,
        opt_size: bool,
        fill: Option<u128>,
        frame: bool,
        alignment: u64,
    ) -> bool {
        let zero = fill == Some(0);
        if opt_size
            && fill.is_some()
            && self.target.dag.pointer.size_in_bits() == 64
            && self.target.target_nodes.is_none()
        {
            return self.sse_fill_is_longer(length, fill.unwrap_or(0));
        }
        if opt_size && let Some(costs) = self.target.tti.as_ref().and_then(|tti| tti.0.size_fill()) {
            // `rep stosd` takes whole aligned dwords; a tail or an unaligned
            // fill is `rep stosb` or stores again (rep_tails -Os
            // grew a byte).
            if length % 4 != 0 || alignment < 4 {
                return false;
            }
            let place = usize::from(!frame);
            let value = match fill {
                Some(0) => costs.value[0],
                Some(0xff) => costs.value[1],
                Some(_) => costs.value[2],
                None => return false,
            };
            let store = costs.store[place];
            let mut stores = length / 4 * store[0] + u64::from(length % 4 >= 2) * store[1] + length % 2 * store[2];
            // DEVIATE X87-ZERO-FILL stores whole doubles of zero as `fldz` (2)
            // and one `fst qword` each: its stores are the pin's to
            // beat, so the fill is taken only where it beats those
            // too.
            if fill == Some(0) && length >= 8 && length % 8 == 0 && !self.volatile_memory && self.x87_zero_fill() {
                stores = stores.min(2 + length / 8 * costs.fst[place]);
            }
            return stores > value + costs.setup[place];
        }
        if self.target.dag.pointer.size_in_bits() != 16 {
            return false;
        }
        let dwords = length / 4;
        let words = u64::from(length % 4 >= 2);
        let bytes = u64::from(length % 2 == 1);
        // The value in a register (`xor ecx, ecx` 3 B, `mov ecx, imm32` 6; each
        // store 4, 3, 3 B) or as immediates (`mov dword ptr [m], imm32` 8, a
        // word 5, a byte 4): whichever is fewer. A 10-byte clear is 14 bytes
        // of stores against 16 of fill (the object's, measured), a 7-byte one
        // 13.
        let setup = if zero { 3 } else { 6 };
        let register = setup + dwords * 4 + words * 3 + bytes * 3;
        let immediates = dwords * 8 + words * 5 + bytes * 4;
        let stored = register.min(immediates);
        if stored <= FILL_BYTES {
            return false;
        }
        if opt_size {
            return true;
        }
        let Some(trade) = self.target.size_trade else {
            return false;
        };
        let stores = (dwords * (trade.store + trade.prefix) as u64 + (words + bytes) * trade.store as u64) as i64;
        let fill = trade.fill + length as i64 * trade.fill_cell;
        (stored - FILL_BYTES) as i64 * trade.milliclocks_per_byte >= (fill - stores) * 1000
    }

    /// DEVIATE SD541 (x86-64, tuned for size): a constant fill whose 16-byte
    /// stores (and the `xorps`, or the pool load of the splat) take more bytes
    /// than a `rep stosq` fill is the fill. The pin counts stores
    /// (`MaxStoresPerMemsetOptSize`), so a 128-byte clear is eight `movaps`
    /// there (pattern_fill -Os +50 B, huffman_decode +12 B over the `rep stosq`
    /// this port had). The sizes are the encodings at a `[rsp+disp8]` address:
    /// `xorps` 3, the splat's `movaps` load 7, `movaps` store 5, `mov qword
    /// [m], 0` 8, and the fill: the value in `rax` (`xor eax, eax` 2, `mov
    /// rax, -1` 7, `movabs` 10), `lea rdi`, `mov ecx` and `rep stosq` 13.
    /// Owner m64. Exit 2026-11-20: a store-cost hook on the target
    /// description, as for SD223. The pin: [`getMemset`][memset] tries the
    /// stores first, within [`MaxStoresPerMemsetOptSize = 8`][limit].
    ///
    /// [memset]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L10376
    /// [limit]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L2937
    fn sse_fill_is_longer(
        &self,
        length: u64,
        byte: u128,
    ) -> bool {
        let value = match byte {
            0 => 2,
            0xff => 7,
            _ => 10,
        };
        let fill = value + 5 + 5 + 3;
        let Some(ops) = Self::mem_ops(length, 16, usize::MAX, self.volatile_memory) else { return false };
        let wide = ops.iter().filter(|width| **width == 16).count() as u64;
        let setup = match (wide, byte) {
            (0, _) => 0,
            (_, 0) => 3,
            _ => 7,
        };
        setup + wide * 5 + (ops.len() as u64 - wide) * 8 > fill
    }

    /// DEVIATE MEM-OS (`Tuning::size_copy_price`, x86-64 tuned for size):
    /// whether the load and store pieces of a constant copy take more bytes
    /// than the call, at the target's prices. A copy from a constant
    /// (stored as immediates) and a volatile one keep the pin's pieces.
    fn copy_is_longer_than_call(
        &self,
        length: u64,
        source: SDValue,
    ) -> bool {
        let Some(price) = self.target.tti.as_ref().and_then(|tti| tti.0.copy_price_for_size()) else {
            return false;
        };
        if self.volatile_memory || self.constant_slice(source).is_some() {
            return false;
        }
        let Some(ops) = Self::mem_ops(length, self.widest_mem_op_sse(length, false), usize::MAX, false) else {
            return false;
        };
        let pieces: u64 = ops
            .iter()
            .map(|width| match width {
                16 | 8 => price.wide_piece,
                _ => price.narrow_piece,
            })
            .sum();
        pieces > price.call
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
        let zero = self.dag.constant(value) == Some(0);
        // A constant byte is a splat in the pool (`v4i32` of the pin); only a
        // value held in a register has no 16-byte form here.
        let widest = self.widest_mem_op_sse(size, self.dag.constant(value).is_none());
        let Some(ops) = Self::mem_ops(size, widest, limit, self.volatile_memory) else { return Ok(None) };
        // DEVIATE X87-ZERO-FILL (owner x87, exit 2026-11-27): at `-Os` on a
        // target whose floats are on the x87 stack, a zero fill of whole
        // doubles is `fldz` and one `fst qword` per double (4 bytes at a short
        // displacement), not two `mov dword` zero stores (8): nbody m32 -Os,
        // 127 bytes of fills against gcc's 92. The pin's
        // `getOptimalMemOpType` is `i32` without SSE (clang stores dwords;
        // `X86TargetLowering::getOptimalMemOpType`, X86ISelLoweringCall.cpp at
        // d1106deb71cc, line not read here). `Tuning::x87_zero_fill_at_size`
        // gates it per target (and not with SSE2, in the Tti getter). Whole
        // doubles only: a dword tail needs the integer zero register anyway,
        // and the mixed form moved x_jpeg_single -Os +5 B through the
        // allocation it changed.
        let fpu_zero =
            zero && size >= 8 && size % 8 == 0 && self.has_opt_size() && !self.volatile_memory && self.x87_zero_fill();
        let ops = if fpu_zero { vec![8u64; (size / 8) as usize] } else { ops };
        let largest = ops.iter().copied().filter(|width| *width != 16).max().unwrap_or(8);
        let largest_vt = EVT::integer(largest as u32 * 8);
        let splat = self.memset_value(value, largest_vt);
        let (mut off, mut left) = (0u64, size);
        let mut chains = Vec::new();
        for width in ops {
            if width > left {
                off -= width - left;
            }
            if fpu_zero && width == 8 {
                let to = self.dag.get_object_ptr_offset(dst, off as i64);
                let info = MemInfo {
                    mem_vt: EVT::Simple(MVT::F64),
                    align: common_alignment(align, off) as u32,
                    volatile: false,
                    base: MemBase::Unknown,
                    ext: None,
                    space: self.space_of(dst),
                    ordering: None,
                };
                let zero_double = self.dag.get_constant_fp(0, EVT::Simple(MVT::F64));
                chains.push(self.dag.get_store(chain, zero_double, to, info));
                off += width;
                left = left.saturating_sub(width);
                continue;
            }
            if width == 16 {
                let align = self.dag.get_constant(u128::from(common_alignment(align, off)), EVT::I32);
                let to = self.dag.get_object_ptr_offset(dst, off as i64);
                let node = if zero {
                    self.dag.get_node(
                        LoweringNodes::need(self.target.dag.nodes.zero128, "128-bit zero")?,
                        EVT::OTHER,
                        &[chain, to, align],
                    )
                } else {
                    let byte = self.dag.constant(value).unwrap_or(0) & 0xff;
                    let splat = (0..16).fold(0u128, |all, at| all | byte << (at * 8));
                    let pointer = self.dag.config().pointer;
                    let pool = self.dag.get_constant_pool(pointer, splat, EVT::Simple(MVT::I128), true);
                    let from =
                        self.dag.get_node(self.target.dag.nodes.wrapper_of(&self.dag, pointer)?, pointer, &[pool]);
                    self.dag.get_node(
                        LoweringNodes::need(self.target.dag.nodes.copy128, "128-bit copy")?,
                        EVT::OTHER,
                        &[chain, to, from, align],
                    )
                };
                self.mark_volatile_piece(node);
                chains.push(node);
                off += width;
                left = left.saturating_sub(width);
                continue;
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
                ordering: None,
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

    /// The low `count` bytes of `bits` as the target lays them in memory.
    fn memory_bytes(
        &self,
        bits: u128,
        count: usize,
    ) -> Vec<u8> {
        if self.dag.config().big_endian {
            bits.to_be_bytes()[16 - count..].to_vec()
        } else {
            bits.to_le_bytes()[..count].to_vec()
        }
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
                bytes.get_mut(at..at + count)?.copy_from_slice(&self.memory_bytes(*bits, count));
            }
            ConstantKind::Float(bits) => match types.get(constant.ty) {
                Type::Float(kind @ (FloatKind::Float | FloatKind::Double)) => {
                    let count = kind.value_bytes() as usize;
                    bytes.get_mut(at..at + count)?.copy_from_slice(&self.memory_bytes(u128::from(*bits), count));
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
            let routine = self.full_arch()?.huge_fill_routine(1).map_err(|_| Refused("no huge fill routine"))?;
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
            let routine = self.full_arch()?.huge_copy_routine(backward).map_err(|_| Refused("no huge copy routine"))?;
            (routine, vec![count, from_selector, from_offset, to_selector, to_offset])
        };
        if routine.reads.len() != values.len() {
            return Err(Refused("a huge routine whose registers are not the operands"));
        }
        let args: Vec<(SDValue, Place)> =
            values.into_iter().zip(&routine.reads).map(|(value, reg)| (value, Place::Register(*reg))).collect();
        let callee = self.dag.get_target_external_symbol(word, routine.name);
        let root = self.memory_root();
        let env = CallEnv { arch: self.full_arch()?, pointer: word, call: self.call_node() };
        let (chain, _) = lib_call(
            &mut self.dag,
            &env,
            root,
            callee,
            &args,
            0,
            0,
            &[],
            &super::super::dagisel_adapter::call_clobbered_flags(),
        );
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
        if let (None, Some(calling)) = (self.arch.full(), self.calling()) {
            return self.lower_hooked_mem_intrinsic(
                calling,
                which,
                [dst, other, size],
                [dst_align, src_align],
                opt_size,
            );
        }
        // `rep movs` and `rep stos` are x86's: a target without string
        // instructions copies past the inline limit with a call
        let strings_exist = self.target.string_instructions;
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
                    && self.string_instructions().is_some()
                    && constant.is_some_and(|bytes| bytes / 4 + u64::from((bytes % 4).count_ones()) > limit as u64);
                let inline = match constant {
                    Some(_) if strings => None,
                    Some(bytes) if opt_size && self.copy_is_longer_than_call(bytes, other) => None,
                    Some(bytes) if matches!(which, Intrinsic::MemCpy) => {
                        self.inline_memcpy(root, dst, other, bytes, limit, dst_align, src_align)?
                    }
                    Some(bytes) => self.inline_memmove(root, dst, other, bytes, limit, dst_align, src_align)?,
                    _ => None,
                };
                // a target with string instructions of its own
                // (`EmitTargetCodeForMemcpy`) copies what is
                // left past the inline limit with them, where the pin would
                // call `memcpy`
                let hooked = match (&inline, which) {
                    (None, Intrinsic::MemCpy) if !strings_exist => self
                        .calling()
                        .and_then(|calling| calling.emit_memcpy_sized(&mut self.dag, root, dst, other, size, opt_size)),
                    _ => None,
                };
                match (inline, which, constant) {
                    (Some(chain), _, _) => chain,
                    (None, Intrinsic::MemCpy, _) if hooked.is_some() => hooked.expect("checked"),
                    (None, Intrinsic::MemCpy, Some(bytes)) if strings_exist && self.rep_movsb_copy() => {
                        self.rep_movs(root, dst, other, bytes, dst_align.min(src_align), dst_align, src_align)?
                    }
                    (None, Intrinsic::MemCpy, Some(bytes))
                        if strings_exist && bytes <= MAX_INLINE_SIZE && alignment >= 4 =>
                    {
                        self.rep_movs(
                            root,
                            dst,
                            other,
                            bytes,
                            alignment.min(self.string_block()),
                            dst_align,
                            src_align,
                        )?
                    }
                    // no `memcpy` to call for a copy the pin leaves to one
                    // (alignment under 4): dwords, as the old selector copies;
                    // x86 takes them unaligned
                    (None, Intrinsic::MemCpy, Some(bytes)) if strings_exist && !self.runtime_has("memcpy") => {
                        self.rep_movs(root, dst, other, bytes, 4, dst_align, src_align)?
                    }
                    _ if strings_exist && matches!(which, Intrinsic::MemCpy) && !self.runtime_has("memcpy") => {
                        self.rep_movs_dwords(root, dst, other, size)
                    }
                    // DEVIATE MEM-1: a length known at run time is `rep movsd`
                    // inline at -O2/-O3 where the target's tuning says so (the
                    // pin calls `memcpy`; -O1 and size keep the call)
                    _ if strings_exist
                        && matches!(which, Intrinsic::MemCpy)
                        && constant.is_none()
                        && !opt_size
                        && !self.target.bytes_strict
                        && self.target.tti.as_ref().is_some_and(|tti| tti.0.inlines_variable_memcpy()) =>
                    {
                        self.rep_movs_dwords(root, dst, other, size)
                    }
                    // a move a pass proved forward, past the inline limit: `rep
                    // movs` ascends, as the move needs (DEVIATE: the pin calls
                    // `memmove`; at m16 the old selector inlines it, 2.2x on
                    // scroll.c)
                    _ if strings_exist
                        && forward
                        && matches!(which, Intrinsic::MemMove)
                        && self.inline_string_move(dst, other) =>
                    {
                        self.rep_movs_dwords(root, dst, other, size)
                    }
                    // proved backward: from the last dword down
                    _ if strings_exist
                        && backward
                        && matches!(which, Intrinsic::MemMove)
                        && self.inline_string_move(dst, other) =>
                    {
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
                        info,
                        operands[0],
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
                    Some(bytes)
                        if !self.fill_is_smaller(
                            bytes,
                            opt_size,
                            self.dag.constant(other).map(|byte| byte & 0xff),
                            self.dag.opcode(dst) == ISD::FrameIndex,
                            alignment,
                        ) =>
                    {
                        self.inline_memset(root, dst, other, bytes, limit, alignment)?
                    }
                    _ => None,
                };
                // a target with no string instructions fills what is left past the inline limit
                // with its own node (`EmitTargetCodeForMemset`), where the pin would call `memset`
                let hooked = match &inline {
                    None if !strings_exist => self
                        .calling()
                        .and_then(|calling| calling.emit_memset(&mut self.dag, root, dst, other, size)),
                    _ => None,
                };
                match (inline, constant) {
                    (Some(chain), _) => chain,
                    (None, _) if hooked.is_some() => hooked.expect("checked"),
                    (None, Some(bytes))
                        if strings_exist
                            && (self.has_min_size()
                                || (opt_size
                                    && self.target.tti.as_ref().is_some_and(|tti| tti.0.rep_stos_at_size()))
                                || (bytes <= MAX_INLINE_SIZE && alignment >= 4)) =>
                    {
                        self.rep_stos(root, dst, other, bytes, alignment)?
                    }
                    // no `memset` to call: dwords, as the old selector fills;
                    // x86 takes them unaligned. Only a size of whole dwords (a
                    // tail costs the bytes the dwords save, and a far one
                    // cannot be named) and not for size: `xor eax, eax` is a
                    // byte more than `xor al, al`. A size of whole words at
                    // word alignment is words (DEVIATE SD442: the pin calls
                    // `memset` there, as for a size not known at compile time).
                    (None, Some(bytes))
                        if strings_exist
                            && !opt_size
                            && !self.runtime_has("memset")
                            && (bytes % 4 == 0 || (bytes % 2 == 0 && alignment >= 2)) =>
                    {
                        self.rep_stos(root, dst, other, bytes, if bytes % 4 == 0 { 4 } else { 2 })?
                    }
                    // for size, whole pointer-width units at that alignment: a
                    // `rep stosb` runs a clock per byte (matmul.nib -Os +669)
                    (None, Some(bytes))
                        if strings_exist
                            && opt_size
                            && !self.runtime_has("memset")
                            && alignment >= self.pointer_bytes()
                            && bytes % self.pointer_bytes() == 0 =>
                    {
                        self.rep_stos(root, dst, other, bytes, self.pointer_bytes())?
                    }
                    _ if strings_exist && !self.runtime_has("memset") => self.rep_stosb(root, dst, other, size),
                    _ => self.mem_libcall(info, operands[0], "memset", root, &[dst, other], size)?,
                }
            }
        };
        self.dag.set_root(chain);
        Ok(())
    }

    /// `getMemcpy`, `getMemmove` and `getMemset` for a target with a calling
    /// hook: the loads and stores of a constant size within the limit, then
    /// the target's node (`EmitTargetCodeFor*`), then the C library
    /// routine.
    pub(super) fn lower_hooked_mem_intrinsic(
        &mut self,
        calling: &'static dyn TargetCalling,
        which: &Intrinsic,
        [dst, other, size]: [SDValue; 3],
        [dst_align, src_align]: [u64; 2],
        opt_size: bool,
    ) -> Result<(), Refused> {
        let root = self.memory_root();
        let constant = self.constant_size(size);
        let tuning = &self.target.tuning;
        let chain = match which {
            Intrinsic::MemSet => {
                let limit = if opt_size { tuning.max_stores_per_memset_opt_size } else { tuning.max_stores_per_memset };
                let inline = match constant {
                    Some(bytes) => self.inline_memset(root, dst, other, bytes, limit, dst_align)?,
                    None => None,
                };
                match inline.or_else(|| calling.emit_memset(&mut self.dag, root, dst, other, size)) {
                    Some(chain) => chain,
                    None => self.hooked_mem_libcall("memset", root, [dst, other, size])?,
                }
            }
            _ => {
                let copy = matches!(which, Intrinsic::MemCpy);
                let limit = match (copy, opt_size) {
                    (true, true) => tuning.max_stores_per_memcpy_opt_size,
                    (true, false) => tuning.max_stores_per_memcpy,
                    (false, true) => tuning.max_stores_per_memmove_opt_size,
                    (false, false) => tuning.max_stores_per_memmove,
                };
                let inline = match constant {
                    Some(bytes) if copy => self.inline_memcpy(root, dst, other, bytes, limit, dst_align, src_align)?,
                    Some(bytes) => self.inline_memmove(root, dst, other, bytes, limit, dst_align, src_align)?,
                    None => None,
                };
                let hooked = |dag: &mut _| {
                    if copy {
                        calling.emit_memcpy_sized(dag, root, dst, other, size, opt_size)
                    } else {
                        calling.emit_memmove(dag, root, dst, other, size)
                    }
                };
                match inline.or_else(|| hooked(&mut self.dag)) {
                    Some(chain) => chain,
                    None => {
                        self.hooked_mem_libcall(if copy { "memcpy" } else { "memmove" }, root, [dst, other, size])?
                    }
                }
            }
        };
        self.dag.set_root(chain);
        Ok(())
    }

    /// The call of `memcpy`, `memmove` or `memset` through the target's
    /// `LowerCall`; the result (the destination) is dropped.
    fn hooked_mem_libcall(
        &mut self,
        name: &str,
        chain: SDValue,
        args: [SDValue; 3],
    ) -> Result<SDValue, Refused> {
        let calling = self.calling().expect("a target with a calling hook");
        let callee = self.dag.get_external_symbol(self.target.dag.pointer, name);
        let pointer = self.module.context.types.ptr(0);
        self.dag.set_root(chain);
        self.lower_call_with(calling, callee, &args, &[ISD::AnyExtend; 3], &[], pointer, false)?;
        Ok(self.dag.get_root())
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
            .full()
            .and_then(|arch| arch.runtime("c"))
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
        let rep = self
            .dag
            .get_node_multi(
                self.target.dag.nodes.string_move.expect("a target with string move"),
                &[EVT::OTHER, EVT::GLUE],
                &[chain, dword, glue],
            );
        // DEVIATE MEM-1: where the target inlines variable copies, a length
        // whose low two bits are known zero (`n << 2`) has no byte tail
        let inlines = !self.target.bytes_strict && self.target.tti.as_ref().is_some_and(|tti| tti.0.inlines_variable_memcpy());
        if self.constant_size(count).is_some_and(|bytes| bytes % 4 == 0)
            || inlines && super::super::known_bits::masked_value_is_zero(&self.dag, count, 3)
        {
            return SDValue { node: rep.node, res: 0 };
        }
        // The tail goes on from where the dword copy left di/si (read back
        // after the copy; the pin's constant tails add the offset instead).
        let three = self.dag.get_constant(3, pointer);
        let bytes = self.dag.get_node(ISD::And, pointer, &[count, three]);
        let mut chain = SDValue { node: rep.node, res: 0 };
        let mut glue = SDValue { node: rep.node, res: 1 };
        let mut left = Vec::new();
        for reg in [di, si] {
            let reg_node = self.dag.get_phys_register(pointer, reg);
            let read =
                self.dag.get_node_multi(ISD::CopyFromReg, &[pointer, EVT::OTHER, EVT::GLUE], &[chain, reg_node, glue]);
            left.push(SDValue { node: read.node, res: 0 });
            chain = SDValue { node: read.node, res: 1 };
            glue = SDValue { node: read.node, res: 2 };
        }
        for (reg, value) in [(cx, bytes), (di, left[0]), (si, left[1])] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let byte = self.dag.get_value_type_node(EVT::I8);
        let rep = self
            .dag
            .get_node_multi(
                self.target.dag.nodes.string_move.expect("a target with string move"),
                &[EVT::OTHER, EVT::GLUE],
                &[chain, byte, glue],
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
        let rep = self
            .dag
            .get_node_multi(
                self.target.dag.nodes.string_move.expect("a target with string move"),
                &[EVT::OTHER, EVT::GLUE],
                &[chain, byte, glue],
            );
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
        let mut code = if flat { vec![0xFD, 0xF3, 0xA5] } else { vec![0xFD, 0xF3, 0x66, 0xA5] };
        if tail {
            code.extend([0x83, 0xC6, 0x03, 0x83, 0xC7, 0x03, 0x89, 0xD1, 0xF3, 0xA4]);
        }
        code.push(0xFC);
        let strings = self.string_instructions();
        if strings.is_none() && !flat {
            return self.backward_rep_movsb(chain, dst, src, size);
        }
        let name_of = |register| self.arch.registers().name(register).expect("a described register");
        let mut names = match strings {
            Some(strings) => vec![name_of(strings.count), name_of(strings.source), name_of(strings.destination)],
            None => self.flat_string_names().to_vec(),
        };
        let mut values = vec![dwords, from, to];
        if tail {
            names.push(name_of(strings.expect("a flat move has no tail").tail));
            values.push(rest);
        }
        // ES is an input the emit step requires (`feeds_call`): the block
        // leaves it as the selector it was given, so late cleanup keeps one
        // load for every string move and drops it after a far access.
        let segment = strings.map(|strings| (name_of(strings.destination_segment), self.segment_of(None)));
        if let Some((name, selector)) = segment {
            names.push(name);
            values.push(selector);
        }
        let block = llrm_mir::intrinsics::AsmBlock {
            code,
            inputs: names.iter().map(|name| (*name).into()).collect(),
            outputs: Vec::new(),
            clobbers: names
                .iter()
                .copied()
                .filter(|name| Some(*name) != segment.map(|(es, _)| es))
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
        let (chain, _) = lower_asm(
            &mut self.dag,
            self.target.dag.nodes.asm.expect("a target with inline asm"),
            chain,
            &name,
            &[],
            &registers,
            &[],
        );
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
        let strings = self.string_instructions();
        let name_of = |register| self.arch.registers().name(register).expect("a described register");
        // A flat target names no string registers: `cx`, `si` and `di` are
        // its operands, and ES already holds what `pop es` loads.
        let segment = strings.map(|strings| (name_of(strings.destination_segment), self.segment_of(None)));
        let operands = match strings {
            Some(strings) => {
                [(name_of(strings.count), count), (name_of(strings.source), from), (name_of(strings.destination), to)]
            }
            None => {
                let [counter, source, destination] = self.flat_string_names();
                [(counter, count), (source, from), (destination, to)]
            }
        };
        let block = llrm_mir::intrinsics::AsmBlock {
            code: vec![0xFD, 0xF3, 0xA4, 0xFC],
            inputs: operands.iter().map(|(name, _)| *name).chain(segment.map(|(es, _)| es)).map(Into::into).collect(),
            outputs: Vec::new(),
            clobbers: operands.iter().map(|(name, _)| (*name).into()).collect(),
            memory: true,
            fixups: Vec::new(),
        };
        let name = llrm_mir::intrinsics::asm_name(&block);
        let registers: Vec<_> = operands
            .into_iter()
            .chain(segment)
            .map(|(name, value)| (self.sized_register(self.arch.register_named(name), pointer), value))
            .collect();
        let (chain, _) = lower_asm(
            &mut self.dag,
            self.target.dag.nodes.asm.expect("a target with inline asm"),
            chain,
            &name,
            &[],
            &registers,
            &[],
        );
        chain
    }

    /// `llvm.experimental.memset.pattern` of a word or dword cell: `rep stosw`
    /// / `rep stosd` of `count` cells, where the pin expands it to a loop
    /// (`fills_by_pattern`). A volatile or huge fill is refused.
    fn visit_memset_pattern(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), Refused> {
        let Some(&[destination, value, count, volatile]) = instruction.operands.first_chunk::<4>() else {
            return Err(Refused("a pattern fill of other than four operands"));
        };
        if !matches!(
            volatile,
            Operand::Constant(id) if matches!(self.module.context.get(id).kind, ConstantKind::Int(0))
        ) {
            return Err(Refused("a volatile pattern fill"));
        }
        let (dst, value, count) = (self.get_value(destination)?, self.get_value(value)?, self.get_value(count)?);
        let pointer = self.target.dag.pointer;
        let block = self.dag.value_type(value).size_in_bits() / 8;
        if !matches!(block, 2 | 4) || self.dag.value_type(count).size_in_bits() > pointer.size_in_bits() {
            return Err(Refused("a pattern fill of other than a word or dword cell, or counted past an address"));
        }
        if matches!(
            self.function.operand_type(&self.module.context, destination).map(|ty| self.module.context.types.get(ty)),
            Some(llrm_mir::types::Type::Pointer(space)) if self.layout.carries(*space)
        ) {
            return Err(Refused("a huge pattern fill"));
        }
        let root = self.memory_root();
        let (cx, di, _) = self.string_registers();
        let acc = self.accumulator(u64::from(block));
        let count = self.dag.get_zext_or_trunc(count, pointer);
        let copy = self.dag.get_copy_to_phys_reg(root, acc, value, None);
        let mut glue = SDValue { node: copy.node, res: 1 };
        let mut chain = SDValue { node: copy.node, res: 0 };
        for (reg, value) in [(cx, count), (di, dst)] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let cell = self.dag.get_value_type_node(EVT::integer(block * 8));
        let rep = self
            .dag
            .get_node_multi(
                self.target.dag.nodes.string_store.expect("a target with string store"),
                &[EVT::OTHER, EVT::GLUE],
                &[chain, cell, glue],
            );
        self.dag.set_root(SDValue { node: rep.node, res: 0 });
        Ok(())
    }

    /// `rep stos` of a size not known at compile time: the unit is the widest
    /// the size is known to be a whole multiple of (`n << 1` bytes of zero are
    /// `n` words), as `rep_stos` takes it from a constant size.
    ///
    /// DEVIATE SD442: the pin calls `memset` for a size not known at compile
    /// time; m16 has no libc `memset`, so this was already `rep stosb` here and
    /// only the unit is new. Every m16 profile has 32-bit operands (the lowest
    /// is the 386; llrm-cc rejects `-march=i286`), so a dword unit needs no
    /// gate. Like every string operation in this file it relies on DF being
    /// clear on entry (the `cld` convention). Exit: a runtime `memset` on
    /// m16 lands and this lowers to the call; this unit choice is then
    /// removed.
    fn rep_stosb(
        &mut self,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: SDValue,
    ) -> SDValue {
        let pointer = self.target.dag.pointer;
        let (cx, di, _) = self.string_registers();
        let word = u64::from(pointer.size_in_bits() / 8);
        let count = self.dag.get_zext_or_trunc(size, pointer);
        let whole = super::super::known_bits::known_bits(&self.dag, size).zero.trailing_ones();
        let block = match self.dag.constant(value) {
            // A constant size keeps the byte fill: its wider unit moved a store
            // and grew matmul and bintree by 4 bytes; the gain is a dynamic
            // one.
            Some(_) if self.dag.constant(size).is_some() => 1,
            // A word in 32-bit code is an operand-size prefix and no gain.
            Some(_) => match (1u64 << whole.min(2)).min(if self.has_opt_size() { word } else { word.max(4) }) {
                2 if word == 4 => 1,
                block => block,
            },
            _ => 1,
        };
        let (acc, val, count) = match (block, self.dag.constant(value)) {
            (1, _) => (self.accumulator(1), value, count),
            (block, Some(byte)) => {
                let mut pattern = byte as u64 & 255;
                pattern |= pattern << 8;
                if block >= 4 {
                    pattern |= pattern << 16;
                }
                let lg = block.trailing_zeros();
                let shift = self.dag.get_constant(u128::from(lg), EVT::I8);
                // `n << lg` with no unsigned wrap is `n << lg` bytes whole: the
                // count is `n`, with no shift pair (`add cx, cx ; shr cx, 1`).
                let peeled = (self.dag.opcode(size) == ISD::Shl
                    && self.dag.node(size).flags.0 & SDNodeFlags::NO_UNSIGNED_WRAP.0 != 0
                    && self.dag.constant(self.dag.operand(size, 1)) == Some(u128::from(lg)))
                .then(|| self.dag.operand(size, 0));
                let counted = match peeled {
                    Some(base) => self.dag.get_zext_or_trunc(base, pointer),
                    None => self.dag.get_node(ISD::Srl, pointer, &[count, shift]),
                };
                (
                    self.accumulator(block),
                    self.dag.get_constant(u128::from(pattern), EVT::integer(block as u32 * 8)),
                    counted,
                )
            }
            _ => unreachable!("a block past a byte has a constant value"),
        };
        let copy = self.dag.get_copy_to_phys_reg(chain, acc, val, None);
        let mut glue = SDValue { node: copy.node, res: 1 };
        let mut chain = SDValue { node: copy.node, res: 0 };
        for (reg, value) in [(cx, count), (di, dst)] {
            let copy = self.dag.get_copy_to_phys_reg(chain, reg, value, Some(glue));
            chain = SDValue { node: copy.node, res: 0 };
            glue = SDValue { node: copy.node, res: 1 };
        }
        let unit = self.dag.get_value_type_node(EVT::integer(block as u32 * 8));
        let rep = self
            .dag
            .get_node_multi(
                self.target.dag.nodes.string_store.expect("a target with string store"),
                &[EVT::OTHER, EVT::GLUE],
                &[chain, unit, glue],
            );
        SDValue { node: rep.node, res: 0 }
    }

    /// `getMemcpy`'s libcall: `name(dst, src, size)` for a copy, `name(dst,
    /// value, size)` for a set; the result is read, and dropped unless the
    /// target's routines return the destination (`mem_libcalls_return_dest`),
    /// where later uses of `destination` read it.
    fn mem_libcall(
        &mut self,
        info: &CallInfo,
        destination: Operand,
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
        let first = self.full_arch()?.first_argument_offset(false);
        // `getMemcpy`'s `RetTy` is the destination's type: a call that is not a
        // tail call reads the returned pointer and drops it
        // (`CopyFromReg $eax` in the initial DAG); one ending the function is a
        // tail call (`jmp memmove`), which returns nothing, so its plan
        // is the void one the sibling-call check compares with
        // the caller's.
        let void = types.void();
        let plan_of = |this: &Self, returns| {
            convention_for(
                this.module,
                this.layout,
                this.full_arch()?,
                first,
                convention,
                returns,
                false,
                &arg_types,
                &[],
            )
            .map_err(|_| Refused("libcall the convention does not describe"))
        };
        let tail_plan = plan_of(self, void)?;
        let target_symbol = self.dag.get_target_external_symbol(self.target.dag.pointer, &routine_symbol);
        let _ = symbol;
        let values = vec![first_two[0], first_two[1], size];
        self.dag.set_root(chain_in);
        let extends = [ISD::AnyExtend; 3];
        // `isTailCall`: the intrinsic is marked `tail` and ends the function,
        // and the target takes the sibling call.
        let tail = match self.tail_candidate.take() {
            Some(tail) if self.eligible_for_sibling_call(info, &tail_plan, false, None, first, &[]) => Some(tail),
            _ => None,
        };
        if let Some(tail) = tail {
            self.tail_calls.push(tail);
            self.sibcall = self.target.tail_calls;
            let _ = self.lower_call(target_symbol, values, &extends, &[], &tail_plan, first, void, false)?;
        } else {
            let plan = plan_of(self, pointer_type)?;
            let result = self.lower_call(target_symbol, values, &extends, &[], &plan, first, pointer_type, false)?;
            // DEVIATE MEMLIB-RETURNED (owner llrm-target-6502, exit 2026-12-05):
            // the pin keeps `dst` across the call.
            if let (true, Some(result), Operand::Value(destination)) =
                (self.target.mem_libcalls_return_dest, result, destination)
            {
                self.node_map.insert(destination, result);
            }
        }
        Ok(self.dag.get_root())
    }

    /// The registers of a string instruction, by root.
    fn string_roots(&self) -> llrm_target::StringRoots {
        self.arch.string_roots().expect("a target with string instructions names their registers")
    }

    /// The view of `root` at `bits`, in the target's register file.
    fn view_at(
        &self,
        root: llrm_lir::registers::RegId,
        bits: u32,
    ) -> llrm_lir::registers::RegId {
        let registers = self.arch.registers();
        registers.view(registers.root(root), bits).expect("the register file has the view a string instruction needs")
    }

    /// The accumulator a string operation of `block`-byte cells stores from:
    /// byte, word and quadword cells by their own width, any other as a dword.
    fn accumulator(
        &self,
        block: u64,
    ) -> llrm_lir::registers::RegId {
        let bits = if matches!(block, 1 | 2 | 8) { block as u32 * 8 } else { 32 };
        self.view_at(self.string_roots().accumulator, bits)
    }

    /// The count, destination and source registers of a string operation, at
    /// the pointer's width.
    fn string_registers(&self) -> (llrm_lir::registers::RegId, llrm_lir::registers::RegId, llrm_lir::registers::RegId) {
        let roots = self.string_roots();
        let bits = self.target.dag.pointer.size_in_bits();
        (self.view_at(roots.count, bits), self.view_at(roots.destination, bits), self.view_at(roots.source, bits))
    }

    /// The operand names of a string move on a flat target: count, source,
    /// destination, as the inline block spells them (the word views).
    fn flat_string_names(&self) -> [&'static str; 3] {
        let roots = self.string_roots();
        [roots.count, roots.source, roots.destination]
            .map(|root| self.arch.registers().name(self.view_at(root, 16)).expect("a described register"))
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
        let block = if self.has_min_size() {
            1
        } else if self.rep_movsb_copy() {
            self.exact_rep_block(size, alignment)
        } else {
            alignment
        };
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
                self.target.dag.nodes.string_move.expect("a target with string move"),
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
        // Far pointers take it at every level: the segment registers are set
        // once for the move and the tail, where the pin's tail would
        // make the far addresses again (a 14-byte far copy: `rep movsd`
        // and `movsw`, not 14 `movsb`; w2).
        if (self.has_opt_size() || self.is_far(dst) || self.is_far(src)) && self.string_instructions().is_some() {
            let mut last = rep;
            let mut rest = left;
            for width in [2u64, 1] {
                if rest >= width {
                    let kind = self.dag.get_value_type_node(EVT::integer(width as u32 * 8));
                    last = self
                        .dag
                        .get_node_multi(
                            self.target.dag.nodes.string_move.expect("a target with string move"),
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

    fn pointer_bytes(&self) -> u64 {
        u64::from(self.target.dag.pointer.size_in_bits() / 8)
    }

    /// DEVIATE REP-STOS-UNIT (ERMS/FSRM assumed, see `Tuning::rep_stos_priced_unit`): the unit of a
    /// constant `rep stos` at `-Os` that takes the fewest bytes,
    /// the pin's (`block`) on a tie. Encodings at x86-64: the value in `eax` (`xor` 2, `mov al` 2,
    /// `mov ax` 4, `mov eax` 5, `mov rax, -1` 7, `movabs` 10), `mov ecx, count` 5, the string op
    /// (2, 3 with a prefix or REX), and each tail store at its shortest (`mov [m], imm`: dword 6, word 7,
    /// byte 3).
    fn cheapest_fill_unit(block: u64, size: u64, byte: u64) -> u64 {
        let bytes = |unit: u64| {
            let value = match (byte, unit) {
                (0, _) | (_, 1) => 2,
                (_, 2) => 4,
                (_, 4) => 5,
                (0xff, _) => 7,
                _ => 10,
            };
            let op = if unit == 1 || unit == 4 { 2 } else { 3 };
            let left = size % unit;
            value + 5 + op + left / 4 * 6 + (left % 4 / 2) * 7 + left % 2 * 3
        };
        if bytes(1) < bytes(block) { 1 } else { block }
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
            let block = if self.has_opt_size() && self.target.tti.as_ref().is_some_and(|tti| tti.0.rep_stos_priced_unit()) {
                Self::cheapest_fill_unit(block, size, byte as u64 & 255)
            } else {
                block
            };
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
        let acc = self.accumulator(block);
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
                self.target.dag.nodes.string_store.expect("a target with string store"),
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
