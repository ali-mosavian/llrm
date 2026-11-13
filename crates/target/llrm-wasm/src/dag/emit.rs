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
//! # See also
//! `InstrEmitter::EmitMachineNode`, pin d1106deb71cc.

use std::collections::BTreeMap;
use llrm_support::hash::HashMap;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};

use super::function::{PhiCopy, Selected};
use crate::dagisel::post_ids::{self as post, CALL_PARAMS, CALL_RESULTS};
use crate::mir::{EH_LABEL, Function, IMPLICIT_DEF, Inst, Mem, Operand, PHI, VReg, ValType};

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
    /// The `.Ltmp` labels made so far.
    labels: i64,
}

impl Emitter<'_> {
    /// The register `ReplacePhysRegs` makes of `SP32` / `SP64`: one for the
    /// function, the one its prologue defines.
    fn stack_pointer(
        &mut self,
        ty: ValType,
    ) -> VReg {
        match self.f.sp {
            Some(reg) => reg,
            None => {
                let reg = self.f.new_vreg(ty);
                self.f.sp = Some(reg);
                reg
            }
        }
    }

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
        let mut copied: HashMap<(u32, u8), (u32, u32)> = HashMap::default();
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
        // The value type of each of them: a frame index is a pointer with no
        // register.
        let mut param_types: Vec<ValType> = Vec::new();
        let mut out: Vec<Inst> = Vec::new();
        // A unit lists its bottom-most node first; `EmitSchedule` emits the
        // glued ones top-down.
        // What the instructions made so far came from: the `!dbg` of the
        // node's `SDNodeOrder`.
        let mut line = 0;
        let mut stamped = 0;
        for &node in selected.units.iter().flat_map(|unit| unit.nodes.iter().rev()) {
            out[stamped..].iter_mut().for_each(|inst| inst.line = line);
            stamped = out.len();
            let data = dag.node(SDValue { node, res: 0 });
            line = dag.lines.get(data.order as usize).copied().flatten().map_or(0, |at| at.line);
            match data.opcode {
                ISD::CopyFromReg => {
                    let Some(ty) = class(data.values[0]) else { continue };
                    let reg = match dag.node(data.ops[1]).data {
                        NodeData::Register(number) => self.builder_reg(number, ty),
                        // `InstrEmitter`: a read of a physical register is a
                        // `COPY` into a new one; the register changes later.
                        NodeData::PhysReg(_) => {
                            let source = self.stack_pointer(ty);
                            let reg = self.f.new_vreg(ty);
                            out.push(Inst::new(copy(ty), &[Operand::Def(reg), Operand::Use(source)]));
                            reg
                        }
                        _ => continue,
                    };
                    self.results.insert((node, 0), reg);
                }
                ISD::CopyToReg => {
                    let register = match dag.node(data.ops[1]).data {
                        NodeData::Register(number) => Some(number),
                        // `ReplacePhysRegs` names `SP32` and `SP64` one
                        // register.
                        NodeData::PhysReg(_) => None,
                        _ => continue,
                    };
                    let value = data.ops[2];
                    match (class(dag.value_type(value)), self.operand(dag, value)) {
                        (Some(ty), Some(Operand::Use(src))) => {
                            let dst = match register {
                                Some(number) => self.builder_reg(number, ty),
                                None => self.stack_pointer(ty),
                            };
                            if dst != src {
                                out.push(Inst::new(copy(ty), &[Operand::Def(dst), Operand::Use(src)]));
                            }
                        }
                        // A static alloca that is a phi's input: the frame
                        // lowering turns the index into the stack pointer
                        // plus its offset.
                        (Some(ty), Some(frame @ Operand::Frame(_))) => {
                            let dst = match register {
                                Some(number) => self.builder_reg(number, ty),
                                None => self.stack_pointer(ty),
                            };
                            out.push(Inst::new(copy(ty), &[Operand::Def(dst), frame]));
                        }
                        _ => {}
                    }
                }
                ISD::Machine(code) => {
                    // `TargetOpcode::IMPLICIT_DEF` is the generic opcode the
                    // matcher selects for `undef`; the wasm
                    // passes know it by `mir::IMPLICIT_DEF`.
                    let code = match code as u16 {
                        llrm_dagisel::generated::ids::target_opcode::IMPLICIT_DEF => IMPLICIT_DEF,
                        code => code,
                    };
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
                        param_types = data
                            .ops
                            .iter()
                            .filter(|&&op| self.operand(dag, op).is_some())
                            .filter_map(|&op| class(dag.value_type(op)))
                            .collect();
                        continue;
                    }
                    if code == CALL_RESULTS {
                        let mut params = std::mem::take(&mut params);
                        let mut param_types = std::mem::take(&mut param_types);
                        if let Some(&Operand::Use(callee)) = params.first() {
                            // `EmitInstrWithCustomInserter`: the function
                            // pointer moves to the end, after
                            // the type (its signature, by index) and the table.
                            params.remove(0);
                            param_types.remove(0);
                            let signature = (
                                param_types,
                                operands
                                    .iter()
                                    .filter_map(
                                        |o| if let Operand::Def(r) = o { Some(self.f.vreg_type(*r)) } else { None },
                                    )
                                    .collect::<Vec<_>>(),
                            );
                            let at = self
                                .f
                                .signatures
                                .iter()
                                .position(|s| *s == signature)
                                .unwrap_or_else(
                                    || {
                                        self.f.signatures.push(signature);
                                        self.f.signatures.len() - 1
                                    },
                                );
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
                    // `Select_FREEZE`: a `FREEZE` of a value that is not undef
                    // is selected as a generic `COPY`, which the wasm passes
                    // know by its class.
                    let code = match (code, operands.first()) {
                        (llrm_dagisel::generated::ids::target_opcode::COPY, Some(&Operand::Def(def))) => {
                            copy(self.f.vreg_type(def))
                        }
                        (code, _) => code,
                    };
                    let mut inst = Inst { opcode: code, operands, mem: None, tag: 0, line: 0 };
                    if let Some(access) = selected.mem_refs.get(&node).and_then(|nodes| nodes.first()) {
                        let info = &selected.accesses[access];
                        let ordered = info.volatile
                            || info.ordering.is_some_and(|o| {
                                !matches!(
                                    o,
                                    llrm_mir::opcode::AtomicOrdering::Unordered
                                        | llrm_mir::opcode::AtomicOrdering::Monotonic
                                )
                            });
                        inst.mem = Some(Mem {
                            size: u64::from(info.mem_vt.size_in_bits() / 8),
                            align: u64::from(info.align),
                            ordered,
                        });
                    }
                    out.push(inst);
                }
                _ => {}
            }
        }
        out[stamped..].iter_mut().for_each(|inst| inst.line = line);
        let at = self.position[&selected.id];
        if selected.landing {
            // `SelectAllBasicBlocks`: a label at the head of every EH pad.
            out.insert(0, Inst::new(EH_LABEL, &[Operand::Imm(self.labels)]));
            self.labels += 1;
        }
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
        // `visitInvoke`: the normal edge, then the pad's.
        if let Some(pad) = selected.unwind {
            let pad = self.position[&pad];
            if !succs.contains(&pad) {
                succs.push(pad);
            }
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
    f.has_var_sized = blocks.first().is_some_and(|block| block.has_var_sized);
    f.results = results.to_vec();
    let layout = blocks.first().map_or(Vec::new(), |block| block.layout.clone());
    f.blocks = vec![Vec::new(); layout.len().max(1)];
    f.succs = vec![Vec::new(); f.blocks.len()];
    let position = layout.iter().enumerate().map(|(at, &id)| (id, at)).collect();
    let mut emitter = Emitter { f, results: HashMap::default(), builder: BTreeMap::new(), names, position, labels: 0 };
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
    let mut starts: HashMap<usize, Vec<Inst>> = HashMap::default();
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
    // The probability of each edge where the IR block's own edges are the
    // machine block's; the rest are unknown (zero).
    f.edge_probs = f.succs.clone().iter().map(|to| vec![0; to.len()]).collect();
    for selected in blocks {
        let at = emitter.position[&selected.id];
        for (slot, &to) in f.succs[at].iter().enumerate() {
            if let Some(&(_, numerator)) = selected.probs.iter().find(|(id, _)| emitter.position.get(id) == Some(&to)) {
                f.edge_probs[at][slot] = numerator;
            }
        }
    }
    f
}
