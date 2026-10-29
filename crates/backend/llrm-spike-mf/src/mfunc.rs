//! The machine function: instructions on virtual registers, in blocks.
//!
//! An instruction's operands are its defined registers first, then its uses
//! and other values. A phi is the successor block's [`Phi`]: the register it
//! defines, and per predecessor block the register that block copied its
//! value to.

use std::collections::BTreeMap;

use llrm_lir::registers::RegId;

/// A register: virtual, numbered in [`MFunction::classes`], or physical.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Reg {
    Virt(u32),
    Phys(RegId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operand {
    Def(Reg),
    Use(Reg),
    Imm(i64),
    Symbol(String, i64),
    Frame(i32),
    Block(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// The pin's instruction by name.
    Op(&'static str),
    /// `COPY dst, src`.
    Copy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inst {
    pub kind: Kind,
    pub operands: Vec<Operand>,
    pub call: bool,
    /// `(def operand, use operand)` pairs that must be one register
    /// (`TIED_TO`).
    pub ties: Vec<(usize, usize)>,
    /// Ends a block (`isTerminator`).
    pub terminator: bool,
    /// Control never falls through it (`isBarrier`).
    pub barrier: bool,
    /// It sets the status flags a later instruction may read.
    pub sets_flags: bool,
    /// It reads the status flags.
    pub reads_flags: bool,
    /// `mayLoad`, `mayStore`, `hasUnmodeledSideEffects`, `isAsCheapAsAMove`.
    pub may_load: bool,
    pub may_store: bool,
    pub side_effects: bool,
    pub cheap_as_move: bool,
    /// It writes a result before it has read every use, so no def may share
    /// a register with a use (`earlyclobber`).
    pub early_clobber: bool,
}

impl Inst {
    pub fn new(
        kind: Kind,
        operands: Vec<Operand>,
    ) -> Inst {
        Inst {
            kind,
            operands,
            call: false,
            ties: Vec::new(),
            terminator: false,
            barrier: false,
            sets_flags: false,
            reads_flags: false,
            may_load: false,
            may_store: false,
            side_effects: false,
            cheap_as_move: false,
            early_clobber: false,
        }
    }

    /// `COPY dst, src`.
    pub fn copy(
        dst: Reg,
        src: Reg,
    ) -> Inst {
        Inst::new(Kind::Copy, vec![Operand::Def(dst), Operand::Use(src)])
    }
}

/// `PHI result, (incoming, predecessor)..`: a predecessor is a block index.
#[derive(Clone, Debug)]
pub struct Phi {
    pub result: Reg,
    pub incoming: Vec<(usize, Reg)>,
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    /// The IR block's number: what a [`Operand::Block`] names.
    pub id: u32,
    pub insts: Vec<Inst>,
    pub phis: Vec<Phi>,
}

/// `C` is the target's register class.
#[derive(Clone, Debug)]
pub struct MFunction<C> {
    pub name: String,
    pub blocks: Vec<Block>,
    /// The class of each virtual register.
    pub classes: Vec<C>,
    /// Each incoming-argument frame index and its offset from the stack
    /// pointer at entry.
    pub fixed: BTreeMap<i32, i64>,
}

impl<C> MFunction<C> {
    pub fn new(name: &str) -> MFunction<C> {
        MFunction { name: name.to_owned(), blocks: Vec::new(), classes: Vec::new(), fixed: BTreeMap::new() }
    }

    /// Lays `block` out at `at`; the phis name predecessors by index, so those
    /// from `at` on move down one.
    pub fn insert_block(
        &mut self,
        at: usize,
        block: Block,
    ) {
        for other in &mut self.blocks {
            for phi in &mut other.phis {
                for (pred, _) in &mut phi.incoming {
                    if *pred >= at {
                        *pred += 1;
                    }
                }
            }
        }
        self.blocks.insert(at, block);
    }

    /// The number a block added now gets for its [`Block::id`].
    pub fn fresh_block_id(&self) -> u32 {
        self.blocks.iter().map(|block| block.id + 1).max().unwrap_or(0)
    }
}
