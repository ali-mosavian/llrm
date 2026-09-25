//! MIR text. A value's type is printed where its definition cannot be read
//! off its operands, a constant's only where its operand's role leaves it
//! open, and an edge's name only where two edges join the same two blocks.

use std::collections::HashSet;

use crate::function::{EdgeId, Function, Instruction, Module, Operand, ValueId, signed};
use crate::opcode::{Opcode, Slot};
use crate::types::{MirContext, TypeId, returns_suffix};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    /// Stage dumps and review: the names the MIR carries.
    Normal,
    /// Structural comparison: values, blocks and edges renamed in order.
    Normalized,
}

pub fn module(context: &MirContext, module: &Module, view: View) -> String {
    let mut out = format!("module {}\n", module.name);
    for one in &module.functions {
        out.push('\n');
        out.push_str(&function(context, one, view));
    }
    out
}

pub fn function(context: &MirContext, function: &Function, view: View) -> String {
    let names = Names::new(function, view);
    let parameters: Vec<String> = function
        .parameters
        .iter()
        .map(|&id| format!("{}: {}", names.value(id), context.display(function.value(id).ty)))
        .collect();
    let mut out = format!("function {}({}){}\n", function.name, parameters.join(", "), returns_suffix(context, &function.returns));
    let printer = Printer { context, function, names };
    for (at, block) in function.blocks.iter().enumerate() {
        if at > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{}:\n", printer.names.blocks[at]));
        for instruction in &block.instructions {
            out.push_str(&printer.instruction(instruction));
        }
    }
    out.push_str("end\n");
    out
}

/// Every printed name, unique in its namespace.
struct Names {
    values: Vec<String>,
    blocks: Vec<String>,
    edges: Vec<String>,
}

impl Names {
    fn new(function: &Function, view: View) -> Self {
        match view {
            View::Normal => Self {
                values: unique(function.values.iter().enumerate().map(|(at, one)| one.name.clone().unwrap_or(format!("value{at}")))),
                blocks: unique(function.blocks.iter().enumerate().map(|(at, one)| one.name.clone().unwrap_or(format!("block{at}")))),
                edges: unique(function.edges.iter().enumerate().map(|(at, one)| one.name.clone().unwrap_or(format!("edge{at}")))),
            },
            View::Normalized => {
                let mut values = vec![String::new(); function.values.len()];
                let defined = function.parameters.iter().chain(
                    function.blocks.iter().flat_map(|block| block.instructions.iter().flat_map(|one| one.results.iter())),
                );
                for (at, id) in defined.enumerate() {
                    if let Some(slot) = values.get_mut(id.0 as usize) {
                        *slot = format!("v{at}");
                    }
                }
                let mut edges = vec![String::new(); function.edges.len()];
                for (at, edge) in function.blocks.iter().flat_map(|block| block.successor_edges()).enumerate() {
                    if let Some(slot) = edges.get_mut(edge.0 as usize) {
                        *slot = format!("e{at}");
                    }
                }
                Self { values, blocks: (0..function.blocks.len()).map(|at| format!("b{at}")).collect(), edges }
            }
        }
    }

    fn value(&self, id: ValueId) -> &str {
        self.values.get(id.0 as usize).map_or("?", String::as_str)
    }
}

/// `names` with repeats suffixed `.1`, `.2`, ...; `true` and `false` are constants.
fn unique(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut taken: HashSet<String> = ["true", "false"].map(str::to_owned).into();
    names
        .map(|name| {
            let mut chosen = name.clone();
            let mut next = 1;
            while !taken.insert(chosen.clone()) {
                chosen = format!("{name}.{next}");
                next += 1;
            }
            chosen
        })
        .collect()
}

struct Printer<'a> {
    context: &'a MirContext,
    function: &'a Function,
    names: Names,
}

impl Printer<'_> {
    fn instruction(&self, instruction: &Instruction) -> String {
        let operands = &instruction.operands;
        let expected = expected_types(self.context, self.function, instruction);
        let operand = |at: usize| self.operand(operands[at], expected[at]);
        let body = match instruction.opcode {
            Opcode::Goto => format!("goto {}", self.target(operands[0])),
            Opcode::If => format!("if {} goto {} else {}", operand(0), self.target(operands[1]), self.target(operands[2])),
            Opcode::Return if operands.is_empty() => "return".to_owned(),
            Opcode::Return => format!("return {}", (0..operands.len()).map(operand).collect::<Vec<_>>().join(", ")),
            Opcode::Unreachable => "unreachable".to_owned(),
            Opcode::Phi => {
                let inputs: Vec<String> =
                    (0..operands.len()).step_by(2).map(|at| format!("from {}: {}", self.source(operands[at]), operand(at + 1))).collect();
                let joined = if inputs.len() == 1 { format!(" {}", inputs[0]) } else { format!("\n        {}", inputs.join("\n        ")) };
                format!("{} = phi{joined}", self.results(instruction, true))
            }
            Opcode::Compare(predicate) => {
                let (mnemonic, operator) = predicate.spelling();
                format!("{} = {mnemonic} {} {operator} {}", self.results(instruction, false), operand(0), operand(1))
            }
            opcode => {
                let stated = inferred(self.context, self.function, instruction).is_none();
                let list = (0..operands.len()).map(operand).collect::<Vec<_>>().join(", ");
                format!("{} = {} {list}", self.results(instruction, stated), opcode.mnemonic())
            }
        };
        format!("    {body}\n")
    }

    fn results(&self, instruction: &Instruction, typed: bool) -> String {
        let one = |&id: &ValueId| {
            let name = self.names.value(id);
            if typed { format!("{name}: {}", self.context.display(self.function.value(id).ty)) } else { name.to_owned() }
        };
        instruction.results.iter().map(one).collect::<Vec<_>>().join(", ")
    }

    fn operand(&self, operand: Operand, expected: Option<TypeId>) -> String {
        match operand {
            Operand::Value(id) => self.names.value(id).to_owned(),
            Operand::Edge(edge) => self.names.edges[edge.0 as usize].clone(),
            Operand::Constant(constant) => {
                let bits = self.context.int_bits(constant.ty).unwrap_or(128);
                let text = match (bits, constant.bits) {
                    (1, 0) => "false".to_owned(),
                    (1, _) => "true".to_owned(),
                    _ => signed(constant.bits, bits).to_string(),
                };
                if bits == 1 || expected == Some(constant.ty) { text } else { format!("{text}:{}", self.context.display(constant.ty)) }
            }
        }
    }

    /// `label`, or `label as edge` where another edge joins the same two blocks.
    fn target(&self, operand: Operand) -> String {
        let Operand::Edge(edge) = operand else { return "?".to_owned() };
        let label = &self.names.blocks[self.function.edge(edge).target.0 as usize];
        if self.parallel(edge) { format!("{label} as {}", self.names.edges[edge.0 as usize]) } else { label.clone() }
    }

    /// A phi input's edge: its source block's label unless that is ambiguous.
    fn source(&self, operand: Operand) -> String {
        let Operand::Edge(edge) = operand else { return "?".to_owned() };
        match self.function.edge_sources().get(edge.0 as usize).copied().flatten() {
            Some(source) if !self.parallel(edge) => self.names.blocks[source.0 as usize].clone(),
            _ => self.names.edges[edge.0 as usize].clone(),
        }
    }

    fn parallel(&self, edge: EdgeId) -> bool {
        let sources = self.function.edge_sources();
        let key = |one: usize| (sources[one], self.function.edges[one].target);
        (0..self.function.edges.len()).filter(|&one| key(one) == key(edge.0 as usize)).count() > 1
    }
}

/// The result type the operands fix, reading only values: what a reader
/// gets without the definition stating it.
pub fn inferred(context: &MirContext, function: &Function, instruction: &Instruction) -> Option<TypeId> {
    let known: Vec<Option<TypeId>> = instruction
        .operands
        .iter()
        .map(|operand| match operand {
            Operand::Value(id) => function.values.get(id.0 as usize).map(|one| one.ty),
            _ => None,
        })
        .collect();
    instruction.opcode.infer(context, &known)
}

/// The type each operand's role gives it, which a constant there need not state.
pub fn expected_types(context: &MirContext, function: &Function, instruction: &Instruction) -> Vec<Option<TypeId>> {
    let value_type = |operand: &Operand| match operand {
        Operand::Value(id) => function.values.get(id.0 as usize).map(|one| one.ty),
        _ => None,
    };
    let result = instruction.results.first().and_then(|id| function.values.get(id.0 as usize)).map(|one| one.ty);
    let slots = instruction.opcode.slots(instruction.operands.len());
    let shared = slots.iter().zip(&instruction.operands).find_map(|(slot, operand)| value_type(operand).filter(|_| *slot == Slot::Shared));
    slots
        .iter()
        .map(|slot| match slot {
            Slot::Bool => Some(context.bool()),
            Slot::Result => result,
            Slot::Shared => shared,
            Slot::Return(at) => function.returns.get(*at).copied(),
            Slot::Free | Slot::Edge => None,
        })
        .collect()
}
