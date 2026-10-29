//! After allocation: the pseudo-instructions expanded, frame indices turned
//! into stack addresses, the prologue and epilogue added, and the machine
//! function lowered to printable instructions.
//!
//! The expansions are `M68kExpandPseudo`'s and `M68kInstrInfo`'s:
//! `MOVI32ri` is `moveq` (`ExpandMOVI`), `MOVZXd32p8` clears the register and
//! loads the byte (`ExpandMOVSZX_RM`), `RET` is `rts`, a register copy is
//! `move.l` (`copyPhysReg`), and the call-stack pseudos go (the frame reserves
//! the call area).
//!
//! DEVIATE: only 32-bit copies, `moveq` immediates and the zero-extending byte
//! load are expanded. Reason: the slice does not reach the rest. Exit: M4c.

use llrm_codegen::backend::frameobjects::Object;
use llrm_lir::registers::RegId;

use crate::frame::{self, Base, FrameOp, Function};
use crate::mc;
use crate::mfunc::{Kind, MFunction, Operand, Reg};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

fn physical(
    register: Reg,
    assignment: &Assignment,
) -> RegId {
    match register {
        Reg::Virt(v) => assignment.0[v as usize],
        Reg::Phys(p) => p,
    }
}

fn mask_bit(root: RegId) -> Option<u32> {
    let index = |all: [RegId; 8]| all.iter().position(|one| *one == root);
    index([reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7])
        .map(|at| at as u32)
        .or_else(
            || index([reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5, reg::A6, reg::SP]).map(|at| 8 + at as u32),
        )
}

fn frame_op(
    op: &FrameOp,
    out: &mut Vec<mc::Inst>,
) {
    use mc::Operand::{Imm, Reg as R};
    let base = |base: &Base| R(if *base == Base::Sp { reg::SP } else { reg::A6 });
    let mask = |registers: &[RegId]| {
        registers.iter().filter_map(|one| mask_bit(REGISTER_INFO.root(*one))).fold(0i64, |all, bit| all | 1 << bit)
    };
    out.push(match op {
        FrameOp::Link { bytes } => mc::Inst { opcode: "LINK16", operands: vec![R(reg::A6), Imm(-bytes)] },
        FrameOp::Unlink => mc::Inst { opcode: "UNLK", operands: vec![R(reg::A6)] },
        FrameOp::SubSp(bytes) => mc::Inst { opcode: "SUB32ai", operands: vec![R(reg::SP), R(reg::SP), Imm(*bytes)] },
        FrameOp::AddSp(bytes) => mc::Inst { opcode: "ADD32ai", operands: vec![R(reg::SP), R(reg::SP), Imm(*bytes)] },
        FrameOp::Save { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32pm", operands: vec![Imm(*offset), base(at), Imm(mask(registers))] }
        }
        FrameOp::Restore { registers, base: at, offset } => {
            mc::Inst { opcode: "MOVM32mp", operands: vec![Imm(mask(registers)), Imm(*offset), base(at)] }
        }
    });
}

/// The function as printable instructions.
pub fn finish(
    function: &MFunction,
    assignment: &Assignment,
) -> Result<Vec<mc::Inst>, String> {
    let insts: Vec<_> = function.blocks.iter().flat_map(|block| block.insts.iter()).collect();
    let has_calls = insts.iter().any(|inst| inst.call);
    let max_call_frame = insts
        .iter()
        .filter(|inst| inst.kind == Kind::Op("ADJCALLSTACKDOWN"))
        .filter_map(|inst| match inst.operands.first() {
            Some(Operand::Imm(bytes)) => Some(*bytes),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // the callee-saved registers the allocation used, in `CSR_STD` order
    let used: Vec<RegId> = frame::callee_saved()
        .into_iter()
        .filter(|saved| {
            insts
                .iter()
                .flat_map(|inst| &inst.operands)
                .any(
                    |operand| match operand {
                        Operand::Def(r) | Operand::Use(r) => REGISTER_INFO.root(physical(*r, assignment)) == *saved,
                        _ => false,
                    },
                )
        })
        .collect();
    let fixed: Vec<i32> = function.fixed.keys().rev().copied().collect();
    let objects: Vec<Object> = fixed
        .iter()
        .map(|index| Object { size: 4, align: 4, offset: function.fixed[index], callee_saved: false, dead: false })
        .collect();
    let layout = frame::layout(&Function {
        fixed: objects.len(),
        objects,
        saved: used,
        max_call_frame,
        adjusts_stack: has_calls,
        ..Function::default()
    });
    let mut out = Vec::new();
    layout.prologue().iter().for_each(|op| frame_op(op, &mut out));
    for inst in insts {
        let operands: Vec<mc::Operand> = {
            let mut converted = Vec::new();
            let mut it = inst.operands.iter().peekable();
            while let Some(operand) = it.next() {
                converted.push(match operand {
                    Operand::Def(r) | Operand::Use(r) => mc::Operand::Reg(physical(*r, assignment)),
                    Operand::Imm(value) => {
                        // a frame index follows the displacement it adds to
                        if let Some(Operand::Frame(index)) = it.peek() {
                            let at = fixed.iter().position(|one| one == index).ok_or("an unknown frame index")?;
                            let (base, disp) = layout.address(at);
                            it.next();
                            converted.push(mc::Operand::Imm(value + disp));
                            converted.push(mc::Operand::Reg(if base == Base::Sp { reg::SP } else { reg::A6 }));
                            continue;
                        }
                        mc::Operand::Imm(*value)
                    }
                    Operand::Symbol(name, addend) => mc::Operand::Symbol(name.clone(), *addend),
                    Operand::Block(id) => mc::Operand::Symbol(format!(".LBB{id}"), 0),
                    Operand::Frame(_) => return Err("a frame index with no displacement".to_owned()),
                });
            }
            converted
        };
        match inst.kind {
            Kind::Copy => {
                let [mc::Operand::Reg(dst), mc::Operand::Reg(src)] = operands[..] else {
                    return Err("a copy of non-registers".to_owned());
                };
                if REGISTER_INFO.root(dst) != REGISTER_INFO.root(src) {
                    out.push(mc::Inst {
                        opcode: "MOV32rr",
                        operands: vec![mc::Operand::Reg(dst), mc::Operand::Reg(src)],
                    });
                }
            }
            Kind::Op("ADJCALLSTACKDOWN" | "ADJCALLSTACKUP") => {}
            Kind::Op("RET") => {
                layout.epilogue().iter().for_each(|op| frame_op(op, &mut out));
                out.push(mc::Inst { opcode: "RTS", operands: vec![] });
            }
            Kind::Op("MOVI32ri") => {
                let [mc::Operand::Reg(dst), mc::Operand::Imm(value)] = operands[..] else {
                    return Err("MOVI32ri operands".to_owned());
                };
                if (-128..=127).contains(&value) {
                    out.push(mc::Inst {
                        opcode: "MOVQ",
                        operands: vec![mc::Operand::Reg(dst), mc::Operand::Imm(value)],
                    });
                } else {
                    out.push(mc::Inst {
                        opcode: "MOV32ri",
                        operands: vec![mc::Operand::Reg(dst), mc::Operand::Imm(value)],
                    });
                }
            }
            Kind::Op("MOVZXd32p8") => {
                let mc::Operand::Reg(dst) = operands[0] else { return Err("MOVZXd32p8 operands".to_owned()) };
                let byte = REGISTER_INFO.view(REGISTER_INFO.root(dst), 8).ok_or("no byte view")?;
                out.push(mc::Inst { opcode: "MOVQ", operands: vec![mc::Operand::Reg(dst), mc::Operand::Imm(0)] });
                let mut load = operands.clone();
                load[0] = mc::Operand::Reg(byte);
                out.push(mc::Inst { opcode: "MOV8dp", operands: load });
            }
            Kind::Op(name) => out.push(mc::Inst { opcode: name, operands }),
        }
    }
    Ok(out)
}
