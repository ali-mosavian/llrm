//! Modules, global values, functions, blocks and instructions, as LLVM has
//! them. A function's values, instructions and blocks live in arenas whose
//! ids are never reused; `layout` and each block's list give the order.

use crate::context::{ConstantId, Context, GlobalId};
use crate::opcode::{Attribute, Flags, Opcode};
use crate::types::{Type, TypeId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MetadataId(pub u32);

/// An instruction's operand: a local value, a constant (a global value is
/// one), or a block.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub name: Option<String>,
    pub(crate) instructions: Vec<InstId>,
    pub(crate) erased: bool,
}

impl Block {
    pub fn instructions(&self) -> &[InstId] {
        &self.instructions
    }
}

/// What a debugger is told of a source variable, from a point of the code on: where it lives in memory, what it is, or that
/// nothing says. Not an instruction: no pass counts it, and a use of a value in one keeps nothing alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugWhat {
    /// The variable is in the memory this address names, from here on (`llvm.dbg.declare`).
    Declare(Operand),
    /// The variable is this value, from here until the next record of the variable (`llvm.dbg.value`).
    Value(Operand),
    /// `bytes` of the variable from byte `offset` are this value, from here on (`DW_OP_piece`): an aggregate the optimiser split.
    Piece { value: Operand, offset: u32, bytes: u32 },
    /// Nothing says where those bytes are: their value was deleted.
    GonePiece { offset: u32, bytes: u32 },
    /// Nothing says where it is: its value was deleted.
    Gone,
}

/// One such statement, standing before the instruction it names. The variable is a node of
/// [`debuginfo`](crate::debuginfo).
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
    Inserted { inst: InstId, block: BlockId, next: Option<InstId> },
    Cloned { from: InstId, to: InstId },
    Moved { inst: InstId, block: BlockId, next: Option<InstId>, from: BlockId },
    Rewritten(InstId),
    Erased { inst: InstId, block: BlockId, next: Option<InstId> },
    BlockCreated(BlockId),
    BlockErased(BlockId),
}

/// A function's log of changes. A copy is another function (`Lineage`), whose edits from then on are its own: it starts with an
/// empty log, as a copy that carried the original's (up to 64k changes, cloned for each numbering of a body) cost 16% of
/// compiling a program with a hundred inlines. The log is no part of what a function is, so two functions are equal whatever
/// they logged.
#[derive(Debug, Default)]
pub(crate) struct ChangeLog(pub(crate) Vec<Change>);

impl Clone for ChangeLog {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for ChangeLog {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

static NEXT_LINEAGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Which function a log of changes belongs to, and how many changes it has held: `take_changes` drains the log
/// and leaves the count. A copy of a function is another one, as its edits from then on are its own.
#[derive(Debug)]
pub(crate) struct Lineage {
    uid: u64,
    logged: usize,
}

impl Default for Lineage {
    fn default() -> Self {
        Self { uid: NEXT_LINEAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed), logged: 0 }
    }
}

impl Clone for Lineage {
    fn clone(&self) -> Self {
        Self { uid: NEXT_LINEAGE.fetch_add(1, std::sync::atomic::Ordering::Relaxed), logged: self.logged }
    }
}

/// Equal whatever the history: two functions with the same body are equal.
impl PartialEq for Lineage {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// A point in one function's history, to ask what changed since.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mark {
    uid: u64,
    at: usize,
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
    pub(crate) value_uses: Vec<Vec<Use>>,
    pub(crate) block_uses: Vec<Vec<Use>>,
    pub(crate) changes: ChangeLog,
    /// How many changes `take_changes` has handed out: the log keeps them, so an analysis computed before can still be
    /// brought up to date.
    pub(crate) taken: usize,
    /// What `-g` says of its variables, kept true by the edits that move, replace or erase what it names.
    pub(crate) debug_records: Vec<DebugRecord>,
    /// The variables a record of which went with the code it stood in (a block erased): what is said of them is not all that was.
    pub(crate) debug_dropped: Vec<MetadataId>,
    /// The position each parameter had when the function was made, once one was removed or added; empty while none moved.
    pub(crate) parameter_origins: Vec<Option<usize>>,
    pub(crate) lineage: Lineage,
}

impl Function {
    pub(crate) fn new(ty: TypeId, void: TypeId) -> Self {
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
            value_uses: Vec::new(),
            block_uses: Vec::new(),
            changes: ChangeLog::default(),
            taken: 0,
            debug_records: Vec::new(),
            debug_dropped: Vec::new(),
            parameter_origins: Vec::new(),
            lineage: Lineage::default(),
        }
    }

    /// An operand's type; a block has none.
    pub fn operand_type(&self, context: &Context, operand: Operand) -> Option<TypeId> {
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
            out.value_uses.push(Vec::new());
        }
        out
    }

    /// Whether `declared` is this function's `declaration()`, found without making one.
    pub fn declares(&self, declared: &Function) -> bool {
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

    /// The position parameter `at` had when the function was made, none for one the passes added: what `-g` names a
    /// parameter by.
    pub fn parameter_origin(&self, at: usize) -> Option<usize> {
        if self.parameter_origins.is_empty() { Some(at) } else { self.parameter_origins.get(at).copied().flatten() }
    }

    pub fn value(&self, id: ValueId) -> &ValueData {
        &self.values[id.0 as usize]
    }

    /// How many instructions were ever made, erased ones too: the next is
    /// `InstId(count)`.
    /// How many values the function has made, parameters and results, the ids a table of them spans.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Each instruction's index in its block, by id; erased and unplaced ones are zero.
    pub fn positions(&self) -> Vec<u32> {
        let mut positions = vec![0; self.instructions.len()];
        for &block in self.layout() {
            for (index, inst) in self.block(block).instructions().iter().enumerate() {
                positions[inst.0 as usize] = index as u32;
            }
        }
        positions
    }

    pub fn instruction(&self, id: InstId) -> &Instruction {
        &self.instructions[id.0 as usize]
    }

    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id.0 as usize]
    }

    pub fn layout(&self) -> &[BlockId] {
        &self.layout
    }

    pub fn entry(&self) -> Option<BlockId> {
        self.layout.first().copied()
    }

    /// The block holding `inst`, while it is placed.
    pub fn parent(&self, inst: InstId) -> Option<BlockId> {
        self.parent[inst.0 as usize]
    }

    pub fn is_erased(&self, inst: InstId) -> bool {
        self.erased[inst.0 as usize]
    }

    pub fn users(&self, value: ValueId) -> &[Use] {
        &self.value_uses[value.0 as usize]
    }

    /// The operand slots naming `block`: terminators' and phis'.
    pub fn block_users(&self, block: BlockId) -> &[Use] {
        &self.block_uses[block.0 as usize]
    }

    pub fn terminator(&self, block: BlockId) -> Option<InstId> {
        self.block(block).instructions.last().copied().filter(|&last| self.instruction(last).opcode.is_terminator())
    }

    /// The blocks `block`'s terminator names, in operand order, once each.
    pub fn successors(&self, block: BlockId) -> Vec<BlockId> {
        let mut out = Vec::new();
        for operand in self.terminator(block).map(|one| self.instruction(one).operands.as_slice()).unwrap_or_default() {
            if let Operand::Block(target) = operand
                && !out.contains(target)
            {
                out.push(*target);
            }
        }
        out
    }

    /// The blocks whose terminators name `block`, once each.
    pub fn predecessors(&self, block: BlockId) -> Vec<BlockId> {
        let mut out = Vec::new();
        for one in self.block_users(block) {
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

    /// Instructions in layout order, with their block.
    pub fn walk(&self) -> impl Iterator<Item = (BlockId, InstId)> + '_ {
        self.layout.iter().flat_map(move |&block| self.block(block).instructions.iter().map(move |&one| (block, one)))
    }

    /// The log of changes since the last `take_changes`. They stay in the log, to `changes_since`, until it holds more
    /// than `LOG` of them.
    pub fn take_changes(&mut self) -> Vec<Change> {
        const LOG: usize = 1 << 16;
        let kept_from = self.lineage.logged - self.changes.0.len();
        let out = self.changes.0[self.taken.max(kept_from) - kept_from..].to_vec();
        self.taken = self.lineage.logged;
        if self.changes.0.len() > LOG {
            let from = self.changes.0.len() - LOG / 2;
            self.changes.0.drain(..from);
        }
        out
    }

    pub(crate) fn log(&mut self, change: Change) {
        self.lineage.logged += 1;
        self.changes.0.push(change);
    }

    /// Where the function stands now.
    pub fn mark(&self) -> Mark {
        Mark { uid: self.lineage.uid, at: self.lineage.logged }
    }

    /// What changed since `mark`, in order; none where `mark` is of another function or what followed it has been
    /// taken.
    pub fn changes_since(&self, mark: Mark) -> Option<&[Change]> {
        let kept_from = self.lineage.logged - self.changes.0.len();
        (mark.uid == self.lineage.uid && (kept_from..=self.lineage.logged).contains(&mark.at)).then(|| &self.changes.0[mark.at - kept_from..])
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

    /// Whether `declared` is this global's `declaration()`, found without making one: what `Declarations` holds of it still
    /// stands.
    pub fn declares(&self, declared: &GlobalValue) -> bool {
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
    pub fn global(&self, id: GlobalId) -> &GlobalValue {
        &self.globals[id.0 as usize]
    }

    /// Every global as its declaration, by id.
    pub fn declarations(&self) -> Vec<GlobalValue> {
        self.globals.iter().map(GlobalValue::declaration).collect()
    }

    pub fn named(&self, name: &str) -> Option<GlobalId> {
        self.globals.iter().position(|one| one.name.as_deref() == Some(name)).map(|at| GlobalId(at as u32))
    }

    pub fn functions(&self) -> impl Iterator<Item = (GlobalId, &GlobalValue, &Function)> + '_ {
        self.globals
            .iter()
            .enumerate()
            .filter_map(|(at, global)| global.function().map(|function| (GlobalId(at as u32), global, function)))
    }

    /// The function named `name`, with the context its types live in.
    pub fn function_mut(&mut self, name: &str) -> Option<(&mut Context, &mut Function)> {
        let global = self.globals.iter_mut().find(|one| one.name.as_deref() == Some(name))?;
        match &mut global.kind {
            GlobalKind::Function(function) => Some((&mut self.context, function)),
            GlobalKind::Variable(_) => None,
        }
    }

    /// The module without the globals `keep` refuses, the rest renumbered
    /// in their order; a constant naming one gone becomes poison.
    pub fn retain_globals(&mut self, keep: &dyn Fn(GlobalId) -> bool) {
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
    pub fn signature(&self, function_type: TypeId) -> (TypeId, &[TypeId], bool) {
        match self.context.types.get(function_type) {
            Type::Function { returns, parameters, variadic } => (*returns, parameters, *variadic),
            other => panic!("{other:?} is not a function type"),
        }
    }
}
