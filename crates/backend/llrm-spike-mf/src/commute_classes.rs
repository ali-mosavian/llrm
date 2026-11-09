//! A commutable instruction whose two register sources are copies into
//! operand slots of different classes, commuted when the values would then
//! join their copies.
//!
//! `CMP v25, v23` with `v25 <- v9` (slot class A|X|Y) and `v23 <- v8` (slot
//! class MEM), where the rest of the function puts `v8` in the first class and
//! `v9` in the second, keeps both copies: each join has an empty class
//! meet. Reading the sources the other way round (`v25 <- v8`, `v23 <- v9`)
//! lets both copies join. The instruction is commutable (`Inst::commutable`,
//! `isCommutable`) and its slots are fixed by the copies' destinations'
//! classes, so the operands trade places together with those classes.
//!
//! A class a value gets is the meet of its web: the values its copies would
//! join (deepest loops first, a join that would leave no class is skipped).
//! The class webs ignore interference and the copies to physical registers, so
//! a target prices both orders on its emitted code before it keeps the swap.
//! The swap is made only where the order as written leaves a source outside its
//! slot's class and the swapped order leaves neither; an order that neither
//! fits stays as it is.
//!
//! DEVIATE: the pin commutes only to remove a tied-definition copy
//! (`RegisterCoalescer::removeCopyByCommutingDef`, `TwoAddressInstructionPass::
//! tryInstructionCommute`); this swap is driven by register classes. The webs
//! ignore interference, which the coalescer would also check. Exit: a target
//! that states the operand order in its .td patterns (6502), owner llrm-6502-b,
//! review 2026-12-10.
//!
//! # See also
//! `llvm/lib/CodeGen/RegisterCoalescer.cpp` `removeCopyByCommutingDef` (L823),
//! `llvm/lib/CodeGen/TargetInstrInfo.cpp` `findCommutedOpIndices` (L337) and
//! `commuteInstruction` (L291), pin d1106deb71cc.

use crate::cfg::Cfg;
use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::Target;

/// The webs the copies of `function` would join, deepest loop first, with the
/// class each has, leaving out the copies at `skip` (`(block, instruction)`).
struct Webs<C> {
    parent: Vec<u32>,
    class: Vec<C>,
}

impl<C: Copy> Webs<C> {
    fn find(
        &mut self,
        mut v: u32,
    ) -> u32 {
        while self.parent[v as usize] != v {
            self.parent[v as usize] = self.parent[self.parent[v as usize] as usize];
            v = self.parent[v as usize];
        }
        v
    }

    fn of<T: Target<Class = C>>(
        function: &MFunction<C>,
        depth: &[usize],
        skip: &[(usize, usize)],
    ) -> Webs<C> {
        let count = function.classes.len();
        let mut webs = Webs { parent: (0..count as u32).collect(), class: function.classes.clone() };
        // the coalescer joins what runs most first: the copies in the deepest
        // loops, then the rest in order
        let mut copies: Vec<(usize, usize, u32, u32)> = Vec::new();
        for (b, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                if let Some((dst, src)) = virtual_copy(inst).filter(|_| !skip.contains(&(b, i))) {
                    copies.push((b, i, dst, src));
                }
            }
        }
        copies.sort_by_key(|(b, ..)| std::cmp::Reverse(depth[*b]));
        for (_, _, dst, src) in copies {
            let (x, y) = (webs.find(dst), webs.find(src));
            if x == y {
                continue;
            }
            if let Some(met) = T::meet(webs.class[x as usize], webs.class[y as usize]) {
                webs.parent[y as usize] = x;
                webs.class[x as usize] = met;
            }
        }
        webs
    }

    /// Whether the web of `v` could be given `class`.
    fn fits<T: Target<Class = C>>(
        &mut self,
        v: u32,
        class: C,
    ) -> bool {
        let root = self.find(v);
        T::meet(self.class[root as usize], class).is_some()
    }
}

/// `(dst, src)` of a copy between two virtual registers.
fn virtual_copy(inst: &Inst) -> Option<(u32, u32)> {
    if inst.kind != Kind::Copy {
        return None;
    }
    match inst.operands.as_slice() {
        [Operand::Def(Reg::Virt(d)), Operand::Use(Reg::Virt(s))] => Some((*d, *s)),
        _ => None,
    }
}

/// The two virtual registers a commutable instruction reads, if it reads two.
fn sources(inst: &Inst) -> Option<(u32, u32)> {
    let mut read = inst.operands.iter().filter_map(|o| if let Operand::Use(r) = o { Some(*r) } else { None });
    match (read.next(), read.next(), read.next()) {
        (Some(Reg::Virt(a)), Some(Reg::Virt(b)), None) => Some((a, b)),
        _ => None,
    }
}

/// Where the single copy defining `v` is in `block`, before `before`.
fn defining_copy(
    block: &[Inst],
    before: usize,
    v: u32,
) -> Option<usize> {
    let at = (0..before)
        .rev()
        .find(|&i| block[i].operands.iter().any(|o| matches!(o, Operand::Def(Reg::Virt(d)) if *d == v)))?;
    virtual_copy(&block[at]).filter(|(d, _)| *d == v).map(|_| at)
}

/// How many operands read each virtual register.
fn use_counts<C>(function: &MFunction<C>) -> Vec<u32> {
    let mut counts = vec![0; function.classes.len()];
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for operand in &inst.operands {
            if let Operand::Use(Reg::Virt(v)) = operand {
                counts[*v as usize] += 1;
            }
        }
    }
    counts
}

/// Commutes each commutable instruction whose copies cannot join in the order
/// written and can in the other: its two register operands, each the only use
/// of a copy's destination, trade places with the classes of those
/// destinations, so the instruction reads the same values through the slot
/// each now fits. Returns how many it swapped.
pub fn commute_for_classes<T: Target>(function: &mut MFunction<T::Class>) -> usize {
    let counts = use_counts(function);
    let cfg = Cfg::new(function);
    let depth: Vec<usize> =
        (0..function.blocks.len()).map(|b| cfg.loops().iter().filter(|(_, body)| body[b]).count()).collect();
    let mut swapped = 0;
    for b in 0..function.blocks.len() {
        for i in 0..function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[i];
            if !inst.commutable {
                continue;
            }
            let Some((a, c)) = sources(inst) else { continue };
            if a == c || counts[a as usize] != 1 || counts[c as usize] != 1 {
                continue;
            }
            let block = &function.blocks[b].insts;
            let (Some(first), Some(second)) = (defining_copy(block, i, a), defining_copy(block, i, c)) else {
                continue;
            };
            let (Some((_, s1)), Some((_, s2))) = (virtual_copy(&block[first]), virtual_copy(&block[second])) else {
                continue;
            };
            let (slot_a, slot_c) = (function.classes[a as usize], function.classes[c as usize]);
            let mut webs = Webs::of::<T>(function, &depth, &[(b, first), (b, second)]);
            let as_written = webs.fits::<T>(s1, slot_a) && webs.fits::<T>(s2, slot_c);
            let swapped_fits = webs.fits::<T>(s2, slot_a) && webs.fits::<T>(s1, slot_c);
            if as_written || !swapped_fits {
                continue;
            }
            // the copy into `a` keeps its source and now fills the second slot
            function.classes.swap(a as usize, c as usize);
            let mut slots = function.blocks[b].insts[i].operands.iter_mut().filter(|o| matches!(o, Operand::Use(_)));
            if let (Some(x), Some(y)) = (slots.next(), slots.next()) {
                std::mem::swap(x, y);
            }
            swapped += 1;
        }
    }
    swapped
}
