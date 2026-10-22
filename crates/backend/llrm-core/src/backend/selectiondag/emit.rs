//! `InstrEmitter`: scheduled units to LIR.
//!
//! # How it works
//! [`emit`] walks the units [`super::schedule::schedule`] returns, a unit's
//! glued nodes top first, and writes one LIR instruction per machine node and
//! per register copy. A node's results are virtual values, numbered in the
//! order they are emitted (`VRBaseMap`); an operand is that value, an
//! immediate, or a memory cell built from the pin's five address operands
//! (base, scale, index, displacement, segment).
//!
//! ```text
//! ADD32rm  r1, [r2 + r3*4 + 8]   (node: src1, base, scale, index, disp, seg)
//!   v4 := add v1, dword [v2 + v3*4 + 8]
//! CopyToReg %5, v4        ->  v5 := mov v4
//! ```
//!
//! A tied source is a source of the LIR form (`^0` in `x86.instr`), so the
//! pin's two-address instruction is three-address here and `twoaddr.rs`
//! ties it again. A memory destination with no result (`ADD32mr`) is the
//! form's destination and its tied source.
//!
//! # Example
//! `x = a + b` with `a`, `b` in virtual registers 1 and 2 and the result
//! copied to register 3: `v10 := add v1, v2` then `v3 := mov v10`.
//!
//! # Runs
//! Nothing runs it yet: the shadow harness calls it once the matcher's output
//! reaches a whole function (D1c). The old selector stays the default.
//!
//! DEVIATE SD39: not translated yet: frame indices, globals, segment
//! overrides, inline assembly and the custom inserters (`CMOV_GR*`, next).
//! Each is an [`EmitError::Unsupported`], never a skipped operand. Reason:
//! the builder (A2) and matcher output do not produce them yet. Exit: with
//! A2 and the custom inserter piece. Owner: sdag-d. Review 2026-12-03.
//!
//! # See also
//! `InstrEmitter::EmitMachineNode`
//! ([L1003](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp#L1003)),
//! `EmitCopyToRegClassNode`, `EmitCopyFromReg`.

use std::collections::HashMap;
use std::sync::Arc;

use llrm_dagisel::instr::{self, InstrDesc, operand_type};

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};
use super::opcodes::OpcodeMap;
use super::schedule::Unit;
use crate::backend::isel::{_read, _written};
use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::Insn;

/// Why a node was not emitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A node or operand this emitter does not translate yet.
    Unsupported(String),
    /// An opcode with no `x86.instr` form (`opcodes_unmapped.txt`).
    Unmapped(String),
}

/// What the emitter asks of the target's register file.
pub trait RegisterFile {
    /// The register a physical register number is, with its width; `None`
    /// for a virtual register.
    fn physical(
        &self,
        reg: u32,
    ) -> Option<Reg>;
}

/// The pin's machine opcode names, by number.
pub struct EmitContext<'a> {
    pub map: &'a OpcodeMap,
    pub names: &'a HashMap<u16, &'a str>,
    pub regs: &'a dyn RegisterFile,
}

struct Emitter<'a, 'd> {
    dag: &'d SelectionDAG,
    ctx: &'a EmitContext<'a>,
    /// `VRBaseMap`: the value each emitted result is.
    values: HashMap<SDValue, u32>,
    /// The LIR value a virtual register number of the DAG is.
    vregs: HashMap<u32, u32>,
    next_value: u32,
    at: i64,
    out: Vec<Arc<Insn>>,
}

/// The LIR for `units` of `dag`, in order.
pub fn emit(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
) -> Result<Vec<Arc<Insn>>, EmitError> {
    let mut emitter =
        Emitter { dag, ctx, values: HashMap::new(), vregs: HashMap::new(), next_value: 1, at: 0, out: Vec::new() };
    for unit in units {
        for &node in unit.nodes.iter().rev() {
            emitter.node(node)?;
        }
    }
    Ok(emitter.out)
}

fn width(vt: EVT) -> u32 {
    match vt {
        EVT::Simple(MVT::I1 | MVT::I8) => 8,
        EVT::Simple(MVT::I16) => 16,
        EVT::Simple(MVT::I32) => 32,
        EVT::Int(bits) => bits,
        _ => 32,
    }
}

impl Emitter<'_, '_> {
    fn node(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        match node.opcode {
            ISD::CopyToReg => {
                let dst = self.register(node.ops[1])?;
                let src = self.operand(node.ops[2])?;
                self.push(Operation::Move, "mov", vec![dst], vec![src]);
                Ok(())
            }
            ISD::CopyFromReg => {
                let reg = self.register_number(node.ops[1])?;
                let result = SDValue { node: n, res: 0 };
                match self.ctx.regs.physical(reg) {
                    // `EmitCopyFromReg`: a virtual source is the value itself.
                    None => {
                        let value = self.vreg(reg);
                        self.values.insert(result, value);
                    }
                    Some(phys) => {
                        let held = self.fresh(width_of(self.dag, result));
                        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Reg(phys)]);
                        self.values.insert(result, held.value);
                    }
                }
                Ok(())
            }
            ISD::Machine(opcode) => self.machine(n, opcode),
            // Chains, entry token, constants and registers are operands.
            _ => Ok(()),
        }
    }

    fn machine(
        &mut self,
        n: u32,
        opcode: u32,
    ) -> Result<(), EmitError> {
        let name =
            *self.ctx.names.get(&(opcode as u16)).ok_or_else(|| EmitError::Unsupported(format!("opcode {opcode}")))?;
        let desc =
            instr::desc(opcode as u16).ok_or_else(|| EmitError::Unsupported(format!("{name}: no descriptor")))?;
        if name == "COPY" {
            return self.copy(n);
        }
        let form = self.ctx.map.form(name).map_err(|_| EmitError::Unmapped(name.to_owned()))?;
        let form = &self.ctx.map.forms()[form];
        let node = self.dag.node(SDValue { node: n, res: 0 });
        // The results: the first `num_defs` values that are not chain or glue.
        let mut dests = Vec::new();
        for (i, vt) in node.values.iter().enumerate().take(usize::from(desc.num_defs)) {
            let held = self.fresh(width(*vt));
            self.values.insert(SDValue { node: n, res: i as u8 }, held.value);
            dests.push(Loc::Held(held));
        }
        let mut operands = self.operands(node.ops.as_slice(), desc, width_of_node(self.dag, n))?;
        // A memory destination (`ADD32mr`, no result): the cell is the
        // destination and its tied source.
        let memory_dest = dests.is_empty() && form.dests.first().is_some_and(|d| d.allows('m'));
        if memory_dest {
            let at = operands
                .iter()
                .position(|o| matches!(o, Loc::Mem(_)))
                .ok_or_else(|| EmitError::Unsupported(format!("{name}: no memory operand")))?;
            dests.push(operands.remove(at));
        }
        let mut sources = Vec::new();
        let mut rest = operands.into_iter();
        for source in &form.sources {
            let taken = match source.tied {
                Some(k) if memory_dest => dests.get(k).cloned(),
                _ => rest.next(),
            };
            sources.push(taken.ok_or_else(|| EmitError::Unsupported(format!("{name}: missing source")))?);
        }
        if rest.next().is_some() {
            return Err(EmitError::Unsupported(format!("{name}: more operands than the form's sources")));
        }
        let op = Operation::named(&form.operation)
            .ok_or_else(|| EmitError::Unsupported(format!("{name}: operation {}", form.operation)))?;
        self.push(op, &form.name, dests, sources);
        Ok(())
    }

    /// `COPY dst <- src`, a register-to-register move.
    fn copy(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let src = self.operand(node.ops[0])?;
        let held = self.fresh(width(node.values[0]));
        self.values.insert(SDValue { node: n, res: 0 }, held.value);
        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![src]);
        Ok(())
    }

    /// The machine node's input operands, a run of five memory operands
    /// becoming one cell. `desc.operands` lists the defs first.
    fn operands(
        &mut self,
        ops: &[SDValue],
        desc: &InstrDesc,
        cell_width: u32,
    ) -> Result<Vec<Loc>, EmitError> {
        let ins = &desc.operands[usize::from(desc.num_defs)..];
        let mut out = Vec::new();
        let mut i = 0;
        while i < ins.len() && i < ops.len() {
            if ins[i].kind == operand_type::MEMORY {
                let group =
                    ops.get(i..i + 5).ok_or_else(|| EmitError::Unsupported("a memory operand cut short".to_owned()))?;
                out.push(Loc::Mem(self.cell(group, cell_width)?));
                i += 5;
            } else {
                out.push(self.operand(ops[i])?);
                i += 1;
            }
        }
        Ok(out)
    }

    /// base, scale, index, displacement, segment.
    fn cell(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
    ) -> Result<Mem, EmitError> {
        let base = self.address_register(group[0])?;
        let scale = self.constant(group[1])?;
        let index = self.address_register(group[2])?;
        let disp = self.constant(group[3])?;
        if self.address_register(group[4])?.is_some() {
            return Err(EmitError::Unsupported("a segment override".to_owned()));
        }
        Ok(Mem { base, index, scale, offset: disp, ..Mem::new(None, cell_width) })
    }

    /// An address component: a value, or the `$noreg` register.
    fn address_register(
        &mut self,
        value: SDValue,
    ) -> Result<Option<Held>, EmitError> {
        if self.dag.opcode(value) == ISD::Register && self.register_number(value)? == 0 {
            return Ok(None);
        }
        match self.operand(value)? {
            Loc::Held(held) => Ok(Some(held)),
            other => Err(EmitError::Unsupported(format!("address component {other:?}"))),
        }
    }

    fn constant(
        &self,
        value: SDValue,
    ) -> Result<i64, EmitError> {
        match self.dag.node(value).data {
            NodeData::Const { bits, .. } => Ok(bits as i64),
            _ => Err(EmitError::Unsupported("a non-constant address part".to_owned())),
        }
    }

    fn operand(
        &mut self,
        value: SDValue,
    ) -> Result<Loc, EmitError> {
        if let NodeData::Const { bits, .. } = self.dag.node(value).data {
            let w = width_of(self.dag, value);
            return Ok(Loc::Imm(Imm { value: sign_extend(bits, w), width: w, address: None }));
        }
        if self.dag.opcode(value) == ISD::Register {
            return self.register(value);
        }
        match self.values.get(&value) {
            Some(&v) => Ok(Loc::Held(Held { value: v, width: width_of(self.dag, value) })),
            None => Err(EmitError::Unsupported(format!("operand {:?} before its definition", self.dag.opcode(value)))),
        }
    }

    fn register_number(
        &self,
        value: SDValue,
    ) -> Result<u32, EmitError> {
        match self.dag.node(value).data {
            NodeData::Register(reg) => Ok(reg),
            _ => Err(EmitError::Unsupported("a register operand without a register".to_owned())),
        }
    }

    fn register(
        &mut self,
        value: SDValue,
    ) -> Result<Loc, EmitError> {
        let reg = self.register_number(value)?;
        Ok(match self.ctx.regs.physical(reg) {
            Some(phys) => Loc::Reg(phys),
            None => Loc::Held(Held { value: self.vreg(reg), width: width_of(self.dag, value) }),
        })
    }

    fn vreg(
        &mut self,
        reg: u32,
    ) -> u32 {
        if let Some(&v) = self.vregs.get(&reg) {
            return v;
        }
        let v = self.new_value();
        self.vregs.insert(reg, v);
        v
    }

    fn new_value(&mut self) -> u32 {
        let v = self.next_value;
        self.next_value += 1;
        v
    }

    fn fresh(
        &mut self,
        width: u32,
    ) -> Held {
        Held { value: self.new_value(), width }
    }

    fn push(
        &mut self,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        let (defines, uses) = (_written(&what.dests), _read(&what));
        let at = self.at;
        self.at += 1;
        self.out.push(Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses)));
    }
}

fn width_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> u32 {
    width(dag.value_type(value))
}

/// The width of the node's memory access: its first result, else its first
/// operand that is a value.
fn width_of_node(
    dag: &SelectionDAG,
    n: u32,
) -> u32 {
    let node = dag.node(SDValue { node: n, res: 0 });
    node.values.first().filter(|vt| **vt != EVT::OTHER).map_or(32, |vt| width(*vt))
}

fn sign_extend(
    bits: u128,
    width: u32,
) -> i64 {
    let shift = 128 - width.clamp(1, 128);
    (((bits << shift) as i128) >> shift) as i64
}
