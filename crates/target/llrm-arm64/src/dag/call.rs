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
//! DEVIATE SD526: not translated are `f16`/`fp128` and vectors, `sret` (`x8`),
//! HFAs, `byval`, varargs, a tail call that needs stack arguments or an
//! argument area of a different size, and a callee reached through the GOT;
//! `accepts` refuses what it can see. Exit: B3.
//!
//! # See also
//! `AArch64ISelLowering.cpp` at the pin: `LowerCall`, `LowerCallResult`,
//! `LowerFormalArguments`, `LowerReturn`; `AArch64CallingConvention.td`.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::target_calling::{FixedObject, In, LiveIn, Out, Site, TargetCalling};
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_lir::registers::RegId;
use llrm_sdag::actions::LoadExt;

use crate::reg;

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

/// The register part `at` of type `vt` is in.
fn register(
    at: usize,
    vt: EVT,
) -> RegId {
    match vt {
        EVT::I32 => W[at],
        EVT::Simple(MVT::F32) => S[at],
        EVT::Simple(MVT::F64) => D[at],
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
/// value was split, and whether it is a variadic argument
/// (`CC_AArch64_DarwinPCS_VarArg`: everything on the stack, in 8-byte slots).
type Part = (EVT, EVT, bool, bool);

/// The location of each part, and the bytes of stack they take.
fn place(parts: &[Part]) -> (Vec<Location>, u64) {
    let (mut next, mut next_float, mut stack) = (0, 0, 0u64);
    let locations = parts
        .iter()
        .map(|&(vt, arg_vt, split, variadic)| {
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

#[derive(Debug, Default)]
pub struct Aarch64Calling;

impl TargetCalling for Aarch64Calling {
    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        let integer = |vt: EVT| matches!(vt, EVT::I32 | EVT::I64 | EVT::Simple(MVT::F32 | MVT::F64));
        let known = |arg_vt: EVT, split: bool| {
            if split {
                arg_vt == EVT::integer(128)
            } else {
                arg_vt.size_in_bits() <= 64 && (arg_vt.is_integer() || arg_vt.is_floating_point())
            }
        };
        let outs: Vec<Part> = outs.iter().map(|o| (dag.value_type(o.value), o.arg_vt, o.split, o.variadic)).collect();
        let ins: Vec<Part> = ins.iter().map(|i| (i.vt, i.arg_vt, i.split, false)).collect();
        if !outs.iter().chain(&ins).all(|&(vt, arg_vt, split, _)| integer(vt) && known(arg_vt, split)) {
            return Err("an arm64 argument or result that is no integer or float");
        }
        // A result is in registers.
        let (outs_in_registers, ins_in_registers) =
            (place(&outs).0.iter().all(is_register), place(&ins).0.iter().all(is_register));
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
        let parts: Vec<Part> =
            outs.iter().zip(&values).map(|(o, v)| (dag.value_type(*v), o.arg_vt, o.split, o.variadic)).collect();
        let (locations, _) = place(&parts);
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
            _ => callee,
        };
        // The arguments are where the callee reads them: no stack adjustment.
        let fpdiff = dag.get_constant_with(0, EVT::I32, true, false);
        let mut operands = vec![chain, callee, fpdiff];
        for &(register, value) in &registers {
            operands.push(dag.get_phys_register(dag.value_type(value), register));
        }
        operands.push(dag.get_register_mask());
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
        let parts: Vec<Part> =
            outs.iter().zip(&values).map(|(o, v)| (dag.value_type(*v), o.arg_vt, o.split, o.variadic)).collect();
        let (locations, stack) = place(&parts);
        let bytes = stack.next_multiple_of(16);
        let mut chain = dag.get_callseq_start(chain, bytes);
        let (mut stores, mut registers, mut stack_pointer) = (Vec::new(), Vec::new(), None);
        for (value, location) in values.iter().copied().zip(&locations) {
            match *location {
                Location::Register(register) => registers.push((register, value)),
                Location::Stack { offset, bytes } => {
                    // `LowerCall`: a store of the part at SP + offset
                    let sp = *stack_pointer
                        .get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, reg::SP, EVT::I64));
                    let at = dag.get_constant(offset as u128, EVT::I64);
                    let address = dag.get_node(ISD::Add, EVT::I64, &[sp, at]);
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
            _ => callee,
        };
        let mut operands = vec![chain, callee];
        for &(register, value) in &registers {
            operands.push(dag.get_phys_register(dag.value_type(value), register));
        }
        operands.push(dag.get_register_mask());
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
        let parts: Vec<Part> = ins.iter().map(|i| (i.vt, i.arg_vt, i.split, false)).collect();
        let (locations, _) = place(&parts);
        ins.iter()
            .zip(locations)
            .map(|(part, location)| match location {
                Location::Register(register) => {
                    let vreg = live_in(register);
                    dag.get_copy_from_reg(chain, vreg, part.vt)
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
                        ext: (mem_vt != part.vt).then_some(LoadExt::ExtLoad),
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
