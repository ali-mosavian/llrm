//! `movsw` and `movsb` as a load, a store and two pointer steps, adapted
//! from llrm-core's `raising_copies`; and `rep stosw` / `rep stosb` as a
//! memset.
//!
//! Only where ES is proven DS's, by `push ds / pop es` (or `push ss`, where
//! the stack is in DGROUP), as the old raise required. The step's sign is the direction flag, known where the body
//! sets it or where it is the runtime's: on entry and after a call, where
//! the runtime contracts keep direction as environment, which is clear. BC
//! relies on that: it copies with a bare `movsw`.

use std::collections::BTreeMap;

use iced_x86::{Code, FlowControl, Mnemonic, Register, RflagsBits};
use llrm_bcmachine::frontends::bc::blocks::Block;
use llrm_bcmachine::frontends::bc::declen::Insn;
use llrm_bcmachine::model::ir::nodes::{Node, span};
use llrm_mir::BinaryOp;

use crate::emit::{Emit, Emitter};
use crate::machine::BodyFacts;
use crate::sites::Recognizer;

pub struct Copies;

impl Recognizer for Copies {
    fn node(&self, e: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let Node::Opaque(one) = node else { return None };
        let raw = &one.insn.insn;
        let width = match raw.code() {
            Code::Movsw_m16_m16 => 2,
            Code::Movsb_m8_m8 => 1,
            _ => return None,
        };
        if raw.has_rep_prefix() || raw.has_repne_prefix() || raw.memory_segment() != Register::DS {
            return None;
        }
        let State { direction: Some(direction), same: true } = before(e.body(), span(node).0, e.unit.objects.names_data(Register::SS))? else { return None };
        let step = direction * width;
        Some((|| {
            let (source, dest) = (e.register(Register::SI)?, e.register(Register::DI)?);
            let from = e.segmented(Register::DS, source)?;
            let ty = e.b.context.types.int(8 * width as u32);
            let value = e.b.load(ty, from, false, "");
            // ES is DS here: DGROUP's.
            let to = e.segmented(Register::DS, dest)?;
            e.b.store(value, to, false);
            let by = e.b.int(16, i128::from(step));
            let (source, dest) = (e.binary(BinaryOp::Add, source, by), e.binary(BinaryOp::Add, dest, by));
            e.set_register(Register::SI, source)?;
            e.set_register(Register::DI, dest)
        })())
    }
}

pub struct Fills;

impl Recognizer for Fills {
    fn node(&self, e: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let Node::Opaque(one) = node else { return None };
        let raw = &one.insn.insn;
        let width = match raw.code() {
            Code::Stosw_m16_AX => 2,
            Code::Stosb_m8_AL => 1,
            _ => return None,
        };
        if !raw.has_rep_prefix() {
            return None;
        }
        let state = before(e.body(), span(node).0, e.unit.objects.names_data(Register::SS))?;
        Some((|| {
            if state != (State { direction: Some(1), same: true }) {
                return Err("rep stos where ES or the direction is unknown".to_owned());
            }
            let value = e.register(Register::AX)?;
            let value = e.constant(value).ok_or("rep stos of a value unknown until run time")?;
            let bytes = value.to_le_bytes();
            if width == 2 && bytes[0] != bytes[1] {
                return Err("rep stosw of a word whose bytes differ".to_owned());
            }
            let (count, dest) = (e.register(Register::CX)?, e.register(Register::DI)?);
            let size = if width == 2 {
                let one = e.b.int(16, 1);
                e.binary(BinaryOp::Shl, count, one)
            } else {
                count
            };
            // ES is DS here: DGROUP's.
            let to = e.segmented(Register::DS, dest)?;
            let &(memset, memset_ty) = e.unit.intrinsics.get(crate::emit::MEMSET).ok_or("@llvm.memset undeclared")?;
            let (byte, volatile) = (e.b.int(8, i128::from(bytes[0])), e.b.int(1, 0));
            e.b.call(memset_ty, llrm_mir::Operand::Constant(memset), &[to, byte, size, volatile], "");
            let end = e.binary(BinaryOp::Add, dest, size);
            e.set_register(Register::DI, end)?;
            let zero = e.b.int(16, 0);
            e.set_register(Register::CX, zero)
        })())
    }
}

/// What a copy needs of the machine: the direction flag, 1 forward or -1
/// back, and whether ES holds DS's selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    direction: Option<i64>,
    same: bool,
}

/// The state before the node at `at`; `stack` whether SS is DS.
fn before(body: &BodyFacts, at: usize, stack: bool) -> Option<State> {
    let seed = body.blocks.first()?.at;
    let mut predecessors: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for block in &body.blocks {
        for &next in &block.succ {
            predecessors.entry(next).or_default().push(block.at);
        }
    }
    // Each block's state on exit, absent while unreached.
    let mut exits: BTreeMap<usize, State> = BTreeMap::new();
    let entry = |exits: &BTreeMap<usize, State>, block: usize| -> Option<State> {
        let mut incoming: Vec<State> = predecessors.get(&block).into_iter().flatten().filter_map(|one| exits.get(one).copied()).collect();
        if block == seed {
            incoming.push(State { direction: Some(1), same: false });
        }
        let first = *incoming.first()?;
        Some(State {
            direction: first.direction.filter(|_| incoming.iter().all(|one| one.direction == first.direction)),
            same: incoming.iter().all(|one| one.same),
        })
    };
    let walk = |state: State, block: &Block, until: Option<usize>| -> Option<State> {
        let (mut state, mut previous) = (state, None);
        for node in body.nodes_of(block) {
            if Some(span(node).0) == until {
                return Some(state);
            }
            state = after(node, previous, state, stack);
            previous = Some(&**node);
        }
        until.is_none().then_some(state)
    };
    let mut moving = true;
    while moving {
        moving = false;
        for block in &body.blocks {
            let Some(state) = entry(&exits, block.at).and_then(|one| walk(one, block, None)) else { continue };
            if exits.insert(block.at, state) != Some(state) {
                moving = true;
            }
        }
    }
    let block = body.blocks.iter().find(|block| block.insns.iter().any(|insn| insn.at == at))?;
    walk(entry(&exits, block.at)?, block, Some(at))
}

/// The state after `node`, `previous` the node before it.
fn after(node: &Node, previous: Option<&Node>, state: State, stack: bool) -> State {
    let Some(insn) = instruction(node) else { return state };
    let raw = &insn.insn;
    let direction = match raw.mnemonic() {
        Mnemonic::Cld => Some(1),
        Mnemonic::Std => Some(-1),
        _ if matches!(insn.flow(), FlowControl::Call | FlowControl::IndirectCall) => Some(1),
        _ if raw.rflags_modified() & RflagsBits::DF != 0 => None,
        _ => state.direction,
    };
    let pushed = previous.and_then(instruction).is_some_and(|one| one.insn.code() == Code::Pushw_DS || stack && one.insn.code() == Code::Pushw_SS);
    let same = if raw.code() == Code::Popw_ES {
        pushed
    } else {
        state.same && node.effects().defs.as_ref().is_some_and(|defs| !defs.contains(&Register::DS) && !defs.contains(&Register::ES))
    };
    State { direction, same }
}

fn instruction(node: &Node) -> Option<&Insn> {
    match node {
        Node::Opaque(one) => Some(&one.insn),
        Node::Long(one) => Some(&one.insn),
        Node::Call(one) => Some(&one.insn),
        Node::Restore(_) | Node::Data(_) => None,
    }
}
