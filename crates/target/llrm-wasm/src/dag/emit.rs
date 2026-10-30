//! `InstrEmitter` for wasm over the builder's DAG: the scheduled units of each
//! block as the instructions of a [`Function`].
//!
//! # How it works
//! A result of a machine node becomes a virtual register, an operand a use or
//! an immediate (constant, symbol, block). `CopyFromReg` of a virtual register
//! names that register; `CopyToReg` is a `COPY` into it. The builder's
//! registers are shared by every block, so a value that crosses blocks keeps
//! one register. A `CALL_PARAMS` and the `CALL_RESULTS` glued to it become one
//! `CALL`, as the custom inserter does
//! (`WebAssemblyTargetLowering::EmitInstrWithCustomInserter`).
//!
//! A phi is a `PHI` in its block over the registers the builder copied the
//! incoming values to (`FinishBasicBlock`); a machine node whose result a
//! `CopyToReg` of a virtual register copies is defined in that register
//! (`InstrEmitter::CreateVirtualRegisters`), so the copy is gone.
//!
//! DEVIATE: direct calls only, no frame indices. Reason: the frame and indirect
//! calls come with the frame lowering and the table. Exit: W8, W10.
//!
//! # See also
//! `InstrEmitter::EmitMachineNode`, pin d1106deb71cc.

use std::collections::{BTreeMap, HashMap};

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};

use super::function::{PhiCopy, Selected};
use crate::dagisel::post_ids::{self as post, CALL_PARAMS, CALL_RESULTS};
use crate::mir::{Function, Inst, Mem, Operand, PHI, VReg, ValType};

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
    /// The position of each IR block.
    position: HashMap<u32, usize>,
}

impl Emitter<'_> {
    fn builder_reg(
        &mut self,
        number: u32,
        ty: ValType,
    ) -> VReg {
        *self.builder.entry(number).or_insert_with(|| self.f.new_vreg(ty))
    }

    /// An operand of a machine node: a register, an immediate, or nothing
    /// (chain, glue).
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
            (ISD::TargetExternalSymbolNode, NodeData::Symbol(id)) => {
                let name = dag.symbol_names[id as usize].clone();
                Some(Operand::Sym(self.f.symbol(&name, 0)))
            }
            (ISD::TargetConstantFP | ISD::ConstantFP, NodeData::ConstFp { bits }) => Some(match vt {
                EVT::Simple(MVT::F32) => Operand::F32(bits as u32),
                _ => Operand::F64(bits as u64),
            }),
            (ISD::BasicBlock, NodeData::Block(block)) => Some(Operand::Block(self.position[&block] as u32)),
            (ISD::Register, _) => None,
            (ISD::TargetFrameIndex, NodeData::FrameIndex(index)) => Some(Operand::Frame(index)),
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
        // The builder register a result is copied to, where a `CopyToReg` takes
        // it whole.
        // `CreateVirtualRegisters` takes the first matching user of the result,
        // and a node's users list the newest first: the latest
        // `CopyToReg` wins.
        let mut copied: HashMap<(u32, u8), (u32, u32)> = HashMap::new();
        for &node in selected.units.iter().flat_map(|unit| &unit.nodes) {
            let data = dag.node(SDValue { node, res: 0 });
            if data.opcode != ISD::CopyToReg {
                continue;
            }
            let (NodeData::Register(number), value) = (dag.node(data.ops[1]).data, data.ops[2]) else { continue };
            if matches!(dag.node(value).opcode, ISD::Machine(_)) {
                let latest = copied.entry((value.node, value.res)).or_insert((node, number));
                if node > latest.0 {
                    *latest = (node, number);
                }
            }
        }
        // The operands of the `CALL_PARAMS` waiting for its `CALL_RESULTS`.
        let mut params: Vec<Operand> = Vec::new();
        let mut out: Vec<Inst> = Vec::new();
        // A unit lists its bottom-most node first; `EmitSchedule` emits the
        // glued ones top-down.
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
                    if let (Some(ty), Some(Operand::Use(src))) =
                        (class(dag.value_type(value)), self.operand(dag, value))
                    {
                        let dst = self.builder_reg(number, ty);
                        if dst != src {
                            out.push(Inst::new(copy(ty), &[Operand::Def(dst), Operand::Use(src)]));
                        }
                    }
                }
                ISD::Machine(code) => {
                    let code = code as u16;
                    let mut operands = Vec::new();
                    for res in 0..data.values.len() {
                        if let Some(ty) = class(data.values[res]) {
                            let reg = match copied.get(&(node, res as u8)) {
                                Some(&(_, number)) => self.builder_reg(number, ty),
                                None => self.f.new_vreg(ty),
                            };
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
                        let mut params = std::mem::take(&mut params);
                        if let Some(&Operand::Use(callee)) = params.first() {
                            // `EmitInstrWithCustomInserter`: the function pointer moves to the end, after
                            // the type (its signature, by index) and the table.
                            params.remove(0);
                            let signature = (
                                params
                                    .iter()
                                    .filter_map(|o| if let Operand::Use(r) = o { Some(self.f.vreg_type(*r)) } else { None })
                                    .collect::<Vec<_>>(),
                                operands
                                    .iter()
                                    .filter_map(|o| if let Operand::Def(r) = o { Some(self.f.vreg_type(*r)) } else { None })
                                    .collect::<Vec<_>>(),
                            );
                            let at = self.f.signatures.iter().position(|s| *s == signature).unwrap_or_else(|| {
                                self.f.signatures.push(signature);
                                self.f.signatures.len() - 1
                            });
                            operands.push(Operand::Imm(at as i64));
                            operands.push(Operand::Sym(self.f.symbol(crate::INDIRECT_FUNCTION_TABLE, 0)));
                            operands.extend(params);
                            operands.push(Operand::Use(callee));
                            out.push(Inst::new(post::CALL_INDIRECT, &operands));
                            continue;
                        }
                        // Defs, then the callee symbol, then the arguments.
                        operands.extend(params);
                        out.push(Inst::new(post::CALL, &operands));
                        continue;
                    }
                    operands.extend(uses);
                    let mut inst = Inst { opcode: code, operands, mem: None, tag: 0 };
                    if let Some(access) = selected.mem_refs.get(&node).and_then(|nodes| nodes.first()) {
                        let info = &selected.accesses[access];
                        inst.mem =
                            Some(Mem { size: u64::from(info.mem_vt.size_in_bits() / 8), align: u64::from(info.align) });
                    }
                    out.push(inst);
                }
                _ => {}
            }
        }
        let at = self.position[&selected.id];
        self.f.blocks[at].extend(out);
        // The machine CFG: the blocks the terminators name, then the next block
        // when control falls into it. An `invoke`'s unwind edge is none
        // (the pin's `LowerInvoke`).
        let mut succs: Vec<usize> = Vec::new();
        let mut falls = true;
        for inst in self.f.blocks[at].iter().filter(|inst| inst.is_terminator()) {
            for operand in &inst.operands {
                if let Operand::Block(to) = *operand {
                    if !succs.contains(&(to as usize)) {
                        succs.push(to as usize);
                    }
                }
            }
            falls = !inst.is_barrier();
        }
        if falls && at + 1 < self.f.blocks.len() {
            succs.push(at + 1);
        }
        self.f.succs[at] = succs;
    }
}

/// The function whose body is `blocks`. `names` gives the linker
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
    f.frame = blocks.first().map_or(Vec::new(), |block| block.frame.clone());
    f.results = results.to_vec();
    let layout = blocks.first().map_or(Vec::new(), |block| block.layout.clone());
    f.blocks = vec![Vec::new(); layout.len().max(1)];
    f.succs = vec![Vec::new(); f.blocks.len()];
    let position = layout.iter().enumerate().map(|(at, &id)| (id, at)).collect();
    let mut emitter = Emitter { f, results: HashMap::new(), builder: BTreeMap::new(), names, position };
    for selected in blocks {
        emitter.block(selected);
    }
    // The phis, at the start of their blocks: `PHI dst, src, pred, ..` in the
    // builder's order.
    let mut phis: BTreeMap<(u32, u32), (ValType, Vec<(u32, u32)>)> = BTreeMap::new();
    for selected in blocks {
        for &PhiCopy { to, dst, src, ty } in &selected.phis {
            // The edge is from each block the IR block's lowering made that
            // reaches `to`; a switch makes several.
            let reaches = |pred: &&Selected| {
                pred.origin == selected.id
                    && emitter.f.succs[emitter.position[&pred.id]].contains(&emitter.position[&to])
            };
            let entry = phis.entry((to, dst)).or_insert((ty, Vec::new()));
            entry.1.extend(blocks.iter().filter(reaches).map(|pred| (src, pred.id)));
        }
    }
    let mut starts: HashMap<usize, Vec<Inst>> = HashMap::new();
    for ((to, dst), (ty, incoming)) in phis {
        if incoming.is_empty() {
            continue;
        }
        let mut operands = vec![Operand::Def(emitter.builder_reg(dst, ty))];
        for (src, pred) in incoming {
            operands.push(Operand::Use(emitter.builder_reg(src, ty)));
            operands.push(Operand::Block(emitter.position[&pred] as u32));
        }
        starts.entry(emitter.position[&to]).or_default().push(Inst::new(PHI, &operands));
    }
    let mut f = emitter.f;
    for (at, mut phis) in starts {
        phis.append(&mut f.blocks[at]);
        f.blocks[at] = phis;
    }
    f
}
