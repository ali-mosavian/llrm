//! Recognizers: what a node means where the core raise would otherwise
//! emit it as the machine does. Each is consulted in order before the core;
//! the first to answer owns the node. Ported from llrm-core's
//! `model/mir/sites.rs`: the runtime's long arithmetic, which MIR computes
//! instead of calling.

use iced_x86::Register;
use llrm_bcmachine::abi::runtime;
use llrm_bcmachine::legacy::calls::LEFT_FIRST;
use llrm_bcmachine::model::ir::nodes::Node;
use llrm_mir::{BinaryOp, CastOp};

use crate::emit::{Desc, Emit, Emitter, Kind};
use crate::machine::{FLAGS, from_contract};

/// Owns a node's meaning, or passes (`None`).
pub trait Recognizer: Sync {
    fn node(&self, emitter: &mut Emitter, node: &Node) -> Option<Emit<()>>;
}

/// In the order they are asked. U3–U5 add theirs here.
pub static RECOGNIZERS: &[&dyn Recognizer] = &[&crate::floats::Floats, &Absorbed, &crate::longs::Longs, &crate::division::Division, &crate::copies::Copies];

/// What a runtime long routine computes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Meaning {
    Multiply,
    Quotient,
    Remainder,
    /// The flags of a signed compare.
    Compare,
}

pub fn meaning(name: &str) -> Option<Meaning> {
    Some(match name {
        "B$MUI4" => Meaning::Multiply,
        "B$DVI4" => Meaning::Quotient,
        "B$RMI4" => Meaning::Remainder,
        "B$CPI4" => Meaning::Compare,
        _ => return None,
    })
}

/// The bytes of two longs each routine pops.
const CONSUMES: i64 = 8;

/// `B$MUI4`, `B$DVI4`, `B$RMI4` and `B$CPI4` as the operation they are:
/// the two pushed longs are its operands, DX:AX (EAX under /G3) its answer.
/// Division by zero, which the routine raises as a BASIC error, is left
/// undefined as `sdiv`'s.
pub struct Absorbed;

impl Recognizer for Absorbed {
    fn node(&self, emitter: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let Node::Call(call) = node else { return None };
        let meaning = meaning(&call.name)?;
        Some(absorb(emitter, &call.name, call.insn.at, meaning))
    }
}

fn absorb(emitter: &mut Emitter, name: &str, at: usize, meaning: Meaning) -> Emit<()> {
    let depth = emitter.depth();
    let (first, second) = (emitter.stack_word(depth - 4, 4)?, emitter.stack_word(depth, 4)?);
    let (left, right) = if LEFT_FIRST[name] { (first, second) } else { (second, first) };
    emitter.popped(CONSUMES)?;
    let contract = emitter.unit.facts.contract(at).ok_or_else(|| format!("{name} has no contract"))?.clone();
    let disturbed: Vec<Register> = runtime::disturbs(&contract).into_iter().filter_map(from_contract).filter(|&one| one != FLAGS).collect();
    emitter.clobber(&disturbed, &format!("{name} clobbers it"));
    let op = match meaning {
        Meaning::Compare => {
            let difference = emitter.binary(BinaryOp::Sub, left, right);
            emitter.set_flags(Desc { kind: Kind::Sub, a: left, b: right, r: difference, bits: 32 });
            return Ok(());
        }
        Meaning::Multiply => BinaryOp::Mul,
        Meaning::Quotient => BinaryOp::SDiv,
        Meaning::Remainder => BinaryOp::SRem,
    };
    let result = emitter.binary(op, left, right);
    // EAX under /G3, DX:AX elsewhere; the code reads whichever it expects.
    emitter.set_register(Register::EAX, result)?;
    let word = emitter.b.context.types.int(16);
    let sixteen = emitter.b.int(32, 16);
    let high = emitter.binary(BinaryOp::LShr, result, sixteen);
    let high = emitter.cast(CastOp::Trunc, high, word);
    emitter.set_register(Register::DX, high)
}
