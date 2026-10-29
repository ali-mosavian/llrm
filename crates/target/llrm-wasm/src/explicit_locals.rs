//! `WebAssemblyExplicitLocals`: every register that is not on the value stack
//! becomes a local, read by `local.get` before its user and written by
//! `local.set` after its definer.
//!
//! DEVIATE: not translated are inline asm operands, `IMPLICIT_DEF`,
//! `FAKE_USE`, debug values and the virtual frame base. Reason: the slice has
//! none. Exit: with the frame lowering (W8).
//!
//! # See also
//! `WebAssemblyExplicitLocals.cpp`, pin d1106deb71cc: `explicitLocals` (L261),
//! `findStartOfTree` (L226).

use std::collections::HashMap;

use crate::mir::{Function, Inst, Operand, VReg, is_argument, is_copy, is_tee};

/// `findStartOfTree`: the first instruction of the stackified tree whose root
/// defines `reg`, as (block, index).
fn find_start_of_tree(
    f: &Function,
    reg: VReg,
) -> (usize, usize) {
    let (block, at) = f.vreg_def(reg).expect("a stackified register has a definition");
    let def = &f.blocks[block][at];
    if def.defs().any(|d| !f.is_stackified(d)) {
        return (block, at);
    }
    for operand in &def.operands {
        if let Operand::Use(used) = operand {
            return find_start_of_tree(f, *used);
        }
    }
    (block, at)
}

struct Locals {
    reg_to_local: HashMap<VReg, u32>,
    current: u32,
}

impl Locals {
    /// `getLocalId`.
    fn id(
        &mut self,
        reg: VReg,
    ) -> u32 {
        *self
            .reg_to_local
            .entry(reg)
            .or_insert_with(
                || {
                    self.current += 1;
                    self.current - 1
                },
            )
    }
}

/// Whether anything changed.
pub fn explicit_locals(f: &mut Function) -> bool {
    let mut changed = false;
    let mut locals = Locals { reg_to_local: HashMap::new(), current: 0 };

    // The arguments are the first locals; their pseudos go.
    while f.blocks[0].first().is_some_and(|i| is_argument(i.opcode)) {
        let arg = f.blocks[0].remove(0);
        let (Operand::Def(reg), Operand::Imm(local)) = (arg.operands[0], arg.operands[1]) else {
            panic!("an ARGUMENT defines a register and names a local")
        };
        locals.reg_to_local.insert(reg, local as u32);
        changed = true;
    }
    locals.current = (f.params.len() + f.locals.len()) as u32;

    let original = f.num_vregs();
    let use_empty: Vec<bool> = (0..original as VReg).map(|r| f.use_empty(r)).collect();
    for (reg, _) in use_empty.iter().enumerate().filter(|(_, empty)| **empty) {
        f.unstackify(reg as VReg);
    }

    for block in 0..f.blocks.len() {
        let mut i = 0;
        while i < f.blocks[block].len() {
            let mut at = i;
            if is_tee(f.blocks[block][at].opcode) {
                changed = true;
                i = tee(f, &mut locals, block, at);
                continue;
            }
            // Definitions: a store to a local after the instruction, or a drop.
            let mut after = 0;
            for k in 0..f.blocks[block][at].operands.len() {
                let Operand::Def(old) = f.blocks[block][at].operands[k] else { continue };
                if f.is_stackified(old) {
                    continue;
                }
                let ty = f.vreg_type(old);
                let new = f.new_vreg(ty);
                let post = if use_empty[old as usize] {
                    Inst::new(ty.drop(), &[Operand::Use(new)])
                } else {
                    Inst::new(ty.local_set(), &[Operand::Imm(locals.id(old).into()), Operand::Use(new)])
                };
                f.blocks[block].insert(at + 1, post);
                after += 1;
                f.blocks[block][at].operands[k] = Operand::Def(new);
                f.stackify(new);
                changed = true;
            }
            // Uses: a `local.get` before the start of each operand's tree, last
            // operand first.
            let mut insert_at = at;
            for k in (0..f.blocks[block][at].operands.len()).rev() {
                let Operand::Use(old) = f.blocks[block][at].operands[k] else { continue };
                if f.is_stackified(old) {
                    let (tree_block, start) = find_start_of_tree(f, old);
                    debug_assert_eq!(tree_block, block, "a stackified tree stays in its block");
                    insert_at = start;
                    continue;
                }
                let local = locals.id(old);
                let ty = f.vreg_type(old);
                let new = f.new_vreg(ty);
                f.blocks[block]
                    .insert(insert_at, Inst::new(ty.local_get(), &[Operand::Def(new), Operand::Imm(local.into())]));
                at += 1;
                f.blocks[block][at].operands[k] = Operand::Use(new);
                f.stackify(new);
                changed = true;
            }
            if is_copy(f.blocks[block][at].opcode) {
                let [Operand::Def(to), Operand::Use(from)] = f.blocks[block][at].operands[..] else {
                    panic!("a COPY defines one register from one")
                };
                f.replace_reg_with(from, to);
                f.blocks[block].remove(at);
                changed = true;
                i = at + after;
                continue;
            }
            i = at + 1 + after;
        }
    }

    let params = f.params.len() as u32;
    f.locals = vec![crate::mir::ValType::I32; (locals.current - params) as usize];
    let mut regs: Vec<_> = locals.reg_to_local.iter().filter(|(_, l)| **l >= params).collect();
    regs.sort();
    for (&reg, &local) in regs {
        f.locals[(local - params) as usize] = f.vreg_type(reg);
        changed = true;
    }
    changed
}

/// A `TEE`: `res` stays on the stack, `also` is stored in a local. Returns the
/// index of the instruction to continue from.
fn tee(
    f: &mut Function,
    locals: &mut Locals,
    block: usize,
    at: usize,
) -> usize {
    let [Operand::Def(res), Operand::Def(also), Operand::Use(src)] = f.blocks[block][at].operands[..] else {
        panic!("a TEE defines two registers from one")
    };
    assert!(f.is_stackified(res) && !f.is_stackified(also));
    let ty = f.vreg_type(src);
    let mut at = at;
    let mut src = src;
    if !f.is_stackified(src) {
        let local = locals.id(src);
        let new = f.new_vreg(ty);
        f.blocks[block].insert(at, Inst::new(ty.local_get(), &[Operand::Def(new), Operand::Imm(local.into())]));
        at += 1;
        f.stackify(new);
        src = new;
    }
    let local = locals.id(also);
    f.blocks[block][at] =
        Inst::new(ty.local_tee(), &[Operand::Def(res), Operand::Imm(local.into()), Operand::Use(src)]);
    at + 1
}
