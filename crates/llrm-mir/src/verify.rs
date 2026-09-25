//! The verifier: structure, definitions, edges, types and dominance. It
//! reports every violation, each naming the value, block or edge at fault.

use std::collections::BTreeSet;

use crate::function::{BlockId, EdgeId, Function, Module, Operand, ValueId, mask};
use crate::opcode::{Opcode, Slot};
use crate::types::{MAX_INT_BITS, MirContext, Type, TypeId};

pub fn verify_module(context: &MirContext, module: &Module) -> Vec<String> {
    module
        .functions
        .iter()
        .flat_map(|function| verify(context, function).into_iter().map(move |one| format!("{}: {one}", function.name)))
        .collect()
}

pub fn verify(context: &MirContext, function: &Function) -> Vec<String> {
    let mut checker = Checker { context, function, errors: Vec::new(), defined: Vec::new() };
    checker.types();
    checker.definitions();
    checker.structure();
    let sources = checker.edges();
    checker.instructions(&sources);
    checker.dominance(&sources);
    checker.errors
}

#[derive(Clone, Copy, PartialEq)]
enum Site {
    Parameter,
    At(BlockId, usize),
}

struct Checker<'a> {
    context: &'a MirContext,
    function: &'a Function,
    errors: Vec<String>,
    defined: Vec<Option<Site>>,
}

impl Checker<'_> {
    fn fail(&mut self, message: String) {
        self.errors.push(message);
    }

    fn value_name(&self, id: ValueId) -> String {
        match self.function.values.get(id.0 as usize).and_then(|one| one.name.clone()) {
            Some(name) => name,
            None => format!("value{}", id.0),
        }
    }

    fn block_name(&self, id: BlockId) -> String {
        self.function.blocks[id.0 as usize].name.clone().unwrap_or_else(|| format!("block{}", id.0))
    }

    fn types(&mut self) {
        let context = self.context;
        let valid = |ty: TypeId| match context.get(ty) {
            Type::Void | Type::Function { .. } => false,
            Type::Int(bits) => (1..=MAX_INT_BITS).contains(bits),
            _ => true,
        };
        for at in 0..self.function.values.len() {
            let ty = self.function.values[at].ty;
            if !valid(ty) {
                self.fail(format!("{} has type {}, which no value can have", self.value_name(ValueId(at as u32)), context.display(ty)));
            }
        }
        for &ty in &self.function.returns {
            if !valid(ty) {
                self.fail(format!("the function returns {}, which no value can have", context.display(ty)));
            }
        }
    }

    fn definitions(&mut self) {
        self.defined = vec![None; self.function.values.len()];
        let parameters = self.function.parameters.iter().map(|&id| (id, Site::Parameter));
        let results = self.function.blocks.iter().enumerate().flat_map(|(block, one)| {
            one.instructions.iter().enumerate().flat_map(move |(at, instruction)| {
                instruction.results.iter().map(move |&id| (id, Site::At(BlockId(block as u32), at)))
            })
        });
        for (id, site) in parameters.chain(results).collect::<Vec<_>>() {
            match self.defined.get(id.0 as usize) {
                None => self.fail(format!("value{} is defined but has no type", id.0)),
                Some(Some(_)) => self.fail(format!("{} is defined twice", self.value_name(id))),
                Some(None) => self.defined[id.0 as usize] = Some(site),
            }
        }
    }

    fn structure(&mut self) {
        let function = self.function;
        if function.blocks.is_empty() {
            self.fail("the function has no blocks".to_owned());
        }
        let mut ids = BTreeSet::new();
        for (block, one) in function.blocks.iter().enumerate() {
            let name = self.block_name(BlockId(block as u32));
            if one.terminator().is_none() {
                self.fail(format!("block {name} does not end in a terminator"));
            }
            let count = one.instructions.len();
            if one.instructions[..count.saturating_sub(1)].iter().any(|one| one.opcode.is_terminator()) {
                self.fail(format!("block {name} has a terminator before its end"));
            }
            let phis = one.instructions.iter().take_while(|one| one.opcode == Opcode::Phi).count();
            if one.instructions[phis..].iter().any(|one| one.opcode == Opcode::Phi) {
                self.fail(format!("block {name} has a phi after an ordinary instruction"));
            }
            for instruction in &one.instructions {
                if !ids.insert(instruction.id) {
                    self.fail(format!("instruction #{} appears twice", instruction.id.0));
                }
            }
        }
    }

    /// Each edge's source: the one terminator that owns it.
    fn edges(&mut self) -> Vec<Option<BlockId>> {
        let function = self.function;
        let mut owners = vec![0usize; function.edges.len()];
        for block in &function.blocks {
            for edge in block.successor_edges() {
                match owners.get_mut(edge.0 as usize) {
                    Some(count) => *count += 1,
                    None => self.fail(format!("edge{} does not exist", edge.0)),
                }
            }
        }
        for (at, &count) in owners.iter().enumerate() {
            if count != 1 {
                self.fail(format!("edge{at} is owned by {count} terminators, not one"));
            }
            if function.edges[at].target.0 as usize >= function.blocks.len() {
                self.fail(format!("edge{at} leads to a block that does not exist"));
            }
        }
        let sources = function.edge_sources();
        if sources.iter().zip(&function.edges).any(|(source, edge)| source.is_some() && edge.target == BlockId(0)) {
            self.fail("the entry block is a branch target".to_owned());
        }
        sources
    }

    fn instructions(&mut self, sources: &[Option<BlockId>]) {
        let function = self.function;
        for (block, one) in function.blocks.iter().enumerate() {
            let block = BlockId(block as u32);
            for instruction in &one.instructions {
                let opcode = instruction.opcode;
                let what = format!("{} (#{})", opcode.mnemonic(), instruction.id.0);
                if instruction.results.len() != opcode.result_count() {
                    self.fail(format!("{what} has {} results, not {}", instruction.results.len(), opcode.result_count()));
                    continue;
                }
                if !opcode.takes(instruction.operands.len()) {
                    self.fail(format!("{what} cannot take {} operands", instruction.operands.len()));
                    continue;
                }
                if opcode == Opcode::Return && instruction.operands.len() != function.returns.len() {
                    self.fail(format!("{what} returns {} values, not {}", instruction.operands.len(), function.returns.len()));
                    continue;
                }
                let result = instruction.results.first().filter(|id| (id.0 as usize) < function.values.len()).map(|&id| function.value(id).ty);
                let mut shared = None;
                let mut types = Vec::new();
                for (slot, operand) in opcode.slots(instruction.operands.len()).into_iter().zip(&instruction.operands) {
                    let ty = match (slot, operand) {
                        (Slot::Edge, Operand::Edge(edge)) if (edge.0 as usize) < function.edges.len() => continue,
                        (Slot::Edge, _) => {
                            self.fail(format!("{what} needs an edge where it has {operand:?}"));
                            continue;
                        }
                        (_, Operand::Edge(_)) => {
                            self.fail(format!("{what} has an edge where it needs a value"));
                            continue;
                        }
                        (_, Operand::Value(id)) => match self.defined.get(id.0 as usize) {
                            Some(Some(_)) => function.value(*id).ty,
                            _ => {
                                self.fail(format!("{what} uses {}, which nothing defines", self.value_name(*id)));
                                continue;
                            }
                        },
                        (_, Operand::Constant(constant)) => {
                            match self.context.int_bits(constant.ty) {
                                Some(bits) if constant.bits & !mask(bits) == 0 => {}
                                _ => self.fail(format!("{what} has a constant that is not a normalized integer")),
                            }
                            constant.ty
                        }
                    };
                    let expected = match slot {
                        Slot::Bool => Some(self.context.bool()),
                        Slot::Result => result,
                        Slot::Shared => Some(*shared.get_or_insert(ty)),
                        Slot::Return(at) => function.returns.get(at).copied(),
                        Slot::Free | Slot::Edge => Some(ty),
                    };
                    if expected != Some(ty) {
                        let wanted = expected.map_or("nothing".to_owned(), |one| self.context.display(one));
                        self.fail(format!("{what} has a {} operand where it needs {wanted}", self.context.display(ty)));
                    }
                    types.push(ty);
                }
                if types.len() == instruction.operands.iter().filter(|one| !matches!(one, Operand::Edge(_))).count()
                    && let Err(message) = opcode.check(self.context, &types, result)
                {
                    self.fail(format!("{what}: {message}"));
                }
                if opcode == Opcode::Phi {
                    self.phi(block, &instruction.operands, sources, &what);
                }
            }
        }
    }

    /// A phi reads exactly once along every edge into its block, and along no other.
    fn phi(&mut self, block: BlockId, operands: &[Operand], sources: &[Option<BlockId>], what: &str) {
        let function = self.function;
        let into: BTreeSet<EdgeId> = (0..function.edges.len() as u32)
            .map(EdgeId)
            .filter(|&edge| sources[edge.0 as usize].is_some() && function.edge(edge).target == block)
            .collect();
        let mut read = BTreeSet::new();
        for operand in operands.iter().step_by(2) {
            if let Operand::Edge(edge) = operand {
                if !into.contains(edge) {
                    self.fail(format!("{what} reads along edge{}, which does not enter its block", edge.0));
                } else if !read.insert(*edge) {
                    self.fail(format!("{what} reads along edge{} twice", edge.0));
                }
            }
        }
        for edge in into.difference(&read) {
            self.fail(format!("{what} has no input along edge{}", edge.0));
        }
    }

    /// Every use is dominated by its definition; a phi's use, at the end of the edge's source.
    fn dominance(&mut self, sources: &[Option<BlockId>]) {
        let function = self.function;
        if function.blocks.is_empty() {
            return;
        }
        let dominators = Dominators::new(function, sources);
        for (block, one) in function.blocks.iter().enumerate() {
            let block = BlockId(block as u32);
            if !dominators.reachable(block) {
                continue;
            }
            for (at, instruction) in one.instructions.iter().enumerate() {
                for (index, operand) in instruction.operands.iter().enumerate() {
                    let Operand::Value(id) = operand else { continue };
                    let Some(Some(Site::At(defined, position))) = self.defined.get(id.0 as usize).copied() else { continue };
                    let fine = if instruction.opcode == Opcode::Phi {
                        let edge = match instruction.operands.get(index.wrapping_sub(1)) {
                            Some(Operand::Edge(edge)) => sources.get(edge.0 as usize).copied().flatten(),
                            _ => None,
                        };
                        edge.is_none_or(|source| !dominators.reachable(source) || dominators.dominates(defined, source))
                    } else if defined == block {
                        position < at
                    } else {
                        dominators.dominates(defined, block)
                    };
                    if !fine {
                        let (name, place) = (self.value_name(*id), self.block_name(block));
                        self.fail(format!("{name} is used in {place} where its definition does not dominate"));
                    }
                }
            }
        }
    }
}

/// Immediate dominators over the blocks reachable from the entry
/// (Cooper, Harvey and Kennedy's iteration in reverse postorder).
struct Dominators {
    idom: Vec<Option<usize>>,
}

impl Dominators {
    fn new(function: &Function, sources: &[Option<BlockId>]) -> Self {
        let count = function.blocks.len();
        let mut successors = vec![Vec::new(); count];
        let mut predecessors = vec![Vec::new(); count];
        for (edge, source) in sources.iter().enumerate() {
            let target = function.edges[edge].target.0 as usize;
            if let Some(source) = source
                && target < count
            {
                successors[source.0 as usize].push(target);
                predecessors[target].push(source.0 as usize);
            }
        }
        let mut order = Vec::new();
        let mut seen = vec![false; count];
        let mut stack = vec![(0usize, 0usize)];
        seen[0] = true;
        while let Some((block, next)) = stack.pop() {
            if let Some(&successor) = successors[block].get(next) {
                stack.push((block, next + 1));
                if !seen[successor] {
                    seen[successor] = true;
                    stack.push((successor, 0));
                }
            } else {
                order.push(block);
            }
        }
        order.reverse();
        let mut rank = vec![usize::MAX; count];
        for (at, &block) in order.iter().enumerate() {
            rank[block] = at;
        }
        let mut idom = vec![None; count];
        idom[0] = Some(0);
        let mut changed = true;
        while changed {
            changed = false;
            for &block in &order[1..] {
                let mut chosen: Option<usize> = None;
                for &predecessor in &predecessors[block] {
                    if idom[predecessor].is_none() {
                        continue;
                    }
                    chosen = Some(match chosen {
                        None => predecessor,
                        Some(mut left) => {
                            let mut right = predecessor;
                            while left != right {
                                while rank[left] > rank[right] {
                                    left = idom[left].unwrap();
                                }
                                while rank[right] > rank[left] {
                                    right = idom[right].unwrap();
                                }
                            }
                            left
                        }
                    });
                }
                if chosen.is_some() && idom[block] != chosen {
                    idom[block] = chosen;
                    changed = true;
                }
            }
        }
        Self { idom }
    }

    fn reachable(&self, block: BlockId) -> bool {
        self.idom[block.0 as usize].is_some()
    }

    fn dominates(&self, over: BlockId, block: BlockId) -> bool {
        let (over, mut at) = (over.0 as usize, block.0 as usize);
        loop {
            if at == over {
                return true;
            }
            match self.idom[at] {
                Some(parent) if parent != at => at = parent,
                _ => return false,
            }
        }
    }
}
