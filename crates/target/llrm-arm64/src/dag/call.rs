//! AAPCS64 (Darwin) as the DAG builder's `TargetCalling`:
//! `LowerFormalArguments`, `LowerCall` and `LowerReturn` for integers and
//! pointers.
//!
//! # How it works
//! `CC_AArch64_DarwinPCS` promotes `i1`/`i8`/`i16` to `i32` and gives the first
//! eight integer parts `x0`..`x7` (`w0`..`w7` for an `i32`); the half of a
//! split `i128` never takes `x7`. What is left goes on the stack at the part's
//! natural size and alignment (a split `i64` at 8 bytes aligned to 16), and the
//! area is rounded up to 16. `RetCC_AArch64_AAPCS` gives a result `x0`/`w0`,
//! with an `i128` in `x0`, `x1`. The builder has split every value into legal
//! `i32`/`i64` parts. A call is `CALLSEQ_START`, the stack stores and the
//! `CopyToReg`s of the register parts (glued), `CALL chain, callee,
//! registers.., mask, glue`, `CALLSEQ_END` and a `CopyFromReg` of each result.
//! A return is `RET_GLUE chain, registers.., glue`.
//!
//! A float part takes `s0`..`s7`/`d0`..`d7` apart from the integer registers,
//! or 4/8 bytes of stack.
//!
//! An `f128` takes `q0`..`q7` (`CCIfType<[f128], CCAssignToReg<[Q0..Q7]>>`) or
//! 16 bytes of stack aligned to 16.
//!
//! DEVIATE SD526: not translated are `f16` and vectors, HFAs, a `byval`
//! argument too large to copy in line, a tail call that needs stack arguments or an
//! argument area of a different size, and a callee reached through the GOT;
//! `accepts` refuses what it can see. Exit: B3.
//!
//! # See also
//! `AArch64ISelLowering.cpp` at the pin: `LowerCall`, `LowerCallResult`,
//! `LowerFormalArguments`, `LowerReturn`; `AArch64CallingConvention.td`.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::target_calling::{
    ByVal, Ext, FixedObject, In, LiveIn, Out, Site, StackObject, TargetCalling, VaWord, VariadicArea,
};
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_lir::registers::RegId;
use llrm_sdag::actions::LoadExt;

use crate::reg;

/// The registers a call clobbers that the scheduler's live-register check reads
/// (`CheckForLiveRegDefMasked`): the condition flags. The AAPCS caller-saved
/// registers are not listed: nothing but a glued argument copy holds one across
/// a call. DEVIATE: the pin's mask lists them; owner arm64, exit 2026-11-30.
const CALL_CLOBBERS: [u16; 1] = [llrm_dagisel::generated::aarch64::ids::reg::NZCV];

/// `AArch64ISD::RET_GLUE`, the pin's number.
pub const RET_GLUE: u16 = llrm_dagisel::generated::aarch64::ids::isd::RET_GLUE;

/// `AArch64ISD::TC_RETURN`, the pin's number.
pub const TC_RETURN: u16 = llrm_dagisel::generated::aarch64::ids::isd::TC_RETURN;

/// `AArch64ISD::CALL`, the pin's number.
pub const CALL: u16 = llrm_dagisel::generated::aarch64::ids::isd::CALL;

const X: [RegId; 8] = [reg::X0, reg::X1, reg::X2, reg::X3, reg::X4, reg::X5, reg::X6, reg::X7];
const W: [RegId; 8] = [reg::W0, reg::W1, reg::W2, reg::W3, reg::W4, reg::W5, reg::W6, reg::W7];

const S: [RegId; 8] = [reg::S0, reg::S1, reg::S2, reg::S3, reg::S4, reg::S5, reg::S6, reg::S7];
const D: [RegId; 8] = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
// the 128-bit root of each `q`
const Q: [RegId; 8] = [reg::V0, reg::V1, reg::V2, reg::V3, reg::V4, reg::V5, reg::V6, reg::V7];

/// The register part `at` of type `vt` is in.
fn register(
    at: usize,
    vt: EVT,
) -> RegId {
    match vt {
        EVT::I32 => W[at],
        EVT::Simple(MVT::F32) => S[at],
        EVT::Simple(MVT::F64) => D[at],
        EVT::Simple(MVT::F128) => Q[at],
        _ => X[at],
    }
}

/// Where `CC_AArch64_DarwinPCS` puts a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    Register(RegId),
    /// Bytes at an offset into the stack arguments.
    Stack {
        offset: i64,
        bytes: u32,
    },
}

/// A part to place: its register type, the type of the whole value, whether the
/// value was split, whether it is a variadic argument and whether it is the
/// `sret` pointer (`x8`) (`CC_AArch64_DarwinPCS_VarArg`: everything on the
/// stack, in 8-byte slots), and the bytes of a `byval` one.
type Part = (EVT, EVT, bool, bool, bool, Option<ByVal>);

/// The location of each part, and the bytes of stack they take.
///
/// `linux` is `CC_AArch64_AAPCS`, whose stack argument takes an 8-byte slot
/// whatever its size and whose variadic arguments are placed like the others;
/// Darwin's packs them by size and puts the variadic ones on the stack.
fn place(
    parts: &[Part],
    linux: bool,
) -> (Vec<Location>, u64) {
    let (mut next, mut next_float, mut stack) = (0, 0, 0u64);
    let locations = parts
        .iter()
        .map(|&(vt, arg_vt, split, variadic, sret, byval)| {
            let variadic = variadic && !linux;
            // `CCIfSRet<CCAssignToReg<[X8]>>`: no argument register is taken.
            if sret {
                return Location::Register(reg::X8);
            }
            // `CCIfByVal<CCPassByVal<8, 8>>`, `CCState::HandleByVal`: the bytes are in the stack area, at least 8 of
            // them, aligned to at least 8.
            if let Some(byval) = byval {
                let align = u64::from(byval.align).max(8);
                let bytes = u64::from(byval.size).max(8).next_multiple_of(8);
                let offset = stack.next_multiple_of(align);
                stack = offset + bytes;
                return Location::Stack { offset: offset as i64, bytes: bytes as u32 };
            }
            if variadic {
                let align = if split { 16 } else { 8 };
                let offset = stack.next_multiple_of(align);
                stack = offset + 8;
                return Location::Stack { offset: offset as i64, bytes: 8 };
            }
            if vt.is_floating_point() {
                if next_float < 8 {
                    next_float += 1;
                    return Location::Register(register(next_float - 1, vt));
                }
                let bytes = vt.size_in_bits() / 8;
                if linux {
                    // an `f128` is 16 bytes aligned to 16 (`CCIfType<[f128],
                    // CCAssignToStack<16, 16>>`)
                    let (align, slot) = if bytes == 16 { (16, 16) } else { (8, 8) };
                    let offset = stack.next_multiple_of(align);
                    stack = offset + slot;
                    return Location::Stack { offset: offset as i64, bytes };
                }
                let offset = stack.next_multiple_of(u64::from(bytes));
                stack = offset + u64::from(bytes);
                return Location::Stack { offset: offset as i64, bytes };
            }
            // The half of a split `i128` leaves `x7` alone; on the stack it is
            // 8 bytes aligned to 16 and shadows `x7`.
            let registers = if split && vt == EVT::I64 { 7 } else { 8 };
            if next < registers {
                next += 1;
                return Location::Register(register(next - 1, vt));
            }
            if linux && !split {
                let offset = stack.next_multiple_of(8);
                stack = offset + 8;
                let bytes = match arg_vt.size_in_bits() {
                    1..=8 => 1,
                    16 => 2,
                    32 => 4,
                    _ => 8,
                };
                return Location::Stack { offset: offset as i64, bytes };
            }
            let (bytes, align): (u32, u64) = match (split, arg_vt.size_in_bits()) {
                (true, _) => (8, 16),
                (false, 1..=8) => (1, 1),
                (false, 16) => (2, 2),
                (false, 32) => (4, 4),
                _ => (8, 8),
            };
            if split {
                next = 8;
            }
            let offset = stack.next_multiple_of(align);
            stack = offset + u64::from(bytes);
            Location::Stack { offset: offset as i64, bytes: bytes as u32 }
        })
        .collect();
    (locations, stack)
}

/// The type a stack part of `bytes` bytes holds.
fn memory_type(bytes: u32) -> EVT {
    match bytes {
        1 => EVT::I8,
        2 => EVT::I16,
        4 => EVT::I32,
        _ => EVT::I64,
    }
}

fn is_register(location: &Location) -> bool {
    matches!(location, Location::Register(_))
}

/// Private machine opcodes for a constant-size copy, move or fill: the
/// builder's inline expansion has no vector type (DEVIATE SD13), so the
/// pin's `q`-register pieces (`findOptimalMemOpLowering`) are made at
/// emission. Exit: a `v16i8` value type in the DAG.
pub const MEMCPY_Q: u32 = 0xF001;
pub const MEMMOVE_Q: u32 = 0xF002;
pub const MEMSET_Q: u32 = 0xF003;

/// Stores the whole `q` register of an unnamed argument to a frame object
/// (`SaveVarArgRegisters`' `f128` store): the DAG has no vector type, and a
/// copy through a `d` register would drop the upper half.
pub const SAVE_Q: u32 = 0xF004;

#[derive(Debug, Default)]
pub struct Aarch64Calling {
    /// AAPCS64 of Linux rather than Darwin's.
    pub linux: bool,
}

impl Aarch64Calling {
    fn block_copy(
        dag: &mut SelectionDAG,
        code: u32,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        let total = u64::try_from(dag.constant(size)?).ok()?;
        if total == 0 || crate::dag::memops::pieces(total).len() > crate::dag::memops::COPY_OPS {
            return None;
        }
        // A target constant stays a constant through selection.
        let size = dag.get_constant_with(total.into(), EVT::I64, true, false);
        Some(dag.get_node(ISD::Machine(code), EVT::OTHER, &[chain, dst, src, size]))
    }
}

impl TargetCalling for Aarch64Calling {
    /// `CCAssignFnForCall`: `ccc` and `fastcc` are AAPCS64; the pin has no
    /// table for `coldcc` and the like.
    fn supports_convention(
        &self,
        cc: u32,
    ) -> bool {
        matches!(cc, 0 | 8)
    }

    /// `LowerFormalArguments` of a variadic function. Darwin's `va_list` is the
    /// address of the first unnamed stack slot. AAPCS64's is `stack`,
    /// `gr_top`, `vr_top`, `gr_offs`, `vr_offs` over save areas
    /// `SaveVarArgRegisters` fills with the `x` and `q` registers the named
    /// arguments left.
    fn variadic_area(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        stack: StackObject,
        live_in: LiveIn,
    ) -> Option<VariadicArea> {
        let parts: Vec<Part> = ins.iter().map(|i| (i.vt, i.arg_vt, i.split, false, i.sret, i.byval)).collect();
        let (locations, named) = place(&parts, self.linux);
        let first = fixed(8, named.next_multiple_of(8) as i64, true);
        let stack_word = VaWord::Frame { index: first, offset: 0 };
        if !self.linux {
            return Some(VariadicArea { chain, words: vec![(0, 8, stack_word)], size: 8 });
        }
        let used = |registers: &[RegId]| {
            locations.iter().filter(|l| matches!(l, Location::Register(r) if registers.contains(r))).count() as u32
        };
        let gprs = used(&X) + used(&W);
        let fprs = used(&S) + used(&D) + used(&Q);
        let (gr_size, vr_size) = (8 * (8 - gprs), 16 * (8 - fprs));
        let mut chain = chain;
        let mut gr_top = VaWord::Constant(0);
        if gr_size > 0 {
            let index = stack(gr_size, 8);
            for (at, number) in (gprs..8).enumerate() {
                let value = dag.get_copy_from_reg(chain, live_in(X[number as usize]), EVT::I64);
                let base = dag.get_frame_index(EVT::I64, index);
                let step = dag.get_constant(at as u128 * 8, EVT::I64);
                let address = dag.get_node(ISD::Add, EVT::I64, &[base, step]);
                let info = MemInfo {
                    mem_vt: EVT::I64,
                    align: 8,
                    volatile: false,
                    base: MemBase::Stack(index),
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                chain = dag.get_store(chain, value, address, info);
            }
            gr_top = VaWord::Frame { index, offset: i64::from(gr_size) };
        }
        let mut vr_top = VaWord::Constant(0);
        if vr_size > 0 {
            let index = stack(vr_size, 16);
            for (at, number) in (fprs..8).enumerate() {
                // The register is a live-in of the function.
                live_in(D[number as usize]);
                let frame = dag.get_target_frame_index(EVT::I64, index);
                let q = dag.get_constant_with(u128::from(number), EVT::I64, true, false);
                let offset = dag.get_constant_with(at as u128 * 16, EVT::I64, true, false);
                chain = dag.get_node(ISD::Machine(SAVE_Q), EVT::OTHER, &[chain, frame, q, offset]);
            }
            vr_top = VaWord::Frame { index, offset: i64::from(vr_size) };
        }
        let words = vec![
            (0, 8, stack_word),
            (8, 8, gr_top),
            (16, 8, vr_top),
            (24, 4, VaWord::Constant(-i64::from(gr_size))),
            (28, 4, VaWord::Constant(-i64::from(vr_size))),
        ];
        Some(VariadicArea { chain, words, size: 32 })
    }

    fn emit_memcpy(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        Self::block_copy(dag, MEMCPY_Q, chain, dst, src, size)
    }

    fn emit_memmove(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        Self::block_copy(dag, MEMMOVE_Q, chain, dst, src, size)
    }

    fn emit_memset(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        let total = u64::try_from(dag.constant(size)?).ok()?;
        let constant = dag.constant(value);
        // A byte that is not constant fills `q` registers whole.
        let ops = if constant.is_none() {
            if total < 16 {
                return None;
            }
            crate::dag::memops::wide_pieces(total)
        } else {
            crate::dag::memops::pieces(total)
        };
        if total == 0 || ops.len() > crate::dag::memops::FILL_OPS {
            return None;
        }
        let value = match constant {
            Some(byte) => dag.get_constant_with(byte & 0xff, EVT::I32, true, false),
            None => dag.get_any_ext_or_trunc(value, EVT::I32),
        };
        let size = dag.get_constant_with(total.into(), EVT::I64, true, false);
        Some(dag.get_node(ISD::Machine(MEMSET_Q), EVT::OTHER, &[chain, dst, value, size]))
    }

    /// An `sret` pointer is an ordinary pointer to the builder; `x8` carries
    /// it. A `byval` one is the address of the bytes the hook copies into the argument area.
    fn passes_in_place(
        &self,
        attribute: &str,
    ) -> bool {
        attribute == "sret" || attribute == "byval"
    }

    fn byval_in_area(&self) -> bool {
        true
    }

    /// `isEligibleForTailCallOptimization`: a caller's `byval` parameter is a pointer into the area a tail call would
    /// reuse.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L10175>
    fn byval_parameter_blocks_tail_call(&self) -> bool {
        true
    }

    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        let integer = |vt: EVT| matches!(
            vt,
            EVT::I32 | EVT::I64 | EVT::Simple(MVT::F32 | MVT::F64 | MVT::F128)
        );
        let known = |arg_vt: EVT, split: bool| {
            if split {
                arg_vt == EVT::integer(128)
            } else {
                (arg_vt.size_in_bits() <= 64 && (arg_vt.is_integer() || arg_vt.is_floating_point()))
                    || arg_vt == EVT::Simple(MVT::F128)
            }
        };
        let outs: Vec<Part> =
            outs.iter().map(|o| (dag.value_type(o.value), o.arg_vt, o.split, o.variadic, o.sret, o.byval)).collect();
        let ins: Vec<Part> = ins.iter().map(|i| (i.vt, i.arg_vt, i.split, false, i.sret, i.byval)).collect();
        if !outs.iter().chain(&ins).all(|&(vt, arg_vt, split, ..)| integer(vt) && known(arg_vt, split)) {
            return Err("an arm64 argument or result that is no integer or float");
        }
        // `getMemcpy` of a `byval` argument expands in line only up to a bound; a larger one is a libcall in the call
        // sequence, which the hook does not make (DEVIATE SD526).
        if outs.iter().any(|&(.., byval)| {
            byval.is_some_and(|byval| {
                byval.size == 0 || crate::dag::memops::pieces(u64::from(byval.size)).len() > crate::dag::memops::COPY_OPS
            })
        }) {
            return Err("an arm64 byval argument too large to copy in line");
        }
        // A result is in registers.
        let (outs_in_registers, ins_in_registers) =
            (place(&outs, self.linux).0.iter().all(is_register), place(&ins, self.linux).0.iter().all(is_register));
        match site {
            Site::Arguments => Ok(()),
            Site::Call if ins_in_registers => Ok(()),
            Site::Return if outs_in_registers => Ok(()),
            _ => Err("an arm64 result that does not fit x0..x7"),
        }
    }

    /// `LowerCall`'s tail-call arm: no call sequence, the arguments in their
    /// registers, and `TC_RETURN chain, callee, fpdiff, registers.., mask,
    /// glue`.
    fn lower_tail_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
    ) -> Option<SDValue> {
        let values: Vec<SDValue> = outs.iter().map(|out| out.value).collect();
        let parts: Vec<Part> = outs
            .iter()
            .zip(&values)
            .map(|(o, v)| (dag.value_type(*v), o.arg_vt, o.split, o.variadic, o.sret, o.byval))
            .collect();
        let (locations, _) = place(&parts, self.linux);
        let mut chain = chain;
        let mut glue = None;
        let mut registers = Vec::new();
        for (value, location) in values.iter().copied().zip(&locations) {
            let Location::Register(register) = *location else { return None };
            let copy = dag.get_copy_to_phys_reg(chain, register, value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            registers.push((register, value));
        }
        let callee = match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                dag.get_target_global_address(EVT::I64, symbol, offset)
            }
            // A libcall the legalizer made (`memset`): `LowerCall`'s
            // `getTargetExternalSymbol`.
            (ISD::ExternalSymbol, NodeData::Symbol(id)) => {
                let name = dag.symbol_names[id as usize].clone();
                dag.get_target_external_symbol(EVT::I64, &name)
            }
            _ => callee,
        };
        // The arguments are where the callee reads them: no stack adjustment.
        let fpdiff = dag.get_constant_with(0, EVT::I32, true, false);
        let mut operands = vec![chain, callee, fpdiff];
        for &(register, value) in &registers {
            operands.push(dag.get_phys_register(dag.value_type(value), register));
        }
        operands.push(dag.get_register_mask_clobbering(&CALL_CLOBBERS));
        operands.extend(glue);
        Some(dag.get_node_multi(ISD::Target(TC_RETURN), &[EVT::OTHER, EVT::GLUE], &operands))
    }

    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        // A variadic argument is an `i64` or an `f64` (`CCPromoteToType`).
        let values: Vec<SDValue> = outs
            .iter()
            .map(|out| match (out.variadic, dag.value_type(out.value)) {
                (true, EVT::I32) => dag.get_node(out.ext.node(), EVT::I64, &[out.value]),
                (true, EVT::Simple(MVT::F32)) => dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[out.value]),
                _ => out.value,
            })
            .collect();
        let parts: Vec<Part> = outs
            .iter()
            .zip(&values)
            .map(|(o, v)| (dag.value_type(*v), o.arg_vt, o.split, o.variadic, o.sret, o.byval))
            .collect();
        let (locations, stack) = place(&parts, self.linux);
        let bytes = stack.next_multiple_of(16);
        let mut chain = dag.get_callseq_start(chain, bytes);
        let (mut stores, mut registers, mut stack_pointer) = (Vec::new(), Vec::new(), None);
        for (index, (value, location)) in values.iter().copied().zip(&locations).enumerate() {
            match *location {
                Location::Register(register) => registers.push((register, value)),
                Location::Stack { offset, bytes } => {
                    // `LowerCall`: a store of the part at SP + offset
                    let sp = *stack_pointer
                        .get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, reg::SP, EVT::I64));
                    let at = dag.get_constant(offset as u128, EVT::I64);
                    let address = dag.get_node(ISD::Add, EVT::I64, &[sp, at]);
                    // a `byval` argument: its bytes are copied there (`getMemcpy`)
                    if let Some(byval) = outs[index].byval {
                        let size = dag.get_constant(u128::from(byval.size), EVT::I64);
                        let copy = Self::block_copy(dag, MEMCPY_Q, chain, address, value, size)
                            .expect("`accepts` kept the copies that expand in line");
                        stores.push(copy);
                        continue;
                    }
                    let info = MemInfo {
                        mem_vt: if dag.value_type(value).is_floating_point() {
                            dag.value_type(value)
                        } else {
                            memory_type(bytes)
                        },
                        align: bytes,
                        volatile: false,
                        base: MemBase::CallArgument(offset as i32),
                        ext: None,
                        space: 0,

                        ordering: None,
                    };
                    stores.push(dag.get_store(chain, value, address, info));
                }
            }
        }
        if !stores.is_empty() {
            chain = dag.get_token_factor(&stores);
        }
        let mut glue = None;
        for &(register, value) in &registers {
            let copy = dag.get_copy_to_phys_reg(chain, register, value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
        }
        // A direct callee is the symbol itself; a register is called through
        // `BLR`.
        let callee = match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                dag.get_target_global_address(EVT::I64, symbol, offset)
            }
            // A libcall the legalizer made (`memset`): `LowerCall`'s
            // `getTargetExternalSymbol`.
            (ISD::ExternalSymbol, NodeData::Symbol(id)) => {
                let name = dag.symbol_names[id as usize].clone();
                dag.get_target_external_symbol(EVT::I64, &name)
            }
            _ => callee,
        };
        let mut operands = vec![chain, callee];
        for &(register, value) in &registers {
            operands.push(dag.get_phys_register(dag.value_type(value), register));
        }
        operands.push(dag.get_register_mask_clobbering(&CALL_CLOBBERS));
        operands.extend(glue);
        let call = dag.get_node_multi(ISD::Target(CALL), &[EVT::OTHER, EVT::GLUE], &operands);
        let end =
            dag.get_callseq_end(SDValue { node: call.node, res: 0 }, bytes, 0, SDValue { node: call.node, res: 1 });
        let mut chain = SDValue { node: end.node, res: 0 };
        let mut glue = SDValue { node: end.node, res: 1 };
        // `LowerCallResult`
        let mut values = Vec::new();
        for (at, result) in ins.iter().enumerate() {
            let copy = dag.get_copy_from_phys_reg(chain, register(at, result.vt), result.vt, glue);
            chain = SDValue { node: copy.node, res: 1 };
            glue = SDValue { node: copy.node, res: 2 };
            values.push(SDValue { node: copy.node, res: 0 });
        }
        (chain, values)
    }

    /// Each register part is read from the virtual register its live-in
    /// register is copied to (`CopyFromReg`); a stack part is loaded from a
    /// fixed object in the caller's outgoing area.
    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue> {
        let parts: Vec<Part> = ins.iter().map(|i| (i.vt, i.arg_vt, i.split, false, i.sret, i.byval)).collect();
        let (locations, _) = place(&parts, self.linux);
        ins.iter()
            .zip(locations)
            .map(|(part, location)| match location {
                Location::Register(register) => {
                    let vreg = live_in(register);
                    dag.get_copy_from_reg(chain, vreg, part.vt)
                }
                Location::Stack { offset, bytes } if part.byval.is_some() => {
                    // `LowerFormalArguments`: a `byval` parameter is the address of its fixed object, written to
                    // (not immutable)
                    let index = fixed(bytes, offset, false);
                    dag.get_frame_index(EVT::I64, index)
                }
                Location::Stack { offset, bytes } => {
                    // `LowerFormalArguments`: a load from the fixed object,
                    // extended to the part
                    let index = fixed(bytes, offset, true);
                    let address = dag.get_frame_index(EVT::I64, index);
                    let mem_vt = if part.vt.is_floating_point() { part.vt } else { memory_type(bytes) };
                    let info = MemInfo {
                        mem_vt,
                        align: bytes,
                        volatile: false,
                        base: MemBase::Stack(index),
                        ext: (mem_vt != part.vt).then_some(match part.ext {
                            Ext::Sign => LoadExt::SExtLoad,
                            Ext::Zero => LoadExt::ZExtLoad,
                            Ext::Any => LoadExt::ExtLoad,
                        }),
                        space: 0,

                        ordering: None,
                    };
                    dag.get_load(part.vt, chain, address, info)
                }
            })
            .collect()
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut chain = chain;
        let mut glue = None;
        let mut operands = vec![chain];
        for (at, out) in outs.iter().enumerate() {
            let vt = dag.value_type(out.value);
            let reg = register(at, vt);
            let copy = dag.get_copy_to_phys_reg(chain, reg, out.value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            operands.push(dag.get_phys_register(vt, reg));
        }
        operands[0] = chain;
        operands.extend(glue);
        dag.get_node(llrm_codegen::backend::selectiondag::isd::ISD::Target(RET_GLUE), EVT::OTHER, &operands)
    }
}
