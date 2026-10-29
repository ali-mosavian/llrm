//! Scheduled machine nodes to an [`MFunc`]: the pin's `InstrEmitter`.
//!
//! # How it works
//! Each unit is walked in order. A `CopyFromReg` of a live-in virtual register
//! is that register's value; of a physical one it is a `COPY`. A `CopyToReg`
//! to a physical register is a `COPY` into it. A machine node becomes one
//! instruction through [`build`], which names its opcode (`ADDWrr`) and reads
//! its operands. `RET_ReallyLR` becomes `Op::Return` over the physical
//! registers it names.
//!
//! DEVIATE: one block; `ADD`/`SUB` register, shifted-register and immediate
//! forms and the integer `LDR`/`STR` (`ui`, `UR`, `roX`) forms only, with no
//! memory operand kept (`MemOperand::default`). Reason: each family adds rows
//! to [`build`] with an encoder byte check. Exit: C2..Cn.

use std::collections::HashMap;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;
use llrm_lir::registers::RegId;

use super::function::Selected;
use crate::REGISTER_INFO;
use crate::mc::inst::{Addr, AddSub, Extend, Gpr, Inst, Memory, MemOperand, Shift, Width};
use crate::mc::visit::Reg;
use crate::mfunc::{MBlock, MFunc, Op, block_label};

/// The machine opcode's name.
fn opcode_name(code: u32) -> Option<&'static str> {
    MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name)
}

/// The machine register number of an arm64 register id: `x5` and `w5` are 5.
fn gpr_of(id: RegId) -> Result<Gpr, String> {
    let name = REGISTER_INFO.name(id).ok_or("a register the arm64 file does not name")?;
    let number = name
        .strip_prefix(['x', 'w'])
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|&n| n <= 30)
        .ok_or_else(|| format!("register {name} is no x/w register"))?;
    Ok(Gpr(number))
}

/// An operand of a machine node as the instruction builders read it.
#[derive(Clone, Copy)]
enum Operand {
    Reg(Gpr),
    Imm(u64),
    /// A frame object, by the builder's frame index.
    Slot(u32),
}

/// `ADDWrr`, `SUBSXri` and the rest of the add/sub family: (subtract, set
/// flags, width, immediate form).
fn add_sub(name: &str) -> Option<(AddSub, Width, bool)> {
    let (op, rest) = if let Some(rest) = name.strip_prefix("ADDS") {
        (AddSub { subtract: false, set_flags: true }, rest)
    } else if let Some(rest) = name.strip_prefix("ADD") {
        (AddSub { subtract: false, set_flags: false }, rest)
    } else if let Some(rest) = name.strip_prefix("SUBS") {
        (AddSub { subtract: true, set_flags: true }, rest)
    } else {
        (AddSub { subtract: true, set_flags: false }, name.strip_prefix("SUB")?)
    };
    let (width, form) = match rest {
        "Wrr" => (Width::W, false),
        "Xrr" => (Width::X, false),
        "Wri" => (Width::W, true),
        "Xri" => (Width::X, true),
        "Wrs" => (Width::W, false),
        "Xrs" => (Width::X, false),
        _ => return None,
    };
    Some((op, width, form))
}

/// What a load or store opcode moves, how it addresses, and whether it stores.
struct MemoryForm {
    width: Width,
    bytes: u8,
    sign: bool,
    store: bool,
    form: AddressForm,
}

#[derive(Clone, Copy, PartialEq)]
enum AddressForm {
    /// `ui`: an offset in units of the access size.
    Scaled,
    /// `UR`: an unscaled signed offset.
    Unscaled,
    /// `roX`: a base and an index register.
    Register,
}

/// `LDRWui`, `STURXi`, `LDRSBWroX` and the rest of the integer load/store family.
fn memory_form(name: &str) -> Option<MemoryForm> {
    let (store, rest) = if let Some(rest) = name.strip_prefix("LDUR") {
        (false, ("UR", rest))
    } else if let Some(rest) = name.strip_prefix("STUR") {
        (true, ("UR", rest))
    } else if let Some(rest) = name.strip_prefix("LDR") {
        (false, ("", rest))
    } else {
        (true, ("", name.strip_prefix("STR")?))
    };
    let (unscaled, rest) = rest;
    let (element, form) = if unscaled == "UR" {
        (rest.strip_suffix('i')?, AddressForm::Unscaled)
    } else if let Some(element) = rest.strip_suffix("ui") {
        (element, AddressForm::Scaled)
    } else {
        (rest.strip_suffix("roX")?, AddressForm::Register)
    };
    let (width, bytes, sign) = match element {
        "BB" => (Width::W, 1, false),
        "HH" => (Width::W, 2, false),
        "W" => (Width::W, 4, false),
        "X" => (Width::X, 8, false),
        "SBW" => (Width::W, 1, true),
        "SBX" => (Width::X, 1, true),
        "SHW" => (Width::W, 2, true),
        "SHX" => (Width::X, 2, true),
        "SW" => (Width::X, 4, true),
        _ => return None,
    };
    Some(MemoryForm { width, bytes, sign, store, form })
}

/// The instruction of machine opcode `name` writing `rd`.
fn build(
    name: &str,
    rd: Gpr,
    operands: &[Operand],
) -> Result<Inst, String> {
    if let Some((op, width, immediate)) = add_sub(name) {
        return match (immediate, operands) {
            (false, [Operand::Reg(rn), Operand::Reg(rm)]) => {
                Ok(Inst::AddSubShifted { op, width, rd, rn: *rn, rm: *rm, shift: Shift::Lsl, amount: 0 })
            }
            // `ADDWrs rn, rm, shifter`: the shift type in bits 6.., the amount below
            (false, [Operand::Reg(rn), Operand::Reg(rm), Operand::Imm(shifter)]) => {
                let shift = match shifter >> 6 {
                    0 => Shift::Lsl,
                    1 => Shift::Lsr,
                    2 => Shift::Asr,
                    other => return Err(format!("{name}: shift type {other}")),
                };
                Ok(Inst::AddSubShifted { op, width, rd, rn: *rn, rm: *rm, shift, amount: (shifter & 0x3f) as u8 })
            }
            // `ADDWri rn, imm12, shift` where the shift operand is 0 or 12
            (true, [Operand::Reg(rn), Operand::Imm(imm), Operand::Imm(shift)]) => Ok(Inst::AddSubImm {
                op,
                width,
                rd,
                rn: *rn,
                imm12: u16::try_from(*imm).map_err(|_| format!("{name}: immediate {imm} is no imm12"))?,
                shift12: *shift == 12,
            }),
            _ => Err(format!("{name}: operands are not those of its form")),
        };
    }
    Err(format!("machine opcode {name} has no arm64 instruction yet"))
}

struct Emitter<'a> {
    dag: &'a SelectionDAG,
    mf: MFunc,
    /// The virtual register each value is in.
    values: HashMap<(u32, u8), Gpr>,
    /// The machine register each of the builder's virtual registers became.
    vregs: HashMap<u32, Gpr>,
    ops: Vec<Op>,
}

impl Emitter<'_> {
    fn vreg(
        &mut self,
        number: u32,
    ) -> Gpr {
        if let Some(&one) = self.vregs.get(&number) {
            return one;
        }
        let one = self.mf.gpr();
        self.vregs.insert(number, one);
        one
    }

    /// The register operand `value` names: a physical or a virtual register.
    fn register_operand(
        &mut self,
        value: SDValue,
    ) -> Result<(Gpr, bool), String> {
        match self.dag.node(value).data {
            NodeData::PhysReg(id) => Ok((gpr_of(id)?, true)),
            NodeData::Register(number) => Ok((self.vreg(number), false)),
            other => Err(format!("a register operand that is {other:?}")),
        }
    }

    fn operand(
        &self,
        value: SDValue,
    ) -> Result<Operand, String> {
        let node = self.dag.node(value);
        match node.data {
            NodeData::Const { bits, .. } if matches!(node.opcode, ISD::TargetConstant | ISD::Constant) => {
                Ok(Operand::Imm(bits as u64))
            }
            NodeData::FrameIndex(index) if node.opcode == ISD::TargetFrameIndex => Ok(Operand::Slot(index as u32)),
            _ => self
                .values
                .get(&(value.node, value.res))
                .map(|&g| Operand::Reg(g))
                .ok_or_else(|| format!("operand {:?} has no register", node.opcode)),
        }
    }

    /// A load or store: operands are `[value,] base, offset...` without chains.
    fn memory(
        &mut self,
        id: u32,
        name: &str,
        access: &MemoryForm,
        operands: &[Operand],
    ) -> Result<(), String> {
        let (value, address) = match (access.store, operands) {
            (true, [Operand::Reg(value), rest @ ..]) => (Some(*value), rest),
            (false, rest) => (None, rest),
            _ => return Err(format!("{name}: a store of something that is no register")),
        };
        let size = i64::from(access.bytes);
        let addr = match (access.form, address) {
            (AddressForm::Register, [Operand::Reg(base), Operand::Reg(index), Operand::Imm(_), Operand::Imm(shifted)]) => {
                Addr::Register { base: *base, index: *index, extend: Extend::Uxtx, shifted: *shifted != 0 }
            }
            (AddressForm::Register, _) => return Err(format!("{name}: a register offset that is not base and index")),
            (form, [base, Operand::Imm(imm)]) => {
                let imm = *imm as i64 * if form == AddressForm::Scaled { size } else { 1 };
                match base {
                    Operand::Reg(base) => Addr::Offset { base: *base, imm },
                    Operand::Slot(slot) => Addr::Slot { slot: *slot, offset: imm },
                    Operand::Imm(_) => return Err(format!("{name}: a base that is a constant")),
                }
            }
            _ => return Err(format!("{name}: operands are not those of its form")),
        };
        let rd = if access.store { None } else { Some(self.mf.gpr()) };
        let mem = Memory::Int { width: access.width, bytes: access.bytes, sign: access.sign, reg: value.or(rd).expect("a value") };
        let mmo = MemOperand::default();
        self.ops.push(Op::I(if access.store { Inst::Store { mem, addr, mmo } } else { Inst::Load { mem, addr, mmo } }));
        if let Some(rd) = rd {
            self.values.insert((id, 0), rd);
        }
        Ok(())
    }

    fn node(
        &mut self,
        id: u32,
    ) -> Result<(), String> {
        let node = self.dag.node(SDValue { node: id, res: 0 });
        match node.opcode {
            ISD::EntryToken | ISD::TokenFactor | ISD::Register => Ok(()),
            ISD::CopyFromReg => {
                let (reg, physical) = self.register_operand(node.ops[1])?;
                let value = if physical {
                    let dst = self.mf.gpr();
                    self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(reg) });
                    dst
                } else {
                    reg
                };
                self.values.insert((id, 0), value);
                Ok(())
            }
            ISD::CopyToReg => {
                let (dst, _) = self.register_operand(node.ops[1])?;
                let Operand::Reg(src) = self.operand(node.ops[2])? else { return Err("a copy of a constant".into()) };
                self.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(src) });
                Ok(())
            }
            ISD::Machine(code) => {
                let name = opcode_name(code).ok_or("a machine node with no name")?;
                if name == "RET_ReallyLR" {
                    let uses = node
                        .ops
                        .iter()
                        .filter_map(|op| match self.dag.node(*op).data {
                            NodeData::PhysReg(id) => Some(gpr_of(id).map(Reg::Gpr)),
                            _ => None,
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    self.ops.push(Op::Return { uses, tail: None });
                    return Ok(());
                }
                let operands = node
                    .ops
                    .iter()
                    .filter(|op| {
                        let vt = self.dag.value_type(**op);
                        vt != EVT::OTHER && vt != EVT::GLUE
                    })
                    .map(|&op| self.operand(op))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(access) = memory_form(name) {
                    return self.memory(id, name, &access, &operands);
                }
                let rd = self.mf.gpr();
                self.ops.push(Op::I(build(name, rd, &operands)?));
                self.values.insert((id, 0), rd);
                Ok(())
            }
            other => Err(format!("node {other:?} is not selected")),
        }
    }
}

/// `blocks` (one) of function `name` as a machine function.
pub fn emit(
    name: &str,
    blocks: &[Selected],
) -> Result<MFunc, String> {
    let [block] = blocks else { return Err("more than one block: branches are not translated".into()) };
    let mut emitter =
        Emitter { dag: &block.dag, mf: MFunc::new(name), values: HashMap::new(), vregs: HashMap::new(), ops: Vec::new() };
    for &(size, align) in &block.frame {
        emitter.mf.slot(size, align);
    }
    for &(reg, vreg) in &block.live_ins {
        let dst = emitter.vreg(vreg);
        emitter.ops.push(Op::Copy { dst: Reg::Gpr(dst), src: Reg::Gpr(gpr_of(reg)?) });
    }
    for unit in &block.units {
        for &node in unit.nodes.iter().rev() {
            emitter.node(node)?;
        }
    }
    let ops = std::mem::take(&mut emitter.ops);
    let mut mf = emitter.mf;
    mf.blocks.push(MBlock { label: block_label(name, 0), ops });
    Ok(mf)
}
