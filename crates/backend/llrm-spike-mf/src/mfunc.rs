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
    /// The registers a call destroys where that is fewer than the target's
    /// caller-saved set: what its callee is known to write (interprocedural
    /// register use, the pin's `RegUsageInfo`). `None` is the calling
    /// convention's own answer.
    pub call_clobbers: Option<std::sync::Arc<std::collections::BTreeSet<RegId>>>,
    /// `(def operand, use operand)` pairs that must be one register
    /// (`TIED_TO`).
    pub ties: Vec<(usize, usize)>,
    /// Ends a block (`isTerminator`).
    pub terminator: bool,
    /// Control never falls through it (`isBarrier`).
    pub barrier: bool,
    /// It sets the status flags a later instruction may read. Read it through
    /// [`Inst::sets_flags`]; write it here.
    pub sets_flags: bool,
    /// It was made again by the spiller and its original sets the flags, but
    /// `sets_flags` is clear so the reader after the original keeps the
    /// original's ([`Target::remade_sets_no_flags`]). The target restores the
    /// truth once it has placed the flag readers.
    pub remade_sets_flags: bool,
    /// It reads the status flags. Read it through [`Inst::reads_flags`].
    pub reads_flags: bool,
    /// `mayLoad`, `mayStore`, `hasUnmodeledSideEffects`, `isAsCheapAsAMove`.
    pub may_load: bool,
    pub may_store: bool,
    pub side_effects: bool,
    pub cheap_as_move: bool,
    /// A load of memory nothing writes (`isDereferenceableInvariantLoad`):
    /// loop-invariant when its address is.
    pub invariant_load: bool,
    /// What its load or store reaches (`MachineMemOperand`), where the target
    /// knows; none: it may reach anything.
    pub memory: Option<llrm_codegen::model::memref::MemRef>,
    /// The spiller may run it again at each use instead of keeping its result
    /// (`isTriviallyReMaterializable`).
    pub rematerializable: bool,
    /// It writes a result before it has read every use, so no def may share
    /// a register with a use (`earlyclobber`).
    pub early_clobber: bool,
    /// Its two register sources may swap places (`isCommutable`).
    pub commutable: bool,
    /// The caller's number for the operation it was made from (the `DebugLoc`
    /// a `MachineInstr` carries); 0 for none. A clone keeps it, code the
    /// allocator adds has none. A target that lists its source places in
    /// [`MFunction::places`] numbers them from 1.
    pub origin: u32,
    /// The instruction as the target's selector names it (the pin's opcode and
    /// its operands), for the pre-RA scheduler's model; none where the target
    /// does not say. The allocator reads `kind` and `operands` only.
    pub form: Option<std::sync::Arc<llrm_lir::Semantics>>,
    /// `operands` as they were when `form` was made: later passes rewrite
    /// registers (a hoisted constant replaces a use), and the form still
    /// names the old ones.
    pub form_operands: Vec<Operand>,
}

impl Inst {
    /// It kills every flag lane at once: a later reader of any lane sees this
    /// instruction's result, never an earlier setter's. A setter that leaves a
    /// lane alone (an `inc` that keeps carry) must say so here, in this
    /// accessor, before a pass may rely on the lane surviving it: the pin's
    /// `optimizeCompareInstr` stops at the first EFLAGS modifier for the same
    /// reason (#9532).
    pub fn sets_flags(&self) -> bool {
        self.sets_flags
    }

    /// It reads the flags as a whole: any lane may be the one it needs, so no
    /// setter may be moved or removed across it.
    pub fn reads_flags(&self) -> bool {
        self.reads_flags
    }

    pub fn new(
        kind: Kind,
        operands: Vec<Operand>,
    ) -> Inst {
        Inst {
            kind,
            operands,
            call: false,
            call_clobbers: None,
            ties: Vec::new(),
            terminator: false,
            barrier: false,
            sets_flags: false,
            remade_sets_flags: false,
            reads_flags: false,
            may_load: false,
            may_store: false,
            side_effects: false,
            cheap_as_move: false,
            invariant_load: false,
            memory: None,
            rematerializable: false,
            early_clobber: false,
            commutable: false,
            origin: 0,
            form: None,
            form_operands: Vec::new(),
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

impl Block {
    /// An instruction with no place that defines a register a later one reads
    /// (a constant, a materialised address) takes the place of the first of
    /// those readers, the user it was made for (the pin emits a node with its
    /// user's `DebugLoc`). Any other stays at place 0: a bare jump, an edge
    /// copy, what follows the last located instruction. Line 0 is the pin's
    /// row for it, which is the format's to write (or to leave to the row
    /// before it).
    pub fn place_unlocated(&mut self) {
        let mut reader: llrm_support::hash::HashMap<Reg, u32> = Default::default();
        for inst in self.insts.iter_mut().rev() {
            if inst.origin == 0 {
                inst.origin = inst
                    .operands
                    .iter()
                    .find_map(|operand| match operand {
                        Operand::Def(reg) => reader.get(reg).copied(),
                        _ => None,
                    })
                    .unwrap_or(0);
            }
            for operand in &inst.operands {
                match operand {
                    Operand::Def(reg) => {
                        reader.remove(reg);
                    }
                    Operand::Use(reg) if inst.origin != 0 => {
                        reader.insert(*reg, inst.origin);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A source variable the debug info names, and where it lives for the whole
/// function: the channel `-g` formats read, whatever the target.
#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    /// Its `!DIType` node, as the module the function was selected from has it.
    pub r#type: llrm_mir::module::MetadataId,
    /// The `!DIExpression` operations of the location, as written.
    pub expr: Vec<llrm_mir::module::DwOp>,
    pub parameter: bool,
    pub place: VariablePlace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariablePlace {
    /// The register an argument arrives in.
    Register(RegId),
    /// Frame object `index`, `offset` bytes into it.
    Frame { index: i32, offset: i64 },
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
    /// The function's own stack objects (static allocas): frame index, size
    /// and alignment in bytes.
    pub locals: Vec<(i32, i64, i64)>,
    /// The function is built for size (`shouldOptimizeForSize`).
    pub optimize_for_size: bool,
    /// The function is `optnone`: the pin's MachineScheduler skips it
    /// (`skipFunction`).
    pub optnone: bool,
    /// The function calls a routine that returns twice (`setjmp`): the
    /// coalescer leaves it alone (`MachineFunction::exposesReturnsTwice`).
    pub returns_twice: bool,
    /// The source places the instructions' [`Inst::origin`]s number, from 1.
    pub places: Vec<llrm_codegen::model::lir::SrcLoc>,
    /// `-g`: the source variables and where each is.
    pub variables: Vec<Variable>,
    /// The IR's edge probabilities by `(from, to)` block id, where the target
    /// states them; `None` leaves the allocator the loop branch heuristic.
    /// An edge a pass made is absent and takes its share of what is left.
    pub odds: Option<crate::placement::Odds>,
}

impl<C> MFunction<C> {
    /// [`Block::place_unlocated`] for every block.
    pub fn place_unlocated(&mut self) {
        self.blocks.iter_mut().for_each(Block::place_unlocated);
    }

    pub fn new(name: &str) -> MFunction<C> {
        MFunction {
            name: name.to_owned(),
            blocks: Vec::new(),
            classes: Vec::new(),
            fixed: BTreeMap::new(),
            locals: Vec::new(),
            optimize_for_size: false,
            optnone: false,
            returns_twice: false,
            places: Vec::new(),
            variables: Vec::new(),
            odds: None,
        }
    }

    /// `from`'s edges now leave `to` (the block that took over its branch):
    /// the odds move with them.
    pub fn move_odds(
        &mut self,
        from: u32,
        to: u32,
    ) {
        let Some(odds) = &mut self.odds else { return };
        let moved: Vec<_> = odds.keys().filter(|(source, _)| *source == from).copied().collect();
        for key in moved {
            let one = odds.remove(&key).expect("a key just listed");
            odds.insert((to, key.1), one);
        }
    }

    /// `middle` is a block on the edge `from` -> `to`: the edge's odds are
    /// `from` -> `middle`'s, and `middle` always goes on.
    pub fn split_odds(
        &mut self,
        from: u32,
        to: u32,
        middle: u32,
    ) {
        let Some(odds) = &mut self.odds else { return };
        let Some((probability, index)) = odds.remove(&(from, to)) else { return };
        odds.insert((from, middle), (probability, index));
        odds.insert((middle, to), (llrm_mir::blockfrequency::BranchProbability::get_one(), 0));
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
