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
        self.value_uses.reset(self.values.len());
        self.block_uses = vec![Vec::new(); self.blocks.len()];
        self.parent = vec![None; self.instructions.len()];
        self.erased = vec![false; self.instructions.len()];
        for &block in &self.layout.clone() {
            for &inst in &self.blocks[block.0 as usize].instructions.clone() {
                self.parent[inst.0 as usize] = Some(block);
            }
        }
        for inst in 0..self.instructions.len() {
            let operands = self.instructions[inst].operands.clone();
            self.value_uses.slots(inst as u32, operands.len());
            for (index, operand) in operands.into_iter().enumerate() {
                self.add_use(operand, Use { user: InstId(inst as u32), index: index as u32 });
            }
        }
    }

    fn add_use(
        &mut self,
        operand: Operand,
        at: Use,
    ) {
        match operand {
            Operand::Value(value) => self.value_uses.link(value.0, at),
            Operand::Block(block) => self.block_uses[block.0 as usize].push(at),
            Operand::Constant(_) => {}
        }
    }

    fn remove_use(
        &mut self,
        operand: Operand,
        at: Use,
    ) {
        match operand {
            Operand::Value(value) => self.value_uses.unlink(value.0, at),
            Operand::Block(block) => {
                let uses = &mut self.block_uses[block.0 as usize];
                USES_SCANNED.with(|scanned| scanned.set(scanned.get() + uses.len()));
                uses.retain(|one| *one != at);
            }
            Operand::Constant(_) => {}
        }
    }

    /// Where the kept use lists disagree with the operands: an instrument,
    /// empty while every change went through this API.
    pub fn check_uses(&self) -> Vec<String> {
        let mut values = vec![BTreeSet::new(); self.values.len()];
        let mut blocks = vec![BTreeSet::new(); self.blocks.len()];
        for (at, instruction) in self.instructions.iter().enumerate() {
            if self.erased[at] {
                continue;
            }
            for (index, operand) in instruction.operands.iter().enumerate() {
                let one = Use { user: InstId(at as u32), index: index as u32 };
                match operand {
                    Operand::Value(value) => values[value.0 as usize].insert(one),
                    Operand::Block(block) => blocks[block.0 as usize].insert(one),
                    Operand::Constant(_) => false,
                };
            }
        }
        let mut problems = Vec::new();
        for (at, expected) in values.iter().enumerate() {
            let list = self.value_uses.users(at as u32).to_vec();
            let kept: BTreeSet<Use> = list.iter().copied().collect();
            if kept != *expected || kept.len() != list.len() {
                problems.push(format!("value {at}: kept {list:?}, operands say {expected:?}"));
            }
        }
        for (at, expected) in blocks.iter().enumerate() {
            let kept: BTreeSet<Use> = self.block_uses[at].iter().copied().collect();
            if kept != *expected || kept.len() != self.block_uses[at].len() {
                problems.push(format!("block {at}: kept {:?}, operands say {expected:?}", self.block_uses[at]));
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
            self.value_uses.push_value();
            value
        });
        self.value_uses.slots(id.0, operands.len());
        for (index, &operand) in operands.iter().enumerate() {
            self.add_use(operand, Use { user: id, index: index as u32 });
        }
        self.instructions.push(Instruction { opcode, ty, operands, flags, result, metadata: Vec::new() });
        self.parent.push(None);
        self.erased.push(false);
        id
    }

    fn slot(
        &self,
        position: Position,
    ) -> Result<(BlockId, usize), String> {
        match position {
            Position::End(block) => Ok((block, self.block(block).instructions.len())),
            Position::After(prev) => {
                let block = self.parent(prev).ok_or_else(|| format!("instruction {} is not placed", prev.0))?;
                let at = self
                    .block(block)
                    .instructions
                    .iter()
                    .position(|one| *one == prev)
                    .expect("a parent holds its instruction");
                Ok((block, at + 1))
            }
            Position::Before(next) => {
                let block = self.parent(next).ok_or_else(|| format!("instruction {} is not placed", next.0))?;
                let at = self
                    .block(block)
                    .instructions
                    .iter()
                    .position(|one| *one == next)
                    .expect("a parent holds its instruction");
                Ok((block, at))
            }
        }
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
        // A phi that names the block as where its value comes from is a use of
        // it too, but no way in. (An erase has already taken its own
        // use of the block away.)
        let entries: Vec<InstId> = self
            .block_users(target)
            .iter()
            .map(|one| one.user)
            .filter(|&one| self.instruction(one).opcode != Opcode::Phi)
            .collect();
        if !entries.is_empty() && entries != [inst] {
            return None;
        }
        self.block(target).iter().find(|&one| self.instruction(one).opcode != Opcode::Phi)
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
        let list = &self.blocks[block.0 as usize].instructions;
        SCANNED.with(|scanned| scanned.set(scanned.get() + list.len()));
        let at = list.iter().position(|one| *one == inst).expect("an instruction is in its parent");
        let next = list.get(at + 1).copied();
        self.settle_records(inst, next, moving);
        self.blocks[block.0 as usize].instructions.remove(at);
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
        let (block, at) = self.slot(position)?;
        if self.block(block).erased {
            return Err(format!("block {} is erased", block.0));
        }
        self.blocks[block.0 as usize].instructions.insert(at, inst);
        self.parent[inst.0 as usize] = Some(block);
        // What follows is what the position named: scanning the block for it
        // made each append linear (#992).
        let next = match position {
            Position::End(_) => None,
            Position::Before(next) => Some(next),
            Position::After(_) => self.blocks[block.0 as usize].instructions.get(at + 1).copied(),
        };
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
            let list = &mut self.blocks[block_from.0 as usize].instructions;
            let at = list.iter().position(|one| *one == inst).expect("an instruction is in its parent");
            list.remove(at);
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
        for &block in &blocks {
            let list = &mut self.blocks[block.0 as usize].instructions;
            SCANNED.with(|scanned| scanned.set(scanned.get() + list.len()));
            list.retain(|one| !moving.contains(one));
            self.order.invalidate(block);
        }
        for &inst in run {
            self.parent[inst.0 as usize] = None;
        }
        // The place is found with the run out of the way: its index, once.
        let (to, at) = self.slot(position)?;
        if self.blocks[to.0 as usize].erased {
            return Err(format!("block {} is erased", to.0));
        }
        let next = self.blocks[to.0 as usize].instructions.get(at).copied();
        self.blocks[to.0 as usize].instructions.splice(at..at, run.iter().copied());
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
        self.remove_use(old, at);
        self.add_use(operand, at);
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
        let old = std::mem::take(&mut self.instructions[inst.0 as usize].operands);
        let was: Box<[Operand]> = old.as_slice().into();
        if old.len() > 4 {
            // A phi of N inputs names a few values and N blocks: each list is
            // scanned once for all the places `inst` names it, not
            // once for each, which was N^2 for the one value
            // N inputs repeat.
            let mut places = crate::hash::HashMap::<BlockId, Vec<u32>>::default();
            for (index, operand) in old.into_iter().enumerate() {
                match operand {
                    Operand::Block(block) => places.entry(block).or_default().push(index as u32),
                    _ => self.remove_use(operand, Use { user: inst, index: index as u32 }),
                }
            }
            for (block, indexes) in places {
                let uses = &mut self.block_uses[block.0 as usize];
                USES_SCANNED.with(|scanned| scanned.set(scanned.get() + uses.len()));
                uses.retain(|one| one.user != inst || indexes.binary_search(&one.index).is_err());
            }
        } else {
            for (index, operand) in old.into_iter().enumerate() {
                self.remove_use(operand, Use { user: inst, index: index as u32 });
            }
        }
        self.value_uses.slots(inst.0, operands.len());
        for (index, &operand) in operands.iter().enumerate() {
            self.add_use(operand, Use { user: inst, index: index as u32 });
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

    /// Replaces an instruction's flags.
    pub fn set_flags(
        &mut self,
        inst: InstId,
        flags: Flags,
    ) {
        self.instructions[inst.0 as usize].flags = flags;
        self.log(Change::Rewritten(inst));
    }

    fn replace_uses(
        &mut self,
        uses: Vec<Use>,
        with: Operand,
    ) {
        for one in uses {
            self.set_operand(one.user, one.index as usize, with);
        }
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
            self.replace_uses(self.users(value).to_vec(), with);
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
            while let Some(one) = self.value_uses.head(value.0) {
                self.value_uses.unlink(value.0, one);
                self.instructions[one.user.0 as usize].operands[one.index as usize] = with;
                self.add_use(with, one);
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
        self.debug_records.insert(at, DebugRecord { before, variable, what });
    }

    /// Says what `variable` is from just before `before` on, after the records
    /// already standing there. Nothing but the debug writers read it.
    pub fn add_debug_record(
        &mut self,
        before: InstId,
        variable: MetadataId,
        what: DebugWhat,
    ) {
        self.debug_records.push(DebugRecord { before, variable, what });
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
            self.replace_uses(self.block_users(block).to_vec(), Operand::Block(with));
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
            return Err(format!("instruction {}'s result still has {} users", inst.0, self.users(result).len()));
        }
        let operands = self.instructions[inst.0 as usize].operands.clone();
        for (index, operand) in operands.into_iter().enumerate() {
            self.remove_use(operand, Use { user: inst, index: index as u32 });
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
        // A value's uses unlink one by one; a block's list is filtered once,
        // not once for each use of it that goes.
        let mut operands: Vec<BlockId> = Vec::new();
        let mut seen_blocks = crate::dense::IdSet::<BlockId>::new();
        for &inst in &insts {
            for (index, &operand) in self.instructions[inst.0 as usize].operands.iter().enumerate() {
                match operand {
                    Operand::Value(value) => self.value_uses.unlink(value.0, Use { user: inst, index: index as u32 }),
                    Operand::Block(block) => {
                        if seen_blocks.insert(block) {
                            operands.push(block);
                        }
                    }
                    Operand::Constant(_) => {}
                }
            }
            if let Some(result) = self.instruction(inst).result {
                self.forget_debug_value(result);
            }
        }
        for block in operands {
            let uses = &mut self.block_uses[block.0 as usize];
            USES_SCANNED.with(|scanned| scanned.set(scanned.get() + uses.len()));
            uses.retain(|one| !gone.contains(&one.user));
        }
        let mut blocks: Vec<BlockId> = insts.iter().filter_map(|&inst| self.parent(inst)).collect();
        blocks.sort_unstable();
        blocks.dedup();
        for block in blocks {
            let list = self.blocks[block.0 as usize].instructions.clone();
            SCANNED.with(|scanned| scanned.set(scanned.get() + list.len()));
            let mut next = None;
            for &inst in list.iter().rev() {
                if gone.contains(&inst) {
                    self.settle_records(inst, next, false);
                    self.log(Change::Erased { inst, block, next });
                } else {
                    next = Some(inst);
                }
            }
            self.blocks[block.0 as usize].instructions.retain(|one| !gone.contains(one));
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
                self.value_uses.push_value();
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
        metadata.retain(|(one, _)| one != kind);
        if let Some(node) = node {
            metadata.push((kind.to_owned(), node));
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
            self.debug_records.push(DebugRecord { before: copy, variable: one.variable, what });
        }
        self.instructions[copy.0 as usize].metadata = original.metadata;
        self.log(Change::Cloned { from: inst, to: copy });
        copy
    }

    /// A new, empty block, in the layout nowhere yet.
    pub fn create_block(
        &mut self,
        name: Option<&str>,
    ) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        let name = name.map(|one| self.unique_name(one));
        self.blocks.push(Block { name, instructions: Vec::new(), erased: false, placed: false });
        self.block_uses.push(Vec::new());
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

    /// Drops every instruction and block, leaving a declaration, as LLVM's
    /// `deleteBody`.
    pub fn delete_body(&mut self) {
        for block in self.layout.clone() {
            for inst in self.block(block).instructions.clone() {
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
        self.value_uses.clear_all();
        self.block_uses.iter_mut().for_each(Vec::clear);
    }

    /// One edge from `predecessor` into `block` gone: its phis lose one input
    /// from it. Unless `keep_one_input_phis`, a phi left with one value (or
    /// none) is replaced by it (or by poison). LLVM's
    /// [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446),
    /// with `PHINode::hasConstantValue` (L468). MIRROR, except a phi has one
    /// input per edge, so one call drops one input.
    pub fn predecessor_removed(
        &mut self,
        context: &Context,
        block: BlockId,
        predecessor: BlockId,
        keep_one_input_phis: bool,
    ) {
        let phis: Vec<InstId> = self
            .block(block)
            .iter()
            .take_while(|&one| self.instruction(one).opcode == Opcode::Phi)
            .collect();
        for phi in phis {
            let mut operands = self.instruction(phi).operands.clone();
            let Some(at) = operands.chunks(2).position(|pair| pair[1] == Operand::Block(predecessor)) else {
                continue;
            };
            operands.drain(2 * at..2 * at + 2);
            let result = self.instruction(phi).result.expect("a phi's value");
            let emptied = operands.is_empty();
            self.set_operands(phi, operands);
            if keep_one_input_phis {
                continue;
            }
            let only = if emptied {
                let poison = context.constant(Constant { ty: self.value(result).ty, kind: ConstantKind::Poison });
                Some(Operand::Constant(poison))
            } else {
                self.phi_constant_value(phi)
            };
            if let Some(only) = only {
                self.replace_value(result, only);
                self.set_operands(phi, Vec::new());
                self.erase(phi).expect("a phi nothing reads");
            }
        }
    }

    /// The one value all of `phi`'s inputs are, itself not counted: LLVM's
    /// `PHINode::hasConstantValue`.
    fn phi_constant_value(
        &self,
        phi: InstId,
    ) -> Option<Operand> {
        let result = Operand::Value(self.instruction(phi).result.expect("a phi's value"));
        let mut only = None;
        for pair in self.instruction(phi).operands.chunks(2) {
            if pair[0] == result {
                continue;
            }
            if only.is_some_and(|one| one != pair[0]) {
                return None;
            }
            only = Some(pair[0]);
        }
        only
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
            if !self.block(block).instructions.is_empty() {
                return Err(format!("block {} still holds instructions", block.0));
            }
            if !self.block_users(block).is_empty() {
                return Err(format!("block {} still has {} users", block.0, self.block_users(block).len()));
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

thread_local! {
    /// Instructions of a block looked over to take instructions out of it: the
    /// cost of erasing, which was a scan of the block per instruction erased.
    pub(crate) static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many instructions of a block this thread has looked over to take
/// instructions out of it, for a test that a pass erasing k of a block's n
/// does not scan k times n.
pub fn instructions_scanned() -> usize {
    SCANNED.with(std::cell::Cell::get)
}

thread_local! {
    pub(crate) static USES_SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many use-list entries this thread has looked over to remove uses, for a
/// test that a pass does not scan a long list once per use it removes.
pub fn uses_scanned() -> usize {
    USES_SCANNED.with(std::cell::Cell::get)
}
