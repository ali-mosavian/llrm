//! Modules, functions, blocks, instructions, values and edges.

use crate::opcode::Opcode;
use crate::types::{MirContext, TypeId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstructionId(pub u32);

/// A value's one type, fixed at its definition, and the name it prints with.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueInfo {
    pub ty: TypeId,
    pub name: Option<String>,
}

/// An integer constant: its bits, normalized modulo its type's width.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Constant {
    pub ty: TypeId,
    pub bits: u128,
}

impl Constant {
    /// `value` modulo the width of `ty`, which must be an integer type.
    pub fn int(context: &MirContext, ty: TypeId, value: i128) -> Self {
        let bits = context.int_bits(ty).expect("an integer constant has an integer type");
        Self { ty, bits: (value as u128) & mask(bits) }
    }
}

/// The low `bits` bits set.
pub fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `bits` as a two's-complement number `width` bits wide.
pub fn signed(bits: u128, width: u32) -> i128 {
    let shift = 128 - width.min(128);
    ((bits << shift) as i128) >> shift
}

/// The ordered operand list is an instruction's only use list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operand {
    Value(ValueId),
    Constant(Constant),
    /// A terminator's operand owns the edge; a phi's names the edge it reads along.
    Edge(EdgeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub id: InstructionId,
    pub opcode: Opcode,
    pub results: Vec<ValueId>,
    pub operands: Vec<Operand>,
}

/// A CFG edge. Its source is the block whose terminator names it, so a phi
/// input names the edge, and two edges between one pair of blocks stay apart.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub target: BlockId,
    pub name: Option<String>,
}

/// Phis, then ordinary instructions, then exactly one terminator.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub name: Option<String>,
    pub instructions: Vec<Instruction>,
}

/// `blocks[0]` is the entry, and nothing branches to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<ValueId>,
    pub returns: Vec<TypeId>,
    pub values: Vec<ValueInfo>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
}

impl Function {
    pub fn value(&self, id: ValueId) -> &ValueInfo {
        &self.values[id.0 as usize]
    }

    pub fn edge(&self, id: EdgeId) -> &Edge {
        &self.edges[id.0 as usize]
    }

    /// The block whose terminator owns each edge, by edge.
    pub fn edge_sources(&self) -> Vec<Option<BlockId>> {
        let mut sources = vec![None; self.edges.len()];
        for (at, block) in self.blocks.iter().enumerate() {
            for edge in block.successor_edges() {
                if let Some(source) = sources.get_mut(edge.0 as usize) {
                    *source = Some(BlockId(at as u32));
                }
            }
        }
        sources
    }
}

impl Block {
    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last().filter(|one| one.opcode.is_terminator())
    }

    /// The edges this block's terminator owns, in operand order.
    pub fn successor_edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.terminator().into_iter().flat_map(|one| one.operands.iter()).filter_map(|operand| match operand {
            Operand::Edge(edge) => Some(*edge),
            _ => None,
        })
    }
}
