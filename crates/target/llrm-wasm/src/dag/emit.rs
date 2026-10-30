//! `InstrEmitter` for wasm over the builder's DAG: the scheduled units of each block as the
//! instructions of a [`Function`].
//!
//! # How it works
//! A result of a machine node becomes a virtual register, an operand a use or an
//! immediate (constant, symbol, block). `CopyFromReg` of a virtual register names that
//! register; `CopyToReg` is a `COPY` into it. The builder's registers are shared by every
//! block, so a value that crosses blocks keeps one register. A `CALL_PARAMS` and the
//! `CALL_RESULTS` glued to it become one `CALL`, as the custom inserter does
//! (`WebAssemblyTargetLowering::EmitInstrWithCustomInserter`).
//!
//! DEVIATE: one block, direct calls, no frame indices. Reason: control flow and the frame
//! come with CFGStackify and the frame lowering. Exit: W7, W8.
//!
//! # See also
//! `InstrEmitter::EmitMachineNode`, pin d1106deb71cc.

use std::collections::{BTreeMap, HashMap};

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};

use super::function::Selected;
use crate::dagisel::post_ids::{self as post, CALL_PARAMS, CALL_RESULTS, END_FUNCTION};
use crate::mir::{Function, Inst, Mem, Operand, VReg, ValType};

/// The register class of a value type, `None` for a chain or glue.
fn class(vt: EVT) -> Option<ValType> {
    match vt {
        EVT::I32 => Some(ValType::I32),
        EVT::I64 => Some(ValType::I64),
        EVT::Simple(MVT::F32) => Some(ValType::F32),
        EVT::Simple(MVT::F64) => Some(ValType::F64),
        _ => None,
    }
}

/// `COPY` of a class.
fn copy(ty: ValType) -> u16 {
    match ty {
        ValType::I32 => post::COPY_I32,
        ValType::I64 => post::COPY_I64,
        ValType::F32 => post::COPY_F32,
        ValType::F64 => post::COPY_F64,
    }
}

struct Emitter<'a> {
    f: Function,
    /// The register of a machine node's result in the block being emitted.
    results: HashMap<(u32, u8), VReg>,
    /// The register of each of the builder's virtual registers.
    builder: BTreeMap<u32, VReg>,
    names: &'a dyn Fn(u32) -> String,
}

impl Emitter<'_> {
    fn builder_reg(
        &mut self,
        number: u32,
        ty: ValType,
    ) -> VReg {
        *self.builder.entry(number).or_insert_with(|| self.f.new_vreg(ty))
    }

    /// An operand of a machine node: a register, an immediate, or nothing (chain, glue).
    fn operand(
        &mut self,
        dag: &SelectionDAG,
        value: SDValue,
    ) -> Option<Operand> {
        let node = dag.node(value);
        let vt = node.values[value.res as usize];
        match (node.opcode, node.data) {
            (ISD::TargetConstant, NodeData::Const { bits, .. }) => {
                let width = vt.size_in_bits();
                let shift = 128 - width;
                Some(Operand::Imm((((bits as i128) << shift) >> shift) as i64))
            }
            (ISD::TargetGlobalAddress, NodeData::Global { symbol, offset }) => {
                let name = (self.names)(symbol);
                Some(Operand::Sym(self.f.symbol(&name, offset)))
            }
            (ISD::BasicBlock, NodeData::Block(block)) => Some(Operand::Block(block)),
            (ISD::Register, _) => None,
            (ISD::TargetFrameIndex, _) => panic!("a frame index: the frame lowering is not translated yet"),
            _ => class(vt).map(|_| {
                let reg = self
                    .results
                    .get(&(value.node, value.res))
                    .copied()
                    .unwrap_or_else(|| panic!("operand {value:?} ({:?}) is not scheduled", node.opcode));
                Operand::Use(reg)
            }),
        }
    }

    fn block(
        &mut self,
        selected: &Selected,
    ) {
        let dag = &selected.dag;
        self.results.clear();
        // The operands of the `CALL_PARAMS` waiting for its `CALL_RESULTS`.
        let mut params: Vec<Operand> = Vec::new();
        let mut out: Vec<Inst> = Vec::new();
        // A unit lists its bottom-most node first; `EmitSchedule` emits the glued ones top-down.
        for &node in selected.units.iter().flat_map(|unit| unit.nodes.iter().rev()) {
            let data = dag.node(SDValue { node, res: 0 });
            match data.opcode {
                ISD::CopyFromReg => {
                    let NodeData::Register(number) = dag.node(data.ops[1]).data else { continue };
                    if let Some(ty) = class(data.values[0]) {
                        let reg = self.builder_reg(number, ty);
                        self.results.insert((node, 0), reg);
                    }
                }
                ISD::CopyToReg => {
                    let NodeData::Register(number) = dag.node(data.ops[1]).data else { continue };
                    let value = data.ops[2];
                    if let (Some(ty), Some(Operand::Use(src))) = (class(dag.value_type(value)), self.operand(dag, value)) {
                        let dst = self.builder_reg(number, ty);
                        out.push(Inst::new(copy(ty), &[Operand::Def(dst), Operand::Use(src)]));
                    }
                }
                ISD::Machine(code) => {
                    let code = code as u16;
                    let mut operands = Vec::new();
                    for res in 0..data.values.len() {
                        if let Some(ty) = class(data.values[res]) {
                            let reg = self.f.new_vreg(ty);
                            self.results.insert((node, res as u8), reg);
                            operands.push(Operand::Def(reg));
                        }
                    }
                    let uses: Vec<Operand> = data.ops.iter().filter_map(|&op| self.operand(dag, op)).collect();
                    if code == CALL_PARAMS {
                        params = uses;
                        continue;
                    }
                    if code == CALL_RESULTS {
                        // Defs, then the callee symbol, then the arguments.
                        operands.extend(std::mem::take(&mut params));
                        out.push(Inst::new(post::CALL, &operands));
                        continue;
                    }
                    operands.extend(uses);
                    let mut inst = Inst { opcode: code, operands, mem: None };
                    if let Some(access) = selected.mem_refs.get(&node).and_then(|nodes| nodes.first()) {
                        let info = &selected.accesses[access];
                        inst.mem = Some(Mem { size: u64::from(info.mem_vt.size_in_bits() / 8), align: u64::from(info.align) });
                    }
                    out.push(inst);
                }
                _ => {}
            }
        }
        self.f.blocks[0].extend(out);
    }
}

/// The function whose body is `blocks`, followed by `END_FUNCTION`. `names` gives the linker
/// name of a global by the id a `GlobalAddress` carries.
pub fn emit(
    name: &str,
    params: &[ValType],
    results: &[ValType],
    blocks: &[Selected],
    names: &dyn Fn(u32) -> String,
) -> Function {
    let mut f = Function::new(name);
    f.params = params.to_vec();
    f.results = results.to_vec();
    let mut emitter = Emitter { f, results: HashMap::new(), builder: BTreeMap::new(), names };
    for selected in blocks {
        emitter.block(selected);
    }
    let mut f = emitter.f;
    f.blocks[0].push(Inst::new(END_FUNCTION, &[]));
    f
}
