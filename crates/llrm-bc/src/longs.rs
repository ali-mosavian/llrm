//! BC's longs as `i32` values. Adapted from llrm-core's `raising_longs`
//! (`scalar`, `unary`, `arguments`, `_constant_stores`, `sign_fills`): a
//! pair of word operations `pairs` found is one operation on the long, its
//! words the halves of the answer. One whose source words are not known
//! halves of one value stays two word operations, as the old raise left it.

use iced_x86::Register;
use llrm_bcmachine::analysis::flags::Flag;
use llrm_bcmachine::frontends::bc::declen::Insn;
use llrm_bcmachine::model::ir::nodes::{Node, span};
use llrm_bcmachine::objectfile::module::{Addr, Space};
use llrm_mir::{BinaryOp, CastOp, IntPredicate, Opcode, Operand, ValueDef};

use crate::emit::{Bit, Desc, Emit, Emitter, Kind};
use crate::pairs::{Pair, Shape, Source};
use crate::sites::Recognizer;

/// The instruction a value is, and its operands.
fn defined(e: &Emitter, value: Operand) -> Option<(Opcode, Vec<Operand>)> {
    let Operand::Value(value) = value else { return None };
    let ValueDef::Instruction(inst) = e.b.function.value(value).def else { return None };
    let one = e.b.function.instruction(inst);
    Some((one.opcode.clone(), one.operands.clone()))
}

/// The `i32` whose halves `low` and `high` are, where they are known to be
/// one: the words of one value, a word and its sign, or two constants.
pub fn whole(e: &mut Emitter, low: Operand, high: Operand) -> Option<Operand> {
    if e.bits_of(low) != 16 || e.bits_of(high) != 16 {
        return None;
    }
    if let (Some(low), Some(high)) = (e.constant(low), e.constant(high)) {
        return Some(e.b.int(32, ((high & 0xFFFF) << 16 | low & 0xFFFF) as i128));
    }
    if signs(e, low, high) {
        if let Some(low) = e.constant(low) {
            return Some(e.b.int(32, i128::from(low as i16)));
        }
        let long = e.b.context.types.int(32);
        return Some(e.cast(CastOp::SExt, low, long));
    }
    split(e, low, high)
}

/// The value `low` and `high` were cut from: `trunc w` and `trunc (w >> 16)`.
fn split(e: &Emitter, low: Operand, high: Operand) -> Option<Operand> {
    let (Opcode::Cast(CastOp::Trunc), lows) = defined(e, low)? else { return None };
    let (Opcode::Cast(CastOp::Trunc), highs) = defined(e, high)? else { return None };
    let (Opcode::Binary(BinaryOp::LShr), shifted) = defined(e, highs[0])? else { return None };
    let long = lows[0];
    (shifted[0] == long && e.constant(shifted[1]) == Some(16) && e.bits_of(long) == 32).then_some(long)
}

/// Whether `high` is `low`'s sign: `cwd`'s `ashr low, 15`.
fn signs(e: &Emitter, low: Operand, high: Operand) -> bool {
    matches!(defined(e, high), Some((Opcode::Binary(BinaryOp::AShr), operands)) if operands[0] == low && e.constant(operands[1]) == Some(15))
}

/// Whether `high:low` is `low` sign-extended.
pub fn extended(e: &Emitter, low: Operand, high: Operand) -> bool {
    signs(e, low, high)
        || split(e, low, high).and_then(|long| defined(e, long)).is_some_and(|(opcode, operands)| opcode == Opcode::Cast(CastOp::SExt) && e.bits_of(operands[0]) == 16)
}

/// A long's pair of word operations as one.
pub struct Longs;

impl Recognizer for Longs {
    fn node(&self, e: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let pair = e.body().pairs.get(&(span(node).0 as i64))?;
        let nodes: Vec<&Node> = (0..pair.nodes.len()).map(|n| if n == 0 { Some(node) } else { e.ahead(n) }).collect::<Option<_>>()?;
        if nodes.iter().zip(&pair.nodes).any(|(one, &at)| span(one).0 as i64 != at) {
            return None;
        }
        let done = long(e, pair, &nodes)?;
        e.consume(nodes.len() - 1);
        Some(done)
    }
}

/// The pair's meaning, or None to leave it to the core.
fn long(e: &mut Emitter, pair: &Pair, nodes: &[&Node]) -> Option<Emit<()>> {
    let memory = pair.memory.filter(|&(_, addr)| fits(e, addr)).map(|(index, _)| (insn(nodes[index]), insn(nodes[1 - index])));
    if pair.memory.is_some() && memory.is_none() {
        return None;
    }
    let long = e.b.context.types.int(32);
    Some(match pair.shape {
        Shape::Load => (|| {
            let at = address(e, memory)?;
            let loaded = e.b.load(long, at, false, "");
            set(e, pair, loaded)
        })(),
        Shape::Store => {
            let value = known(e, pair.low, pair.high)?;
            address(e, memory).map(|at| e.b.store(value, at, false))
        }
        Shape::Constant(value) => {
            let value = e.b.int(32, i128::from(value));
            address(e, memory).map(|at| e.b.store(value, at, false))
        }
        Shape::Alu(name, source) => {
            let value = known(e, pair.low, pair.high)?;
            let operand = operand(e, source)?;
            (|| {
                let operand = match operand {
                    Some(operand) => operand,
                    None => {
                        let at = address(e, memory)?;
                        e.b.load(long, at, false, "")
                    }
                };
                arithmetic(e, pair, name, value, operand, nodes[1])
            })()
        }
        Shape::Not => {
            let value = known(e, pair.low, pair.high)?;
            let ones = e.b.int(32, -1);
            let result = e.binary(BinaryOp::Xor, value, ones);
            set(e, pair, result)
        }
        Shape::Neg => {
            let value = known(e, pair.low, pair.high)?;
            let zero = e.b.int(32, 0);
            let result = e.binary(BinaryOp::Sub, zero, value);
            (|| {
                set(e, pair, result)?;
                let high = e.register(pair.high)?;
                // The flags are `neg hi`'s, of the word it negated.
                let (zero, one) = (e.b.int(16, 0), e.b.int(16, 1));
                let negated = e.binary(BinaryOp::Sub, zero, high);
                e.flags(Some(Desc { kind: Kind::Neg, a: negated, b: one, r: high, bits: 16 }), nodes[2].effects());
                Ok(())
            })()
        }
        Shape::Push(source) => {
            let value = operand(e, source)?;
            (|| {
                let value = match value {
                    Some(value) => value,
                    None => {
                        let at = address(e, memory)?;
                        e.b.load(long, at, false, "")
                    }
                };
                e.push(value)
            })()
        }
    })
}

/// A long operand, known now, or None if it is in memory; None outer where
/// its register words are not known halves of one value.
fn operand(e: &mut Emitter, source: Source) -> Option<Option<Operand>> {
    Some(match source {
        Source::Memory => None,
        Source::Immediate(value) => Some(e.b.int(32, i128::from(value))),
        Source::Pair(low, high) => Some(known(e, low, high)?),
    })
}

/// The long in `low:high`, where its words are known halves of one value.
fn known(e: &mut Emitter, low: Register, high: Register) -> Option<Operand> {
    let (low, high) = (e.register(low).ok()?, e.register(high).ok()?);
    whole(e, low, high)
}

/// `name` of two longs; the flags are the high word's `adc`, `sbb` or logic:
/// the long's but for ZF, which sees only the high word.
fn arithmetic(e: &mut Emitter, pair: &Pair, name: &str, value: Operand, operand: Operand, high: &Node) -> Emit<()> {
    let (op, kind) = match name {
        "add" => (BinaryOp::Add, Kind::Add),
        "sub" => (BinaryOp::Sub, Kind::Sub),
        "and" => (BinaryOp::And, Kind::Logic),
        "or" => (BinaryOp::Or, Kind::Logic),
        "xor" => (BinaryOp::Xor, Kind::Logic),
        other => return Err(format!("a long {other}")),
    };
    let result = e.binary(op, value, operand);
    set(e, pair, result)?;
    let effects = high.effects();
    e.flags(Some(Desc { kind, a: value, b: operand, r: result, bits: 32 }), effects);
    if !(effects.flags_written & Flag::ZF).is_empty() {
        let word = e.register(pair.high)?;
        let zero = e.b.int(16, 0);
        let z = e.icmp(IntPredicate::Eq, word, zero);
        e.set_bit(Bit::Z, z);
    }
    Ok(())
}

/// The pair's words, from `value`.
fn set(e: &mut Emitter, pair: &Pair, value: Operand) -> Emit<()> {
    let word = e.b.context.types.int(16);
    let low = e.cast(CastOp::Trunc, value, word);
    let sixteen = e.b.int(32, 16);
    let high = e.binary(BinaryOp::LShr, value, sixteen);
    let high = e.cast(CastOp::Trunc, high, word);
    e.set_register(pair.low, low)?;
    e.set_register(pair.high, high)
}

/// The low word's address; the high word's is checked as the core would.
fn address(e: &mut Emitter, memory: Option<(&Insn, &Insn)>) -> Emit<Operand> {
    let (low, high) = memory.ok_or("a long pair without memory")?;
    e.pointer(high)?;
    e.pointer(low)
}

/// Whether the long at `addr` is inside one object.
fn fits(e: &Emitter, addr: Addr) -> bool {
    addr.space != Space::Segment || e.unit.objects.at(addr.index, addr.disp).is_some_and(|object| addr.disp + 4 <= object.end)
}

fn insn(node: &Node) -> &Insn {
    match node {
        Node::Long(one) => &one.insn,
        Node::Opaque(one) => &one.insn,
        Node::Call(one) => &one.insn,
        _ => unreachable!("a pair is of instructions"),
    }
}
