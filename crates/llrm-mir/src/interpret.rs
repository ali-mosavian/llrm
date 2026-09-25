//! Executes verified MIR: the oracle a transformed function is checked against.

use crate::function::{BlockId, EdgeId, Function, Operand, mask, signed};
use crate::opcode::{Opcode, Predicate};
use crate::types::MirContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trap {
    Unreachable,
    /// More instructions ran than the caller allowed.
    OutOfFuel,
    /// What the verifier refuses: an unset value, a missing edge, a bad arity.
    Malformed(String),
}

/// `function` applied to `arguments`, each an integer's bits; the returned values' bits.
pub fn run(context: &MirContext, function: &Function, arguments: &[u128], mut fuel: u64) -> Result<Vec<u128>, Trap> {
    if arguments.len() != function.parameters.len() {
        return Err(Trap::Malformed(format!("{} arguments for {} parameters", arguments.len(), function.parameters.len())));
    }
    let width = |id: crate::function::ValueId| context.int_bits(function.value(id).ty).unwrap_or(128);
    let mut values: Vec<Option<u128>> = vec![None; function.values.len()];
    for (&id, &argument) in function.parameters.iter().zip(arguments) {
        values[id.0 as usize] = Some(argument & mask(width(id)));
    }
    let read = |values: &[Option<u128>], operand: &Operand| match operand {
        Operand::Value(id) => values[id.0 as usize].ok_or_else(|| Trap::Malformed(format!("value{} is read before it is set", id.0))),
        Operand::Constant(constant) => Ok(constant.bits),
        Operand::Edge(_) => Err(Trap::Malformed("an edge read as a value".to_owned())),
    };
    let mut block = BlockId(0);
    let mut arrived: Option<EdgeId> = None;
    loop {
        let instructions = &function.blocks[block.0 as usize].instructions;
        // Phis read together, along the edge taken, before any of them is written.
        let phis = instructions.iter().take_while(|one| one.opcode == Opcode::Phi).count();
        let mut chosen = Vec::new();
        for phi in &instructions[..phis] {
            let input = phi.operands.chunks(2).find(|pair| Some(pair[0]) == arrived.map(Operand::Edge));
            let Some(pair) = input else { return Err(Trap::Malformed(format!("phi #{} has no input along its edge", phi.id.0))) };
            chosen.push((phi.results[0], read(&values, &pair[1])?));
        }
        for (id, value) in chosen {
            values[id.0 as usize] = Some(value);
        }
        let mut next = None;
        for instruction in &instructions[phis..] {
            fuel = fuel.checked_sub(1).ok_or(Trap::OutOfFuel)?;
            let operands = &instruction.operands;
            let edge = |at: usize| match operands.get(at) {
                Some(Operand::Edge(edge)) => Ok(*edge),
                _ => Err(Trap::Malformed(format!("#{} has no edge at {at}", instruction.id.0))),
            };
            let taken = match instruction.opcode {
                Opcode::Goto => Some(edge(0)?),
                Opcode::If => Some(if read(&values, &operands[0])? != 0 { edge(1)? } else { edge(2)? }),
                Opcode::Return => return operands.iter().map(|one| read(&values, one)).collect(),
                Opcode::Unreachable => return Err(Trap::Unreachable),
                opcode => {
                    let result = instruction.results[0];
                    let bits = width(result);
                    let inputs: Vec<u128> = operands.iter().map(|one| read(&values, one)).collect::<Result<_, _>>()?;
                    let from = match operands.first() {
                        Some(Operand::Value(id)) => width(*id),
                        Some(Operand::Constant(constant)) => context.int_bits(constant.ty).unwrap_or(128),
                        _ => bits,
                    };
                    values[result.0 as usize] = Some(evaluate(opcode, &inputs, from) & mask(bits));
                    None
                }
            };
            if taken.is_some() {
                next = taken;
                break;
            }
        }
        let Some(taken) = next else { return Err(Trap::Malformed(format!("block{} has no terminator", block.0))) };
        arrived = Some(taken);
        block = function.edge(taken).target;
    }
}

/// One ordinary instruction over its operands' bits, `from` bits wide; the
/// caller masks the result to its type.
fn evaluate(opcode: Opcode, inputs: &[u128], from: u32) -> u128 {
    let (a, b) = (inputs[0], inputs.get(1).copied().unwrap_or(0));
    match opcode {
        Opcode::Copy | Opcode::Truncate | Opcode::ZeroExtend => a,
        Opcode::SignExtend => signed(a, from) as u128,
        Opcode::Select => if a != 0 { b } else { inputs[2] },
        Opcode::Add(_) => a.wrapping_add(b),
        Opcode::Sub(_) => a.wrapping_sub(b),
        Opcode::Mul(_) => a.wrapping_mul(b),
        Opcode::And => a & b,
        Opcode::Or => a | b,
        Opcode::Xor => a ^ b,
        Opcode::Compare(predicate) => {
            let (x, y) = (signed(a, from), signed(b, from));
            u128::from(match predicate {
                Predicate::Equal => a == b,
                Predicate::NotEqual => a != b,
                Predicate::SignedLess => x < y,
                Predicate::SignedLessEqual => x <= y,
                Predicate::SignedGreater => x > y,
                Predicate::SignedGreaterEqual => x >= y,
                Predicate::UnsignedLess => a < b,
                Predicate::UnsignedLessEqual => a <= b,
                Predicate::UnsignedGreater => a > b,
                Predicate::UnsignedGreaterEqual => a >= b,
            })
        }
        Opcode::Phi | Opcode::Goto | Opcode::If | Opcode::Return | Opcode::Unreachable => unreachable!("not an ordinary opcode"),
    }
}
