//! POC (branch only): the copies of a wasm function joined by the shared,
//! translated `RegisterCoalescer`, through the spike machine function.
//!
//! wasm's virtual registers become `MFunction` values of four classes, each
//! with one register, so a join needs only the same type. The translated
//! coalescer then runs over the converted `LirBody`; which values it joined is
//! read back from the instructions it kept, and the wasm function is renamed
//! to match.

use std::collections::HashMap;

use llrm_lir::registers::{RegId, Regs};
use llrm_spike_mf::mfunc::Block;
use llrm_spike_mf::regalloc::Target;
use llrm_spike_mf::tolir::to_lir;
use llrm_spike_mf::{Inst as MInst, Kind, MFunction, Operand as MOperand, Reg};

use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{Function, Operand, VReg, ValType, is_copy};
use crate::{REGISTER_INFO, reg};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    I32,
    I64,
    F32,
    F64,
}

pub struct WasmLocals;

impl Target for WasmLocals {
    type Class = Class;

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        (one == other).then_some(one)
    }

    fn any() -> Class {
        Class::I32
    }

    fn candidates(class: Class) -> Vec<RegId> {
        vec![match class {
            Class::I32 => reg::WI32,
            Class::I64 => reg::WI64,
            Class::F32 => reg::WF32,
            Class::F64 => reg::WF64,
        }]
    }

    fn view(
        root: RegId,
        _class: Class,
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        REGISTER_INFO.root(register)
    }

    fn reserved(root: RegId) -> bool {
        root == reg::SP32 || root == reg::SP64 || root == reg::FP32 || root == reg::FP64
    }

    fn caller_saved() -> Vec<RegId> {
        Vec::new()
    }
}

fn class(ty: ValType) -> Class {
    match ty {
        ValType::I32 => Class::I32,
        ValType::I64 => Class::I64,
        ValType::F32 => Class::F32,
        ValType::F64 => Class::F64,
    }
}

fn name_of(opcode: u16) -> &'static str {
    use crate::dagisel::opcode_names::OPCODE_NAMES;
    OPCODE_NAMES.binary_search_by_key(&opcode, |&(id, _)| id).map_or("UNNAMED", |at| OPCODE_NAMES[at].1)
}

/// The function as the spike machine function: no phis (wasm's are gone by
/// now), operands in their order.
fn convert(f: &Function) -> MFunction<Class> {
    let mut out = MFunction::new(&f.name);
    out.classes = f.vregs.iter().map(|ty| class(*ty)).collect();
    for (at, insts) in f.blocks.iter().enumerate() {
        let mut block = Block { id: at as u32, ..Block::default() };
        for inst in insts {
            let operands = inst
                .operands
                .iter()
                .map(|operand| match *operand {
                    Operand::Def(r) => MOperand::Def(Reg::Virt(r)),
                    Operand::Use(r) => MOperand::Use(Reg::Virt(r)),
                    Operand::Imm(v) => MOperand::Imm(v),
                    Operand::Sym(s) => MOperand::Symbol(f.symbols[s as usize].0.clone(), f.symbols[s as usize].1),
                    Operand::Block(b) => MOperand::Block(b),
                    Operand::Frame(i) => MOperand::Frame(i),
                    Operand::F32(bits) => MOperand::Imm(i64::from(bits)),
                    Operand::F64(bits) => MOperand::Imm(bits as i64),
                })
                .collect();
            let mut made = if is_copy(inst.opcode) {
                let mut copy = MInst::new(Kind::Copy, operands);
                copy.cheap_as_move = true;
                copy
            } else {
                MInst::new(Kind::Op(name_of(inst.opcode)), operands)
            };
            if let Some(d) = desc(inst.opcode) {
                made.terminator = d.flags & flag::TERMINATOR != 0;
                made.barrier = d.flags & flag::BARRIER != 0;
                made.call = d.flags & flag::CALL != 0;
                made.may_load = d.may_load();
                made.may_store = d.may_store();
            }
            block.insts.push(made);
        }
        out.blocks.push(block);
    }
    out
}

/// Joins the copies of `f`; the number of values it joined.
pub fn coalesce(f: &mut Function) -> usize {
    let machine = convert(f);
    let registers = Regs(&REGISTER_INFO);
    let mut converted = match to_lir::<WasmLocals>(&machine, registers) {
        Ok(converted) => converted,
        Err(why) => {
            if std::env::var_os("LLRM_WASM_COALESCE_DEBUG").is_some() {
                eprintln!("{}: not converted: {why}", f.name);
            }
            return 0;
        }
    };
    converted.coalesce(registers, false);
    // Old value -> the value the kept instruction names there.
    let mut renamed: HashMap<u32, u32> = HashMap::new();
    let mut mismatched = 0;
    for block in converted.body.blocks.iter() {
        for insn in block.insns.iter() {
            let Some(&(b, i)) = converted.origin.get((insn.at / 2) as usize) else { continue };
            let wasm = &f.blocks[b][i];
            let defs: Vec<u32> = wasm
                .operands
                .iter()
                .filter_map(|o| if let Operand::Def(r) = o { Some(*r + 1) } else { None })
                .collect();
            let uses: Vec<u32> = wasm
                .operands
                .iter()
                .filter_map(|o| if let Operand::Use(r) = o { Some(*r + 1) } else { None })
                .collect();
            if defs.len() != insn.defines.len() || uses.len() != insn.uses.len() {
                mismatched += 1;
                continue;
            }
            for (old, new) in defs.iter().zip(&insn.defines).chain(uses.iter().zip(&insn.uses)) {
                renamed.insert(*old, *new);
            }
        }
    }
    if mismatched > 0 && std::env::var_os("LLRM_WASM_COALESCE_DEBUG").is_some() {
        eprintln!("{}: {mismatched} instructions with other operand counts", f.name);
    }
    let rename = |r: VReg| -> VReg { renamed.get(&(r + 1)).map_or(r, |new| new - 1) };
    let mut joined = 0;
    for block in f.blocks.iter_mut() {
        for inst in block.iter_mut() {
            for operand in inst.operands.iter_mut() {
                match operand {
                    Operand::Def(r) | Operand::Use(r) => {
                        let new = rename(*r);
                        joined += usize::from(new != *r);
                        *r = new;
                    }
                    _ => {}
                }
            }
        }
        block.retain(|inst| {
            !(is_copy(inst.opcode)
                && matches!(
                    (inst.operands.first(), inst.operands.get(1)),
                    (Some(Operand::Def(a)), Some(Operand::Use(b))) if a == b
                ))
        });
    }
    joined
}
