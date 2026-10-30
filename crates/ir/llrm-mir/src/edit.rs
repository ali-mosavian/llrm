//! Changing a function, as LLVM's API does. Every change keeps the use
//! lists true and goes into the change log; ids are never reused.

use std::collections::BTreeSet;

use crate::context::{Constant, ConstantKind, Context};
use crate::module::{
    Block, BlockId, Change, DebugRecord, DebugWhat, Function, InstId, Instruction, MetadataId, NameIndex, Names,
    Operand, Use, ValueData, ValueDef, ValueId,
};
use crate::opcode::{Flags, Opcode};
use crate::types::{Type, TypeId};
use crate::uselist::Held;

/// Where an instruction goes: before or after another, or at a block's end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Before(InstId),
    /// Right after another: `Before` what follows it, or the end of its block.
    After(InstId),
    End(BlockId),
}

// Names built into the index or probed in it, for a test that making a name
// does not look at every name of the function.
#[cfg(test)]
thread_local! {
    pub(crate) static NAMES_LOOKED_AT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Function {
    /// Parents and use lists from the arenas, after reading.
    pub(crate) fn index(&mut self) {
        self.uses.reset(self.values.len(), self.blocks.len());
        self.parent = vec![None; self.instructions.len()];
        self.erased = vec![false; self.instructions.len()];
        for &block in &self.layout.clone() {
            for inst in self.iter_block(block).collect::<Vec<_>>() {
                self.parent[inst.0 as usize] = Some(block);
            }
        }
        for inst in 0..self.instructions.len() {
            let operands = self.instructions[inst].operands.clone();
            let phi = self.instructions[inst].opcode == Opcode::Phi;
            self.uses.slots(inst as u32, operands.len());
            for (index, operand) in operands.into_iter().enumerate() {
                self.add_use(operand, Use { user: InstId(inst as u32), index: index as u32 }, phi);
            }
        }
    }

    fn add_use(
        &mut self,
        operand: Operand,
        at: Use,
        phi: bool,
    ) {
        if let Some(held) = Held::of(operand, phi) {
            self.uses.link(held, at);
        }
    }

    fn remove_use(
        &mut self,
        operand: Operand,
        at: Use,
        phi: bool,
    ) {
        if let Some(held) = Held::of(operand, phi) {
            self.uses.unlink(held, at);
        }
    }

    /// Where the kept use lists disagree with the operands: an instrument,
    /// empty while every change went through this API.
    pub fn check_uses(&self) -> Vec<String> {
        let mut values = vec![BTreeSet::new(); self.values.len()];
        let mut blocks = vec![BTreeSet::new(); self.blocks.len()];
        let mut phi_blocks = vec![BTreeSet::new(); self.blocks.len()];
        for (at, instruction) in self.instructions.iter().enumerate() {
            if self.erased[at] {
                continue;
            }
            for (index, operand) in instruction.operands.iter().enumerate() {
                let one = Use { user: InstId(at as u32), index: index as u32 };
                match operand {
                    Operand::Value(value) => values[value.0 as usize].insert(one),
                    Operand::Block(block) if instruction.opcode == Opcode::Phi => {
                        phi_blocks[block.0 as usize].insert(one)
                    }
                    Operand::Block(block) => blocks[block.0 as usize].insert(one),
                    Operand::Constant(_) => false,
                };
            }
        }
        let mut problems = Vec::new();
        for (at, expected) in values.iter().enumerate() {
            let list = self.uses.users(Held::Value(at as u32)).iter().copied().collect::<Vec<_>>();
            let kept: BTreeSet<Use> = list.iter().copied().collect();
            if kept != *expected || kept.len() != list.len() {
                problems.push(format!("value {at}: kept {list:?}, operands say {expected:?}"));
            }
        }
        for (at, expected) in blocks.iter().enumerate() {
            let list = self.uses.users(Held::Block(at as u32)).iter().copied().collect::<Vec<_>>();
            let kept: BTreeSet<Use> = list.iter().copied().collect();
            if kept != *expected || kept.len() != list.len() {
                problems.push(format!("block {at}: kept {list:?}, operands say {expected:?}"));
            }
        }
        for (at, expected) in phi_blocks.iter().enumerate() {
            let list = self.uses.users(Held::PhiBlock(at as u32)).iter().copied().collect::<Vec<_>>();
            let kept: BTreeSet<Use> = list.iter().copied().collect();
            if kept != *expected || kept.len() != list.len() {
                problems.push(format!("phis of block {at}: kept {list:?}, operands say {expected:?}"));
            }
        }
        problems
    }

    /// `name`, or `name` with the least number appended that no live value
    /// or block of the function holds, as LLVM's symbol table makes names.
    /// The name is taken from then on.
    fn unique_name(
        &mut self,
        name: &str,
    ) -> String {
        let Names { held, taken_below } = self
            .names
            .0
            .get_or_insert_with(
                || {
                    let mut held: crate::hash::HashMap<String, u32> = Default::default();
                    let live = self.blocks.iter().filter(|one| !one.erased).filter_map(|one| one.name.as_deref());
                    for held_name in self.values.iter().filter_map(|one| one.name.as_deref()).chain(live) {
                        #[cfg(test)]
                        NAMES_LOOKED_AT.with(|one| one.set(one.get() + 1));
                        *held.entry(held_name.to_owned()).or_default() += 1;
                    }
                    Names { held, taken_below: Default::default() }
                },
            );
        let unique = if !held.contains_key(name) {
            name.to_owned()
        } else {
            // `name1` .. below the remembered number were taken when it was
            // made and no name has been released since: a name taken N times
            // is not searched N times for each new one.
            let from = taken_below.get(name).copied().unwrap_or(1);
            let free = (from..)
                .find(|n| {
                    #[cfg(test)]
                    NAMES_LOOKED_AT.with(|one| one.set(one.get() + 1));
                    !held.contains_key(&format!("{name}{n}"))
                })
                .expect("some number is free");
            taken_below.insert(name.to_owned(), free + 1);
            format!("{name}{free}")
        };
        *held.entry(unique.clone()).or_default() += 1;
        unique
    }

    /// A block's name is free again once the block is erased.
    fn release_name(
        &mut self,
        block: BlockId,
    ) {
        let (Some(names), Some(name)) = (self.names.0.as_mut(), self.blocks[block.0 as usize].name.as_deref()) else {
            return;
        };
        if let Some(count) = names.held.get_mut(name) {
            *count -= 1;
            if *count == 0 {
                names.held.remove(name);
                names.taken_below.clear();
            }
        }
    }

    /// Room for `count` more instructions and their results, for a builder that
    /// knows how many it will make.
    pub fn reserve_instructions(
        &mut self,
        count: usize,
    ) {
        self.instructions.reserve(count);
        self.parent.reserve(count);
        self.erased.reserve(count);
        self.values.reserve(count);
        // Each instruction is created and placed: two edits logged.
        self.changes.0.reserve((count * 2).min(1 << 17));
    }

    /// A new instruction, placed nowhere yet; its result, if its type is not
    /// `void`, is named `name` or numbered.
    pub fn create_instruction(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
        name: Option<&str>,
    ) -> InstId {
        let id = InstId(self.instructions.len() as u32);
        let result = (ty != self.void).then(|| {
            let value = ValueId(self.values.len() as u32);
            let name = name.map(|one| self.unique_name(one));
            self.values.push(ValueData { ty, name, def: ValueDef::Instruction(id) });
            self.uses.push_value();
            value
        });
        self.uses.slots(id.0, operands.len());
        for (index, &operand) in operands.iter().enumerate() {
            self.add_use(operand, Use { user: id, index: index as u32 }, opcode == Opcode::Phi);
        }
        self.instructions.push(Instruction { opcode, ty, operands, flags, result, metadata: Vec::new(), link: (0, 0) });
        self.parent.push(None);
        self.erased.push(false);
        id
    }

    /// The instruction after `inst` in its list.
    fn next_raw(
        &self,
        inst: InstId,
    ) -> Option<InstId> {
        match self.instructions[inst.0 as usize].link.1 {
            0 => None,
            n => Some(InstId(n - 1)),
        }
    }

    /// Links `inst` last in `block`'s list.
    pub(crate) fn link_last(
        &mut self,
        block: BlockId,
        inst: InstId,
    ) {
        let b = &mut self.blocks[block.0 as usize];
        let id = inst.0 + 1;
        self.instructions[inst.0 as usize].link = (b.tail, 0);
        if b.tail != 0 {
            self.instructions[(b.tail - 1) as usize].link.1 = id;
        } else {
            b.head = id;
        }
        b.tail = id;
        b.len += 1;
    }

    /// Links `inst` before `next` in `block`'s list.
    fn link_before(
        &mut self,
        block: BlockId,
        inst: InstId,
        next: InstId,
    ) {
        let id = inst.0 + 1;
        let prev = self.instructions[next.0 as usize].link.0;
        self.instructions[inst.0 as usize].link = (prev, next.0 + 1);
        self.instructions[next.0 as usize].link.0 = id;
        let b = &mut self.blocks[block.0 as usize];
        if prev != 0 {
            self.instructions[(prev - 1) as usize].link.1 = id;
        } else {
            b.head = id;
        }
        b.len += 1;
    }

    /// Takes `inst` out of `block`'s list.
    fn unlink(
        &mut self,
        block: BlockId,
        inst: InstId,
    ) {
        let (prev, next) = self.instructions[inst.0 as usize].link;
        let b = &mut self.blocks[block.0 as usize];
        if prev != 0 {
            self.instructions[(prev - 1) as usize].link.1 = next;
        } else {
            b.head = next;
        }
        if next != 0 {
            self.instructions[(next - 1) as usize].link.0 = prev;
        } else {
            b.tail = prev;
        }
        self.instructions[inst.0 as usize].link = (0, 0);
        b.len -= 1;
    }

    /// Links `inst` at `position`; the block and what follows it.
    fn link_at(
        &mut self,
        inst: InstId,
        position: Position,
    ) -> Result<(BlockId, Option<InstId>), String> {
        let (block, next) = match position {
            Position::End(block) => (block, None),
            Position::Before(next) => {
                (self.parent(next).ok_or_else(|| format!("instruction {} is not placed", next.0))?, Some(next))
            }
            Position::After(prev) => {
                let block = self.parent(prev).ok_or_else(|| format!("instruction {} is not placed", prev.0))?;
                (block, self.next_raw(prev))
            }
        };
        if self.blocks[block.0 as usize].erased {
            return Err(format!("block {} is erased", block.0));
        }
        match next {
            Some(next) => self.link_before(block, inst, next),
            None => self.link_last(block, inst),
        }
        Ok((block, next))
    }

    /// What stood before `from` now stands before `to`, ahead of what stood
    /// there: it was said earlier in the program, and where two records of
    /// a variable stand together the last one is what the variable is.
    fn reanchor_records(
        &mut self,
        from: InstId,
        to: InstId,
    ) {
        let (moved, mut kept): (Vec<DebugRecord>, Vec<DebugRecord>) =
            std::mem::take(&mut self.debug_records).into_iter().partition(|one| one.before == from);
        let at = kept.iter().position(|one| one.before == to).unwrap_or(kept.len());
        kept.splice(at..at, moved.into_iter().map(|one| DebugRecord { before: to, ..one }));
        self.debug_records = kept;
    }

    /// What stood before `from` now stands before `to`, ahead of what stood
    /// there (`BasicBlock::flushTerminatorDbgRecords`): a pass that replaces a
    /// terminator keeps what was said before the old one.
    pub fn move_records_before(
        &mut self,
        from: InstId,
        to: InstId,
    ) {
        self.reanchor_records(from, to);
    }

    /// The first instruction after the phis of the block that the unconditional
    /// branch `inst` alone enters.
    fn sole_successor_start(
        &self,
        inst: InstId,
    ) -> Option<InstId> {
        if self.instruction(inst).opcode != Opcode::Br {
            return None;
        }
        let [Operand::Block(target)] = self.instruction(inst).operands[..] else { return None };
        // A branch to its own block enters no other: its records have no start
        // to go to but itself, which is going away.
        if self.parent(inst) == Some(target) {
            return None;
        }
        // Two entries decide it: a list of N was walked whole for each erase.
        // (An erase has already taken its own use of the block away.)
        let mut entries = self.block_users(target).iter().map(|one| one.user);
        match (entries.next(), entries.next()) {
            (None, _) => {}
            (Some(only), None) if only == inst => {}
            _ => return None,
        }
        self.iter_block(target).find(|&one| self.instruction(one).opcode != Opcode::Phi)
    }

    /// Takes `inst` out of its block. What was said before it is said before
    /// what followed it; with none after it, it goes with the instruction
    /// where that is moved (`moving`: a block moved whole takes its last
    /// instruction along), else with the code, when it is erased.
    fn detach(
        &mut self,
        inst: InstId,
        moving: bool,
    ) -> Option<(BlockId, Option<InstId>)> {
        let block = self.parent(inst)?;
        let next = self.next_raw(inst);
        self.settle_records(inst, next, moving);
        self.unlink(block, inst);
        self.order.invalidate(block);
        self.parent[inst.0 as usize] = None;
        Some((block, next))
    }

    /// Where the records said before `inst` go when it leaves its block, `next`
    /// being what follows it.
    fn settle_records(
        &mut self,
        inst: InstId,
        next: Option<InstId>,
        moving: bool,
    ) {
        // What stood before it stays where the source says it was: before what
        // followed it. A last instruction has none to hand them to (a
        // block is erased with its terminator): they go with it.
        match next {
            Some(next) => self.reanchor_records(inst, next),
            // An unconditional branch to a block nothing else enters is the end
            // of one block and the start of the other: what was said before it
            // is said before what the other block starts with, once the two are
            // one.
            None if moving => {}
            None if self.sole_successor_start(inst).is_some() => {
                let start = self.sole_successor_start(inst).expect("checked");
                self.reanchor_records(inst, start);
            }
            None => {
                let (gone, kept): (Vec<DebugRecord>, Vec<DebugRecord>) =
                    std::mem::take(&mut self.debug_records).into_iter().partition(|one| one.before == inst);
                self.debug_records = kept;
                for one in gone {
                    if !self.debug_dropped.contains(&one.variable) {
                        self.debug_dropped.push(one.variable);
                    }
                }
            }
        }
    }

    fn attach(
        &mut self,
        inst: InstId,
        position: Position,
    ) -> Result<(BlockId, Option<InstId>), String> {
        let (block, next) = self.link_at(inst, position)?;
        self.parent[inst.0 as usize] = Some(block);
        self.order.invalidate(block);
        Ok((block, next))
    }

    /// Places an instruction placed nowhere yet.
    pub fn insert(
        &mut self,
        inst: InstId,
        position: Position,
    ) -> Result<(), String> {
        if self.is_erased(inst) || self.parent(inst).is_some() {
            return Err(format!("instruction {} is already placed or erased", inst.0));
        }
        let (block, next) = self.attach(inst, position)?;
        self.log(Change::Inserted { inst, block, next });
        Ok(())
    }

    /// Moves a placed instruction.
    pub fn move_to(
        &mut self,
        inst: InstId,
        position: Position,
    ) -> Result<(), String> {
        if position == Position::Before(inst) {
            return Ok(());
        }
        let (from, _) = self.detach(inst, true).ok_or_else(|| format!("instruction {} is not placed", inst.0))?;
        let (block, next) = self.attach(inst, position)?;
        self.log(Change::Moved { inst, block, next, from });
        Ok(())
    }

    /// Moves the placed instructions `run`, in order, to the end of `block`,
    /// and what the records say stands where it did: a record before an
    /// instruction of the run moves with it. (Moving them one by one with
    /// `move_to` would leave each record behind, before the next one, and carry
    /// it on to the last: a block's records would end up after its code.)
    pub fn move_run(
        &mut self,
        run: &[InstId],
        block: BlockId,
    ) -> Result<(), String> {
        for &inst in run {
            let block_from = self.parent(inst).ok_or_else(|| format!("instruction {} is not placed", inst.0))?;
            self.unlink(block_from, inst);
            self.order.invalidate(block_from);
            self.parent[inst.0 as usize] = None;
            let (to, next) = self.attach(inst, Position::End(block))?;
            self.log(Change::Moved { inst, block: to, next, from: block_from });
        }
        Ok(())
    }

    /// `BasicBlock::splice`: the placed instructions `run`, in order, moved to
    /// `position` (not inside the run), as moving them one by one before it
    /// would. Each block is rewritten once, not once per instruction: O(the
    /// blocks it touches), where `move_to` for each is O(run x block).
    pub fn splice(
        &mut self,
        run: &[InstId],
        position: Position,
    ) -> Result<(), String> {
        let mut moving = crate::dense::IdSet::<InstId>::new();
        for &inst in run {
            if self.parent(inst).is_none() {
                return Err(format!("instruction {} is not placed", inst.0));
            }
            if !moving.insert(inst) {
                return Err(format!("instruction {} is twice in the run", inst.0));
            }
        }
        let anchor = match position {
            Position::Before(one) | Position::After(one) => Some(one),
            Position::End(_) => None,
        };
        if anchor.is_some_and(|one| moving.contains(&one)) {
            return Err("the position is inside the run".to_owned());
        }
        let froms: Vec<BlockId> = run.iter().map(|&inst| self.parent(inst).expect("placed")).collect();
        let mut blocks = froms.clone();
        blocks.sort_unstable();
        blocks.dedup();
        for &inst in run {
            let from = self.parent(inst).expect("placed");
            self.unlink(from, inst);
        }
        for &block in &blocks {
            self.order.invalidate(block);
        }
        for &inst in run {
            self.parent[inst.0 as usize] = None;
        }
        // The place is found with the run out of the way.
        let (to, next) = match position {
            Position::End(block) => (block, None),
            Position::Before(one) => {
                (self.parent(one).ok_or_else(|| format!("instruction {} is not placed", one.0))?, Some(one))
            }
            Position::After(one) => {
                let block = self.parent(one).ok_or_else(|| format!("instruction {} is not placed", one.0))?;
                (block, self.next_raw(one))
            }
        };
        if self.blocks[to.0 as usize].erased {
            return Err(format!("block {} is erased", to.0));
        }
        for &inst in run {
            match next {
                Some(next) => self.link_before(to, inst, next),
                None => self.link_last(to, inst),
            }
        }
        self.order.invalidate(to);
        for (&inst, &from) in run.iter().zip(&froms) {
            self.parent[inst.0 as usize] = Some(to);
            self.log(Change::Moved { inst, block: to, next, from });
        }
        Ok(())
    }

    pub fn set_operand(
        &mut self,
        inst: InstId,
        index: usize,
        operand: Operand,
    ) {
        let at = Use { user: inst, index: index as u32 };
        let old = std::mem::replace(&mut self.instructions[inst.0 as usize].operands[index], operand);
        let phi = self.instructions[at.user.0 as usize].opcode == Opcode::Phi;
        self.remove_use(old, at, phi);
        self.add_use(operand, at, phi);
        self.log(Change::Operand { inst, index: index as u32, was: old });
        self.log(Change::Rewritten(inst));
    }

    /// Replaces all of an instruction's operands, as LLVM's
    /// `removeIncomingValue` and `addIncoming` change a phi's.
    pub fn set_operands(
        &mut self,
        inst: InstId,
        operands: Vec<Operand>,
    ) {
        let phi = self.instructions[inst.0 as usize].opcode == Opcode::Phi;
        let old = std::mem::take(&mut self.instructions[inst.0 as usize].operands);
        let was: Box<[Operand]> = old.as_slice().into();
        // The operands before the first that differs keep their uses, as
        // `removeIncomingValue` moves only what follows the entry it removes.
        let kept = if self.uses.fits(inst.0, operands.len()) {
            old.iter().zip(&operands).take_while(|(before, after)| before == after).count()
        } else {
            0
        };
        for (index, operand) in old.into_iter().enumerate().skip(kept) {
            self.remove_use(operand, Use { user: inst, index: index as u32 }, phi);
        }
        self.uses.slots(inst.0, operands.len());
        for (index, &operand) in operands.iter().enumerate().skip(kept) {
            self.add_use(operand, Use { user: inst, index: index as u32 }, phi);
        }
        self.instructions[inst.0 as usize].operands = operands;
        self.log(Change::Operands { inst, was });
        self.log(Change::Rewritten(inst));
    }

    /// Makes the store `inst` not volatile.
    pub fn make_store_plain(
        &mut self,
        inst: InstId,
    ) {
        if let Opcode::Store { volatile, .. } = &mut self.instructions[inst.0 as usize].opcode
            && *volatile
        {
            *volatile = false;
            self.log(Change::Rewritten(inst));
        }
    }

    /// `AllocaInst::setAlignment`.
    pub fn set_alloca_align(
        &mut self,
        inst: InstId,
        align: u64,
    ) {
        let Opcode::Alloca { align: held, .. } = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("an alloca")
        };
        if *held != Some(align) {
            *held = Some(align);
            self.log(Change::Rewritten(inst));
        }
    }

    /// Makes the store `inst` volatile.
    pub fn make_store_volatile(
        &mut self,
        inst: InstId,
    ) {
        if let Opcode::Store { volatile, .. } = &mut self.instructions[inst.0 as usize].opcode
            && !*volatile
        {
            *volatile = true;
            self.log(Change::Rewritten(inst));
        }
    }

    /// Gives the `icmp` `inst` the predicate `predicate`, as LLVM's
    /// `CmpInst::setPredicate`.
    pub fn set_icmp_predicate(
        &mut self,
        inst: InstId,
        predicate: crate::opcode::IntPredicate,
    ) {
        let Opcode::ICmp(held) = &mut self.instructions[inst.0 as usize].opcode else { panic!("an icmp") };
        *held = predicate;
        self.log(Change::Rewritten(inst));
    }

    /// `FCmpInst::setPredicate`.
    pub fn set_fcmp_predicate(
        &mut self,
        inst: InstId,
        predicate: crate::opcode::FloatPredicate,
    ) {
        let Opcode::FCmp(held) = &mut self.instructions[inst.0 as usize].opcode else { panic!("an fcmp") };
        *held = predicate;
        self.log(Change::Rewritten(inst));
    }

    /// `Value::setName`: `value` is named `name`, or `name` with the least
    /// number appended that nothing live holds.
    pub fn set_name(
        &mut self,
        value: ValueId,
        name: &str,
    ) {
        let name = self.unique_name(name);
        self.values[value.0 as usize].name = Some(name);
    }

    /// `Value::takeName`: `to` is named what `from` was, which goes unnamed;
    /// the name stays taken in the function.
    pub fn take_name(
        &mut self,
        from: ValueId,
        to: ValueId,
    ) {
        let name = self.values[from.0 as usize].name.take();
        self.values[to.0 as usize].name = name;
    }

    /// `BasicBlock::takeName`: `to` is named what `from` was, which goes
    /// unnamed; the name stays taken in the function.
    pub fn take_block_name(
        &mut self,
        from: BlockId,
        to: BlockId,
    ) {
        let name = self.blocks[from.0 as usize].name.take();
        self.blocks[to.0 as usize].name = name;
    }

    /// `BasicBlock::setName`: `block` is named `name`, or `name` with the least
    /// number appended that nothing live holds.
    pub fn set_block_name(
        &mut self,
        block: BlockId,
        name: &str,
    ) {
        self.release_name(block);
        let name = self.unique_name(name);
        self.blocks[block.0 as usize].name = Some(name);
    }

    /// Replaces an instruction's opcode in place (a `getelementptr`'s source
    /// type, say); operands and users stay.
    pub fn set_opcode(
        &mut self,
        inst: InstId,
        opcode: Opcode,
    ) {
        self.instructions[inst.0 as usize].opcode = opcode;
        self.log(Change::Rewritten(inst));
    }

    /// Gives an instruction's result another type (`Value::mutateType`); the
    /// operands and users stay, and what reads the old type is the caller's to
    /// put right.
    pub fn set_type(
        &mut self,
        inst: InstId,
        ty: TypeId,
    ) {
        self.instructions[inst.0 as usize].ty = ty;
        if let Some(result) = self.instructions[inst.0 as usize].result {
            self.values[result.0 as usize].ty = ty;
        }
        self.log(Change::Rewritten(inst));
    }

    /// Replaces an instruction's flags.
    pub fn set_flags(
        &mut self,
        inst: InstId,
        flags: Flags,
    ) {
        self.instructions[inst.0 as usize].flags = flags;
        self.log(Change::Rewritten(inst));
    }

    /// Every use of `value` now reads `with`. A record that names `value` still
    /// does: a pass that rewrites the uses of a value (to rebuild a loop
    /// around it, say) is not saying that the variable it was made of is
    /// another value, and a record is no use.
    pub fn replace_all_uses_with(
        &mut self,
        value: ValueId,
        with: Operand,
    ) {
        if with != Operand::Value(value) {
            // As LLVM's `Value::doRAUW`: each use moves from the head, so the
            // list is not copied.
            while let Some(one) = self.uses.head(Held::Value(value.0)) {
                self.set_operand(one.user, one.index as usize, with);
            }
        }
    }

    /// As [`replace_all_uses_with`](Self::replace_all_uses_with) where `with`
    /// is `value` itself, found again (common subexpression, a load's
    /// stored value, a constant folded to): the records that named `value` name
    /// `with`.
    pub fn replace_value(
        &mut self,
        value: ValueId,
        with: Operand,
    ) {
        if with != Operand::Value(value) {
            self.retarget_debug_records(value, with);
            // Every use goes: the list is taken, not scanned for each of its
            // uses (a value of U uses read U^2 entries).
            self.log(Change::Substituted { from: value, to: with });
            while let Some(one) = self.uses.head(Held::Value(value.0)) {
                self.uses.unlink(Held::Value(value.0), one);
                self.instructions[one.user.0 as usize].operands[one.index as usize] = with;
                self.add_use(with, one, self.instructions[one.user.0 as usize].opcode == Opcode::Phi);
                self.log(Change::Operand { inst: one.user, index: one.index, was: Operand::Value(value) });
                self.log(Change::Rewritten(one.user));
            }
        }
    }

    /// The variables a record of which went with deleted code.
    pub fn debug_dropped(&self) -> &[MetadataId] {
        &self.debug_dropped
    }

    /// What `-g` says of the variables, in the order of the program.
    pub fn debug_records(&self) -> &[DebugRecord] {
        &self.debug_records
    }

    /// As [`add_debug_record`](Self::add_debug_record), ahead of the records
    /// already standing before `before`: a pass that says what the
    /// instruction before `before` made says it earlier than what the code was
    /// already told to say there.
    pub fn add_debug_record_first(
        &mut self,
        before: InstId,
        variable: MetadataId,
        what: DebugWhat,
    ) {
        let at = self.debug_records.iter().position(|one| one.before == before).unwrap_or(self.debug_records.len());
        self.debug_records.insert(at, DebugRecord { before, variable, what, expression: None, location: None });
    }

    /// Says what `variable` is from just before `before` on, after the records
    /// already standing there. Nothing but the debug writers read it.
    pub fn add_debug_record(
        &mut self,
        before: InstId,
        variable: MetadataId,
        what: DebugWhat,
    ) {
        self.debug_records.push(DebugRecord { before, variable, what, expression: None, location: None });
    }

    /// The records of what `value` was now say `with`.
    fn retarget_debug_records(
        &mut self,
        value: ValueId,
        with: Operand,
    ) {
        for record in &mut self.debug_records {
            if let DebugWhat::Declare(at) | DebugWhat::Value(at) | DebugWhat::Piece { value: at, .. } = &mut record.what
                && *at == Operand::Value(value)
            {
                *at = with;
            }
        }
    }

    /// What named `value` says nothing now (a use of it in a record is no use,
    /// so it can go while the records stand).
    fn forget_debug_value(
        &mut self,
        value: ValueId,
    ) {
        // The memory a variable lived in is gone, and nothing is said of what
        // it was: only what the variable was set to says that.
        self.debug_records.retain(|one| {
            !matches!(
                one.what,
                DebugWhat::Declare(at) if at == Operand::Value(value)
            )
        });
        for record in &mut self.debug_records {
            match record.what {
                DebugWhat::Value(at) if at == Operand::Value(value) => record.what = DebugWhat::Gone,
                DebugWhat::Piece { value: at, offset, bytes } if at == Operand::Value(value) => {
                    record.what = DebugWhat::GonePiece { offset, bytes }
                }
                _ => {}
            }
        }
    }

    /// Every terminator and phi naming `block` now names `with`.
    pub fn replace_block_uses_with(
        &mut self,
        block: BlockId,
        with: BlockId,
    ) {
        if with != block {
            for held in [Held::Block(block.0), Held::PhiBlock(block.0)] {
                while let Some(one) = self.uses.head(held) {
                    self.set_operand(one.user, one.index as usize, Operand::Block(with));
                }
            }
        }
    }

    /// Removes an instruction whose result nothing uses, as LLVM's
    /// `eraseFromParent` requires.
    pub fn erase(
        &mut self,
        inst: InstId,
    ) -> Result<(), String> {
        if self.is_erased(inst) {
            return Err(format!("instruction {} is already erased", inst.0));
        }
        if let Some(result) = self.instruction(inst).result
            && !self.users(result).is_empty()
        {
            return Err(format!(
                "instruction {}'s result still has {} users",
                inst.0,
                self.users(result).iter().count()
            ));
        }
        let operands = self.instructions[inst.0 as usize].operands.clone();
        let phi = self.instructions[inst.0 as usize].opcode == Opcode::Phi;
        for (index, operand) in operands.into_iter().enumerate() {
            self.remove_use(operand, Use { user: inst, index: index as u32 }, phi);
        }
        if let Some(result) = self.instruction(inst).result {
            self.forget_debug_value(result);
        }
        if let Some((block, next)) = self.detach(inst, false) {
            self.log(Change::Erased { inst, block, next });
        }
        self.erased[inst.0 as usize] = true;
        Ok(())
    }

    /// `erase` of each of `insts`, as erasing them one by one from the last in
    /// program order to the first, but each block is rewritten once rather
    /// than once per instruction: erasing k of the n instructions of a block
    /// one at a time is O(kn). A result may be used by another of `insts`.
    pub fn erase_all(
        &mut self,
        insts: &[InstId],
    ) -> Result<(), String> {
        let mut gone = crate::dense::IdSet::<InstId>::new();
        let insts: Vec<InstId> = insts.iter().copied().filter(|&inst| gone.insert(inst)).collect();
        for &inst in &insts {
            if self.is_erased(inst) {
                return Err(format!("instruction {} is already erased", inst.0));
            }
            if let Some(result) = self.instruction(inst).result
                && let Some(user) = self.users(result).iter().find(|one| !gone.contains(&one.user))
            {
                return Err(format!("instruction {}'s result still has a user, {}", inst.0, user.user.0));
            }
        }
        for &inst in &insts {
            for (index, &operand) in self.instructions[inst.0 as usize].operands.iter().enumerate() {
                if let Some(held) = Held::of(operand, self.instructions[inst.0 as usize].opcode == Opcode::Phi) {
                    self.uses.unlink(held, Use { user: inst, index: index as u32 });
                }
            }
            if let Some(result) = self.instruction(inst).result {
                self.forget_debug_value(result);
            }
        }
        let mut blocks: Vec<BlockId> = insts.iter().filter_map(|&inst| self.parent(inst)).collect();
        blocks.sort_unstable();
        blocks.dedup();
        for block in blocks {
            let mut at = self.blocks[block.0 as usize].tail;
            let mut next = None;
            while at != 0 {
                let inst = InstId(at - 1);
                let before = self.instructions[inst.0 as usize].link.0;
                if gone.contains(&inst) {
                    self.settle_records(inst, next, false);
                    self.log(Change::Erased { inst, block, next });
                    self.unlink(block, inst);
                } else {
                    next = Some(inst);
                }
                at = before;
            }
            self.order.invalidate(block);
        }
        for inst in insts {
            self.parent[inst.0 as usize] = None;
            self.erased[inst.0 as usize] = true;
        }
        Ok(())
    }

    /// Adds `attr` to the call `inst`'s argument `index`, as LLVM's
    /// `CallBase::addParamAttr`.
    pub fn add_argument_attr(
        &mut self,
        inst: InstId,
        index: usize,
        attr: crate::opcode::Attribute,
    ) {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("a call")
        };
        info.argument_attrs[index].push(attr);
        self.log(Change::Rewritten(inst));
    }

    /// `CallBase::addParamAttr(index, Attribute::getWithAlignment(align))`: the
    /// argument's `align` is `align`, an earlier one replaced.
    pub fn set_argument_align(
        &mut self,
        inst: InstId,
        index: usize,
        align: u64,
    ) {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("a call")
        };
        let attrs = &mut info.argument_attrs[index];
        attrs.retain(|one| !matches!(one, crate::opcode::Attribute::Int(name, _) if name == "align"));
        attrs.push(crate::opcode::Attribute::Int("align".to_owned(), align));
        self.log(Change::Rewritten(inst));
    }

    /// Adds `attr` to the call `inst` itself, as LLVM's `CallBase::addFnAttr`.
    pub fn add_call_attr(
        &mut self,
        inst: InstId,
        attr: crate::opcode::Attribute,
    ) {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("a call")
        };
        info.attrs.push(attr);
        self.log(Change::Rewritten(inst));
    }

    /// Replaces the call `inst`'s attributes, as `CallBase::setAttributes`.
    pub fn set_call_attributes(
        &mut self,
        inst: InstId,
        attrs: Vec<crate::opcode::Attribute>,
        return_attrs: Vec<crate::opcode::Attribute>,
        argument_attrs: Vec<Vec<crate::opcode::Attribute>>,
    ) {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("a call")
        };
        info.attrs = attrs;
        info.return_attrs = return_attrs;
        info.argument_attrs = argument_attrs;
        self.log(Change::Rewritten(inst));
    }

    /// `CallBase::removeUBImplyingAttrs` of
    /// `Instruction::dropUBImplyingAttrsAndMetadata`: the call `inst` keeps
    /// no attribute of its result or arguments that makes it undefined (a
    /// fact's `implies_ub`, and `dereferenceable_or_null`). Whether any was
    /// dropped.
    pub fn drop_ub_implying_attrs(
        &mut self,
        inst: InstId,
    ) -> bool {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            return false;
        };
        let implies_ub = |attr: &crate::opcode::Attribute| {
            crate::facts::Fact::of_attribute(attr).is_some_and(crate::facts::Fact::implies_ub)
                || matches!(
                    attr,
                    crate::opcode::Attribute::Int(name, _) if name == "dereferenceable_or_null"
                )
        };
        let before = info.return_attrs.len() + info.argument_attrs.iter().map(Vec::len).sum::<usize>();
        info.return_attrs.retain(|attr| !implies_ub(attr));
        for attrs in &mut info.argument_attrs {
            attrs.retain(|attr| !implies_ub(attr));
        }
        let changed = before != info.return_attrs.len() + info.argument_attrs.iter().map(Vec::len).sum::<usize>();
        if changed {
            self.log(Change::Rewritten(inst));
        }
        changed
    }

    /// Gives the call `inst` calling convention `convention`.
    pub fn set_call_convention(
        &mut self,
        inst: InstId,
        convention: u32,
    ) {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &mut self.instructions[inst.0 as usize].opcode else {
            panic!("a call")
        };
        info.calling_convention = convention;
        self.log(Change::Rewritten(inst));
    }

    /// New parameters of `types` stand before parameter `at`, bare of
    /// attributes; the function's type follows.
    pub fn insert_parameters(
        &mut self,
        context: &mut crate::context::Context,
        at: usize,
        types: &[TypeId],
    ) -> Vec<ValueId> {
        let made: Vec<ValueId> = types
            .iter()
            .map(|&ty| {
                let value = ValueId(self.values.len() as u32);
                self.values.push(ValueData { ty, name: None, def: ValueDef::Argument(0) });
                self.uses.push_value();
                value
            })
            .collect();
        self.track_parameters();
        self.parameter_origins.splice(at..at, made.iter().map(|_| None));
        self.parameters.splice(at..at, made.iter().copied());
        if self.parameter_attrs.len() > at {
            self.parameter_attrs.splice(at..at, made.iter().map(|_| Vec::new()));
        }
        self.parameters_changed(context);
        self.log(Change::Parameters);
        made
    }

    /// Parameter `at`, which nothing uses, is gone; the function's type
    /// follows.
    pub fn remove_parameter(
        &mut self,
        context: &mut crate::context::Context,
        at: usize,
    ) {
        assert!(self.users(self.parameters[at]).is_empty(), "a parameter removed is unused");
        self.forget_debug_value(self.parameters[at]);
        self.track_parameters();
        self.parameter_origins.remove(at);
        self.parameters.remove(at);
        if self.parameter_attrs.len() > at {
            self.parameter_attrs.remove(at);
        }
        self.parameters_changed(context);
        self.log(Change::Parameters);
    }

    /// From the first change of the parameters on, each one's origin is kept.
    fn track_parameters(&mut self) {
        if self.parameter_origins.is_empty() {
            self.parameter_origins = (0..self.parameters.len()).map(Some).collect();
        }
    }

    fn parameters_changed(
        &mut self,
        context: &mut crate::context::Context,
    ) {
        for (index, &parameter) in self.parameters.clone().iter().enumerate() {
            self.values[parameter.0 as usize].def = ValueDef::Argument(index as u32);
        }
        let Type::Function { returns, variadic, .. } = context.types.get(self.ty).clone() else {
            panic!("a function type")
        };
        let parameters = self.parameters.iter().map(|&one| self.values[one.0 as usize].ty).collect();
        self.ty = context.types.intern(Type::Function { returns, parameters, variadic });
    }

    /// The call `inst` is made under `function_type`.
    pub fn set_call_type(
        &mut self,
        inst: InstId,
        function_type: TypeId,
    ) {
        let Opcode::Call(info) = &mut self.instructions[inst.0 as usize].opcode else { panic!("a call") };
        info.function_type = function_type;
        self.log(Change::Rewritten(inst));
    }

    /// The call `inst`'s argument `at` becomes `with`, as the callee's
    /// parameter did, under `function_type`.
    pub fn replace_argument(
        &mut self,
        inst: InstId,
        at: usize,
        with: &[Operand],
        function_type: TypeId,
    ) {
        let mut operands = self.instructions[inst.0 as usize].operands.clone();
        operands.splice(at..=at, with.iter().copied());
        self.set_operands(inst, operands);
        let Opcode::Call(info) = &mut self.instructions[inst.0 as usize].opcode else { panic!("a call") };
        info.function_type = function_type;
        if info.argument_attrs.len() <= at {
            info.argument_attrs.resize(at + 1, Vec::new());
        }
        info.argument_attrs.splice(at..=at, with.iter().map(|_| Vec::new()));
    }

    /// Says the access `inst` is aligned to `align` bytes; any other
    /// instruction is left as it is.
    pub fn set_access_align(
        &mut self,
        inst: InstId,
        bytes: u64,
    ) {
        let (Opcode::Load { align, .. } | Opcode::Store { align, .. }) = &mut self.instructions[inst.0 as usize].opcode
        else {
            return;
        };
        *align = Some(bytes);
        self.log(Change::Rewritten(inst));
    }

    /// `Instruction::dropPoisonGeneratingAnnotations`: the flags, metadata and
    /// call attributes of `inst` that make its result poison when the
    /// operands it was proved for change. `ctlz`, `cttz` and `abs` have no
    /// MIR spelling, so their poison-on-zero operand is not here.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instruction.h#L614>
    pub fn drop_poison_generating_annotations(
        &mut self,
        inst: InstId,
    ) {
        use crate::opcode::Attribute;
        let poison_attribute = |attr: &Attribute| match attr {
            Attribute::Range { .. } => true,
            Attribute::Flag(name) => name == "nonnull",
            Attribute::Int(name, _) => name == "align",
            _ => false,
        };
        let instruction = &mut self.instructions[inst.0 as usize];
        instruction.flags = instruction.flags.without(Flags::POISON_GENERATING);
        instruction.metadata.retain(|(kind, _)| !matches!(kind.as_str(), "range" | "nonnull" | "align" | "nofpclass"));
        if let Opcode::Call(info) | Opcode::Invoke(info) = &mut instruction.opcode {
            info.return_attrs.retain(|attr| !poison_attribute(attr));
            for attrs in &mut info.argument_attrs {
                attrs.retain(|attr| !poison_attribute(attr));
            }
        }
        self.log(Change::Rewritten(inst));
    }

    /// Attaches `node` to `inst` as metadata of `kind`.
    pub fn annotate(
        &mut self,
        inst: InstId,
        kind: &str,
        node: MetadataId,
    ) {
        self.instructions[inst.0 as usize].metadata.push((kind.to_owned(), node));
        self.log(Change::Rewritten(inst));
    }

    /// `Instruction::setMetadata`: `inst` has `node` as its metadata of `kind`
    /// alone, or none of that kind where `node` is `None`.
    pub fn set_metadata(
        &mut self,
        inst: InstId,
        kind: &str,
        node: Option<MetadataId>,
    ) {
        let metadata = &mut self.instructions[inst.0 as usize].metadata;
        let before = metadata.clone();
        metadata.retain(|(one, _)| one != kind);
        if let Some(node) = node {
            metadata.push((kind.to_owned(), node));
        }
        if self.instructions[inst.0 as usize].metadata != before {
            self.log(Change::Rewritten(inst));
        }
    }

    /// Every metadata node the function names (on an instruction, or in a debug
    /// record) is the one `map` says it is: nodes made last are numbered
    /// last, and the code that named them before they were made named them by a
    /// number it chose.
    pub fn renumber_metadata(
        &mut self,
        map: &dyn Fn(MetadataId) -> MetadataId,
    ) {
        for inst in &mut self.instructions {
            for (_, node) in &mut inst.metadata {
                *node = map(*node);
            }
        }
        for record in &mut self.debug_records {
            record.variable = map(record.variable);
        }
        for node in &mut self.debug_dropped {
            *node = map(*node);
        }
    }

    /// Takes the metadata of `kind` off `inst`.
    pub fn unannotate(
        &mut self,
        inst: InstId,
        kind: &str,
    ) {
        let metadata = &mut self.instructions[inst.0 as usize].metadata;
        let held = metadata.len();
        metadata.retain(|(one, _)| one != kind);
        // `!range` and the like are facts about the result.
        if metadata.len() != held {
            self.log(Change::Rewritten(inst));
        }
    }

    /// A copy of `inst`, placed nowhere, its result unnamed.
    pub fn clone_instruction(
        &mut self,
        inst: InstId,
    ) -> InstId {
        let original = self.instruction(inst).clone();
        let copy = self.create_instruction(original.opcode, original.ty, original.operands, original.flags, None);
        // What was said before the original is said before the copy, but of the
        // copy's own values, which are not the original's: the copy is
        // on a path of its own, where the variable has what the copy computes,
        // not what the original did. A constant, or an argument, is the
        // same on every path.
        let said: Vec<DebugRecord> = self.debug_records.iter().filter(|one| one.before == inst).copied().collect();
        let same_everywhere = |this: &Self, operand: Operand| match operand {
            Operand::Value(value) => matches!(this.value(value).def, ValueDef::Argument(_)),
            _ => true,
        };
        for one in said {
            let what = match one.what {
                DebugWhat::Value(operand) if !same_everywhere(self, operand) => DebugWhat::Gone,
                DebugWhat::Piece { value, offset, bytes } if !same_everywhere(self, value) => {
                    DebugWhat::GonePiece { offset, bytes }
                }
                other => other,
            };
            self.debug_records.push(DebugRecord {
                before: copy,
                variable: one.variable,
                what,
                expression: one.expression,
                location: one.location,
            });
        }
        self.instructions[copy.0 as usize].metadata = original.metadata;
        self.log(Change::Cloned { from: inst, to: copy });
        copy
    }

    /// `CloneBasicBlock(BB, VMap)`
    /// ([CloneFunction.cpp:40](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CloneFunction.cpp#L40)):
    /// a copy of `block` in the layout nowhere yet, its instructions copied in
    /// order. `map` gets `block` and each result of the original (not the
    /// `void` ones) with the copy's. The copies still read the original's
    /// values: `remap_instruction` rewrites them.
    pub fn clone_block(
        &mut self,
        block: BlockId,
        map: &mut ValueMap,
    ) -> BlockId {
        let copy = self.create_block(None);
        map.blocks.insert(block, copy);
        for inst in self.iter_block(block).collect::<Vec<_>>() {
            let duplicate = self.clone_instruction(inst);
            self.insert(duplicate, Position::End(copy)).expect("a new block takes its instructions");
            if let (Some(from), Some(to)) = (self.instruction(inst).result, self.instruction(duplicate).result) {
                map.values.insert(from, Operand::Value(to));
            }
        }
        copy
    }

    /// `RemapInstruction(I, VMap)`
    /// ([ValueMapper.cpp:1014](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ValueMapper.cpp#L1014)):
    /// the operands `map` has an entry for are replaced; the rest stay.
    pub fn remap_instruction(
        &mut self,
        inst: InstId,
        map: &ValueMap,
    ) {
        let operands = self
            .instruction(inst)
            .operands
            .iter()
            .map(|&one| match one {
                Operand::Value(value) => map.values.get(&value).copied().unwrap_or(one),
                Operand::Block(block) => map.blocks.get(&block).map_or(one, |&copy| Operand::Block(copy)),
                Operand::Constant(_) => one,
            })
            .collect();
        self.set_operands(inst, operands);
    }

    /// A new, empty block, in the layout nowhere yet.
    pub fn create_block(
        &mut self,
        name: Option<&str>,
    ) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        let name = name.map(|one| self.unique_name(one));
        self.blocks.push(Block { name, erased: false, placed: false, ..Block::default() });
        self.uses.push_block();
        self.log(Change::BlockCreated(id));
        id
    }

    /// Puts `block` in the layout after `after`, or last.
    pub fn insert_block(
        &mut self,
        block: BlockId,
        after: Option<BlockId>,
    ) -> Result<(), String> {
        if self.is_placed(block) || self.block(block).erased {
            return Err(format!("block {} is already placed or erased", block.0));
        }
        let at = match after {
            None => self.layout.len(),
            Some(after) => {
                self.layout
                    .iter()
                    .position(|one| *one == after)
                    .ok_or_else(|| format!("block {} is not placed", after.0))?
                    + 1
            }
        };
        self.layout.insert(at, block);
        self.blocks[block.0 as usize].placed = true;
        Ok(())
    }

    /// Puts `block` in the layout right before the placed `before`
    /// (`BasicBlock::Create(..., InsertBefore)`).
    pub fn insert_block_before(
        &mut self,
        block: BlockId,
        before: BlockId,
    ) -> Result<(), String> {
        if self.is_placed(block) || self.block(block).erased {
            return Err(format!("block {} is already placed or erased", block.0));
        }
        let at = self
            .layout
            .iter()
            .position(|one| *one == before)
            .ok_or_else(|| format!("block {} is not placed", before.0))?;
        self.layout.insert(at, block);
        self.blocks[block.0 as usize].placed = true;
        Ok(())
    }

    /// `BasicBlock::moveBefore`: the placed `block` moved to stand right
    /// before the placed `before`.
    pub fn move_block_before(
        &mut self,
        block: BlockId,
        before: BlockId,
    ) -> Result<(), String> {
        if block == before {
            return Ok(());
        }
        let from = self
            .layout
            .iter()
            .position(|one| *one == block)
            .ok_or_else(|| format!("block {} is not placed", block.0))?;
        self.layout.remove(from);
        let at = self
            .layout
            .iter()
            .position(|one| *one == before)
            .ok_or_else(|| format!("block {} is not placed", before.0))?;
        self.layout.insert(at, block);
        Ok(())
    }

    /// `BasicBlock::moveAfter`: the placed `block` moved to stand right after
    /// the placed `after`.
    pub fn move_block_after(
        &mut self,
        block: BlockId,
        after: BlockId,
    ) -> Result<(), String> {
        if block == after {
            return Ok(());
        }
        let from = self
            .layout
            .iter()
            .position(|one| *one == block)
            .ok_or_else(|| format!("block {} is not placed", block.0))?;
        self.layout.remove(from);
        let at = self
            .layout
            .iter()
            .position(|one| *one == after)
            .ok_or_else(|| format!("block {} is not placed", after.0))?;
        self.layout.insert(at + 1, block);
        Ok(())
    }

    /// Drops every instruction and block, leaving a declaration, as LLVM's
    /// `deleteBody`.
    pub fn delete_body(&mut self) {
        for block in self.layout.clone() {
            for inst in self.iter_block(block).collect::<Vec<_>>() {
                let (block, next) = self.detach(inst, false).expect("a placed instruction");
                self.log(Change::Erased { inst, block, next });
            }
            self.blocks[block.0 as usize].erased = true;
            self.blocks[block.0 as usize].placed = false;
            self.log(Change::BlockErased(block));
        }
        self.names = NameIndex::default();
        self.layout.clear();
        self.debug_records.clear();
        self.erased.fill(true);
        self.uses.clear_all();
    }

    /// One edge from `predecessor` into `block` gone: its phis lose one input
    /// from it. Unless `keep_one_input_phis`, a phi left with no input is
    /// replaced by poison, and one whose inputs are all alike by that value.
    /// LLVM's
    /// [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446).
    /// MIRROR, one call drops one input.
    pub fn predecessor_removed(
        &mut self,
        context: &Context,
        block: BlockId,
        predecessor: BlockId,
        keep_one_input_phis: bool,
    ) {
        let phis: Vec<InstId> =
            self.iter_block(block).take_while(|&one| self.instruction(one).opcode == Opcode::Phi).collect();
        // Return early if there are no PHI nodes to update.
        let Some(&front) = phis.first() else { return };
        let num_preds = self.instruction(front).operands.len() / 2;
        for phi in phis {
            self.remove_incoming_value(context, phi, predecessor, !keep_one_input_phis);
            if keep_one_input_phis {
                continue;
            }
            // With a single predecessor, removeIncomingValue erased the phi.
            if num_preds == 1 {
                continue;
            }
            if let Some(constant) = self.phi_constant_value(context, phi) {
                self.replace_value(self.instruction(phi).result.expect("a phi's value"), constant);
                self.set_operands(phi, Vec::new());
                self.erase(phi).expect("a phi nothing reads");
            }
        }
    }

    /// `PHINode::removeIncomingValue(BB, DeletePHIIfEmpty)`: the last input
    /// takes the removed one's place. Whether an input from `predecessor` was
    /// there.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L148>
    pub fn remove_incoming_value(
        &mut self,
        context: &Context,
        phi: InstId,
        predecessor: BlockId,
        delete_phi_if_empty: bool,
    ) -> bool {
        let mut operands = self.instruction(phi).operands.clone();
        let Some(at) = operands.chunks(2).position(|pair| pair[1] == Operand::Block(predecessor)) else {
            return false;
        };
        let last = operands.len() / 2 - 1;
        if at != last {
            operands[2 * at] = operands[2 * last];
            operands[2 * at + 1] = operands[2 * last + 1];
        }
        operands.truncate(2 * last);
        let emptied = operands.is_empty();
        self.set_operands(phi, operands);
        if emptied && delete_phi_if_empty {
            let result = self.instruction(phi).result.expect("a phi's value");
            let poison = context.constant(Constant { ty: self.value(result).ty, kind: ConstantKind::Poison });
            self.replace_value(result, Operand::Constant(poison));
            self.erase(phi).expect("a phi nothing reads");
        }
        true
    }

    /// `PHINode::hasConstantValue`: the one value all of `phi`'s inputs are
    /// but itself (poison if it has only itself).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L212>
    fn phi_constant_value(
        &self,
        context: &Context,
        phi: InstId,
    ) -> Option<Operand> {
        let result = self.instruction(phi).result.expect("a phi's value");
        let this = Operand::Value(result);
        let mut inputs = self.instruction(phi).operands.chunks(2).map(|pair| pair[0]);
        let mut only = inputs.next()?;
        for value in inputs {
            if value != only && value != this {
                if only != this {
                    return None;
                }
                only = value;
            }
        }
        if only == this {
            let poison = context.constant(Constant { ty: self.value(result).ty, kind: ConstantKind::Poison });
            return Some(Operand::Constant(poison));
        }
        Some(only)
    }

    /// Removes an empty block nothing names.
    pub fn erase_block(
        &mut self,
        block: BlockId,
    ) -> Result<(), String> {
        self.erase_blocks(&[block])
    }

    /// Removes empty blocks nothing names, leaving the layout in one pass:
    /// erasing `n` blocks one by one scans it `n` times.
    pub fn erase_blocks(
        &mut self,
        blocks: &[BlockId],
    ) -> Result<(), String> {
        for &block in blocks {
            if !self.blocks[block.0 as usize].is_empty() {
                return Err(format!("block {} still holds instructions", block.0));
            }
            if !self.block_users(block).is_empty() || !self.phi_block_users(block).is_empty() {
                let users = self.block_users(block).iter().count() + self.phi_block_users(block).iter().count();
                return Err(format!("block {} still has {users} users", block.0));
            }
        }
        let mut gone = crate::dense::IdSet::<BlockId>::new();
        for &block in blocks {
            gone.insert(block);
        }
        LAYOUT_SCANNED.with(|scanned| scanned.set(scanned.get() + self.layout.len()));
        self.layout.retain(|one| !gone.contains(one));
        for &block in blocks {
            self.release_name(block);
            self.blocks[block.0 as usize].erased = true;
            self.blocks[block.0 as usize].placed = false;
            self.log(Change::BlockErased(block));
        }
        Ok(())
    }
}

thread_local! {
    /// Layout entries looked over to take blocks out of it.
    static LAYOUT_SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many layout entries this thread has looked over to erase blocks, for a
/// test that erasing k of n blocks does not scan k times n.
pub fn layout_scanned() -> usize {
    LAYOUT_SCANNED.with(std::cell::Cell::get)
}

/// `ValueToValueMapTy`: what `clone_block` copied, original to copy.
#[derive(Default)]
pub struct ValueMap {
    pub values: crate::dense::IdMap<ValueId, Operand>,
    pub blocks: crate::dense::IdMap<BlockId, BlockId>,
}
