//! Early `MachineLICM` for arm64: what a loop recomputes from values it does
//! not change (a global's page, a constant) is made once before it.
//!
//! # How it works
//! The walk is the shared [`licm_core`] one, over the allocator's
//! [`MFunction`]. The policy is the pin's `IsProfitableToHoist` for a
//! rematerializable instruction (`MOVaddr`, `MOVi32imm`): it always moves, the
//! allocator makes it again where a register is short. Nothing that reads
//! memory or the flags, has an effect, or touches a physical register moves.
//!
//! # Example
//! ```text
//!   pre:  b loop                          pre:  adrp x9, _g@PAGE ; b loop
//!   loop: adrp x9, _g@PAGE          =>    loop: ldr w0, [x9, _g@PAGEOFF]
//!         ldr w0, [x9, _g@PAGEOFF]
//! ```
//!
//! # See also
//! `llrm_codegen::backend::licm_core` (the walk), `lir_view` (runs it before
//! allocation).

use llrm_codegen::backend::licm_core::{self, Block, Facts, Inst as CoreInst, Policy};
use llrm_spike_mf::mfunc::{Inst, Kind, MFunction, Operand, Reg};

#[derive(Clone)]
struct Wrapped {
    inst: Inst,
    defs: Vec<u32>,
    uses: Vec<u32>,
    /// It names a physical register: the allocator's input, not a value.
    physical: bool,
}

impl Wrapped {
    fn new(inst: Inst) -> Wrapped {
        let (mut defs, mut uses, mut physical) = (Vec::new(), Vec::new(), false);
        for operand in &inst.operands {
            match operand {
                Operand::Def(Reg::Virt(n)) => defs.push(*n),
                Operand::Use(Reg::Virt(n)) => uses.push(*n),
                Operand::Def(Reg::Phys(_)) | Operand::Use(Reg::Phys(_)) => physical = true,
                _ => {}
            }
        }
        Wrapped { inst, defs, uses, physical }
    }
}

impl CoreInst for Wrapped {
    fn defines(&self) -> &[u32] {
        &self.defs
    }

    fn uses(&self) -> &[u32] {
        &self.uses
    }

    fn is_copy(&self) -> bool {
        self.inst.kind == Kind::Copy
    }

    fn is_move(&self) -> bool {
        self.inst.kind == Kind::Copy || self.inst.cheap_as_move
    }

    fn is_meta(&self) -> bool {
        false
    }

    fn is_jump(&self) -> bool {
        self.inst.terminator
    }

    fn identical(
        &self,
        other: &Wrapped,
    ) -> bool {
        let blank = |inst: &Inst| -> Vec<Operand> {
            inst.operands
                .iter()
                .map(|operand| match operand {
                    Operand::Def(_) => Operand::Imm(0),
                    other => other.clone(),
                })
                .collect()
        };
        self.inst.kind == other.inst.kind && blank(&self.inst) == blank(&other.inst)
    }

    fn rename_use(
        &mut self,
        from: u32,
        to: u32,
    ) {
        for operand in self.inst.operands.iter_mut() {
            if *operand == Operand::Use(Reg::Virt(from)) {
                *operand = Operand::Use(Reg::Virt(to));
            }
        }
        self.uses.iter_mut().filter(|value| **value == from).for_each(|value| *value = to);
    }
}

struct Arm64;

impl Policy<Wrapped> for Arm64 {
    fn begin(
        &mut self,
        _facts: &Facts<'_, Wrapped>,
    ) {
    }

    fn movable(
        &mut self,
        facts: &Facts<'_, Wrapped>,
        at: i64,
        insns: &[Wrapped],
        index: usize,
    ) -> bool {
        let _ = at;
        let one = &insns[index];
        let inst = &one.inst;
        inst.rematerializable
            // `CheapInstr && CreatesCopy`: a constant a phi takes would need a
            // copy.
            && !(inst.cheap_as_move && one.defs.iter().any(|value| facts.copied.contains(value)))
            && !one.physical
            && !one.defs.is_empty()
            && one.defs.iter().all(|value| facts.defs.get(value) == Some(&1))
            && inst.ties.is_empty()
            && !(inst.call
                || inst.terminator
                || inst.barrier
                || inst.may_load
                || inst.may_store
                || inst.side_effects
                || inst.sets_flags
                || inst.reads_flags
                || inst.early_clobber)
    }
}

/// The blocks a block can go to: its branch targets and, unless it ends in a
/// jump nothing falls out of, the next block.
fn successors<C>(
    function: &MFunction<C>,
    at: usize,
) -> Vec<i64> {
    let block = &function.blocks[at];
    let mut succ: Vec<i64> = block
        .insts
        .iter()
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|operand| match operand {
            Operand::Block(to) => Some(*to as i64),
            _ => None,
        })
        .collect();
    let falls = !block.insts.last().is_some_and(|last| last.barrier);
    if falls && let Some(next) = function.blocks.get(at + 1) {
        succ.push(next.id as i64);
    }
    succ.sort_unstable();
    succ.dedup();
    succ
}

/// Moves what the loops of `function` recompute to their preheaders; whether
/// anything moved.
pub fn hoist<C>(function: &mut MFunction<C>) -> bool {
    let Some(first) = function.blocks.first() else { return false };
    let blocks: Vec<Block<Wrapped>> = (0..function.blocks.len())
        .map(|at| Block {
            at: function.blocks[at].id as i64,
            succ: successors(function, at),
            phis: Vec::new(),
            insns: function.blocks[at].insts.iter().cloned().map(Wrapped::new).collect(),
        })
        .collect();
    let Some(moved) = licm_core::hoist(&blocks, first.id as i64, &mut Arm64) else { return false };
    for (block, new) in function.blocks.iter_mut().zip(moved) {
        block.insts = new.insns.into_iter().map(|one| one.inst).collect();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(
        dst: u32,
        name: &str,
    ) -> Inst {
        let mut inst = Inst::new(
            Kind::Op("Adr"),
            vec![Operand::Def(Reg::Virt(dst)), Operand::Symbol(name.into(), 0), Operand::Imm(7)],
        );
        inst.rematerializable = true;
        inst
    }

    fn jump(to: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("Branch"), vec![Operand::Block(to)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }

    fn block(
        id: u32,
        insts: Vec<Inst>,
    ) -> llrm_spike_mf::mfunc::Block {
        llrm_spike_mf::mfunc::Block { id, insts, phis: Vec::new() }
    }

    /// Two arms of a loop each made the page of the same global (2 `adrp`
    /// where the pin makes 1, `EliminateCSE`): the page is made once in the
    /// preheader and the arms read it.
    #[test]
    fn equal_pages_made_in_a_loops_arms_are_made_once_in_the_preheader() {
        let mut cond = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(3)]);
        cond.terminator = true;
        let user = |reg: u32| Inst::new(Kind::Op("Store"), vec![Operand::Use(Reg::Virt(reg))]);
        let mut function = MFunction::<()>::new("f");
        function.blocks = vec![
            // preheader, falls into 1
            block(0, vec![]),
            // header: to 3 or falls to 2
            block(1, vec![cond]),
            // arm
            block(2, vec![page(10, "g"), user(10), jump(4)]),
            // arm, falls to 4
            block(3, vec![page(11, "g"), user(11)]),
            // latch
            block(4, vec![Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(1)])]),
            // exit
            block(5, vec![]),
        ];
        assert!(hoist(&mut function));
        let pages = |at: usize| function.blocks[at].insts.iter().filter(|inst| inst.rematerializable).count();
        assert_eq!((pages(0), pages(2), pages(3)), (1, 0, 0), "{function:?}");
        let made = function.blocks[0].insts[0].operands[0].clone();
        let Operand::Def(made) = made else { panic!("not a def") };
        for arm in [2, 3] {
            assert!(function.blocks[arm].insts.iter().any(|inst| inst.operands == vec![Operand::Use(made)]), "{arm}");
        }
    }
}
