//! What the edits since a mark touched, and how many times each kind has been
//! made: the record consumers of the change log read instead of the log.
//!
//! - [`Touched`] is the digest of the log since a [`Mark`]: the blocks reached,
//!   the CFG edges added and removed, the values added, removed and rewritten,
//!   and the loads, stores and calls added, removed, moved and rewritten. An
//!   instruction added and erased inside the window is in neither.
//!   `Function::touched_since` derives it, in time of the changes since the
//!   mark. `Function::reach_of` derives only what `reach` reads: not `named`,
//!   which reads every operand list an edit replaced.
//! - [`Epochs`] are counters the editor bumps as it logs a change, never reset:
//!   a memo that stores the epochs it read is valid exactly while they are
//!   equal, in O(1) and without the log (which a pass may no longer hold).
//!
//! The classification is by what an instruction is now. A `Rewritten` that
//! changed an opcode is seen as the new one; the old opcode goes into the log
//! with the invertible log (rollback), and the epochs then read it.

use crate::context::Context;
use crate::dense::IdSet;
use crate::module::{BlockId, Change, Function, InstId, Mark, Operand, ValueId};
use crate::opcode::Opcode;
use crate::types::Type;

/// A CFG edge that was added or removed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Edge {
    Added,
    Removed,
}

/// The digest of the changes since a mark.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Touched {
    /// Blocks where an instruction was added, removed, moved to or from, or
    /// rewritten, and blocks made or erased.
    pub blocks: IdSet<BlockId>,
    pub blocks_created: Vec<BlockId>,
    pub blocks_erased: Vec<BlockId>,
    /// Edges between blocks, by what the terminators named at the mark and name
    /// now; an edge in both is in neither.
    pub cfg: Vec<(BlockId, BlockId, Edge)>,
    /// Results of instructions added since the mark and still there.
    pub values_added: IdSet<ValueId>,
    /// Results of instructions the mark saw and since erased.
    pub values_removed: IdSet<ValueId>,
    /// Results of instructions whose operands, opcode or flags changed.
    pub operands_changed: IdSet<ValueId>,
    /// Loads, stores and calls added and still there.
    pub memory_added: IdSet<InstId>,
    /// Loads, stores and calls the mark saw and since erased.
    pub memory_removed: IdSet<InstId>,
    /// Loads, stores and calls moved, and not added since the mark.
    pub memory_moved: IdSet<InstId>,
    /// Loads, stores and calls rewritten, and not added since the mark.
    pub memory_rewritten: IdSet<InstId>,
    /// The function's parameters were inserted or removed.
    pub parameters: bool,
    /// Pointer (and aggregate) values added, removed or with changed operands,
    /// and those that loads, stores and calls added, removed, moved or
    /// rewritten name: what a flow-insensitive points-to solve is built from.
    /// Empty until `with_pointers` fills it (it needs the types).
    pub pointers: IdSet<ValueId>,
    /// Instructions added, moved or rewritten since the mark and still there:
    /// where `reach` starts.
    pub insts: IdSet<InstId>,
    /// The results and operands of the instructions added, erased, moved,
    /// cloned or rewritten, and the operands an edit replaced: every value
    /// whose reads or definition an edit touched.
    pub named: IdSet<ValueId>,
    /// A terminator was added, erased or moved.
    pub terminators_made: bool,
    /// A terminator was rewritten (it may name the same blocks).
    pub terminators_rewritten: bool,
}

/// What a change reached: the blocks holding an instruction it touched, or one
/// that reads a value a touched instruction makes, or one that reads the value
/// such a reader makes, and so on; and of those the blocks whose branch, or
/// assume, or call, is one that reads a value the change made, which can alter
/// what holds below them.
#[derive(Clone, Debug, Default)]
pub struct Reach {
    pub blocks: IdSet<BlockId>,
    pub conditions: IdSet<BlockId>,
    /// A block was made or erased, or a terminator added, erased or moved.
    pub cfg: bool,
    /// A terminator was rewritten: the CFG is the same where it names the same
    /// blocks in the same order, which the caller checks.
    pub rewired: bool,
}

impl Reach {
    /// The CFG may differ from the one the mark saw.
    pub fn cfg_changed(&self) -> bool {
        self.cfg || self.rewired
    }
}

// Log entries and operands the digest's edge search has looked at, for a test
// that its work is linear in the edits.
#[cfg(test)]
thread_local! {
    pub(crate) static VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// Operands the digest has read to name the values an edit touched, for a test
// that `reach` does not pay for them.
#[cfg(test)]
thread_local! {
    pub(crate) static SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Touched {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// `self` with `pointers` found, as `context` types the values.
    #[must_use]
    pub fn with_pointers(
        mut self,
        function: &Function,
        context: &Context,
    ) -> Self {
        let wide = |value: ValueId| {
            !matches!(
                context.types.get(function.value(value).ty),
                Type::Int(_) | Type::Float(_) | Type::Void
            )
        };
        let mut pointers = IdSet::<ValueId>::new();
        for value in self.values_added.iter().chain(self.values_removed.iter()).chain(self.operands_changed.iter()) {
            if wide(value) {
                pointers.insert(value);
            }
        }
        let memory = self
            .memory_added
            .iter()
            .chain(self.memory_removed.iter())
            .chain(self.memory_moved.iter())
            .chain(self.memory_rewritten.iter());
        for inst in memory {
            let one = function.instruction(inst);
            for value in one.operands.iter().filter_map(|operand| match operand {
                Operand::Value(value) => Some(*value),
                _ => None,
            }) {
                if wide(value) {
                    pointers.insert(value);
                }
            }
            if let Some(result) = one.result.filter(|&result| wide(result)) {
                pointers.insert(result);
            }
        }
        self.pointers = pointers;
        self
    }

    /// What the touched instructions reach through the uses of the values they
    /// make (`Reach`), found in `function`.
    pub fn reach(
        &self,
        function: &Function,
    ) -> Reach {
        let mut blocks = self.blocks.clone();
        let mut conditions = IdSet::<BlockId>::new();
        let mut work: Vec<InstId> = self.insts.iter().collect();
        let mut seen: IdSet<InstId> = self.insts.clone();
        while let Some(inst) = work.pop() {
            let one = function.instruction(inst);
            if let Some(block) = function.parent(inst) {
                blocks.insert(block);
                if one.opcode.is_terminator() || matches!(one.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
                    conditions.insert(block);
                }
            }
            if let Some(result) = one.result {
                for user in function.users(result) {
                    if seen.insert(user.user) {
                        work.push(user.user);
                    }
                }
            }
        }
        Reach {
            blocks,
            conditions,
            cfg: !self.blocks_created.is_empty()
                || !self.blocks_erased.is_empty()
                || self.terminators_made
                || !self.cfg.is_empty(),
            rewired: self.terminators_rewritten,
        }
    }

    /// Whether a load, a store or a call was added, removed, moved or
    /// rewritten.
    pub fn memory(&self) -> bool {
        !(self.memory_added.is_empty()
            && self.memory_removed.is_empty()
            && self.memory_moved.is_empty()
            && self.memory_rewritten.is_empty())
    }
}

/// How many edits of each kind a function has had. Counters, never reset, and
/// no part of what a function is (two functions are equal whatever they
/// counted).
#[derive(Clone, Copy, Debug, Default)]
pub struct Epochs {
    /// A terminator or a block was made, erased, moved or rewritten.
    pub cfg: u64,
    /// A load, a store or a call was made, erased, moved or rewritten.
    pub memory: u64,
    /// An instruction that can make, move or name a pointer or an aggregate:
    /// memory, address arithmetic, casts, phis, selects.
    pub pointers: u64,
    /// A call or an invoke was made, erased, moved or rewritten.
    pub calls: u64,
}

impl PartialEq for Epochs {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Epochs {
    pub(crate) fn bump(
        &mut self,
        function: &Function,
        change: &Change,
    ) {
        let inst = match *change {
            Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => {
                self.cfg += 1;
                return;
            }
            Change::Operand { .. } | Change::Operands { .. } | Change::Substituted { .. } => return,
            Change::Inserted { inst, .. }
            | Change::Moved { inst, .. }
            | Change::Rewritten(inst)
            | Change::Erased { inst, .. } => inst,
            Change::Cloned { to, .. } => to,
        };
        let opcode = &function.instruction(inst).opcode;
        self.cfg += u64::from(opcode.is_terminator());
        self.memory += u64::from(touches_memory(opcode));
        self.calls += u64::from(matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_)));
        self.pointers += u64::from(touches_pointers(opcode));
    }
}

/// A load, a store or a call.
pub fn touches_memory(opcode: &Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
    )
}

/// What can make, move or name a pointer or an aggregate, whatever its type.
fn touches_pointers(opcode: &Opcode) -> bool {
    touches_memory(opcode)
        || matches!(
            opcode,
            Opcode::Alloca { .. }
                | Opcode::GetElementPtr { .. }
                | Opcode::Cast(_)
                | Opcode::Phi
                | Opcode::Select
                | Opcode::Freeze
                | Opcode::ExtractValue(_)
                | Opcode::InsertValue(_)
                | Opcode::LandingPad { .. }
                | Opcode::Resume
        )
}

impl Function {
    /// The digest of the changes since `mark`, or none where the history from
    /// there is gone (a mark of another function, or one the log has dropped).
    pub fn touched_since(
        &self,
        mark: Mark,
    ) -> Option<Touched> {
        Some(self.touched_by(self.changes_since(mark)?))
    }

    /// What `changes` reached, none where the parameters changed.
    pub fn reach_of(
        &self,
        changes: &[Change],
    ) -> Option<Reach> {
        let touched = self.touched_with(changes, false, false);
        (!touched.parameters).then(|| touched.reach(self))
    }

    /// The digest of `changes`, the log since some mark.
    pub fn touched_by(
        &self,
        changes: &[Change],
    ) -> Touched {
        self.touched_with(changes, true, true)
    }

    /// `touched_by`, with the edges added and removed (`cfg`) found only where
    /// asked: a consumer of `reach` needs to know that a terminator changed
    /// (`terminators_rewritten`), not which edges differ, and a switch of N
    /// arms rewritten for each of N edits costs N each time. The same goes for
    /// `named`, which reads every operand list an edit replaced: a consumer of
    /// `reach` does not read it, and `names` is false for it.
    fn touched_with(
        &self,
        changes: &[Change],
        edges: bool,
        names: bool,
    ) -> Touched {
        let mut out = Touched::default();
        let mut added = IdSet::<InstId>::new();
        let mut erased = IdSet::<InstId>::new();
        let mut moved = IdSet::<InstId>::new();
        let mut rewritten = IdSet::<InstId>::new();
        // Terminators seen, with the block each stood in at the mark.
        let mut terminators: Vec<(InstId, Option<BlockId>)> = Vec::new();
        let mut seen = IdSet::<InstId>::new();
        let mut note_terminator = |inst: InstId, at_mark: Option<BlockId>| {
            if self.instruction(inst).opcode.is_terminator() && seen.insert(inst) {
                terminators.push((inst, at_mark));
            }
        };
        for change in changes {
            match *change {
                Change::Inserted { inst, block, .. } => {
                    out.blocks.insert(block);
                    added.insert(inst);
                    note_terminator(inst, None);
                }
                Change::Cloned { to, .. } => {
                    if let Some(block) = self.parent(to) {
                        out.blocks.insert(block);
                    }
                    added.insert(to);
                    note_terminator(to, None);
                }
                Change::Moved { inst, block, from, .. } => {
                    out.blocks.insert(block);
                    out.blocks.insert(from);
                    moved.insert(inst);
                    note_terminator(inst, Some(from));
                }
                Change::Rewritten(inst) => {
                    if let Some(block) = self.parent(inst) {
                        out.blocks.insert(block);
                        note_terminator(inst, Some(block));
                    }
                    rewritten.insert(inst);
                    if let Some(result) = self.instruction(inst).result {
                        out.operands_changed.insert(result);
                    }
                }
                Change::Operand { inst, .. } | Change::Operands { inst, .. } => {
                    out.blocks.extend(self.parent(inst));
                    if let Some(result) = self.instruction(inst).result {
                        out.operands_changed.insert(result);
                    }
                    note_terminator(inst, self.parent(inst));
                }
                Change::Erased { inst, block, .. } => {
                    out.blocks.insert(block);
                    if !added.remove(&inst) {
                        erased.insert(inst);
                    }
                    note_terminator(inst, Some(block));
                }
                Change::BlockCreated(block) => {
                    out.blocks.insert(block);
                    out.blocks_created.push(block);
                }
                Change::BlockErased(block) => {
                    out.blocks.insert(block);
                    out.blocks_erased.push(block);
                }
                Change::Parameters => out.parameters = true,
                // What it changed is in the `Rewritten` of each user.
                Change::Substituted { .. } => {}
            }
        }
        // An instruction added and erased inside the window changed nothing.
        let ghosts: IdSet<InstId> = changes
            .iter()
            .filter_map(|one| match *one {
                Change::Erased { inst, .. } if !erased.contains(&inst) => Some(inst),
                _ => None,
            })
            .collect();
        for inst in added.iter() {
            let one = self.instruction(inst);
            if let Some(result) = one.result {
                out.values_added.insert(result);
                out.operands_changed.remove(&result);
            }
            if touches_memory(&one.opcode) {
                out.memory_added.insert(inst);
            }
        }
        for inst in erased.iter() {
            let one = self.instruction(inst);
            if let Some(result) = one.result {
                out.values_removed.insert(result);
                out.operands_changed.remove(&result);
            }
            if touches_memory(&one.opcode) {
                out.memory_removed.insert(inst);
            }
        }
        for inst in ghosts.iter() {
            if let Some(result) = self.instruction(inst).result {
                out.operands_changed.remove(&result);
            }
        }
        for inst in moved.iter().filter(|one| !added.contains(one) && !erased.contains(one)) {
            if touches_memory(&self.instruction(inst).opcode) {
                out.memory_moved.insert(inst);
            }
        }
        for inst in rewritten.iter().filter(|one| !added.contains(one) && !erased.contains(one)) {
            if touches_memory(&self.instruction(inst).opcode) {
                out.memory_rewritten.insert(inst);
            }
        }
        if !edges {
            terminators.clear();
        }
        // What each terminator named at the mark: its operands now, with the
        // replaced ones put back, in one pass over the log (not one per
        // terminator).
        // (A list by instruction, not a dense map: a map would be as long as
        // the function for the few terminators an edit touches.)
        let mut thens: Vec<(u32, Vec<Operand>)> =
            terminators.iter().map(|&(inst, _)| (inst.0, self.instruction(inst).operands.clone())).collect();
        thens.sort_unstable_by_key(|one| one.0);
        if !thens.is_empty() {
            let find = |thens: &mut Vec<(u32, Vec<Operand>)>, inst: InstId| {
                thens.binary_search_by_key(&inst.0, |one| one.0).ok()
            };
            for change in changes.iter().rev() {
                #[cfg(test)]
                VISITED.with(|visited| visited.set(visited.get() + 1));
                match change {
                    Change::Operand { inst, index, was } => {
                        if let Some(at) = find(&mut thens, *inst) {
                            thens[at].1[*index as usize] = *was;
                        }
                    }
                    Change::Operands { inst, was } => {
                        if let Some(at) = find(&mut thens, *inst) {
                            thens[at].1 = was.to_vec();
                        }
                    }
                    _ => {}
                }
            }
        }
        for (inst, at_mark) in terminators {
            let at = thens.binary_search_by_key(&inst.0, |one| one.0).expect("a terminator listed");
            self.edges_of(inst, at_mark, &thens[at].1, !added.contains(&inst), !self.is_erased(inst), &mut out.cfg);
        }
        // A terminator replaced by another naming the same block leaves the
        // edge: a pair both added and removed is in neither.
        let pair = |&(from, to, _): &(BlockId, BlockId, Edge)| (from.0, to.0);
        let mut made: Vec<(u32, u32)> = out.cfg.iter().filter(|one| one.2 == Edge::Added).map(pair).collect();
        let mut gone: Vec<(u32, u32)> = out.cfg.iter().filter(|one| one.2 == Edge::Removed).map(pair).collect();
        made.sort_unstable();
        gone.sort_unstable();
        out.cfg.retain(|one| {
            let key = pair(one);
            !(made.binary_search(&key).is_ok() && gone.binary_search(&key).is_ok())
        });
        let name_inst = |named: &mut IdSet<ValueId>, inst: InstId| {
            let one = self.instruction(inst);
            named.extend(one.result);
            for operand in &one.operands {
                #[cfg(test)]
                SCANNED.with(|scanned| scanned.set(scanned.get() + 1));
                if let Operand::Value(value) = operand {
                    named.insert(*value);
                }
            }
        };
        for change in changes {
            match change {
                Change::Inserted { inst, .. }
                | Change::Erased { inst, .. }
                | Change::Moved { inst, .. }
                | Change::Cloned { to: inst, .. } => {
                    out.terminators_made |= self.instruction(*inst).opcode.is_terminator();
                    if names {
                        name_inst(&mut out.named, *inst);
                    }
                }
                Change::Rewritten(inst) => {
                    out.terminators_rewritten |= self.instruction(*inst).opcode.is_terminator();
                    if names {
                        name_inst(&mut out.named, *inst);
                    }
                }
                Change::Operand { inst, was, .. } => {
                    out.terminators_rewritten |= self.instruction(*inst).opcode.is_terminator();
                    if let (true, Operand::Value(value)) = (names, was) {
                        out.named.insert(*value);
                    }
                }
                Change::Operands { inst, was } => {
                    out.terminators_rewritten |= self.instruction(*inst).opcode.is_terminator();
                    if names {
                        for operand in was.iter() {
                            #[cfg(test)]
                            SCANNED.with(|scanned| scanned.set(scanned.get() + 1));
                            if let Operand::Value(value) = operand {
                                out.named.insert(*value);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for inst in added.iter().chain(moved.iter()).chain(rewritten.iter()) {
            if !self.is_erased(inst) {
                out.insts.insert(inst);
            }
        }
        out
    }

    /// The edges terminator `inst` added and removed since the changes began:
    /// the blocks its operands named then against now.
    fn edges_of(
        &self,
        inst: InstId,
        at_mark: Option<BlockId>,
        then: &[Operand],
        existed: bool,
        exists: bool,
        out: &mut Vec<(BlockId, BlockId, Edge)>,
    ) {
        #[cfg(test)]
        VISITED.with(|visited| visited.set(visited.get() + then.len() + self.instruction(inst).operands.len()));
        // The blocks named, once each and in order: a switch of N arms names
        // N, which a scan of the list for each would make N^2.
        let targets = |operands: &[Operand]| -> (Vec<BlockId>, IdSet<BlockId>) {
            let mut seen = IdSet::<BlockId>::new();
            let list = operands
                .iter()
                .filter_map(|operand| if let Operand::Block(block) = operand { Some(*block) } else { None })
                .filter(|block| seen.insert(*block))
                .collect();
            (list, seen)
        };
        let before = at_mark.filter(|_| existed);
        let after = self.parent(inst).filter(|_| exists);
        let (old, old_set) = if before.is_some() { targets(then) } else { Default::default() };
        let (new, new_set) =
            if after.is_some() { targets(&self.instruction(inst).operands) } else { Default::default() };
        // From another block, every edge differs.
        let same = before == after;
        if let Some(from) = before {
            for &to in old.iter().filter(|to| !same || !new_set.contains(to)) {
                out.push((from, to, Edge::Removed));
            }
        }
        if let Some(from) = after {
            for &to in new.iter().filter(|to| !same || !old_set.contains(to)) {
                out.push((from, to, Edge::Added));
            }
        }
    }
}
