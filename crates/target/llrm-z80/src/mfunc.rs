//! The Z80 machine function and `InstrEmitter`: the scheduled DAG's machine
//! nodes as instructions on virtual registers.
//!
//! # How it works
//! Each machine node becomes an instruction whose operands are the results it
//! defines (`MCInstrDesc::getNumDefs`) and then its value operands; chains,
//! glue and the register mask are dropped. `CopyToReg` and `CopyFromReg` become
//! [`Kind::Copy`]. A flags operand (`Flags` class) is the physical `flags`
//! register: the compare defines it and the branch reads it.
//!
//! The allocator has no sub-register operands, so the pin's `EXTRACT_SUBREG`
//! and `REG_SEQUENCE` of a pair's bytes are the pseudos `LO`, `HI` and `PACK`,
//! which [`crate::expand`] turns into byte moves.
//!
//! A phi is the successor block's [`Phi`]: the register the phi reads, and per
//! predecessor the register that block copied its value to.
//!
//! DEVIATE: spike-local, not `llrm-lir`'s function. No jump tables, switches
//! or variadics. Reason: there is no neutral machine function yet. Exit: the
//! neutral-LIR task.
//!
//! # See also
//! `llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp`.

use std::collections::{HashMap, HashSet};

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_dagisel::instr::OperandDesc;
pub use llrm_spike_mf::{Block, Inst, Kind, Operand, Phi, Reg};

use crate::dagisel::function::Selection;
use crate::dagisel::generated::ids::reg as pin;
use crate::dagisel::generated::instrinfo::{self, flag};
use crate::dagisel::generated::z80_z80::MACHINE_OPCODES;
pub use crate::regalloc::{Class, MFunction};

impl Class {
    /// The pin's class id (`Z80::GR16RegClassID`) as a class; the flags have
    /// none.
    fn of_pin(id: i16) -> Option<Class> {
        Some(match u16::try_from(id).ok()? {
            pin::GR8RegClassID => Class::Gr8,
            pin::ACCRegClassID => Class::Acc,
            pin::GR16RegClassID => Class::Gr16,
            pin::HLRRegClassID => Class::Hlr,
            _ => return None,
        })
    }

    /// The class of a value of type `vt` that nothing constrains.
    fn of_type(vt: EVT) -> Class {
        match vt {
            EVT::I16 => Class::Gr16,
            _ => Class::Gr8,
        }
    }
}

/// The register of a class that has only one: `ACC` is `a`, `HLR` is `hl`.
///
/// An operand in such a class is that physical register, with a copy in or out
/// around the instruction, as x86's fixed-register operands are. Left as a
/// virtual register of a one-register class, a value joined to the argument
/// register `a` is live across an instruction that needs `a` for another
/// operand, and no register is left for it (`add8` failed to allocate).
fn fixed_register(id: i16) -> Option<llrm_lir::registers::RegId> {
    match u16::try_from(id).ok()? {
        pin::ACCRegClassID => Some(crate::reg::A),
        pin::HLRRegClassID => Some(crate::reg::HL),
        _ => None,
    }
}

/// The register a pin register number (`Z80::HL`) names, if it is one the
/// allocator hands out: the flags and the stack pointer are not.
fn allocatable(id: u16) -> Option<llrm_lir::registers::RegId> {
    Some(match id {
        pin::A => crate::reg::A,
        pin::B => crate::reg::B,
        pin::BC => crate::reg::BC,
        pin::C => crate::reg::C,
        pin::D => crate::reg::D,
        pin::DE => crate::reg::DE,
        pin::E => crate::reg::E,
        pin::H => crate::reg::H,
        pin::HL => crate::reg::HL,
        pin::IY => crate::reg::IY,
        pin::L => crate::reg::L,
        _ => return None,
    })
}

/// Whether the pin's class id is the flags.
fn is_flags(id: i16) -> bool {
    u16::try_from(id) == Ok(pin::FlagsRegClassID)
}

/// The class the pin's description of `name` gives operand `index`.
pub fn operand_class(
    name: &str,
    index: usize,
) -> Option<Class> {
    let code = MACHINE_OPCODES.iter().find(|(_, one)| *one == name)?.0;
    Class::of_pin(instrinfo::desc(code)?.operands.get(index)?.reg_class)
}

fn opcode_name(code: u32) -> Option<&'static str> {
    let at = MACHINE_OPCODES.binary_search_by_key(&(code as u16), |&(one, _)| one).ok()?;
    Some(MACHINE_OPCODES[at].1)
}

/// Whether a result type carries a value (not a chain or glue).
fn is_value(vt: EVT) -> bool {
    vt != EVT::OTHER && vt != EVT::GLUE
}

struct Emitter<'a> {
    dag: &'a SelectionDAG,
    function: MFunction,
    /// The virtual register each defined result is in.
    values: HashMap<(u32, u8), Reg>,
    /// The builder's virtual registers, by number.
    builder: HashMap<u32, Reg>,
    /// The argument register each virtual register is a copy of.
    arrives_in: HashMap<Reg, llrm_lir::registers::RegId>,
    /// The results of the instructions emitted so far.
    made: HashSet<Reg>,
    /// For each flags value, the instructions that make it (the compare and the
    /// copies into its registers), to be issued again where the flags were
    /// overwritten before a reader.
    flag_groups: HashMap<(u32, u8), Vec<Inst>>,
    /// The flags value the machine's flags hold now, if known.
    flags_now: Option<(u32, u8)>,
    /// The byte each `ZEXT8` widened.
    widened: HashMap<Reg, Reg>,
    /// For the block being emitted: the nodes that read each value.
    users: HashMap<(u32, u8), Vec<u32>>,
    /// The values known to be in `a` when they are made.
    in_acc: std::collections::HashSet<(u32, u8)>,
}

impl Emitter<'_> {
    fn new_reg(
        &mut self,
        class: Class,
    ) -> Reg {
        self.function.classes.push(class);
        Reg::Virt(self.function.classes.len() as u32 - 1)
    }

    /// The builder's register `number`, made at the class of `bits` bits.
    fn numbered(
        &mut self,
        number: u32,
        class: Class,
    ) -> Reg {
        if let Some(&made) = self.builder.get(&number) {
            return made;
        }
        let made = self.new_reg(class);
        self.builder.insert(number, made);
        made
    }

    fn register(
        &mut self,
        node: SDValue,
    ) -> Option<Reg> {
        match self.dag.node(node).data {
            NodeData::PhysReg(id) => Some(Reg::Phys(id)),
            NodeData::Register(number) => {
                let class = Class::of_type(self.dag.value_type(node));
                Some(self.numbered(number, class))
            }
            _ => None,
        }
    }

    /// `AddRegisterOperand`: a register an instruction reads must be in the
    /// class the operand names; where it cannot be narrowed to it, the value is
    /// copied into a new register of that class.
    fn constrain(
        &mut self,
        operand: Operand,
        wanted: Option<&OperandDesc>,
        into: &mut Vec<Inst>,
    ) -> Operand {
        let (Operand::Use(Reg::Virt(v)), Some(wanted)) = (&operand, wanted) else { return operand };
        let Some(class) = Class::of_pin(wanted.reg_class) else { return operand };
        let have = self.function.classes[*v as usize];
        match <crate::regalloc::Z80Registers as llrm_spike_mf::Target>::meet(have, class) {
            Some(narrowed) => {
                self.function.classes[*v as usize] = narrowed;
                operand
            }
            None => {
                let copy = self.new_reg(class);
                into.push(Inst::copy(copy, Reg::Virt(*v)));
                Operand::Use(copy)
            }
        }
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
                // C symbols take SDCC's underscore; the runtime's own routines
                // are named as they are
                Operand::Symbol(format!("_{name}"), offset)
            }
            (ISD::TargetExternalSymbolNode | ISD::ExternalSymbol, NodeData::Symbol(id)) => {
                let name = self.dag.symbol_names.get(id as usize).ok_or("an external symbol with no name")?;
                Operand::Symbol(name.clone(), 0)
            }
            (ISD::BasicBlock, NodeData::Block(id)) => Operand::Block(id),
            _ => match self.values.get(&(value.node, value.res)) {
                Some(reg) => Operand::Use(*reg),
                None => return Err(format!("operand {:?} before its definition", node.opcode)),
            },
        }))
    }

    /// A register operand of `value`, which must be one.
    fn used(
        &mut self,
        value: SDValue,
        what: &str,
    ) -> Result<Reg, String> {
        match self.operand(value)? {
            Some(Operand::Use(reg)) => Ok(reg),
            _ => Err(format!("{what} of a value that is no register")),
        }
    }

    fn emit(
        &mut self,
        node: u32,
        into: &mut Vec<Inst>,
    ) -> Result<(), String> {
        let n = self.dag.node(SDValue { node, res: 0 });
        match n.opcode {
            ISD::CopyToReg => {
                let dst = self.register(n.ops[1]).ok_or("CopyToReg of no register")?;
                let src = self.used(n.ops[2], "CopyToReg")?;
                into.push(Inst::copy(dst, src));
            }
            ISD::CopyFromReg => {
                let src = self.register(n.ops[1]).ok_or("CopyFromReg of no register")?;
                let dst = self.new_reg(Class::of_type(n.values[0]));
                self.values.insert((node, 0), dst);
                if let Some(&register) = self.arrives_in.get(&src) {
                    self.arrives_in.insert(dst, register);
                }
                into.push(Inst::copy(dst, src));
            }
            ISD::Machine(code) => {
                let name = opcode_name(code).ok_or_else(|| format!("a machine node with no name: {code}"))?;
                match name {
                    "EXTRACT_SUBREG" => return self.extract_subreg(node, into),
                    "REG_SEQUENCE" => return self.reg_sequence(node, into),
                    _ => {}
                }
                let (name, code) = self.byte_form(node, name, code);
                let desc = instrinfo::desc(code as u16).ok_or_else(|| format!("{name}: no description"))?;
                let mut operands = Vec::new();
                let mut defines_flags = false;
                let mut flags_key = None;
                for (at, vt) in n.values.iter().enumerate().take(usize::from(desc.num_defs)) {
                    let wanted = desc.operands.get(at);
                    let reg = if wanted.is_some_and(|one| is_flags(one.reg_class)) {
                        defines_flags = true;
                        flags_key = Some((node, at as u8));
                        Reg::Phys(crate::reg::FLAGS)
                    } else {
                        // a result in a one-register class is copied out of
                        // that register: the copy takes
                        // the general class (isel's CopyFromReg takes
                        // getRegClassFor(VT))
                        let class = wanted
                            .filter(|one| fixed_register(one.reg_class).is_none())
                            .and_then(|one| Class::of_pin(one.reg_class))
                            .unwrap_or(Class::of_type(*vt));
                        self.new_reg(class)
                    };
                    self.values.insert((node, at as u8), reg);
                    self.made.insert(reg);
                    if wanted.is_some_and(|one| fixed_register(one.reg_class) == Some(crate::reg::A)) {
                        self.in_acc.insert((node, at as u8));
                    }
                    operands.push(Operand::Def(reg));
                }
                let mut reads_flags = false;
                // flags another instruction overwrote since their compare:
                // compare again
                for op in &n.ops {
                    let key = (op.node, op.res);
                    if self.flags_now != Some(key) {
                        if let Some(group) = self.flag_groups.get(&key) {
                            into.extend(group.clone());
                            self.flags_now = Some(key);
                        }
                    }
                }
                for op in n.ops.clone() {
                    if let Some(operand) = self.operand(op)? {
                        reads_flags |= matches!(
                            operand,
                            Operand::Use(Reg::Phys(id)) if id == crate::reg::FLAGS
                        );
                        let wanted = desc.operands.get(operands.len());
                        let operand = if wanted.is_some_and(|one| fixed_register(one.reg_class).is_some()) {
                            operand
                        } else {
                            self.constrain(operand, wanted, into)
                        };
                        operands.push(operand);
                    }
                }
                if let ("ZEXT8", [Operand::Def(wide), Operand::Use(byte)]) = (name, &operands[..]) {
                    self.widened.insert(*wide, *byte);
                }
                self.commute_into_fixed(desc, &mut operands);
                if desc.has(flag::CALL) {
                    self.commute_call_arguments(&operands, into);
                }
                // a register the instruction overwrites besides its results
                // (`Defs = [A]`) is a def no one reads: the
                // allocator keeps nothing else in it
                operands.extend(
                    desc.implicit_defs.iter().filter_map(|&id| allocatable(id)).map(|id| Operand::Def(Reg::Phys(id))),
                );
                // operands in a one-register class: the register, copied in and
                // out
                let mut after = Vec::new();
                let group_start = into.len();
                for (at, operand) in operands.iter_mut().enumerate() {
                    let Some(fixed) = desc.operands.get(at).and_then(|one| fixed_register(one.reg_class)) else {
                        continue;
                    };
                    match *operand {
                        Operand::Def(reg) if reg != Reg::Phys(fixed) => {
                            after.push(Inst::copy(reg, Reg::Phys(fixed)));
                            *operand = Operand::Def(Reg::Phys(fixed));
                        }
                        Operand::Use(reg) if reg != Reg::Phys(fixed) => {
                            into.push(Inst::copy(Reg::Phys(fixed), reg));
                            *operand = Operand::Use(Reg::Phys(fixed));
                        }
                        _ => {}
                    }
                }
                let ties = desc
                    .operands
                    .iter()
                    .enumerate()
                    .filter(|(_, one)| one.tied_to >= 0)
                    .map(|(at, one)| (one.tied_to as usize, at))
                    .collect();
                into.push(Inst {
                    call: desc.has(flag::CALL),
                    ties,
                    terminator: desc.has(flag::TERMINATOR),
                    barrier: desc.has(flag::BARRIER),
                    sets_flags: defines_flags || desc.implicit_defs.contains(&pin::F),
                    reads_flags: reads_flags || desc.implicit_uses.contains(&pin::F),
                    may_load: desc.has(flag::MAY_LOAD),
                    may_store: desc.has(flag::MAY_STORE),
                    side_effects: desc.has(flag::UNMODELED_SIDE_EFFECTS),
                    cheap_as_move: desc.has(flag::CHEAP_AS_A_MOVE),
                    commutable: desc.has(flag::COMMUTABLE),
                    rematerializable: desc.has(flag::REMATERIALIZABLE),
                    ..Inst::new(Kind::Op(name), operands)
                });
                match flags_key {
                    Some(key) => {
                        self.flag_groups.insert(key, into[group_start..].to_vec());
                        self.flags_now = Some(key);
                    }
                    None if desc.implicit_defs.contains(&pin::F) || desc.has(flag::CALL) => self.flags_now = None,
                    None => {}
                }
                into.extend(after);
            }
            _ => {}
        }
        Ok(())
    }

    /// A commutable instruction whose result is tied to a fixed register (`add
    /// a, r`) takes as that operand the value that arrives in the register,
    /// if only the other one does not: the copy into the register is then
    /// the identity (`TwoAddressInstructionPass::tryInstructionCommute`,
    /// steered by the argument copies it would otherwise leave).
    fn commute_into_fixed(
        &self,
        desc: &llrm_dagisel::instr::InstrDesc,
        operands: &mut [Operand],
    ) {
        if !desc.has(flag::COMMUTABLE) || operands.len() < 3 {
            return;
        }
        let Some(fixed) = desc.operands.get(1).and_then(|one| fixed_register(one.reg_class)) else { return };
        let arrives = |operand: &Operand| match operand {
            Operand::Use(reg) => self.arrives_in.get(reg).copied(),
            _ => None,
        };
        // made here and read by nothing else, so the instruction may overwrite
        // it
        let made = |operand: &Operand| {
            let Operand::Use(reg) = operand else { return false };
            self.made.contains(reg)
                && self
                    .values
                    .iter()
                    .find(|(_, one)| *one == reg)
                    .is_some_and(|(key, _)| self.users.get(key).is_some_and(|users| users.len() == 1))
        };
        // a value made just before can be made in the register; one that lived
        // on from an earlier block or iteration is wherever its loop keeps it
        if arrives(&operands[2]) == Some(fixed) && arrives(&operands[1]) != Some(fixed)
            || arrives(&operands[1]) != Some(fixed)
                && arrives(&operands[2]) != Some(fixed)
                && made(&operands[2])
                && !made(&operands[1])
        {
            operands.swap(1, 2);
        }
    }

    /// `LD8rm` and `LD8mr` become `LD8Arp` and `LD8prA` when the byte is read
    /// in `a` anyway (an ALU operand, a compare, a return) or made there:
    /// the pointer may then be any pair and need not be `hl`.
    fn byte_form(
        &self,
        node: u32,
        name: &'static str,
        code: u32,
    ) -> (&'static str, u32) {
        use crate::dagisel::generated::ids::opcode;
        let n = self.dag.node(SDValue { node, res: 0 });
        match name {
            "LD8rm" => {
                let readers = self.users.get(&(node, 0)).map(Vec::as_slice).unwrap_or(&[]);
                let in_a = |user: u32| {
                    let user = self.dag.node(SDValue { node: user, res: 0 });
                    let position = user.ops.iter().position(|op| op.node == node && op.res == 0);
                    match user.opcode {
                        // a byte stored: the store reads it in `a` too
                        ISD::Machine(code) if opcode_name(code) == Some("LD8mr") && position == Some(0) => true,
                        ISD::Machine(code) => instrinfo::desc(code as u16)
                            .zip(position)
                            .and_then(|(desc, at)| desc.operands.get(at + usize::from(desc.num_defs)))
                            .is_some_and(|one| fixed_register(one.reg_class) == Some(crate::reg::A)),
                        ISD::CopyToReg => matches!(
                            self.dag.node(user.ops[1]).data,
                            NodeData::PhysReg(id) if id == crate::reg::A
                        ),
                        _ => false,
                    }
                };
                if !readers.is_empty() && readers.iter().all(|&user| in_a(user)) {
                    ("LD8Arp", u32::from(opcode::LD8Arp))
                } else {
                    (name, code)
                }
            }
            "LD8mr" if n.ops.first().is_some_and(|value| self.in_acc.contains(&(value.node, value.res))) => {
                ("LD8prA", u32::from(opcode::LD8prA))
            }
            _ => (name, code),
        }
    }

    /// A call of a commutative runtime routine (multiply) whose two arguments
    /// arrive in each other's registers is made with them exchanged, so neither
    /// is moved.
    fn commute_call_arguments(
        &self,
        operands: &[Operand],
        into: &mut [Inst],
    ) {
        let Some(Operand::Symbol(callee, _)) = operands.first() else { return };
        if !matches!(callee.as_str(), "__mulqi3" | "__mulhi3") {
            return;
        }
        let args: Vec<_> = operands
            .iter()
            .filter_map(|one| match one {
                Operand::Use(Reg::Phys(register)) => Some(*register),
                _ => None,
            })
            .collect();
        let [first, second] = args[..] else { return };
        let position = |register| {
            into.iter()
                .rposition(|inst| inst.kind == Kind::Copy && inst.operands[0] == Operand::Def(Reg::Phys(register)))
        };
        let (Some(a), Some(b)) = (position(first), position(second)) else { return };
        let source = |inst: &Inst| match inst.operands[1] {
            Operand::Use(reg) => reg,
            _ => unreachable!("a copy reads a register"),
        };
        let (from_a, from_b) = (source(&into[a]), source(&into[b]));
        if self.arrives_in.get(&from_a) == Some(&second) && self.arrives_in.get(&from_b) == Some(&first) {
            into[a].operands[1] = Operand::Use(from_b);
            into[b].operands[1] = Operand::Use(from_a);
        }
    }

    /// The constant of a `TargetConstant` operand.
    fn constant(
        &self,
        value: SDValue,
    ) -> Result<i64, String> {
        match self.dag.node(value).data {
            NodeData::Const { bits, .. } => Ok(bits as i64),
            _ => Err("a sub-register index that is no constant".to_owned()),
        }
    }

    /// `EXTRACT_SUBREG pair, sub_lo|sub_hi` is `LO` or `HI`.
    fn extract_subreg(
        &mut self,
        node: u32,
        into: &mut Vec<Inst>,
    ) -> Result<(), String> {
        let n = self.dag.node(SDValue { node, res: 0 });
        let src = self.used(n.ops[0], "EXTRACT_SUBREG")?;
        let name = match self.constant(n.ops[1])? {
            i if i == i64::from(pin::sub_lo) => "LO",
            i if i == i64::from(pin::sub_hi) => "HI",
            other => return Err(format!("EXTRACT_SUBREG of sub-register {other}")),
        };
        let dst = self.new_reg(Class::Gr8);
        self.values.insert((node, 0), dst);
        // the low byte of a widened byte is that byte
        match self.widened.get(&src) {
            Some(&byte) if name == "LO" => into.push(Inst::copy(dst, byte)),
            _ => into.push(Inst::new(Kind::Op(name), vec![Operand::Def(dst), Operand::Use(src)])),
        }
        Ok(())
    }

    /// `REG_SEQUENCE GR16, lo, sub_lo, hi, sub_hi` is `PACK pair, lo, hi`.
    fn reg_sequence(
        &mut self,
        node: u32,
        into: &mut Vec<Inst>,
    ) -> Result<(), String> {
        let n = self.dag.node(SDValue { node, res: 0 });
        let (mut lo, mut hi) = (None, None);
        // a selected load's chain result may follow the pairs, and an ALU
        // operand brings its flags result
        let flags = Some(&Reg::Phys(crate::reg::FLAGS));
        let made = |op: &SDValue| match self.values.get(&(op.node, op.res)) {
            Some(reg) => Some(reg) != flags,
            None => op.res == 0,
        };
        let pairs: Vec<SDValue> =
            n.ops[1..].iter().copied().filter(|op| is_value(self.dag.value_type(*op)) && made(op)).collect();
        for part in pairs.chunks(2) {
            let [value, index] = part else {
                return Err(format!(
                    "a REG_SEQUENCE with an odd operand: {:?}",
                    n.ops.iter().map(|op| (self.dag.opcode(*op), self.dag.value_type(*op))).collect::<Vec<_>>()
                ));
            };
            let reg = self.used(*value, "REG_SEQUENCE")?;
            match self.constant(*index)? {
                i if i == i64::from(pin::sub_lo) => lo = Some(reg),
                i if i == i64::from(pin::sub_hi) => hi = Some(reg),
                other => return Err(format!("REG_SEQUENCE of sub-register {other}")),
            }
        }
        let (Some(lo), Some(hi)) = (lo, hi) else { return Err("a REG_SEQUENCE of one byte".to_owned()) };
        let dst = self.new_reg(Class::Gr16);
        self.values.insert((node, 0), dst);
        into.push(Inst::new(Kind::Op("PACK"), vec![Operand::Def(dst), Operand::Use(lo), Operand::Use(hi)]));
        Ok(())
    }
}

/// `InstrEmitter` for one function's blocks.
pub fn emit(
    name: &str,
    selection: &Selection,
) -> Result<MFunction, String> {
    let first = selection.blocks.first().ok_or("a function with no block")?;
    let mut emitter = Emitter {
        dag: &first.dag,
        function: MFunction::new(name),
        values: HashMap::new(),
        builder: HashMap::new(),
        arrives_in: HashMap::new(),
        made: HashSet::new(),
        widened: HashMap::new(),
        flag_groups: HashMap::new(),
        flags_now: None,
        users: HashMap::new(),
        in_acc: Default::default(),
    };
    // the frame objects the selector made; the allocas came first, at their
    // index
    emitter.function.locals.extend(selection.locals.iter().enumerate().map(|(at, size)| (at as i32, *size, 1)));
    let width_class = |bits: u32| if bits > 8 { Class::Gr16 } else { Class::Gr8 };
    for one in &selection.blocks {
        emitter.dag = &one.dag;
        emitter.values.clear();
        emitter.in_acc.clear();
        emitter.flag_groups.clear();
        emitter.flags_now = None;
        emitter.users.clear();
        for (at, node) in one.dag.nodes().iter().enumerate() {
            for op in &node.ops {
                emitter.users.entry((op.node, op.res)).or_default().push(at as u32);
            }
        }
        let mut insts = Vec::new();
        // the registers the block is entered with, read before anything runs
        for &(register, number) in &one.live_ins {
            let class = if crate::REGISTER_INFO.bytes(register) == Some(2) { Class::Gr16 } else { Class::Gr8 };
            let into = emitter.numbered(number, class);
            emitter.arrives_in.insert(into, register);
            insts.push(Inst::copy(into, Reg::Phys(register)));
        }
        // A unit lists its glued nodes last first.
        for unit in &one.units {
            for &node in unit.nodes.iter().rev() {
                emitter.emit(node, &mut insts)?;
            }
        }
        emitter.function.fixed.extend(one.dag.fixed_offsets.clone());
        emitter.function.blocks.push(Block { id: one.block.0, insts, phis: Vec::new() });
    }
    phis(&mut emitter, selection, width_class)?;
    Ok(emitter.function)
}

/// `HandlePHINodesInSuccessorBlocks`: each copy an edge exports is an input of
/// a phi in the successor.
fn phis(
    emitter: &mut Emitter,
    selection: &Selection,
    width_class: impl Fn(u32) -> Class,
) -> Result<(), String> {
    for (pred, one) in selection.blocks.iter().enumerate() {
        for edge in &one.edges {
            // a block that ends without a branch falls into the next one
            let to = match edge.to {
                Some(to) => to.0,
                None => match only_successor(&emitter.function.blocks[pred])? {
                    Some(to) => to,
                    None => emitter.function.blocks.get(pred + 1).ok_or("a last block that falls through")?.id,
                },
            };
            let at = emitter.function.blocks.iter().position(|block| block.id == to).ok_or("a phi in no block")?;
            for &(input, result) in &edge.copies {
                let class = width_class(selection.widths.get(&result).copied().unwrap_or(16));
                let result = emitter.numbered(result, class);
                let incoming = emitter.numbered(input, class);
                let phis = &mut emitter.function.blocks[at].phis;
                match phis.iter_mut().find(|phi| phi.result == result) {
                    Some(phi) => phi.incoming.push((pred, incoming)),
                    None => phis.push(Phi { result, incoming: vec![(pred, incoming)] }),
                }
            }
        }
    }
    Ok(())
}

/// The one block `block` ends by branching to, none if it falls through.
fn only_successor(block: &Block) -> Result<Option<u32>, String> {
    let mut targets = block
        .insts
        .iter()
        .filter(|inst| inst.terminator)
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| if let Operand::Block(id) = operand { Some(*id) } else { None });
    let Some(first) = targets.next() else { return Ok(None) };
    if targets.any(|other| other != first) {
        return Err("phi copies on every edge of a block that branches two ways".to_owned());
    }
    Ok(Some(first))
}
