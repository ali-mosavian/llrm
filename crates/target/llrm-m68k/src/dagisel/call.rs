//! `M68kTargetLowering`'s calls: `LowerCall`, `LowerCallResult`,
//! `LowerFormalArguments`, `LowerReturn` and `LowerGlobalAddress`.
//!
//! # How it works
//! `CC_M68k_C` puts every argument in a 4-byte stack slot, pushed in order and
//! removed by the caller; a byte or word is promoted to `i32` first. A result
//! is in `d0` (`bd0`, `wd0` for a byte or word) and a pointer in `a0`. Where
//! each goes is `machines/calling.toml`, read through `Convention::place`.
//! The values arrive already split into legal `i8`/`i16`/`i32` parts, as
//! LLVM's `Outs` and `Ins` are.
//!
//! DEVIATE: not translated are `inreg` (`d0`, `d1`), `nest` (`a1`), `byval`,
//! `sret`, varargs, tail calls, the Fast convention and callee-pop. Reason:
//! the slice does not reach them. Exit: M3d.
//!
//! DEVIATE: the DAG's global node carries no target flag (`TF=`): a callee is
//! always the direct form, and a global that PIC would reach through the GOT
//! is refused. Reason: `NodeData::Global` has no flags field. Exit: S2.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kISelLowering.cpp`, pin d1106deb71cc:
//! `LowerMemArgument` (L454), `LowerMemOpCallTo` (L523), `LowerCall` (L544),
//! `LowerCallResult` (L906), `LowerFormalArguments` (L941), `LowerReturn`
//! (L1095), `LowerGlobalAddress` (L2727).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_target::calling::{Convention, Kind, Place};

use super::lowering::m68kisd;
use crate::abi;
use crate::reg;

/// How a value is widened to the location `CC_M68k` gives it: `CCValAssign`'s
/// `AExt`, `SExt` and `ZExt`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ext {
    Any,
    Sign,
    Zero,
}

impl Ext {
    fn node(self) -> ISD {
        match self {
            Ext::Any => ISD::AnyExtend,
            Ext::Sign => ISD::SignExtend,
            Ext::Zero => ISD::ZeroExtend,
        }
    }
}

/// An `ISD::OutputArg` with its value: an argument of a call or a returned
/// value.
#[derive(Clone, Copy, Debug)]
pub struct Out {
    pub value: SDValue,
    pub ext: Ext,
    /// `ArgFlags.isPointer()`: a returned pointer is in `a0`, not `d0`.
    pub pointer: bool,
}

/// An `ISD::InputArg`: a formal argument or a call's result.
#[derive(Clone, Copy, Debug)]
pub struct In {
    pub vt: EVT,
    pub pointer: bool,
}

/// A fixed stack object, as `MachineFrameInfo::CreateFixedObject` takes it:
/// size and offset from the first argument, and whether it is immutable.
pub type FixedObject<'a> = &'a mut dyn FnMut(u32, i64, bool) -> i32;

/// The bytes of the `i32` slot an `i8` or `i16` argument sits in, high end:
/// m68k is big-endian (`LowerMemArgument`).
fn slot_offset(
    vt: EVT,
    offset: i64,
) -> i64 {
    match vt {
        EVT::I8 => offset + 3,
        EVT::I16 => offset + 2,
        _ => offset,
    }
}

/// The m68k C calling convention applied to DAG nodes.
pub struct M68kCalling {
    convention: Convention,
}

impl Default for M68kCalling {
    fn default() -> Self {
        Self { convention: abi::c() }
    }
}

impl M68kCalling {
    /// Where each argument is: its offset in the stack arguments, and the bytes
    /// they take (`AnalyzeCallOperands`, `getAlignedCallFrameSize`).
    fn place(
        &self,
        count: usize,
    ) -> (Vec<i64>, i64) {
        let placement = self.convention.place(&vec![Kind::Word; count]);
        let offsets = placement
            .places
            .iter()
            .map(|place| match place {
                Place::Stack(offset) => *offset,
                Place::Registers(_) => unreachable!("CC_M68k_C takes no register argument"),
            })
            .collect();
        (offsets, placement.stack_bytes)
    }

    /// The register `RetCC_M68k_C` puts a result of `vt` in.
    fn result_register(
        &self,
        vt: EVT,
        pointer: bool,
    ) -> llrm_lir::registers::RegId {
        let names = if pointer {
            self.convention.results.get("pointer")
        } else {
            self.convention.results.get(&(vt.size_in_bits() / 8).max(1).to_string())
        };
        abi::register(&names.expect("RetCC_M68k_C names a register")[0])
    }

    /// `LowerCall`: the arguments stored in the outgoing stack area, the `CALL`
    /// and the results read back. Returns the chain and the result values.
    pub fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        let (offsets, bytes) = self.place(outs.len());
        let mut chain = dag.get_callseq_start(chain, bytes as u64);
        let mut stack_pointer = None;
        let mut stores = Vec::new();
        for (out, offset) in outs.iter().zip(offsets) {
            // `CCPromoteToType<i32>`
            let mut arg = out.value;
            if dag.value_type(arg) != EVT::I32 {
                arg = dag.get_node(out.ext.node(), EVT::I32, &[arg]);
            }
            let sp = *stack_pointer.get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, reg::SP, EVT::I32));
            // `LowerMemOpCallTo`
            let slot = dag.get_constant(offset as u128, EVT::I32);
            let address = dag.get_node(ISD::Add, EVT::I32, &[sp, slot]);
            let info = MemInfo {
                mem_vt: EVT::I32,
                align: 2,
                volatile: false,
                base: MemBase::CallArgument(offset as i32),
                ext: None,
                space: 0,
            };
            stores.push(dag.get_store(chain, arg, address, info));
        }
        if !stores.is_empty() {
            chain = dag.get_token_factor(&stores);
        }
        let callee = self.callee(dag, callee);
        let mask = dag.get_register_mask();
        let call = dag.get_node_multi(m68kisd::CALL, &[EVT::OTHER, EVT::GLUE], &[chain, callee, mask]);
        let end = dag.get_callseq_end(
            SDValue { node: call.node, res: 0 },
            bytes as u64,
            0,
            SDValue { node: call.node, res: 1 },
        );
        let mut chain = SDValue { node: end.node, res: 0 };
        let mut glue = SDValue { node: end.node, res: 1 };
        // `LowerCallResult`
        let mut values = Vec::new();
        for result in ins {
            let register = self.result_register(result.vt, result.pointer);
            let copy = dag.get_copy_from_phys_reg(chain, register, result.vt, glue);
            chain = SDValue { node: copy.node, res: 1 };
            glue = SDValue { node: copy.node, res: 2 };
            values.push(SDValue { node: copy.node, res: 0 });
        }
        (chain, values)
    }

    /// The callee as `LowerCall` leaves it: a `GlobalAddress` is the
    /// `TargetGlobalAddress` (the direct form, see the DEVIATE above).
    fn callee(
        &self,
        dag: &mut SelectionDAG,
        callee: SDValue,
    ) -> SDValue {
        match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                dag.get_target_global_address(EVT::I32, symbol, offset)
            }
            _ => callee,
        }
    }

    /// `LowerFormalArguments`: each argument loaded from its slot in the
    /// caller's outgoing area. `fixed` creates the stack object.
    pub fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
    ) -> Vec<SDValue> {
        let (offsets, _) = self.place(ins.len());
        ins.iter()
            .zip(offsets)
            .map(|(arg, offset)| {
                // `LowerMemArgument`
                let offset = slot_offset(arg.vt, offset);
                let index = fixed(arg.vt.size_in_bits() / 8, offset, true);
                let address = dag.get_frame_index(EVT::I32, index);
                let info = MemInfo {
                    mem_vt: arg.vt,
                    align: 1,
                    volatile: false,
                    base: MemBase::Stack(index),
                    ext: None,
                    space: 0,
                };
                dag.get_load(arg.vt, chain, address, info)
            })
            .collect()
    }

    /// `LowerReturn`: the values copied to their registers, then
    /// `RET chain, bytes-to-pop, registers.., glue`.
    pub fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut chain = chain;
        let pop = dag.get_constant_with(0, EVT::I32, true, false);
        let mut operands = vec![chain, pop];
        let mut glue = None;
        for out in outs {
            // `CCIfType<[i1], CCPromoteToType<i8>>`
            let mut value = out.value;
            if dag.value_type(value) == EVT::I1 {
                value = dag.get_node(out.ext.node(), EVT::I8, &[value]);
            }
            let vt = dag.value_type(value);
            let register = self.result_register(vt, out.pointer);
            let copy = dag.get_copy_to_phys_reg(chain, register, value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            operands.push(dag.get_phys_register(vt, register));
        }
        operands[0] = chain;
        operands.extend(glue);
        dag.get_node(m68kisd::RET, EVT::OTHER, &operands)
    }
}

/// `LowerGlobalAddress`: `WrapperPC (TargetGlobalAddress)`, the offset folded
/// into the symbol. A global the PIC model reaches through the GOT is not
/// translated (see the DEVIATE above): `lower_operation` assumes the small
/// code model without PIC, where every reference is direct.
pub fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let NodeData::Global { symbol, offset } = dag.node(node).data else {
        unreachable!("GlobalAddress has a global payload")
    };
    let address = dag.get_target_global_address(EVT::I32, symbol, offset);
    dag.get_node(m68kisd::WRAPPER_PC, EVT::I32, &[address])
}
