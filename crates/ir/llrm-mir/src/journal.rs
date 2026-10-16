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
        assert!(
            checkpoint.uid == self.lineage.uid && checkpoint.depth <= self.journal.frames.len(),
            "a checkpoint of this function, still open"
        );
        self.journal.frames.truncate(checkpoint.depth);
        let frame = self.journal.frames.pop().expect("an open checkpoint");
        if let Some(whole) = frame.whole {
            self.restore_whole(*whole);
        } else {
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
