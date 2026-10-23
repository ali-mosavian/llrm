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
//! DEVIATE SD400: `EmitLoweredSelect` expands one `CMOV_GR*` at a time: a run
//! of selects on one condition (case 1) and the cascaded form (case 2) each
//! get their own diamond, the same values with more branches. Reason: the
//! pin looks ahead in the block it is rewriting; this emitter is a single
//! pass. Exit: D1c, with the block-level pass. Owner: sdag-d. Review
//! 2026-12-03.
//!
//! # Runs
//! Nothing runs it yet: the shadow harness calls it once the matcher's output
//! reaches a whole function (D1c). The old selector stays the default.
//!
//! DEVIATE SD39: not translated yet: frame indices of locals (fixed objects are
//! cells), globals, segment
//! overrides, inline assembly and the custom inserters but `CMOV_GR*`.
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
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_classes::{Role, class_tag};

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};
use super::opcodes::OpcodeMap;
use super::schedule::Unit;
use crate::backend::isel::{_read, _written};
use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{Insn, Phi};

/// Why a node was not emitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmitError {
    /// A node or operand this emitter does not translate yet.
    Unsupported(String),
    /// An opcode with no `x86.instr` form (`opcodes_unmapped.txt`).
    Unmapped(String),
}

/// Where the frame's fixed objects are.
pub trait FrameMap {
    /// The cell of fixed object `index` (`FrameIndex<-1>` is the first stack
    /// argument), `width` bits wide, or `None` if there is no such object.
    fn cell(
        &self,
        index: i32,
        width: u32,
    ) -> Option<Mem>;
}

/// The pin's machine opcode names, by number.
pub struct EmitContext<'a> {
    /// The LIR address of each MIR block, by `BlockId`: what a branch names.
    pub block_at: &'a HashMap<u32, i64>,
    pub frame: &'a dyn FrameMap,
    /// How addresses are formed: real mode constrains a base and an index to
    /// their register classes (SD4).
    pub address_mode: AddrMode,
    pub map: &'a OpcodeMap,
    pub names: &'a HashMap<u16, &'a str>,
}

/// One block of the emitted code: `phis` first, then `insns`. A custom
/// inserter (`EmitLoweredSelect`) splits the block it is in.
#[derive(Clone, Debug, Default)]
pub struct EmitBlock {
    /// The block's address, as a branch names it.
    pub at: i64,
    pub phis: Vec<Phi>,
    pub insns: Vec<Arc<Insn>>,
}

struct Emitter<'a, 'd> {
    dag: &'d SelectionDAG,
    ctx: &'a EmitContext<'a>,
    /// `VRBaseMap`: the value each emitted result is.
    values: HashMap<SDValue, u32>,
    /// The LIR value a virtual register number of the DAG is.
    state: &'a mut EmitState,
    /// The blocks so far; the last is where the next instruction goes.
    blocks: Vec<EmitBlock>,
    /// The register tag each address component's value is constrained to
    /// (`constrainRegClass`).
    classes: Vec<(u32, &'static str)>,
}

/// What outlives one MIR block: the LIR value of each virtual register of the
/// DAG, the next value number and the next address (blocks, instructions).
#[derive(Debug)]
pub struct EmitState {
    pub vregs: HashMap<u32, u32>,
    pub next_value: u32,
    pub at: i64,
}

impl Default for EmitState {
    fn default() -> EmitState {
        EmitState { vregs: HashMap::new(), next_value: 1, at: 1 }
    }
}

impl EmitState {
    /// The LIR value a virtual register of the DAG is, made on first use.
    pub fn vreg(&mut self, reg: u32) -> u32 {
        if let Some(&v) = self.vregs.get(&reg) {
            return v;
        }
        let v = self.next_value;
        self.next_value += 1;
        self.vregs.insert(reg, v);
        v
    }

    pub fn next_address(&mut self) -> i64 {
        let at = self.at;
        self.at += 1;
        at
    }
}

/// What [`emit`] makes: the blocks, and the register tags (`registers.regs`)
/// that address components are confined to, by value.
#[derive(Clone, Debug, Default)]
pub struct Emitted {
    pub blocks: Vec<EmitBlock>,
    pub classes: Vec<(u32, &'static str)>,
}

/// The LIR blocks for `units` of `dag`, in order: one, plus the three a
/// `select` without `cmov` makes.
pub fn emit(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
) -> Result<Emitted, EmitError> {
    emit_in(dag, units, ctx, &mut EmitState::default(), 0)
}

/// [`emit`] for one MIR block of a function: values and addresses continue
/// from `state`, and the first block made is at `first_at`.
pub fn emit_in(
    dag: &SelectionDAG,
    units: &[Unit],
    ctx: &EmitContext<'_>,
    state: &mut EmitState,
    first_at: i64,
) -> Result<Emitted, EmitError> {
    let mut emitter = Emitter {
        dag,
        ctx,
        values: HashMap::new(),
        state,
        blocks: vec![EmitBlock { at: first_at, ..EmitBlock::default() }],
        classes: Vec::new(),
    };
    for unit in units {
        for &node in unit.nodes.iter().rev() {
            emitter.node(node)?;
        }
    }
    Ok(Emitted { blocks: emitter.blocks, classes: emitter.classes })
}

/// `X86::CondCode` order, the jump on each.
const CONDITIONS: [&str; 16] =
    ["jo", "jno", "jb", "jae", "je", "jne", "jbe", "ja", "js", "jns", "jp", "jnp", "jl", "jge", "jle", "jg"];

/// A LIR operand's width: bytes, as the old selector writes it.
fn width(vt: EVT) -> u32 {
    match vt {
        EVT::Simple(MVT::I1 | MVT::I8) => 1,
        EVT::Simple(MVT::I16) => 2,
        EVT::Simple(MVT::I32) => 4,
        EVT::Int(bits) => bits.div_ceil(8),
        _ => 4,
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
                let result = SDValue { node: n, res: 0 };
                match self.dag.node(node.ops[1]).data {
                    // `EmitCopyFromReg`: a virtual source is the value itself.
                    NodeData::Register(reg) => {
                        let value = self.vreg(reg);
                        self.values.insert(result, value);
                    }
                    NodeData::PhysReg(register) => {
                        let phys = Reg { register, width: width_of(self.dag, result) };
                        let held = self.fresh(width_of(self.dag, result));
                        self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Reg(phys)]);
                        self.values.insert(result, held.value);
                    }
                    _ => return Err(EmitError::Unsupported("CopyFromReg without a register".to_owned())),
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
        if let Some(done) = self.special(n, name)? {
            return Ok(done);
        }
        if name == "JMP_1" || name == "JCC_1" {
            return self.jump(n, name == "JCC_1");
        }
        if name == "RET" {
            // The value is already in its register (`CopyToReg`); the frame
            // pass adds the epilogue before it.
            self.push(Operation::Return, "ret", Vec::new(), Vec::new());
            return Ok(());
        }
        if matches!(name, "CMOV_GR8" | "CMOV_GR16" | "CMOV_GR32") {
            return self.lowered_select(n);
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

    /// `EmitLoweredSelect` for one `CMOV_GR*` pseudo (the pin's custom
    /// inserter): branch on the condition around a block that holds the false
    /// value, and join with a phi.
    ///
    /// ```text
    /// this:   jCC sink              this -> false, sink
    /// false:  (falls into sink)     false -> sink
    /// sink:   r := phi [false: src1] [this: src2]
    /// ```
    ///
    /// The operands are `src1` (the value when the condition is false),
    /// `src2`, and the condition code.
    fn lowered_select(
        &mut self,
        n: u32,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let held = |loc: Loc| match loc {
            Loc::Held(held) => Ok(held),
            other => Err(EmitError::Unsupported(format!("a select of {other:?}"))),
        };
        let when_false = held(self.operand(node.ops[0])?)?;
        let when_true = held(self.operand(node.ops[1])?)?;
        let cc = self.constant(node.ops[2])? as usize;
        let jump = *CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?;
        let result = self.fresh(width(node.values[0]));
        self.values.insert(SDValue { node: n, res: 0 }, result.value);
        let this = self.blocks.last().expect("a block").at;
        let (false_at, sink_at) = (self.next_address(), self.next_address());
        let target = Some(sink_at);
        let what = Semantics { target, name: Some(jump.into()), ..Semantics::new(Operation::Branch) };
        self.push_semantics(what);
        let phi = Phi { result: result.value, incoming: vec![(false_at, when_false.value), (this, when_true.value)] };
        self.blocks.push(EmitBlock { at: false_at, ..EmitBlock::default() });
        self.blocks.push(EmitBlock { at: sink_at, phis: vec![phi], ..EmitBlock::default() });
        Ok(())
    }

    fn next_address(&mut self) -> i64 {
        self.state.next_address()
    }

    /// Opcodes `x86.instr` has no row for but LIR has a form of: the constant
    /// loads (`MOV32r0` is `mov r, 0`, which a later pass makes `xor r, r`),
    /// `MOV32ImmSExti8`, and `LEA32r`/`LEA16r` (`lea r, [address]`). `None`:
    /// not one of them.
    fn special(
        &mut self,
        n: u32,
        name: &str,
    ) -> Result<Option<()>, EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let width = width(node.values[0]);
        let constant = match name {
            "MOV32r0" => Some(0),
            "MOV32r1" => Some(1),
            "MOV32r_1" => Some(-1),
            _ => None,
        };
        if let Some(value) = constant {
            let held = self.fresh(width);
            self.values.insert(SDValue { node: n, res: 0 }, held.value);
            self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![Loc::Imm(Imm { value, width, address: None })]);
            return Ok(Some(()));
        }
        match name {
            "MOV32ImmSExti8" => {
                let held = self.fresh(width);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                let source = self.operand(node.ops[0])?;
                self.push(Operation::Move, "mov", vec![Loc::Held(held)], vec![source]);
                Ok(Some(()))
            }
            "LEA32r" | "LEA16r" => {
                let cell = self.cell(node.ops.get(..5).ok_or_else(|| EmitError::Unsupported("a short lea".to_owned()))?, width)?;
                let held = self.fresh(width);
                self.values.insert(SDValue { node: n, res: 0 }, held.value);
                self.push(Operation::Address, "lea", vec![Loc::Held(held)], vec![Loc::Address(cell.address())]);
                Ok(Some(()))
            }
            _ => Ok(None),
        }
    }

    /// `JMP_1 dest` and `JCC_1 dest, cc`: a jump or a conditional branch to a
    /// MIR block.
    fn jump(
        &mut self,
        n: u32,
        conditional: bool,
    ) -> Result<(), EmitError> {
        let node = self.dag.node(SDValue { node: n, res: 0 });
        let block = node
            .ops
            .iter()
            .find_map(|op| match self.dag.node(*op).data {
                NodeData::Block(id) => Some(id),
                _ => None,
            })
            .ok_or_else(|| EmitError::Unsupported("a jump without a block".to_owned()))?;
        let target = *self.ctx.block_at.get(&block).ok_or_else(|| EmitError::Unsupported(format!("block {block}")))?;
        let (op, name) = if conditional {
            let cc = node
                .ops
                .iter()
                .find_map(|op| self.dag.constant(*op))
                .ok_or_else(|| EmitError::Unsupported("a branch without a condition".to_owned()))?
                as usize;
            (Operation::Branch, *CONDITIONS.get(cc).ok_or_else(|| EmitError::Unsupported(format!("condition code {cc}")))?)
        } else {
            (Operation::Jump, "jmp")
        };
        self.push_semantics(Semantics { target: Some(target), name: Some(name.into()), ..Semantics::new(op) });
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
        if matches!(
            self.dag.opcode(group[0]),
            ISD::FrameIndex | ISD::TargetFrameIndex
        ) {
            return self.frame_cell(group, cell_width);
        }
        let base = self.address_register(group[0], Role::Base)?;
        let scale = self.constant(group[1])?;
        let index = self.address_register(group[2], Role::Index)?;
        let disp = self.constant(group[3])?;
        if self.address_register(group[4], Role::Segment)?.is_some() {
            return Err(EmitError::Unsupported("a segment override".to_owned()));
        }
        Ok(Mem { base, index, scale, offset: disp, ..Mem::new(None, cell_width) })
    }

    /// A cell of the frame: the fixed object plus the displacement operand.
    fn frame_cell(
        &mut self,
        group: &[SDValue],
        cell_width: u32,
    ) -> Result<Mem, EmitError> {
        let NodeData::FrameIndex(index) = self.dag.node(group[0]).data else {
            return Err(EmitError::Unsupported("a frame index without an index".to_owned()));
        };
        if self.address_register(group[2], Role::Index)?.is_some()
            || self.address_register(group[4], Role::Segment)?.is_some()
        {
            return Err(EmitError::Unsupported("an indexed or segment-overridden frame cell".to_owned()));
        }
        let disp = self.constant(group[3])?;
        let cell = self
            .ctx
            .frame
            .cell(index, cell_width)
            .ok_or_else(|| EmitError::Unsupported(format!("frame index {index}")))?;
        Ok(Mem { offset: cell.offset + disp, ..cell })
    }

    /// An address component: a value, or the `$noreg` register.
    fn address_register(
        &mut self,
        value: SDValue,
        role: Role,
    ) -> Result<Option<Held>, EmitError> {
        // `$noreg` is virtual register 0.
        if self.dag.node(value).data == NodeData::Register(0) {
            return Ok(None);
        }
        match self.operand(value)? {
            Loc::Held(held) => {
                if let Some(tag) = class_tag(self.ctx.address_mode, role) {
                    self.classes.push((held.value, tag));
                }
                Ok(Some(held))
            }
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
            return Ok(Loc::Imm(Imm { value: sign_extend(bits, w * 8), width: w, address: None }));
        }
        if self.dag.opcode(value) == ISD::Register {
            return self.register(value);
        }
        match self.values.get(&value) {
            Some(&v) => Ok(Loc::Held(Held { value: v, width: width_of(self.dag, value) })),
            None => Err(EmitError::Unsupported(format!("operand {:?} before its definition", self.dag.opcode(value)))),
        }
    }

    /// A `Register` node: a physical register, or a virtual one's value.
    fn register(
        &mut self,
        value: SDValue,
    ) -> Result<Loc, EmitError> {
        let width = width_of(self.dag, value);
        match self.dag.node(value).data {
            NodeData::PhysReg(register) => Ok(Loc::Reg(Reg { register, width })),
            NodeData::Register(reg) => Ok(Loc::Held(Held { value: self.vreg(reg), width })),
            _ => Err(EmitError::Unsupported("a register operand without a register".to_owned())),
        }
    }

    fn vreg(
        &mut self,
        reg: u32,
    ) -> u32 {
        self.state.vreg(reg)
    }

    fn new_value(&mut self) -> u32 {
        let v = self.state.next_value;
        self.state.next_value += 1;
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
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        self.push_semantics(what);
    }

    fn push_semantics(
        &mut self,
        what: Semantics,
    ) {
        let (defines, uses) = (_written(&what.dests), _read(&what));
        let at = self.next_address();
        let insn = Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses));
        self.blocks.last_mut().expect("a block").insns.push(insn);
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
    node.values.first().filter(|vt| **vt != EVT::OTHER).map_or(4, |vt| width(*vt))
}

fn sign_extend(
    bits: u128,
    width: u32,
) -> i64 {
    let shift = 128 - width.clamp(1, 128);
    (((bits << shift) as i128) >> shift) as i64
}
