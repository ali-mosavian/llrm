//! `MemorySSA::verifyMemorySSA` and its helpers, from
//! [`MemorySSA.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1908).
//! LLVM asserts; here each check returns its message.

use llrm_mir::module::Function;

use super::{AccessId, Kind, Mssa};
use crate::cfg;

/// `MemorySSA::VerificationLevel`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Fast,
    Full,
}

impl Mssa {
    /// [`MemorySSA::verifyMemorySSA`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1908).
    pub fn verify_memory_ssa(
        &self,
        function: &Function,
        level: Level,
    ) -> Result<(), String> {
        self.verify_ordering_domination_and_def_uses(function, level)?;
        self.verify_domination_numbers(function)?;
        if level == Level::Full {
            self.verify_prev_def_in_phis(function)?;
        }
        Ok(())
    }

    /// [`MemorySSA::verifyPrevDefInPhis`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1945).
    fn verify_prev_def_in_phis(
        &self,
        function: &Function,
    ) -> Result<(), String> {
        for &block in function.layout() {
            let Some(phi) = self.get_memory_phi(block) else { continue };
            for &(pred, incoming) in &self.access(phi).incoming {
                // If the predecessor has no unreachable predecessors, get the
                // last def looking at the immediate dominators;
                // an unreachable one may hold any access.
                if !self.dominance.reachable(function, cfg::id(pred)) {
                    continue;
                }
                let mut node = Some(cfg::id(pred));
                while let Some(at) = node {
                    if let Some(defs) = self.get_block_defs(cfg::block(at)) {
                        let last = *defs.last().expect("a non-empty defs list");
                        if last != incoming {
                            return Err("Incorrect incoming access into phi.".to_owned());
                        }
                        break;
                    }
                    node = self.dominance.immediate(function, at);
                }
            }
        }
        Ok(())
    }

    /// [`MemorySSA::verifyDominationNumbers`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1982).
    fn verify_domination_numbers(
        &self,
        function: &Function,
    ) -> Result<(), String> {
        let valid = self.block_numbering_valid.borrow();
        if valid.is_empty() {
            return Ok(());
        }
        let numbering = self.block_numbering.borrow();
        for &block in function.layout() {
            if !valid.contains(&block) {
                continue;
            }
            // It's correct to say an empty block has valid numbering.
            let Some(list) = self.get_block_accesses(block) else { continue };
            // Block numbering starts at 1.
            let mut last = 0;
            for access in list {
                let Some(&this) = numbering.get(access) else {
                    return Err("MemoryAccess has no domination number in a valid block!".to_owned());
                };
                if this <= last {
                    return Err("Domination numbers should be strictly increasing!".to_owned());
                }
                last = this;
            }
        }
        Ok(())
    }

    /// [`MemorySSA::verifyOrderingDominationAndDefUses`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2024):
    /// the order and existence of accesses match those of the memory
    /// instructions; each definition dominates its uses; each use is in its
    /// definition's use list.
    fn verify_ordering_domination_and_def_uses(
        &self,
        function: &Function,
        level: Level,
    ) -> Result<(), String> {
        let mut actual_accesses: Vec<AccessId> = Vec::new();
        let mut actual_defs: Vec<AccessId> = Vec::new();
        for &block in function.layout() {
            let list = self.get_block_accesses(block);
            let defs = self.get_block_defs(block);
            if let Some(phi) = self.get_memory_phi(block) {
                actual_accesses.push(phi);
                actual_defs.push(phi);
                self.verify_dominates_uses(phi, "Memory PHI does not dominate it's uses")?;
                if level == Level::Full {
                    let preds = function.predecessors(block);
                    if self.access(phi).incoming.len() != preds.len() {
                        return Err("Incomplete MemoryPhi Node".to_owned());
                    }
                    for &(pred, value) in &self.access(phi).incoming {
                        self.verify_use_in_defs(Some(value), phi)?;
                        if !preds.iter().any(|one| *one == pred) {
                            return Err("Incoming phi block not a block predecessor".to_owned());
                        }
                    }
                }
            }
            for inst in function.iter_block(block) {
                let Some(access) = self.get_memory_access(inst) else { continue };
                if !(list.is_some() && (self.access(access).kind == Kind::Use || defs.is_some())) {
                    return Err(
                        "We have memory affecting instructions in this block but they are not in the access list or defs list"
                            .to_owned(),
                    );
                }
                actual_accesses.push(access);
                if self.access(access).kind == Kind::Def {
                    actual_defs.push(access);
                    self.verify_dominates_uses(access, "Memory Def does not dominate it's uses")?;
                }
                if level == Level::Full {
                    self.verify_use_in_defs(self.access(access).defining, access)?;
                }
            }
            // Either we hit the assert, really have no accesses, or we have
            // both accesses and an access list. Same with defs.
            if list.is_none() && defs.is_none() {
                continue;
            }
            let list = list.unwrap_or(&[]);
            if list.len() != actual_accesses.len() {
                return Err("We don't have the same number of accesses in the block as on the access list".to_owned());
            }
            if defs.is_none() && !actual_defs.is_empty() {
                return Err("Either we should have a defs list, or we should have no defs".to_owned());
            }
            if let Some(defs) = defs {
                if defs.len() != actual_defs.len() {
                    return Err("We don't have the same number of defs in the block as on the def list".to_owned());
                }
            }
            if list != actual_accesses.as_slice() {
                return Err("Not the same accesses in the same order".to_owned());
            }
            actual_accesses.clear();
            if let Some(defs) = defs {
                if defs != actual_defs.as_slice() {
                    return Err("Not the same defs in the same order".to_owned());
                }
            }
            actual_defs.clear();
        }
        Ok(())
    }

    /// Each use of `def` (a user, and for a phi the edge) is dominated by it.
    fn verify_dominates_uses(
        &self,
        def: AccessId,
        message: &str,
    ) -> Result<(), String> {
        for &user in
            self.access(def).users.iter().chain(&self.access(def).phi_users).chain(&self.access(def).optimized_users)
        {
            let user_access = self.access(user);
            if user_access.kind == Kind::Phi {
                for (at, &(_, value)) in user_access.incoming.iter().enumerate() {
                    if value == def && !self.dominates_use(def, user, Some(at)) {
                        return Err(message.to_owned());
                    }
                }
            } else if !self.dominates_use(def, user, None) {
                return Err(message.to_owned());
            }
        }
        Ok(())
    }

    /// [`MemorySSA::verifyUseInDefs`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L2116).
    fn verify_use_in_defs(
        &self,
        def: Option<AccessId>,
        user: AccessId,
    ) -> Result<(), String> {
        match def {
            // The live on entry use may cause us to get a null def here.
            None if self.is_live_on_entry_def(user) => Ok(()),
            None => Err("Null def but use not point to live on entry def".to_owned()),
            Some(def)
                if self.access(def).users.contains(&user)
                    || self.access(def).phi_users.contains(&user)
                    || self.access(def).optimized_users.contains(&user) =>
            {
                Ok(())
            }
            Some(_) => Err("Did not find use in def's use list".to_owned()),
        }
    }
}
