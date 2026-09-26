//! Changing a function, as LLVM's API does. Every change keeps the use
//! lists true and goes into the change log; ids are never reused.

use std::collections::BTreeSet;

use crate::module::{Block, BlockId, Change, Function, InstId, Instruction, Operand, Use, ValueData, ValueDef, ValueId};
use crate::opcode::{Flags, Opcode};
use crate::types::TypeId;

/// Where an instruction goes: before another, or at a block's end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Before(InstId),
    End(BlockId),
}

impl Function {
    /// Parents and use lists from the arenas, after reading.
    pub(crate) fn index(&mut self) {
        self.value_uses = vec![Vec::new(); self.values.len()];
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
            for (index, operand) in operands.into_iter().enumerate() {
                self.add_use(operand, Use { user: InstId(inst as u32), index: index as u32 });
            }
        }
    }

    fn uses_of(&mut self, operand: Operand) -> Option<&mut Vec<Use>> {
        match operand {
            Operand::Value(value) => Some(&mut self.value_uses[value.0 as usize]),
            Operand::Block(block) => Some(&mut self.block_uses[block.0 as usize]),
            Operand::Constant(_) => None,
        }
    }

    fn add_use(&mut self, operand: Operand, at: Use) {
        if let Some(uses) = self.uses_of(operand) {
            uses.push(at);
        }
    }

    fn remove_use(&mut self, operand: Operand, at: Use) {
        if let Some(uses) = self.uses_of(operand) {
            uses.retain(|one| *one != at);
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
            let kept: BTreeSet<Use> = self.value_uses[at].iter().copied().collect();
            if kept != *expected || kept.len() != self.value_uses[at].len() {
                problems.push(format!("value {at}: kept {:?}, operands say {expected:?}", self.value_uses[at]));
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
    fn unique_name(&self, name: &str) -> String {
        let taken = |candidate: &str| {
            self.values.iter().any(|one| one.name.as_deref() == Some(candidate))
                || self.blocks.iter().any(|one| !one.erased && one.name.as_deref() == Some(candidate))
        };
        if !taken(name) {
            return name.to_owned();
        }
        (1..).map(|n| format!("{name}{n}")).find(|candidate| !taken(candidate)).expect("some number is free")
    }

    /// A new instruction, placed nowhere yet; its result, if its type is not
    /// `void`, is named `name` or numbered.
    pub fn create_instruction(&mut self, opcode: Opcode, ty: TypeId, operands: Vec<Operand>, flags: Flags, name: Option<&str>) -> InstId {
        let id = InstId(self.instructions.len() as u32);
        let result = (ty != self.void).then(|| {
            let value = ValueId(self.values.len() as u32);
            let name = name.map(|one| self.unique_name(one));
            self.values.push(ValueData { ty, name, def: ValueDef::Instruction(id) });
            self.value_uses.push(Vec::new());
            value
        });
        for (index, &operand) in operands.iter().enumerate() {
            self.add_use(operand, Use { user: id, index: index as u32 });
        }
        self.instructions.push(Instruction { opcode, ty, operands, flags, result, metadata: Vec::new() });
        self.parent.push(None);
        self.erased.push(false);
        id
    }

    fn slot(&self, position: Position) -> Result<(BlockId, usize), String> {
        match position {
            Position::End(block) => Ok((block, self.block(block).instructions.len())),
            Position::Before(next) => {
                let block = self.parent(next).ok_or_else(|| format!("instruction {} is not placed", next.0))?;
                let at = self.block(block).instructions.iter().position(|one| *one == next).expect("a parent holds its instruction");
                Ok((block, at))
            }
        }
    }

    /// Whatever follows `inst` in its block.
    fn next_of(&self, block: BlockId, inst: InstId) -> Option<InstId> {
        let list = &self.block(block).instructions;
        list.iter().position(|one| *one == inst).and_then(|at| list.get(at + 1).copied())
    }

    fn detach(&mut self, inst: InstId) -> Option<(BlockId, Option<InstId>)> {
        let block = self.parent(inst)?;
        let next = self.next_of(block, inst);
        self.blocks[block.0 as usize].instructions.retain(|one| *one != inst);
        self.parent[inst.0 as usize] = None;
        Some((block, next))
    }

    fn attach(&mut self, inst: InstId, position: Position) -> Result<(BlockId, Option<InstId>), String> {
        let (block, at) = self.slot(position)?;
        if self.block(block).erased {
            return Err(format!("block {} is erased", block.0));
        }
        self.blocks[block.0 as usize].instructions.insert(at, inst);
        self.parent[inst.0 as usize] = Some(block);
        Ok((block, self.next_of(block, inst)))
    }

    /// Places an instruction placed nowhere yet.
    pub fn insert(&mut self, inst: InstId, position: Position) -> Result<(), String> {
        if self.is_erased(inst) || self.parent(inst).is_some() {
            return Err(format!("instruction {} is already placed or erased", inst.0));
        }
        let (block, next) = self.attach(inst, position)?;
        self.changes.push(Change::Inserted { inst, block, next });
        Ok(())
    }

    /// Moves a placed instruction.
    pub fn move_to(&mut self, inst: InstId, position: Position) -> Result<(), String> {
        if position == Position::Before(inst) {
            return Ok(());
        }
        self.detach(inst).ok_or_else(|| format!("instruction {} is not placed", inst.0))?;
        let (block, next) = self.attach(inst, position)?;
        self.changes.push(Change::Moved { inst, block, next });
        Ok(())
    }

    pub fn set_operand(&mut self, inst: InstId, index: usize, operand: Operand) {
        let at = Use { user: inst, index: index as u32 };
        let old = std::mem::replace(&mut self.instructions[inst.0 as usize].operands[index], operand);
        self.remove_use(old, at);
        self.add_use(operand, at);
        self.changes.push(Change::Rewritten(inst));
    }

    fn replace_uses(&mut self, uses: Vec<Use>, with: Operand) {
        for one in uses {
            self.set_operand(one.user, one.index as usize, with);
        }
    }

    /// Every use of `value` now reads `with`.
    pub fn replace_all_uses_with(&mut self, value: ValueId, with: Operand) {
        if with != Operand::Value(value) {
            self.replace_uses(self.users(value).to_vec(), with);
        }
    }

    /// Every terminator and phi naming `block` now names `with`.
    pub fn replace_block_uses_with(&mut self, block: BlockId, with: BlockId) {
        if with != block {
            self.replace_uses(self.block_users(block).to_vec(), Operand::Block(with));
        }
    }

    /// Removes an instruction whose result nothing uses, as LLVM's
    /// `eraseFromParent` requires.
    pub fn erase(&mut self, inst: InstId) -> Result<(), String> {
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
        if let Some((block, next)) = self.detach(inst) {
            self.changes.push(Change::Erased { inst, block, next });
        }
        self.erased[inst.0 as usize] = true;
        Ok(())
    }

    /// A copy of `inst`, placed nowhere, its result unnamed.
    pub fn clone_instruction(&mut self, inst: InstId) -> InstId {
        let original = self.instruction(inst).clone();
        let copy = self.create_instruction(original.opcode, original.ty, original.operands, original.flags, None);
        self.instructions[copy.0 as usize].metadata = original.metadata;
        self.changes.push(Change::Cloned { from: inst, to: copy });
        copy
    }

    /// A new, empty block, in the layout nowhere yet.
    pub fn create_block(&mut self, name: Option<&str>) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        let name = name.map(|one| self.unique_name(one));
        self.blocks.push(Block { name, instructions: Vec::new(), erased: false });
        self.block_uses.push(Vec::new());
        self.changes.push(Change::BlockCreated(id));
        id
    }

    /// Puts `block` in the layout after `after`, or last.
    pub fn insert_block(&mut self, block: BlockId, after: Option<BlockId>) -> Result<(), String> {
        if self.layout.contains(&block) || self.block(block).erased {
            return Err(format!("block {} is already placed or erased", block.0));
        }
        let at = match after {
            None => self.layout.len(),
            Some(after) => self.layout.iter().position(|one| *one == after).ok_or_else(|| format!("block {} is not placed", after.0))? + 1,
        };
        self.layout.insert(at, block);
        Ok(())
    }

    /// Drops every instruction and block, leaving a declaration, as LLVM's
    /// `deleteBody`.
    pub fn delete_body(&mut self) {
        for block in self.layout.clone() {
            for inst in self.block(block).instructions.clone() {
                let (block, next) = self.detach(inst).expect("a placed instruction");
                self.changes.push(Change::Erased { inst, block, next });
            }
            self.blocks[block.0 as usize].erased = true;
            self.changes.push(Change::BlockErased(block));
        }
        self.layout.clear();
        self.erased.fill(true);
        self.value_uses.iter_mut().for_each(Vec::clear);
        self.block_uses.iter_mut().for_each(Vec::clear);
    }

    /// Removes an empty block nothing names.
    pub fn erase_block(&mut self, block: BlockId) -> Result<(), String> {
        if !self.block(block).instructions.is_empty() {
            return Err(format!("block {} still holds instructions", block.0));
        }
        if !self.block_users(block).is_empty() {
            return Err(format!("block {} still has {} users", block.0, self.block_users(block).len()));
        }
        self.layout.retain(|one| *one != block);
        self.blocks[block.0 as usize].erased = true;
        self.changes.push(Change::BlockErased(block));
        Ok(())
    }
}
