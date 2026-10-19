//! Modules, global values, functions, blocks and instructions, as LLVM has
//! them. A function's values, instructions and blocks live in arenas whose
//! ids are never reused; `layout` and each block's list give the order.

use crate::context::{ConstantId, Context, GlobalId};
use crate::opcode::{Attribute, Flags, Opcode};
use crate::types::{Type, TypeId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub u32);

impl crate::dense::Dense for ValueId {
    fn index(self) -> usize {
        self.0 as usize
    }
    fn at(index: usize) -> Self {
        Self(u32::try_from(index).expect("an id fits u32"))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);

impl crate::dense::Dense for BlockId {
    fn index(self) -> usize {
        self.0 as usize
    }
    fn at(index: usize) -> Self {
        Self(u32::try_from(index).expect("an id fits u32"))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstId(pub u32);

impl crate::dense::Dense for InstId {
    fn index(self) -> usize {
        self.0 as usize
    }
    fn at(index: usize) -> Self {
        Self(u32::try_from(index).expect("an id fits u32"))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MetadataId(pub u32);

/// An instruction's operand: a local value, a constant (a global value is
/// one), or a block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Operand {
    Value(ValueId),
    Constant(ConstantId),
    Block(BlockId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDef {
    Argument(u32),
    Instruction(InstId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueData {
    pub ty: TypeId,
    pub name: Option<String>,
    pub def: ValueDef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    /// The result's type; `void` when there is none.
    pub ty: TypeId,
    pub operands: Vec<Operand>,
    pub flags: Flags,
    pub result: Option<ValueId>,
    pub metadata: Vec<(String, MetadataId)>,
}

/// A basic block. Its instructions are a doubly linked list through the
/// function's `link` arena (LLVM's `simple_ilist`): the block holds the first,
/// the last and the count, so an insert, an erase or a move at a known
/// instruction is O(1). Read it with `Function::iter_block`, `first`, `last`
/// and `next_in_block`; no `Vec` of it is kept.
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub name: Option<String>,
    /// The first and last instruction, as an id + 1 (0: none).
    pub(crate) head: u32,
    pub(crate) tail: u32,
    pub(crate) len: u32,
    pub(crate) erased: bool,
    /// In the function's layout.
    pub(crate) placed: bool,
}

/// Two blocks are equal by what they hold, not by whether their view is built.
impl PartialEq for Block {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.name == other.name
            && self.head == other.head
            && self.tail == other.tail
            && self.len == other.len
            && self.erased == other.erased
            && self.placed == other.placed
    }
}

impl Block {
    /// `BasicBlock::front`.
    pub fn first(&self) -> Option<InstId> {
        (self.head != 0).then(|| InstId(self.head - 1))
    }

    /// `BasicBlock::back`.
    pub fn last(&self) -> Option<InstId> {
        (self.tail != 0).then(|| InstId(self.tail - 1))
    }

    /// `BasicBlock::size`.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// `BasicBlock::empty`.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// A block read through its function, so that its instructions can be listed:
/// `first`, `last`, `len`, `is_empty` and the fields read the block as it is;
/// `instructions` and `iter` build the view from the links on the first ask
/// after an edit (and count the rebuild as a scan).
pub struct BlockRef<'a> {
    function: &'a Function,
    block: &'a Block,
}

impl std::ops::Deref for BlockRef<'_> {
    type Target = Block;

    fn deref(&self) -> &Block {
        self.block
    }
}

/// Each instruction's place in its block, numbered lazily like LLVM's
/// `Instruction::Order` (`BasicBlock::renumberInstructions`): an edit of a
/// block invalidates it, the next ask renumbers the block once. Not part of
/// what a function is: a copy starts with none and two functions compare equal
/// whatever they hold.
#[derive(Default)]
pub(crate) struct OrderCache(pub(crate) std::cell::RefCell<OrderNumbers>);

#[derive(Default)]
pub(crate) struct OrderNumbers {
    /// By instruction.
    pub(crate) at: Vec<u32>,
    /// By block: whether the numbers of its instructions are true.
    pub(crate) valid: Vec<bool>,
}

impl OrderCache {
    pub(crate) fn invalidate(
        &self,
        block: BlockId,
    ) {
        let mut numbers = self.0.borrow_mut();
        if let Some(one) = numbers.valid.get_mut(block.0 as usize) {
            *one = false;
        }
    }
}

impl Clone for OrderCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for OrderCache {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.write_str("OrderCache")
    }
}

impl PartialEq for OrderCache {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

/// What a debugger is told of a source variable, from a point of the code on:
/// where it lives in memory, what it is, or that nothing says. Not an
/// instruction: no pass counts it, and a use of a value in one keeps nothing
/// alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugWhat {
    /// The variable is in the memory this address names, from here on
    /// (`llvm.dbg.declare`).
    Declare(Operand),
    /// The variable is this value, from here until the next record of the
    /// variable (`llvm.dbg.value`).
    Value(Operand),
    /// `bytes` of the variable from byte `offset` are this value, from here on
    /// (`DW_OP_piece`): an aggregate the optimiser split.
    Piece { value: Operand, offset: u32, bytes: u32 },
    /// Nothing says where those bytes are: their value was deleted.
    GonePiece { offset: u32, bytes: u32 },
    /// Nothing says where it is: its value was deleted.
    Gone,
}

/// One such statement, standing before the instruction it names. The variable
/// is a node of [`debuginfo`](crate::debuginfo).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugRecord {
    pub before: InstId,
    pub variable: MetadataId,
    pub what: DebugWhat,
}

/// An operand slot: which instruction, and which of its operands.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Use {
    pub user: InstId,
    pub index: u32,
}

/// What a mutation did, in order, for the rewrite ledger. Positions are
/// where the instruction was or went: before `next` in `block`, or at its
/// end when `next` is `None`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
    Inserted {
        inst: InstId,
        block: BlockId,
        next: Option<InstId>,
    },
    Cloned {
        from: InstId,
        to: InstId,
    },
    Moved {
        inst: InstId,
        block: BlockId,
        next: Option<InstId>,
        from: BlockId,
    },
    Rewritten(InstId),
    /// Every use of `value` is about to become an operand that is the same
    /// value (`replace_value`: a common subexpression, a load's stored value, a
    /// constant folded to), each as an `Operand` entry then a `Rewritten`
    /// one. What an analysis said of any value stays true.
    Substituted {
        from: ValueId,
        to: Operand,
    },
    /// Operand `index` of `inst` was `was`: logged before the `Rewritten` of
    /// the change, by what reads what an instruction read before (the
    /// liveness of a value it stopped reading, the edge a branch stopped
    /// taking).
    Operand {
        inst: InstId,
        index: u32,
        was: Operand,
    },
    /// The operands of `inst` were `was`, before the `Rewritten` that follows.
    Operands {
        inst: InstId,
        was: Box<[Operand]>,
    },
    Erased {
        inst: InstId,
        block: BlockId,
        next: Option<InstId>,
    },
    BlockCreated(BlockId),
    BlockErased(BlockId),
    /// Parameters were inserted or removed: what a body says of its arguments
    /// (a parameter's index, the function's type) is other than it was, though
    /// no instruction is.
    Parameters,
}

/// A function's log of changes. A copy is another function (`Lineage`), whose
/// edits from then on are its own: it starts with an empty log, as a copy that
/// carried the original's (up to 64k changes, cloned for each numbering of a
/// body) cost 16% of compiling a program with a hundred inlines. The log is no
/// part of what a function is, so two functions are equal whatever they logged.
#[derive(Debug, Default)]
pub(crate) struct ChangeLog(pub(crate) Vec<Change>);

impl Clone for ChangeLog {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for ChangeLog {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

thread_local! {
    static STAMPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has hashed a function's attributes, for a test
/// that the manager does it once per invalidation, not once per result.
pub fn attribute_stamps() -> usize {
    STAMPS.with(std::cell::Cell::get)
}

static NEXT_LINEAGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Which function a log of changes belongs to, and how many changes it has
/// held: `take_changes` drains the log and leaves the count. A copy of a
/// function is another one, as its edits from then on are its own.
#[derive(Debug)]
pub(crate) struct Lineage {
    uid: u64,
    /// Which fork of the history this is (`Function::fork` makes another).
    branch: u64,
    /// The branches this one forked from, each with the position it forked at:
    /// a mark in one of them, at or before that position, is a point in this
    /// history too.
    ancestors: Vec<(u64, usize)>,
    logged: usize,
}

impl Default for Lineage {
    fn default() -> Self {
        let uid = NEXT_LINEAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { uid, branch: uid, ancestors: Vec::new(), logged: 0 }
    }
}

impl Clone for Lineage {
    fn clone(&self) -> Self {
        crate::counts::bump(crate::counts::Event::FunctionClones);
        let uid = NEXT_LINEAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { uid, branch: uid, ancestors: Vec::new(), logged: self.logged }
    }
}

/// Equal whatever the history: two functions with the same body are equal.
impl PartialEq for Lineage {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

/// A point in one function's history, to ask what changed since.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Mark {
    uid: u64,
    /// The fork of the function this point is in: two forks edited apart share
    /// a `uid` and can both log a change at one `at`, and are not one point.
    branch: u64,
    at: usize,
}

impl Mark {
    /// How many changes the function had logged at this point.
    pub fn position(&self) -> usize {
        self.at
    }

    /// The function's history this is a point in: equal for the points of one
    /// function, other for a copy of it, which has its own.
    pub fn lineage(&self) -> u64 {
        self.uid
    }

    /// Whether `other` is a point in the same function's history: not a copy
    /// that took its own.
    pub fn same_history(
        self,
        other: Mark,
    ) -> bool {
        self.uid == other.uid
    }
}

/// A function: its values, instructions and blocks in arenas whose ids are
/// never reused, their use lists, and the log of what changed. The arenas
/// are private so that every change goes through `edit`, which keeps the
/// use lists true.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    /// The function type.
    pub ty: TypeId,
    pub parameter_attrs: Vec<Vec<Attribute>>,
    pub return_attrs: Vec<Attribute>,
    pub attrs: Vec<Attribute>,
    pub personality: Option<ConstantId>,
    /// LLVM's calling convention number; 0 is C's.
    pub calling_convention: u32,
    pub(crate) void: TypeId,
    pub(crate) parameters: Vec<ValueId>,
    pub(crate) values: Vec<ValueData>,
    pub(crate) instructions: Vec<Instruction>,
    /// Each instruction's block; `None` while unplaced or once erased.
    pub(crate) parent: Vec<Option<BlockId>>,
    pub(crate) erased: Vec<bool>,
    pub(crate) blocks: Vec<Block>,
    /// The blocks in order, entry first; empty for a declaration.
    pub(crate) layout: Vec<BlockId>,
    pub(crate) uses: crate::uselist::UseLists,
    /// Each instruction's (previous, next) in its block, as an id + 1 (0:
    /// none): the block lists' links.
    pub(crate) link: Vec<(u32, u32)>,
    pub(crate) order: OrderCache,
    pub(crate) changes: ChangeLog,
    /// How many edits of each kind the function has had: no part of what it is.
    pub(crate) epochs: crate::touched::Epochs,
    /// How many changes `take_changes` has handed out: the log keeps them, so
    /// an analysis computed before can still be brought up to date.
    pub(crate) taken: usize,
    /// What `-g` says of its variables, kept true by the edits that move,
    /// replace or erase what it names.
    pub(crate) debug_records: Vec<DebugRecord>,
    /// The variables a record of which went with the code it stood in (a block
    /// erased): what is said of them is not all that was.
    pub(crate) debug_dropped: Vec<MetadataId>,
    /// The position each parameter had when the function was made, once one was
    /// removed or added; empty while none moved.
    pub(crate) parameter_origins: Vec<Option<usize>>,
    pub(crate) lineage: Lineage,
    /// The names of the live values and blocks, for `unique_name`.
    pub(crate) names: NameIndex,
}

/// A count per name held by a live value or block, made at the first new name
/// and kept by the edits after it, and per base name the number below which
/// every `base{n}` is taken. A cache: it is never part of what two functions
/// are equal by.
#[derive(Clone, Debug, Default)]
pub(crate) struct NameIndex(pub(crate) Option<Names>);

#[derive(Clone, Debug, Default)]
pub(crate) struct Names {
    pub(crate) held: crate::hash::HashMap<String, u32>,
    pub(crate) taken_below: crate::hash::HashMap<String, u32>,
}

impl PartialEq for NameIndex {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Function {
    pub(crate) fn new(
        ty: TypeId,
        void: TypeId,
    ) -> Self {
        Self {
            ty,
            parameter_attrs: Vec::new(),
            return_attrs: Vec::new(),
            attrs: Vec::new(),
            personality: None,
            calling_convention: 0,
            void,
            parameters: Vec::new(),
            values: Vec::new(),
            instructions: Vec::new(),
            parent: Vec::new(),
            erased: Vec::new(),
            blocks: Vec::new(),
            layout: Vec::new(),
            uses: Default::default(),
            link: Vec::new(),
            order: OrderCache::default(),
            changes: ChangeLog::default(),
            epochs: crate::touched::Epochs::default(),
            taken: 0,
            debug_records: Vec::new(),
            debug_dropped: Vec::new(),
            parameter_origins: Vec::new(),
            lineage: Lineage::default(),
            names: NameIndex::default(),
        }
    }

    /// An operand's type; a block has none.
    #[inline]
    pub fn operand_type(
        &self,
        context: &Context,
        operand: Operand,
    ) -> Option<TypeId> {
        match operand {
            Operand::Value(id) => Some(self.value(id).ty),
            Operand::Constant(id) => Some(context.get(id).ty),
            Operand::Block(_) => None,
        }
    }

    pub fn is_declaration(&self) -> bool {
        self.layout.is_empty()
    }

    /// Its declaration: type, attributes and unnamed parameters, no body.
    pub fn declaration(&self) -> Function {
        let mut out = Function::new(self.ty, self.void);
        out.parameter_attrs = self.parameter_attrs.clone();
        out.return_attrs = self.return_attrs.clone();
        out.attrs = self.attrs.clone();
        out.calling_convention = self.calling_convention;
        for &parameter in &self.parameters {
            out.parameters.push(ValueId(out.values.len() as u32));
            out.values.push(ValueData { name: None, ..self.value(parameter).clone() });
            out.uses.push_value();
        }
        out
    }

    /// Whether `declared` is this function's `declaration()`, found without
    /// making one.
    pub fn declares(
        &self,
        declared: &Function,
    ) -> bool {
        self.ty == declared.ty
            && self.void == declared.void
            && self.calling_convention == declared.calling_convention
            && self.attrs == declared.attrs
            && self.return_attrs == declared.return_attrs
            && self.parameter_attrs == declared.parameter_attrs
            && self.parameters.len() == declared.parameters.len()
            && self.parameters.iter().zip(&declared.parameters).all(|(one, other)| {
                let (mine, theirs) = (self.value(*one), declared.value(*other));
                mine.ty == theirs.ty && mine.def == theirs.def && theirs.name.is_none()
            })
    }

    pub fn parameters(&self) -> &[ValueId] {
        &self.parameters
    }

    /// The position parameter `at` had when the function was made, none for one
    /// the passes added: what `-g` names a parameter by.
    pub fn parameter_origin(
        &self,
        at: usize,
    ) -> Option<usize> {
        if self.parameter_origins.is_empty() { Some(at) } else { self.parameter_origins.get(at).copied().flatten() }
    }

    pub fn value(
        &self,
        id: ValueId,
    ) -> &ValueData {
        &self.values[id.0 as usize]
    }

    /// How many instructions were ever made, erased ones too: the next is
    /// `InstId(count)`.
    /// How many values the function has made, parameters and results, the ids a
    /// table of them spans.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Each instruction's index in its block, by id; erased and unplaced ones
    /// are zero.
    pub fn positions(&self) -> Vec<u32> {
        let mut positions = vec![0; self.instructions.len()];
        for &block in self.layout() {
            for (index, inst) in self.iter_block(block).enumerate() {
                positions[inst.0 as usize] = index as u32;
            }
        }
        positions
    }

    pub fn instruction(
        &self,
        id: InstId,
    ) -> &Instruction {
        &self.instructions[id.0 as usize]
    }

    pub fn block(
        &self,
        id: BlockId,
    ) -> BlockRef<'_> {
        BlockRef { function: self, block: &self.blocks[id.0 as usize] }
    }

    /// `BasicBlock::begin` to `end` without the view: a cursor over the links.
    pub fn iter_block(
        &self,
        id: BlockId,
    ) -> impl Iterator<Item = InstId> + '_ {
        let mut at = self.blocks[id.0 as usize].head;
        std::iter::from_fn(move || {
            (at != 0).then(|| {
                let inst = InstId(at - 1);
                at = self.link[inst.0 as usize].1;
                inst
            })
        })
    }

    /// `inst`'s number in its block, the block renumbered if an edit made its
    /// numbers false.
    fn number_of(
        &self,
        inst: InstId,
    ) -> Option<(BlockId, u32)> {
        let block = self.parent(inst)?;
        let mut numbers = self.order.0.borrow_mut();
        if numbers.valid.len() < self.blocks.len() {
            numbers.valid.resize(self.blocks.len(), false);
        }
        if numbers.at.len() < self.instructions.len() {
            numbers.at.resize(self.instructions.len(), 0);
        }
        if !numbers.valid[block.0 as usize] {
            for (at, one) in self.iter_block(block).enumerate() {
                numbers.at[one.0 as usize] = at as u32;
            }
            numbers.valid[block.0 as usize] = true;
        }
        Some((block, numbers.at[inst.0 as usize]))
    }

    /// `Instruction::getNextNode`: what follows `inst` in its block.
    pub fn next_in_block(
        &self,
        inst: InstId,
    ) -> Option<InstId> {
        self.parent(inst)?;
        match self.link[inst.0 as usize].1 {
            0 => None,
            n => Some(InstId(n - 1)),
        }
    }

    /// `Instruction::getPrevNode`: what stands before `inst` in its block.
    pub fn prev_in_block(
        &self,
        inst: InstId,
    ) -> Option<InstId> {
        self.parent(inst)?;
        match self.link[inst.0 as usize].0 {
            0 => None,
            n => Some(InstId(n - 1)),
        }
    }

    /// `Instruction::comesBefore`: whether `a` stands before `b` in their one
    /// block (false for two in different blocks, or one that is not placed).
    pub fn comes_before(
        &self,
        a: InstId,
        b: InstId,
    ) -> bool {
        match (self.number_of(a), self.number_of(b)) {
            (Some((x, i)), Some((y, j))) => x == y && i < j,
            _ => false,
        }
    }

    pub fn layout(&self) -> &[BlockId] {
        &self.layout
    }

    /// Whether `block` is in the layout.
    pub fn is_placed(
        &self,
        block: BlockId,
    ) -> bool {
        self.blocks[block.0 as usize].placed
    }

    pub fn entry(&self) -> Option<BlockId> {
        self.layout.first().copied()
    }

    /// One more than the largest block number: LLVM's `getMaxNumber`.
    pub fn block_capacity(&self) -> usize {
        self.blocks.len()
    }

    /// The block holding `inst`, while it is placed.
    pub fn parent(
        &self,
        inst: InstId,
    ) -> Option<BlockId> {
        self.parent[inst.0 as usize]
    }

    pub fn is_erased(
        &self,
        inst: InstId,
    ) -> bool {
        self.erased[inst.0 as usize]
    }

    pub fn users(
        &self,
        value: ValueId,
    ) -> crate::uselist::Users<'_> {
        self.uses.users(crate::uselist::Held::Value(value.0))
    }

    /// The operand slots naming `block`: terminators' and phis'.
    pub fn block_users(
        &self,
        block: BlockId,
    ) -> crate::uselist::Users<'_> {
        self.uses.users(crate::uselist::Held::Block(block.0))
    }

    pub fn terminator(
        &self,
        block: BlockId,
    ) -> Option<InstId> {
        self.blocks[block.0 as usize].last().filter(|&last| self.instruction(last).opcode.is_terminator())
    }

    /// The blocks `block`'s terminator names, in operand order, once each.
    pub fn successors(
        &self,
        block: BlockId,
    ) -> crate::blocklist::BlockList {
        let operands = self.terminator(block).map(|one| self.instruction(one).operands.as_slice()).unwrap_or_default();
        if operands.len() > SCAN {
            return Self::listed_once(operands);
        }
        let mut out = crate::blocklist::BlockList::new();
        for operand in operands {
            if let Operand::Block(target) = operand
                && !out.contains(target)
            {
                out.push(*target);
            }
        }
        out
    }

    /// The blocks `operands` name, once each, by a set: a switch of N cases
    /// named N blocks, and a scan of what was listed for each was N^2.
    #[cold]
    fn listed_once(operands: &[Operand]) -> crate::blocklist::BlockList {
        let mut seen = crate::dense::IdSet::<u32>::new();
        operands
            .iter()
            .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
            .filter(|target| seen.insert(target.0))
            .collect()
    }

    /// The blocks whose terminators name `block`, once each.
    pub fn predecessors(
        &self,
        block: BlockId,
    ) -> crate::blocklist::BlockList {
        let users = self.block_users(block);
        if users.len() > SCAN {
            return self.parents_once(users);
        }
        let mut out = crate::blocklist::BlockList::new();
        for one in users {
            let user = one.user;
            if self.instruction(user).opcode.is_terminator()
                && let Some(parent) = self.parent(user)
                && !out.contains(&parent)
            {
                out.push(parent);
            }
        }
        out
    }

    /// The blocks whose terminators are among `users`, once each, by a set.
    #[cold]
    fn parents_once(
        &self,
        users: crate::uselist::Users<'_>,
    ) -> crate::blocklist::BlockList {
        let mut seen = crate::dense::IdSet::<u32>::new();
        users
            .iter()
            .filter(|one| self.instruction(one.user).opcode.is_terminator())
            .filter_map(|one| self.parent(one.user))
            .filter(|parent| seen.insert(parent.0))
            .collect()
    }

    /// Instructions in layout order, with their block.
    pub fn walk(&self) -> impl Iterator<Item = (BlockId, InstId)> + '_ {
        self.layout.iter().flat_map(move |&block| self.iter_block(block).map(move |one| (block, one)))
    }

    /// The log of changes since the last `take_changes`. They stay in the log,
    /// to `changes_since`, until it holds more than `LOG` of them.
    pub fn take_changes(&mut self) -> Vec<Change> {
        // Twice what it was: the entries that tell what an operand was go with
        // every rewrite.
        const LOG: usize = 1 << 17;
        let kept_from = self.lineage.logged - self.changes.0.len();
        let out = self.changes.0[self.taken.max(kept_from) - kept_from..].to_vec();
        self.taken = self.lineage.logged;
        if self.changes.0.len() > LOG {
            let from = self.changes.0.len() - LOG / 2;
            self.changes.0.drain(..from);
        }
        out
    }

    pub(crate) fn log(
        &mut self,
        change: Change,
    ) {
        let mut epochs = self.epochs;
        epochs.bump(self, &change);
        self.epochs = epochs;
        self.lineage.logged += 1;
        self.changes.0.push(change);
    }

    /// How many edits of each kind the function has had: a memo that read
    /// these is true while they are as they were.
    pub fn epochs(&self) -> crate::touched::Epochs {
        self.epochs
    }

    /// Where the function stands now.
    /// A number for the attributes the function carries, which are written
    /// through the `pub` fields and move no mark: equal before and after means
    /// no analysis that reads them has anything new to read.
    pub fn attributes_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        STAMPS.with(|count| count.set(count.get() + 1));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.ty.hash(&mut hasher);
        self.parameter_attrs.hash(&mut hasher);
        self.return_attrs.hash(&mut hasher);
        self.attrs.hash(&mut hasher);
        self.personality.hash(&mut hasher);
        self.calling_convention.hash(&mut hasher);
        hasher.finish()
    }

    pub fn mark(&self) -> Mark {
        Mark { uid: self.lineage.uid, branch: self.lineage.branch, at: self.lineage.logged }
    }

    /// A copy to edit and put in this function's place, which carries on its
    /// history: a mark taken of this one is a point in the copy's, and what
    /// the copy logs follows it. For a copy that replaces the function, not
    /// one kept beside it (`clone` is that: its edits are its own).
    pub fn fork(&self) -> Self {
        crate::counts::bump(crate::counts::Event::FunctionForks);
        let mut copy = self.clone();
        let mut ancestors = self.lineage.ancestors.clone();
        ancestors.push((self.lineage.branch, self.lineage.logged));
        copy.lineage = Lineage {
            uid: self.lineage.uid,
            branch: NEXT_LINEAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ancestors,
            logged: self.lineage.logged,
        };
        copy
    }

    /// Puts `fork`, a `fork` of this function that was edited, in its place
    /// with the history both made: a mark taken before the fork still
    /// resolves, and what an analysis kept past it is brought up to date
    /// from.
    pub fn adopt(
        &mut self,
        mut fork: Self,
    ) {
        debug_assert!(fork.lineage.uid == self.lineage.uid, "a fork of this function");
        let mut history = std::mem::take(&mut self.changes.0);
        history.append(&mut fork.changes.0);
        fork.changes.0 = history;
        fork.taken = self.taken;
        // A fork that changed nothing is the function: its points are the
        // function's own.
        if fork.lineage.ancestors.last() == Some(&(self.lineage.branch, fork.lineage.logged)) {
            fork.lineage.branch = self.lineage.branch;
            fork.lineage.ancestors = std::mem::take(&mut self.lineage.ancestors);
        }
        *self = fork;
    }

    /// What changed since `mark`, in order; none where `mark` is of another
    /// function or what followed it has been taken.
    pub fn changes_since(
        &self,
        mark: Mark,
    ) -> Option<&[Change]> {
        let kept_from = self.lineage.logged - self.changes.0.len();
        // A point of this branch, or of one it forked from at or before where
        // it forked.
        let ours = mark.uid == self.lineage.uid
            && (mark.branch == self.lineage.branch
                || self.lineage.ancestors.iter().any(|&(branch, at)| branch == mark.branch && mark.at <= at));
        (ours && (kept_from..=self.lineage.logged).contains(&mark.at)).then(|| &self.changes.0[mark.at - kept_from..])
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Linkage {
    #[default]
    External,
    Internal,
    Private,
    Weak,
    WeakOdr,
    LinkOnce,
    LinkOnceOdr,
    Common,
    ExternWeak,
    AvailableExternally,
}

pub const LINKAGE: [(Linkage, &str); 9] = [
    (Linkage::Internal, "internal"),
    (Linkage::Private, "private"),
    (Linkage::Weak, "weak"),
    (Linkage::WeakOdr, "weak_odr"),
    (Linkage::LinkOnce, "linkonce"),
    (Linkage::LinkOnceOdr, "linkonce_odr"),
    (Linkage::Common, "common"),
    (Linkage::ExternWeak, "extern_weak"),
    (Linkage::AvailableExternally, "available_externally"),
];

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UnnamedAddr {
    #[default]
    None,
    Local,
    Global,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlobalVariable {
    pub ty: TypeId,
    pub constant: bool,
    /// `externally_initialized`: the initializer may change before the program
    /// runs.
    pub externally_initialized: bool,
    pub initializer: Option<ConstantId>,
    pub align: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GlobalKind {
    Variable(GlobalVariable),
    Function(Box<Function>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlobalValue {
    pub name: Option<String>,
    pub linkage: Linkage,
    pub unnamed_addr: UnnamedAddr,
    pub address_space: u32,
    pub kind: GlobalKind,
}

impl GlobalValue {
    /// This global as its declaration: a function without its body.
    pub fn declaration(&self) -> GlobalValue {
        GlobalValue {
            name: self.name.clone(),
            linkage: self.linkage,
            unnamed_addr: self.unnamed_addr,
            address_space: self.address_space,
            kind: match &self.kind {
                GlobalKind::Function(function) => GlobalKind::Function(Box::new(function.declaration())),
                GlobalKind::Variable(variable) => GlobalKind::Variable(variable.clone()),
            },
        }
    }

    /// Whether `declared` is this global's `declaration()`, found without
    /// making one: what `Declarations` holds of it still stands.
    pub fn declares(
        &self,
        declared: &GlobalValue,
    ) -> bool {
        self.name == declared.name
            && self.linkage == declared.linkage
            && self.unnamed_addr == declared.unnamed_addr
            && self.address_space == declared.address_space
            && match (&self.kind, &declared.kind) {
                (GlobalKind::Function(one), GlobalKind::Function(other)) => one.declares(other),
                (GlobalKind::Variable(one), GlobalKind::Variable(other)) => one == other,
                _ => false,
            }
    }

    pub fn function(&self) -> Option<&Function> {
        match &self.kind {
            GlobalKind::Function(function) => Some(function),
            GlobalKind::Variable(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetadataOperand {
    Null,
    Node(MetadataId),
    String(String),
    Constant(ConstantId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataNode {
    pub distinct: bool,
    pub operands: Vec<MetadataOperand>,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub context: Context,
    pub datalayout: Option<String>,
    /// Variables and functions, in definition order.
    pub globals: Vec<GlobalValue>,
    pub metadata: Vec<MetadataNode>,
    pub named_metadata: Vec<(String, Vec<MetadataId>)>,
}

impl Module {
    pub fn global(
        &self,
        id: GlobalId,
    ) -> &GlobalValue {
        &self.globals[id.0 as usize]
    }

    /// Every global as its declaration, by id.
    pub fn declarations(&self) -> Vec<GlobalValue> {
        self.globals.iter().map(GlobalValue::declaration).collect()
    }

    pub fn named(
        &self,
        name: &str,
    ) -> Option<GlobalId> {
        self.globals.iter().position(|one| one.name.as_deref() == Some(name)).map(|at| GlobalId(at as u32))
    }

    pub fn functions(&self) -> impl Iterator<Item = (GlobalId, &GlobalValue, &Function)> + '_ {
        self.globals
            .iter()
            .enumerate()
            .filter_map(|(at, global)| global.function().map(|function| (GlobalId(at as u32), global, function)))
    }

    /// The function named `name`, with the context its types live in.
    pub fn function_mut(
        &mut self,
        name: &str,
    ) -> Option<(&mut Context, &mut Function)> {
        let global = self.globals.iter_mut().find(|one| one.name.as_deref() == Some(name))?;
        match &mut global.kind {
            GlobalKind::Function(function) => Some((&mut self.context, function)),
            GlobalKind::Variable(_) => None,
        }
    }

    /// The module without the globals `keep` refuses, the rest renumbered
    /// in their order; a constant naming one gone becomes poison.
    pub fn retain_globals(
        &mut self,
        keep: &dyn Fn(GlobalId) -> bool,
    ) {
        let mut renumbered = Vec::with_capacity(self.globals.len());
        let mut next = 0;
        for at in 0..self.globals.len() as u32 {
            renumbered.push(keep(GlobalId(at)).then(|| {
                next += 1;
                GlobalId(next - 1)
            }));
        }
        self.context.renumber_globals(&|global| renumbered[global.0 as usize]);
        let mut at = 0;
        self.globals.retain(|_| {
            at += 1;
            renumbered[at - 1].is_some()
        });
    }

    /// A function type's return type and parameters.
    pub fn signature(
        &self,
        function_type: TypeId,
    ) -> (TypeId, &[TypeId], bool) {
        match self.context.types.get(function_type) {
            Type::Function { returns, parameters, variadic } => (*returns, parameters, *variadic),
            other => panic!("{other:?} is not a function type"),
        }
    }
}

/// Operands scanned for a repeat before a set is used.
const SCAN: usize = 16;
