//! The copies of a wasm function joined by the shared, translated
//! `RegisterCoalescer`, through the spike machine function.
//!
//! wasm's virtual registers become `MFunction` values of four classes, each
//! with one register that only tags the value type, so a join needs equal
//! types; no pass may read them as a register file. The translated coalescer
//! runs over the converted `LirBody`; the joins it made come back as its
//! join map, and the wasm function is renamed to match.
//!
//! DEVIATE: the converted function's instruction names are the pin's opcode
//! names and no more: operand classes, flags and costs are the defaults.
//! Exit: the neutral machine function, which takes the generated
//! instruction description.

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

    /// `WebAssemblySubtarget::enableMachineScheduler` is false, and with it
    /// `enableJoinGlobalCopies`: one top-down list per block.
    fn joins_global_copies() -> bool {
        false
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

pub(crate) fn name_of(opcode: u16) -> &'static str {
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
    if std::env::var_os("LLRM_WASM_MIR").is_some() {
        eprintln!("{}", dump(f));
    }
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
    let map = converted.joined();
    let rename = |r: VReg| -> VReg { map.survivor(r + 1) - 1 };
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
    if std::env::var_os("LLRM_WASM_MIR").is_some() {
        eprintln!("after:\n{}", dump(f));
    }
    joined
}

/// The function's blocks as text, for `LLRM_WASM_MIR`.
pub fn dump(f: &Function) -> String {
    let mut out = format!("function {}\n", f.name);
    for (at, insts) in f.blocks.iter().enumerate() {
        out.push_str(&format!("bb.{at}: succs {:?}\n", f.succs[at]));
        for inst in insts {
            let operands: Vec<String> = inst
                .operands
                .iter()
                .map(|o| match o {
                    Operand::Def(r) => format!("def %{r}"),
                    Operand::Use(r) => format!("%{r}"),
                    Operand::Imm(v) => format!("{v}"),
                    Operand::Block(b) => format!("bb.{b}"),
                    other => format!("{other:?}"),
                })
                .collect();
            out.push_str(&format!("    {} {}\n", name_of(inst.opcode), operands.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::Inst;

    /// `%b = COPY %a; %c = ADD %b, %b; RETURN %c`: `%a` dies at the copy, so
    /// the copy is one register.
    fn function(second_use_of_a: bool) -> Function {
        let mut f = Function::default();
        for _ in 0..4 {
            f.new_vreg(ValType::I32);
        }
        let (a, b, c) = (0, 1, 2);
        let mut insts = vec![
            Inst::new(op::CONST_I32, &[Operand::Def(a), Operand::Imm(7)]),
            Inst::new(ValType::I32.copy(), &[Operand::Def(b), Operand::Use(a)]),
        ];
        if second_use_of_a {
            // `%a` is read after `%b` is written to: they overlap.
            insts.push(Inst::new(op::ADD_I32, &[Operand::Def(3), Operand::Use(a), Operand::Use(b)]));
            insts.push(Inst::new(op::MUL_I32, &[Operand::Def(c), Operand::Use(b), Operand::Use(3)]));
        } else {
            insts.push(Inst::new(op::ADD_I32, &[Operand::Def(c), Operand::Use(b), Operand::Use(b)]));
        }
        insts.push(Inst::new(op::RETURN, &[Operand::Use(c)]));
        f.blocks = vec![insts];
        f.succs = vec![vec![]];
        f
    }

    /// Every copy stayed a `local.get`/`local.set` pair at `-O2`: the shared
    /// coalescer was not run on wasm's functions.
    #[test]
    fn a_copy_whose_source_dies_is_joined() {
        let mut f = function(false);
        coalesce(&mut f);
        assert!(f.blocks[0].iter().all(|inst| !is_copy(inst.opcode)), "{:?}", f.blocks[0]);
    }

    /// A source read after the destination is written keeps its own register
    /// only where the two hold different values; `%b` is the same value as
    /// `%a` here, so the join still happens.
    #[test]
    fn a_copy_of_the_same_value_is_joined_even_when_the_source_lives_on() {
        let mut f = function(true);
        coalesce(&mut f);
        assert!(f.blocks[0].iter().all(|inst| !is_copy(inst.opcode)), "{:?}", f.blocks[0]);
    }
}
