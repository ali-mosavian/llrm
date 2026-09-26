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
    Moved { inst: InstId, block: BlockId, next: Option<InstId> },
    Rewritten(InstId),
    Erased { inst: InstId, block: BlockId, next: Option<InstId> },
    BlockCreated(BlockId),
    BlockErased(BlockId),
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
    pub(crate) changes: Vec<Change>,
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
            changes: Vec::new(),
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

    pub fn parameters(&self) -> &[ValueId] {
        &self.parameters
    }

    pub fn value(&self, id: ValueId) -> &ValueData {
        &self.values[id.0 as usize]
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

    /// The log of changes since the last `take_changes`.
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
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

    /// A function type's return type and parameters.
    pub fn signature(&self, function_type: TypeId) -> (TypeId, &[TypeId], bool) {
        match self.context.types.get(function_type) {
            Type::Function { returns, parameters, variadic } => (*returns, parameters, *variadic),
            other => panic!("{other:?} is not a function type"),
        }
    }
}
