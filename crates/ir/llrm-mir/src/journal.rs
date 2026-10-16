//! Speculation without a copy: `checkpoint` marks the function, `rollback`
//! puts it back as it was (equal, use lists in their order), `release` keeps
//! what was done.
//!
//! While a checkpoint is open, the first write to a part of the function
//! (an instruction, a block, a use list, the layout) saves what the part
//! held; nothing is saved when none is open. A rollback truncates the arenas
//! to their length at the checkpoint, writes the saved parts back and cuts
//! the change log to the checkpoint, so that the edits it undid leave no
//! trace for a consumer (LLVM's `Rewriter` undo, GCC's `cfg_hooks` snapshots
//! do the same by saving what a pass changes). Marks taken past the
//! checkpoint stop resolving (`Lineage::cuts`). The rare edits that change
//! more than the parts (`renumber_metadata`, `delete_body`, the parameter
//! setters) save the whole function.

use crate::dense::IdMap;
use crate::module::{Block, BlockId, DebugRecord, Function, InstId, Instruction, MetadataId, Operand, Use};

/// What the first write after a checkpoint saved.
#[derive(Default)]
struct Frame {
    /// The arenas' lengths: what is past them was made after the checkpoint.
    instructions: usize,
    values: usize,
    blocks: usize,
    /// Where the log stood, and what `take_changes` had handed out.
    logged: usize,
    taken: usize,
    saved_instructions: IdMap<u32, (Instruction, Option<BlockId>, bool)>,
    saved_blocks: IdMap<u32, Block>,
    saved_value_uses: IdMap<u32, Vec<Use>>,
    saved_block_uses: IdMap<u32, Vec<Use>>,
    layout: Option<Vec<BlockId>>,
    debug: Option<(Vec<DebugRecord>, Vec<MetadataId>)>,
    /// The whole function, for an edit that changes more than the parts.
    whole: Option<Box<Function>>,
    /// `LLRM_CHECK_ROLLBACK`: the function as it was, which a rollback must
    /// give back (an edit that bypassed the editor does not).
    check: Option<Box<Function>>,
}

/// The edits a rollback undid, to make again.
#[derive(Default)]
pub struct Redo {
    changes: Vec<crate::module::Change>,
    instructions: IdMap<u32, (Instruction, Option<BlockId>, bool)>,
    blocks: IdMap<u32, Block>,
    value_uses: IdMap<u32, Vec<Use>>,
    block_uses: IdMap<u32, Vec<Use>>,
    layout: Option<Vec<BlockId>>,
    debug: Option<(Vec<DebugRecord>, Vec<MetadataId>)>,
    instruction_tail: Vec<Instruction>,
    parent_tail: Vec<Option<BlockId>>,
    erased_tail: Vec<bool>,
    value_tail: Vec<crate::module::ValueData>,
    value_uses_tail: Vec<Vec<Use>>,
    block_tail: Vec<Block>,
    block_uses_tail: Vec<Vec<Use>>,
    whole: Option<Box<Function>>,
}

/// The open checkpoints, innermost last. Not part of what a function is.
#[derive(Default)]
pub(crate) struct Journal {
    frames: Vec<Frame>,
}

impl Clone for Journal {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for Journal {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(out, "Journal({} open)", self.frames.len())
    }
}

impl PartialEq for Journal {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

/// An open checkpoint of one function.
#[derive(Debug)]
#[must_use = "a checkpoint is rolled back or released"]
pub struct Checkpoint {
    uid: u64,
    depth: usize,
}

impl Function {
    /// Starts recording what is done from here, to `rollback` or `release`.
    pub fn checkpoint(&mut self) -> Checkpoint {
        let frame = Frame {
            instructions: self.instructions.len(),
            values: self.values.len(),
            blocks: self.blocks.len(),
            logged: self.lineage.logged,
            taken: self.taken,
            check: checked().then(|| {
                let mut copy = self.clone();
                copy.journal = Journal::default();
                Box::new(copy)
            }),
            ..Frame::default()
        };
        self.journal.frames.push(frame);
        Checkpoint { uid: self.lineage.uid, depth: self.journal.frames.len() }
    }

    /// Puts the function back as it was at `checkpoint`, and any checkpoint
    /// opened since.
    pub fn rollback(
        &mut self,
        checkpoint: Checkpoint,
    ) {
        drop(self.undo(checkpoint, false));
    }

    /// `rollback`, giving the edits undone back as a [`Redo`] to make again:
    /// for a speculation tried and set aside, the other tried, and the first
    /// found to be the one to keep.
    pub fn rollback_keeping(
        &mut self,
        checkpoint: Checkpoint,
    ) -> Redo {
        self.undo(checkpoint, true).expect("asked for")
    }

    fn undo(
        &mut self,
        checkpoint: Checkpoint,
        keep: bool,
    ) -> Option<Redo> {
        assert!(
            checkpoint.uid == self.lineage.uid && checkpoint.depth <= self.journal.frames.len(),
            "a checkpoint of this function, still open"
        );
        self.journal.frames.truncate(checkpoint.depth);
        let frame = self.journal.frames.pop().expect("an open checkpoint");
        let mut redo = keep.then(|| Redo { changes: self.changes_from(frame.logged), ..Redo::default() });
        if let Some(whole) = frame.whole {
            if let Some(redo) = redo.as_mut() {
                let mut after = self.clone();
                after.journal = Journal::default();
                redo.whole = Some(Box::new(after));
            }
            self.restore_whole(*whole);
        } else {
            if let Some(redo) = redo.as_mut() {
                redo.instructions = frame
                    .saved_instructions
                    .keys()
                    .map(|at| {
                        let i = at as usize;
                        (at, (self.instructions[i].clone(), self.parent[i], self.erased[i]))
                    })
                    .collect();
                redo.blocks = frame.saved_blocks.keys().map(|at| (at, self.blocks[at as usize].clone())).collect();
                redo.value_uses =
                    frame.saved_value_uses.keys().map(|at| (at, self.value_uses[at as usize].clone())).collect();
                redo.block_uses =
                    frame.saved_block_uses.keys().map(|at| (at, self.block_uses[at as usize].clone())).collect();
                redo.layout = frame.layout.as_ref().map(|_| self.layout.clone());
                redo.debug = frame.debug.as_ref().map(|_| (self.debug_records.clone(), self.debug_dropped.clone()));
                redo.instruction_tail = self.instructions[frame.instructions..].to_vec();
                redo.parent_tail = self.parent[frame.instructions..].to_vec();
                redo.erased_tail = self.erased[frame.instructions..].to_vec();
                redo.value_tail = self.values[frame.values..].to_vec();
                redo.value_uses_tail = self.value_uses[frame.values..].to_vec();
                redo.block_tail = self.blocks[frame.blocks..].to_vec();
                redo.block_uses_tail = self.block_uses[frame.blocks..].to_vec();
            }
            self.instructions.truncate(frame.instructions);
            self.parent.truncate(frame.instructions);
            self.erased.truncate(frame.instructions);
            self.values.truncate(frame.values);
            self.value_uses.truncate(frame.values);
            self.blocks.truncate(frame.blocks);
            self.block_uses.truncate(frame.blocks);
            for (at, (instruction, parent, erased)) in frame.saved_instructions.iter() {
                let at = at as usize;
                self.instructions[at] = instruction.clone();
                self.parent[at] = *parent;
                self.erased[at] = *erased;
            }
            for (at, block) in frame.saved_blocks.iter() {
                self.blocks[at as usize] = block.clone();
            }
            for (at, uses) in frame.saved_value_uses.iter() {
                self.value_uses[at as usize] = uses.clone();
            }
            for (at, uses) in frame.saved_block_uses.iter() {
                self.block_uses[at as usize] = uses.clone();
            }
            if let Some(layout) = frame.layout {
                self.layout = layout;
            }
            if let Some((records, dropped)) = frame.debug {
                self.debug_records = records;
                self.debug_dropped = dropped;
            }
        }
        // Names made since are in the index, and names given up are not.
        self.names = Default::default();
        self.cut_log(frame.logged, frame.taken);
        if let Some(was) = frame.check {
            assert!(*self == *was, "LLRM_CHECK_ROLLBACK: the rollback did not give the function back as it was");
        }
        redo
    }

    /// Makes again the edits `redo` has, on the function as it was when they
    /// were undone.
    pub fn redo(
        &mut self,
        redo: Redo,
    ) {
        assert!(self.journal.frames.is_empty() || redo.whole.is_none(), "no whole-function redo inside a checkpoint");
        if let Some(whole) = redo.whole {
            // The log the undone edits made goes back with them.
            let mut after = *whole;
            after.lineage = std::mem::take(&mut self.lineage);
            after.changes = std::mem::take(&mut self.changes);
            after.epochs = self.epochs;
            after.taken = self.taken;
            after.journal = std::mem::take(&mut self.journal);
            *self = after;
            for change in redo.changes {
                self.log(change);
            }
        } else {
            for (at, (instruction, parent, erased)) in redo.instructions.iter() {
                self.save_instruction(InstId(at));
                let at = at as usize;
                self.instructions[at] = instruction.clone();
                self.parent[at] = *parent;
                self.erased[at] = *erased;
            }
            for (at, block) in redo.blocks.iter() {
                self.save_block(BlockId(at));
                self.blocks[at as usize] = block.clone();
            }
            for (at, uses) in redo.value_uses.iter() {
                self.save_uses(Operand::Value(crate::module::ValueId(at)));
                self.value_uses[at as usize] = uses.clone();
            }
            for (at, uses) in redo.block_uses.iter() {
                self.save_uses(Operand::Block(BlockId(at)));
                self.block_uses[at as usize] = uses.clone();
            }
            if let Some(layout) = redo.layout {
                self.save_layout();
                self.layout = layout;
            }
            if let Some((records, dropped)) = redo.debug {
                self.save_debug();
                self.debug_records = records;
                self.debug_dropped = dropped;
            }
            self.instructions.extend(redo.instruction_tail);
            self.parent.extend(redo.parent_tail);
            self.erased.extend(redo.erased_tail);
            self.values.extend(redo.value_tail);
            self.value_uses.extend(redo.value_uses_tail);
            self.blocks.extend(redo.block_tail);
            self.block_uses.extend(redo.block_uses_tail);
            self.names = Default::default();
            for change in redo.changes {
                self.log(change);
            }
        }
    }

    /// Keeps what was done since `checkpoint`.
    pub fn release(
        &mut self,
        checkpoint: Checkpoint,
    ) {
        assert!(
            checkpoint.uid == self.lineage.uid && checkpoint.depth <= self.journal.frames.len(),
            "a checkpoint of this function, still open"
        );
        // An outer checkpoint saved each part at its own first write, so
        // what this one saved is not needed.
        self.journal.frames.truncate(checkpoint.depth - 1);
    }

    fn restore_whole(
        &mut self,
        mut whole: Function,
    ) {
        whole.lineage = std::mem::take(&mut self.lineage);
        whole.changes = std::mem::take(&mut self.changes);
        whole.epochs = self.epochs;
        whole.taken = self.taken;
        whole.journal = std::mem::take(&mut self.journal);
        *self = whole;
    }

    /// The saves each open checkpoint lacks, made before the part is written.
    fn each_frame(
        &mut self,
        mut save: impl FnMut(&mut Frame, &Function),
    ) {
        if self.journal.frames.is_empty() {
            return;
        }
        let mut frames = std::mem::take(&mut self.journal.frames);
        for frame in &mut frames {
            save(frame, self);
        }
        self.journal.frames = frames;
    }

    pub(crate) fn save_instruction(
        &mut self,
        inst: InstId,
    ) {
        self.each_frame(|frame, this| {
            let at = inst.0;
            if (at as usize) < frame.instructions && !frame.saved_instructions.contains_key(&at) {
                frame
                    .saved_instructions
                    .insert(
                        at,
                        (this.instructions[at as usize].clone(), this.parent[at as usize], this.erased[at as usize]),
                    );
            }
        });
    }

    pub(crate) fn save_block(
        &mut self,
        block: BlockId,
    ) {
        self.each_frame(|frame, this| {
            let at = block.0;
            if (at as usize) < frame.blocks && !frame.saved_blocks.contains_key(&at) {
                frame.saved_blocks.insert(at, this.blocks[at as usize].clone());
            }
        });
    }

    /// The use list `operand` names, if it has one.
    pub(crate) fn save_uses(
        &mut self,
        operand: Operand,
    ) {
        match operand {
            Operand::Value(value) => self.each_frame(|frame, this| {
                let at = value.0;
                if (at as usize) < frame.values && !frame.saved_value_uses.contains_key(&at) {
                    frame.saved_value_uses.insert(at, this.value_uses[at as usize].clone());
                }
            }),
            Operand::Block(block) => self.each_frame(|frame, this| {
                let at = block.0;
                if (at as usize) < frame.blocks && !frame.saved_block_uses.contains_key(&at) {
                    frame.saved_block_uses.insert(at, this.block_uses[at as usize].clone());
                }
            }),
            Operand::Constant(_) => {}
        }
    }

    pub(crate) fn save_layout(&mut self) {
        self.each_frame(|frame, this| {
            if frame.layout.is_none() {
                frame.layout = Some(this.layout.clone());
            }
        });
    }

    pub(crate) fn save_debug(&mut self) {
        self.each_frame(|frame, this| {
            if frame.debug.is_none() {
                frame.debug = Some((this.debug_records.clone(), this.debug_dropped.clone()));
            }
        });
    }

    /// For an edit that changes more than the parts: the function as it is.
    pub(crate) fn save_whole(&mut self) {
        self.each_frame(|frame, this| {
            if frame.whole.is_none() {
                let mut whole = this.clone();
                whole.journal = Journal::default();
                frame.whole = Some(Box::new(whole));
            }
        });
    }
}

thread_local! {
    static CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts the clones of a function: the cost speculation by checkpoint is
/// there to avoid. Part of no function's value.
#[derive(Debug, Default)]
pub(crate) struct CloneWitness;

impl Clone for CloneWitness {
    fn clone(&self) -> Self {
        CLONES.with(|count| count.set(count.get() + 1));
        Self
    }
}

impl PartialEq for CloneWitness {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

/// How many times this thread has cloned a function, for a test
/// that a pass prices a change without a copy.
pub fn clones() -> usize {
    CLONES.with(std::cell::Cell::get)
}

/// `LLRM_CHECK_ROLLBACK` is set.
fn checked() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LLRM_CHECK_ROLLBACK").is_some())
}
