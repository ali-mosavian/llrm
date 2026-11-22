//! A byte sign-extended to a long that is only read as a word is `ext.w`.
//!
//! # How it works
//! [`narrow_extensions`] runs before allocation. A `MOVSXd32?8` (byte to long,
//! `ext.w` then `ext.l` on the 68000 and 68010) whose result is used only by
//! copies into word registers becomes the `MOVSXd16?8` of the same operands
//! (`ext.w`, 2 B and 4 clk less). The selector makes the long form for a word
//! result as the pin does (`(i16 (sext i8))` after the extending-load patterns
//! match, so the pin's llc has the dead `ext.l` too); the 68020's `extb.l`
//! is one instruction either way and is left alone.
//!
//! DEVIATE M68K-NARROW-SEXT (owner w9, exit 2026-12-31): the pin keeps the
//! long extension. Translation: the narrowing of a def to the width its uses
//! demand, as LLVM's `PeepholeOptimizer` and X86's `OptimizeExtInstr` do for
//! sub-register uses.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::REGISTER_INFO;
use crate::dagisel::generated::instrinfo;
use crate::dagisel::hooks::Cpu as SelectCpu;
use crate::dagisel::local;
use crate::dagisel::opcodes;
use crate::mfunc::{Class, MFunction, op_inst};

/// The number of extensions narrowed.
pub fn narrow_extensions(
    function: &mut MFunction,
    cpu: SelectCpu,
) -> usize {
    if cpu.at_least(SelectCpu::M68020) {
        return 0;
    }
    let words: Vec<_> =
        REGISTER_INFO.entries().filter(|(_, entry)| entry.name.starts_with("wd")).map(|(id, _)| id).collect();
    let count = function.classes.len();
    let mut defs = vec![0u32; count];
    // a register read by anything but a copy into a word register
    let mut wide_use = vec![false; count];
    for block in &function.blocks {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 1;
                wide_use[v as usize] = true;
            }
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    wide_use[*v as usize] = true;
                }
            }
        }
        for inst in &block.insts {
            // a copy into a word register, virtual or the result's `wd0`
            let word_copy = match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(_))]) => {
                    function.classes[*to as usize] == Class::Dr16
                }
                (Kind::Copy, [Operand::Def(Reg::Phys(to)), Operand::Use(Reg::Virt(_))]) => words.contains(to),
                _ => false,
            };
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                    Operand::Use(Reg::Virt(v)) if !word_copy => wide_use[*v as usize] = true,
                    _ => {}
                }
            }
        }
    }
    let mut narrowed = 0;
    let (Some(widen), Some(widen_code)) = (opcodes::code_of("MOVSXd16d8"), opcodes::code_of("MOVSXd32d8")) else {
        return 0;
    };
    let Some(widen_desc) = instrinfo::desc(widen) else { return 0 };
    for b in 0..function.blocks.len() {
        let mut out = Vec::with_capacity(function.blocks[b].insts.len());
        for inst in std::mem::take(&mut function.blocks[b].insts) {
            let narrow_def = match (inst.kind, inst.operands.first()) {
                (Kind::Op(name), Some(&Operand::Def(Reg::Virt(x))))
                    if defs[x as usize] == 1
                        && !wide_use[x as usize]
                        && function.classes[x as usize] == Class::Dr32 =>
                {
                    Some((name, x))
                }
                _ => None,
            };
            let Some((name, x)) = narrow_def else {
                out.push(inst);
                continue;
            };
            if opcodes::code_of(name) == Some(widen_code) {
                // `ext.w; ext.l` of a register: `ext.w`
                out.push(op_inst("MOVSXd16d8", widen_desc, inst.operands.clone()));
            } else if let Some((plain, 8)) = crate::sextacc::plain_of(name)
                && let Some(plain_code) = opcodes::code_of(plain).or_else(|| local::code_of(plain))
                && let Some(plain_desc) = instrinfo::desc(plain_code).or_else(|| local::desc(plain_code))
            {
                // an extending load of a byte: the plain load, then `ext.w`
                let byte = function.classes.len() as u32;
                function.classes.push(Class::Dr8);
                let mut operands = inst.operands.clone();
                operands[0] = Operand::Def(Reg::Virt(byte));
                out.push(op_inst(plain, plain_desc, operands));
                out.push(op_inst(
                    "MOVSXd16d8",
                    widen_desc,
                    vec![Operand::Def(Reg::Virt(x)), Operand::Use(Reg::Virt(byte))],
                ));
            } else {
                out.push(inst);
                continue;
            }
            function.classes[x as usize] = Class::Dr16;
            narrowed += 1;
        }
        function.blocks[b].insts = out;
    }
    narrowed
}
