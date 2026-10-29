//! An m68k machine function and `InstrEmitter`: the scheduled DAG's machine
//! nodes as instructions on virtual registers.
//!
//! # How it works
//! Each machine node becomes an instruction whose operands are its results
//! that are defined (`MCInstrDesc::getNumDefs`) and then its value operands;
//! a chain, glue or register-mask operand is dropped. `CopyToReg` and
//! `CopyFromReg` become [`Kind::Copy`]. A value is a virtual register, a
//! physical one (a `Register` node), an immediate, a frame index, a symbol or
//! a block.
//!
//! A phi is the successor block's [`Phi`]: the register the phi reads, and per
//! predecessor the register that block copied its value to.
//!
//! DEVIATE: spike-local, not `llrm-lir`'s function (its `RegId` is numbered by
//! iced's x86 registers). No jump tables, switches or variadics. Reason: there
//! is no neutral machine function yet (neutral-lir-need.md). Exit: the
//! neutral-LIR task.
//!
//! # See also
//! `llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp`, pin d1106deb71cc.

use std::collections::HashMap;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_dagisel::instr::OperandDesc;
pub use llrm_spike_mf as mf;
pub use llrm_spike_mf::{Block, Inst, Kind, Operand, Phi, Reg};

use crate::dagisel::function::Selected;
use crate::dagisel::generated::ids::reg as pin;
use crate::dagisel::generated::instrinfo::{self, flag};
use crate::dagisel::generated::m68k_m68000::MACHINE_OPCODES;

/// The register class a virtual register is allocated from
/// (`M68kRegisterInfo`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Dr8,
    Dr16,
    Dr32,
    Ar32,
    Xr32,
}

impl Class {
    /// The pin's class id (`M68k::DR32RegClassID`) as a class.
    fn of_pin(id: i16) -> Option<Class> {
        let id = u16::try_from(id).ok()?;
        Some(match id {
            pin::DR8RegClassID => Class::Dr8,
            pin::DR16RegClassID => Class::Dr16,
            pin::DR32RegClassID => Class::Dr32,
            pin::AR32RegClassID => Class::Ar32,
            pin::XR32RegClassID | pin::XR16RegClassID => Class::Xr32,
            _ => return None,
        })
    }

    /// The class both `self` and `other` allow (`constrainRegClass`), if any.
    pub fn meet(
        self,
        other: Class,
    ) -> Option<Class> {
        match (self, other) {
            (one, two) if one == two => Some(one),
            (Class::Xr32, one) | (one, Class::Xr32) => Some(one),
            _ => None,
        }
    }

    /// The class of a value of type `vt` that nothing constrains.
    fn of_type(vt: EVT) -> Class {
        match vt {
            EVT::I8 | EVT::I1 => Class::Dr8,
            EVT::I16 => Class::Dr16,
            _ => Class::Xr32,
        }
    }
}

/// The machine function over m68k's classes.
pub type MFunction = mf::MFunction<Class>;

fn opcode_name(code: u32) -> Option<&'static str> {
    MACHINE_OPCODES.iter().find(|(value, _)| u32::from(*value) == code).map(|(_, name)| *name)
}

/// Whether a result type carries a value (not a chain, glue or flags register).
fn is_value(vt: EVT) -> bool {
    vt != EVT::OTHER && vt != EVT::GLUE
}

struct Emitter<'a> {
    /// The block being emitted.
    dag: &'a SelectionDAG,
    function: MFunction,
    /// The virtual register each defined result is in.
    values: HashMap<(u32, u8), Reg>,
    /// The builder's virtual registers, by number.
    builder: HashMap<u32, Reg>,
}

impl Emitter<'_> {
    fn new_reg(
        &mut self,
        class: Class,
    ) -> Reg {
        self.function.classes.push(class);
        Reg::Virt(self.function.classes.len() as u32 - 1)
    }

    fn register(
        &mut self,
        node: SDValue,
    ) -> Option<Reg> {
        match self.dag.node(node).data {
            NodeData::PhysReg(id) => Some(Reg::Phys(id)),
            NodeData::Register(number) => {
                let class = Class::of_type(self.dag.value_type(node));
                let made = self.builder.get(&number).copied();
                Some(made.unwrap_or_else(|| {
                    let made = self.new_reg(class);
                    self.builder.insert(number, made);
                    made
                }))
            }
            _ => None,
        }
    }

    /// `InstrEmitter`'s `constrainRegClass`: a register an instruction reads
    /// must be in the class the operand names.
    fn constrain(
        &mut self,
        operand: &Operand,
        wanted: Option<&OperandDesc>,
    ) -> Result<(), String> {
        let (Operand::Use(Reg::Virt(v)), Some(wanted)) = (operand, wanted) else { return Ok(()) };
        let Some(class) = Class::of_pin(wanted.reg_class) else { return Ok(()) };
        let have = self.function.classes[*v as usize];
        self.function.classes[*v as usize] =
            have.meet(class).ok_or_else(|| format!("v{v} is {have:?} where {class:?} is read"))?;
        Ok(())
    }

    fn operand(
        &mut self,
        value: SDValue,
    ) -> Result<Option<Operand>, String> {
        let vt = self.dag.value_type(value);
        if !is_value(vt) && !matches!(self.dag.opcode(value), ISD::BasicBlock) {
            return Ok(None);
        }
        let node = self.dag.node(value);
        Ok(Some(match (node.opcode, node.data) {
            (ISD::RegisterMask, _) => return Ok(None),
            (ISD::Register, _) => Operand::Use(self.register(value).ok_or("a register node with no register")?),
            (ISD::Constant | ISD::TargetConstant, NodeData::Const { bits, .. }) => {
                let width = vt.size_in_bits().clamp(1, 64);
                let shift = 64 - width;
                Operand::Imm(((bits as u64) << shift) as i64 >> shift)
            }
            (ISD::FrameIndex | ISD::TargetFrameIndex, NodeData::FrameIndex(index)) => Operand::Frame(index),
            (ISD::TargetGlobalAddress | ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                let name = self.dag.global_names.get(symbol as usize).ok_or("a global with no name")?;
                Operand::Symbol(name.clone(), offset)
            }
            (ISD::BasicBlock, NodeData::Block(id)) => Operand::Block(id),
            _ => match self.values.get(&(value.node, value.res)) {
                Some(reg) => Operand::Use(*reg),
                None => return Err(format!("operand {:?} before its definition", node.opcode)),
            },
        }))
    }

    fn emit(
        &mut self,
        node: u32,
        into: &mut Vec<Inst>,
    ) -> Result<(), String> {
        let value = SDValue { node, res: 0 };
        let n = self.dag.node(value);
        match n.opcode {
            ISD::CopyToReg => {
                let dst = self.register(n.ops[1]).ok_or("CopyToReg of no register")?;
                // a result the instruction defines implicitly (the condition
                // codes of a compare) is already in that register
                let source = n.ops[2];
                if matches!(dst, Reg::Phys(_))
                    && matches!(self.dag.opcode(source), ISD::Machine(_))
                    && !self.values.contains_key(&(source.node, source.res))
                {
                    return Ok(());
                }
                let Some(Operand::Use(src)) = self.operand(n.ops[2])? else {
                    return Err("CopyToReg of a value that is no register".to_owned());
                };
                into.push(Inst::copy(dst, src));
            }
            ISD::CopyFromReg => {
                let src = self.register(n.ops[1]).ok_or("CopyFromReg of no register")?;
                let dst = self.new_reg(Class::of_type(n.values[0]));
                self.values.insert((node, 0), dst);
                into.push(Inst::copy(dst, src));
            }
            ISD::Machine(code) => {
                let name = opcode_name(code).ok_or("a machine node with no name")?;
                let desc = instrinfo::desc(code as u16).ok_or_else(|| format!("{name}: no description"))?;
                let mut operands = Vec::new();
                for (at, vt) in n.values.iter().enumerate().take(usize::from(desc.num_defs)) {
                    let class = desc
                        .operands
                        .get(at)
                        .and_then(|one| Class::of_pin(one.reg_class))
                        .unwrap_or_else(|| Class::of_type(*vt));
                    let reg = self.new_reg(class);
                    self.values.insert((node, at as u8), reg);
                    operands.push(Operand::Def(reg));
                }
                for op in n.ops.clone() {
                    if let Some(operand) = self.operand(op)? {
                        self.constrain(&operand, desc.operands.get(operands.len()))?;
                        operands.push(operand);
                    }
                }
                let ties = desc
                    .operands
                    .iter()
                    .enumerate()
                    .filter(|(_, one)| one.tied_to >= 0)
                    .map(|(at, one)| (one.tied_to as usize, at))
                    .collect();
                let ccr = pin::CCR;
                into.push(Inst {
                    call: desc.has(flag::CALL),
                    ties,
                    terminator: desc.has(flag::TERMINATOR),
                    barrier: desc.has(flag::BARRIER),
                    sets_flags: desc.implicit_defs.contains(&ccr),
                    reads_flags: desc.implicit_uses.contains(&ccr),
                    ..Inst::new(Kind::Op(name), operands)
                });
            }
            _ => {}
        }
        Ok(())
    }
}

/// `InstrEmitter` for one function's blocks.
pub fn emit(
    name: &str,
    blocks: &[Selected],
) -> Result<MFunction, String> {
    let first = blocks.first().ok_or("a function with no block")?;
    let mut emitter =
        Emitter { dag: &first.dag, function: MFunction::new(name), values: HashMap::new(), builder: HashMap::new() };
    for one in blocks {
        emitter.dag = &one.dag;
        emitter.values.clear();
        let mut insts = Vec::new();
        // A unit lists its glued nodes last first.
        for unit in &one.units {
            for &node in unit.nodes.iter().rev() {
                emitter.emit(node, &mut insts)?;
            }
        }
        emitter.function.fixed.extend(one.dag.fixed_offsets.clone());
        emitter.function.blocks.push(Block { id: one.block.0, insts, phis: Vec::new() });
    }
    // `HandlePHINodesInSuccessorBlocks`: each export is an input of a phi in
    // the block the phi is in.
    for (pred, one) in blocks.iter().enumerate() {
        for update in &one.phis {
            let at = blocks.iter().position(|other| other.block == update.phi_block).ok_or("a phi in no block")?;
            let (Some(&result), Some(&incoming)) =
                (emitter.builder.get(&update.result), emitter.builder.get(&update.incoming))
            else {
                continue;
            };
            let phis = &mut emitter.function.blocks[at].phis;
            match phis.iter_mut().find(|phi| phi.result == result) {
                Some(phi) => phi.incoming.push((pred, incoming)),
                None => phis.push(Phi { result, incoming: vec![(pred, incoming)] }),
            }
        }
    }
    Ok(emitter.function)
}
