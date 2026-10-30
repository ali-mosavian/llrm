//! `WebAssemblyTargetLowering::LowerCall`, `LowerReturn` and
//! `LowerFormalArguments` as the DAG builder's `TargetCalling`.
//!
//! # How it works
//! Wasm passes no value in a register: a formal argument is an `ARGUMENT idx`
//! node (the local the parameter lands in), a call is one `CALL chain, callee,
//! args..` node with a result per returned part and the chain last, and a
//! return is `RETURN chain, value`.
//!
//! A variadic call stores its variadic arguments to a frame object and passes
//! its address as one more argument; a callee that is variadic gets that
//! pointer as its last parameter.
//!
//! DEVIATE: integers and floats only, one returned part, no byval, tail call,
//! swift or funcref call, and no variadic function body (`va_start`); `accepts`
//! refuses what it can see. Reason: the callers the runtime needs. Exit: the
//! varargs definition step.
//!
//! # See also
//! `WebAssemblyISelLowering.cpp` at the pin d1106deb71cc: `LowerCall` (L1281),
//! `CanLowerReturn` (L1583), `LowerReturn` (L1591), `LowerFormalArguments`
//! (L1621).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::target_calling::{FixedObject, In, LiveIn, Out, Site, TargetCalling};
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};

use crate::dagisel::generated::ids::isd;

/// `WebAssemblyISD::CALL`: the first number after the generated ones
/// (`GENERATED_OPCODE_END`, one past `WrapperREL`).
pub const CALL: u16 = isd::WrapperREL + 1;

/// `TargetFrameLowering::getStackAlignment` of wasm.
const STACK_ALIGN: u32 = 16;

/// `WebAssemblyISD::RET_CALL`.
pub const RET_CALL: u16 = CALL + 1;

#[derive(Debug)]
pub struct WasmCalling {
    /// The pointer type: `i32` for wasm32, `i64` for wasm64.
    pub pointer: EVT,
}

fn is_value(vt: EVT) -> bool {
    matches!(vt, EVT::I32 | EVT::I64 | EVT::Simple(MVT::F32 | MVT::F64))
}

impl TargetCalling for WasmCalling {
    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        if !outs.iter().all(|out| is_value(dag.value_type(out.value))) || !ins.iter().all(|part| is_value(part.vt)) {
            return Err("a wasm argument or result that is no i32, i64, f32 or f64");
        }
        // `canLowerReturn`: without multivalue a function returns one value.
        match site {
            Site::Return if outs.len() > 1 => Err("a wasm return of more than one part"),
            Site::Call if ins.len() > 1 => Err("a wasm call result of more than one part"),
            _ => Ok(()),
        }
    }

    /// An `sret` pointer is an ordinary first argument; `byval` is not
    /// translated.
    fn passes_in_place(
        &self,
        attribute: &str,
    ) -> bool {
        attribute == "sret"
    }

    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        self.lower_call_with_frame(dag, chain, callee, outs, ins, false, &mut |_, _| {
            unreachable!("only a variadic call makes a frame object")
        })
    }

    fn lower_call_with_frame(
        &self,
        dag: &mut SelectionDAG,
        mut chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
        variadic: bool,
        frame: &mut dyn FnMut(u32, u32) -> i32,
    ) -> (SDValue, Vec<SDValue>) {
        // A direct callee is `Wrapper (TargetGlobalAddress)`; `Select` unwraps
        // it again.
        let callee = match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                let address = dag.get_target_global_address(self.pointer, symbol, offset);
                dag.get_node(ISD::Target(isd::Wrapper), self.pointer, &[address])
            }
            _ => callee,
        };
        let fixed = outs.iter().take_while(|out| !out.variadic).count();
        let mut operands = vec![chain, callee];
        operands.extend(outs[..fixed].iter().map(|out| out.value));
        if variadic {
            // `CCState::AllocateStack` per variadic value (size and alignment
            // of its type), then the buffer is
            // `getAlignedCallFrameSize` bytes at the stack alignment.
            let mut offsets = Vec::new();
            let mut bytes = 0u32;
            for out in &outs[fixed..] {
                let size = dag.value_type(out.value).size_in_bits() / 8;
                bytes = bytes.next_multiple_of(size) + size;
                offsets.push(bytes - size);
            }
            let buffer = if bytes == 0 {
                dag.get_constant(0, self.pointer)
            } else {
                let size = bytes.next_multiple_of(STACK_ALIGN);
                let index = frame(size, STACK_ALIGN);
                let at = dag.get_frame_index(self.pointer, index);
                let stores: Vec<SDValue> = outs[fixed..]
                    .iter()
                    .zip(&offsets)
                    .map(|(out, &offset)| {
                        let vt = dag.value_type(out.value);
                        let offset = dag.get_constant(u128::from(offset), self.pointer);
                        let address = dag.get_node(ISD::Add, self.pointer, &[at, offset]);
                        let info = MemInfo {
                            mem_vt: vt,
                            align: vt.size_in_bits() / 8,
                            volatile: false,
                            base: MemBase::Stack(index),
                            ext: None,
                            space: 0,
                        };
                        dag.get_store(chain, out.value, address, info)
                    })
                    .collect();
                chain = dag.get_token_factor(&stores);
                at
            };
            operands[0] = chain;
            operands.push(buffer);
        }
        let mut vts: Vec<EVT> = ins.iter().map(|part| part.vt).collect();
        vts.push(EVT::OTHER);
        let call = dag.get_node_multi(ISD::Target(CALL), &vts, &operands);
        let values = (0..ins.len() as u8).map(|res| SDValue { node: call.node, res }).collect();
        (SDValue { node: call.node, res: ins.len() as u8 }, values)
    }

    /// `WebAssemblySelectionDAGInfo::EmitTargetCodeForMemcpy`: `MEMCPY` rather
    /// than `MEMORY_COPY`, since `memory.copy` traps on a bad pointer even
    /// for no bytes; the custom inserter guards it.
    fn emit_memcpy(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        let memory = dag.get_constant(0, EVT::I32);
        let length = dag.get_zext_or_trunc(size, self.pointer);
        Some(dag.get_node(ISD::Target(isd::MEMCPY), EVT::OTHER, &[chain, memory, memory, dst, src, length]))
    }

    /// `EmitTargetCodeForMemset`: only the low byte of the value matters.
    fn emit_memset(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        let memory = dag.get_constant(0, EVT::I32);
        let value = dag.get_any_ext_or_trunc(value, EVT::I32);
        let length = dag.get_zext_or_trunc(size, self.pointer);
        Some(dag.get_node(ISD::Target(isd::MEMSET), EVT::OTHER, &[chain, memory, dst, value, length]))
    }

    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        _chain: SDValue,
        ins: &[In],
        _fixed: FixedObject,
        _live_in: LiveIn,
    ) -> Vec<SDValue> {
        ins.iter()
            .enumerate()
            .map(|(at, part)| {
                let index = dag.get_constant_with(at as u128, EVT::I32, true, false);
                dag.get_node(ISD::Target(isd::ARGUMENT), part.vt, &[index])
            })
            .collect()
    }

    /// The buffer pointer is one more `ARGUMENT`, after the fixed ones.
    fn variadic_buffer(
        &self,
        dag: &mut SelectionDAG,
        fixed: usize,
    ) -> Option<SDValue> {
        let index = dag.get_constant_with(fixed as u128, EVT::I32, true, false);
        Some(dag.get_node(ISD::Target(isd::ARGUMENT), self.pointer, &[index]))
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut operands = vec![chain];
        operands.extend(outs.iter().map(|out| out.value));
        dag.get_node(ISD::Target(isd::RETURN), EVT::OTHER, &operands)
    }
}
