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
    pub instructions: Vec<InstId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    /// The function type.
    pub ty: TypeId,
    pub parameters: Vec<ValueId>,
    pub parameter_attrs: Vec<Vec<Attribute>>,
    pub return_attrs: Vec<Attribute>,
    pub attrs: Vec<Attribute>,
    pub personality: Option<ConstantId>,
    pub values: Vec<ValueData>,
    pub instructions: Vec<Instruction>,
    pub blocks: Vec<Block>,
    /// The blocks in order, entry first; empty for a declaration.
    pub layout: Vec<BlockId>,
}

impl Function {
    pub fn is_declaration(&self) -> bool {
        self.layout.is_empty()
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

    pub fn entry(&self) -> Option<BlockId> {
        self.layout.first().copied()
    }

    /// Instructions in layout order, with their block.
    pub fn walk(&self) -> impl Iterator<Item = (BlockId, InstId)> + '_ {
        self.layout.iter().flat_map(move |&block| self.block(block).instructions.iter().map(move |&one| (block, one)))
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
    Function(Function),
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

    /// A function type's return type and parameters.
    pub fn signature(&self, function_type: TypeId) -> (TypeId, &[TypeId], bool) {
        match self.context.types.get(function_type) {
            Type::Function { returns, parameters, variadic } => (*returns, parameters, *variadic),
            other => panic!("{other:?} is not a function type"),
        }
    }
}
