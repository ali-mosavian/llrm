//! `move.b %d0, (0,%a1,%a2)` with the loop counter in `%a1` and the stack
//! array's `lea` in `%a2` becomes `(0,%a2,%d1)`: the pointer is the base and
//! the integer the index.
//!
//! # How it works
//! [`address_roles`] runs after selection, before allocation. `SelectARII`
//! takes the base and the index in the order the DAG's `add` has them, and
//! only a wrapper counts as an address base, so a `lea` of a frame index can
//! end up as the index. The operand in the base slot is constrained to an
//! address register, so a counter there lives in `%a1` and its increment and
//! compare go through copies to a data register. When the index alone is
//! defined by an address-register instruction, the two swap, and the old
//! base, which no longer needs an address register, takes the class its other
//! uses allow.
//!
//! DEVIATE: the pin keeps the DAG's order. Reason: a second register and two
//! copies per loop (seed 210: +62 B with the pin's 0-based counter). Exit:
//! review 2026-11-20.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::mfunc::{Class, MFunction, operand_class};
use crate::scaledindex::memory_at;

pub fn address_roles(function: &mut MFunction) {
    let count = function.classes.len();
    // defined once, by an instruction whose result is an address register
    let mut defs = vec![0u32; count];
    let mut address = vec![false; count];
    for block in &function.blocks {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 2;
            }
        }
        for inst in &block.insts {
            for (position, operand) in inst.operands.iter().enumerate() {
                if let Operand::Def(Reg::Virt(v)) = operand {
                    defs[*v as usize] += 1;
                    address[*v as usize] = matches!(
                        inst.kind,
                        Kind::Op(name) if operand_class(name, position) == Some(Class::Ar32)
                    );
                }
            }
        }
    }
    let held = |v: u32| defs[v as usize] == 1 && address[v as usize];
    let mut freed: Vec<u32> = Vec::new();
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            let Kind::Op(name) = inst.kind else { continue };
            let Some((base, index)) = memory_at(name) else { continue };
            let (Operand::Use(Reg::Virt(b)), Operand::Use(Reg::Virt(i))) =
                (&inst.operands[base], &inst.operands[index])
            else {
                continue;
            };
            let (b, i) = (*b, *i);
            if b != i && !held(b) && held(i) {
                inst.operands.swap(base, index);
                freed.push(b);
            }
        }
    }
    freed.sort_unstable();
    freed.dedup();
    // the class each freed register's remaining operands allow
    let mut allowed: Vec<Option<Class>> = vec![Some(Class::Xr32); count];
    for block in &function.blocks {
        for inst in &block.insts {
            let Kind::Op(name) = inst.kind else { continue };
            for (position, operand) in inst.operands.iter().enumerate() {
                let (Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v))) = operand else { continue };
                if freed.binary_search(v).is_ok() {
                    let slot = &mut allowed[*v as usize];
                    *slot = slot.and_then(|class| class.meet(operand_class(name, position)?));
                }
            }
        }
    }
    for v in freed {
        if let Some(class) = allowed[v as usize] {
            function.classes[v as usize] = class;
        }
    }
}
