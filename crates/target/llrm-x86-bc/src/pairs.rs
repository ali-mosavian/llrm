//! Which consecutive nodes are the halves of one BC long: the fact the long
//! recognizer and the carving of DGROUP share. Adapted from llrm-core's
//! `pairs` (shapes over values) and `raising_longs` (`_constant_stores`,
//! `arguments`), found here over nodes by `model::ir::lift`'s classification.
//!
//! BC keeps a long in AX:DX or CX:BX, low word first; each half's memory
//! operand names two bytes, the high half's the two above the low's.

use std::collections::BTreeMap;
use std::sync::Arc;

use iced_x86::{Code, Register};
use llrm_x86_bcmachine::frontends::bc::blocks::Block;
use llrm_x86_bcmachine::model::ir::Loc;
use llrm_x86_bcmachine::model::ir::lift::{self, Decoded, Kind as Lifted};
use llrm_x86_bcmachine::model::ir::nodes::{Long, Node, span};
use llrm_x86_bcmachine::objectfile::module::{Addr, Space};
use llrm_x86_bcmachine::support::hash::IndexMap;

/// What a pair computes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// `mov lo,[m] / mov hi,[m+2]`, either order.
    Load,
    /// `mov [m],lo / mov [m+2],hi`, either order.
    Store,
    /// `op lo,x / op' hi,x'`: and, or, xor, add/adc, sub/sbb.
    Alu(&'static str, Source),
    /// `not lo / not hi`.
    Not,
    /// `neg lo / adc hi,0 / neg hi`.
    Neg,
    /// `mov word [m],imm / mov word [m+2],imm`, the long `value`.
    Constant(u32),
    /// `push hi / push lo`: the long's four bytes.
    Push(Source),
}

/// Where a pair's other long is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// `[m]` and `[m+2]`.
    Memory,
    Immediate(u32),
    /// The other pair's words, low and high.
    Pair(Register, Register),
}

/// One long, as consecutive nodes of a block.
#[derive(Clone, Debug)]
pub struct Pair {
    pub shape: Shape,
    /// The pair's low and high words: AX and DX, or CX and BX.
    pub low: Register,
    pub high: Register,
    /// Its nodes' addresses, in order.
    pub nodes: Vec<i64>,
    /// Which node's memory operand is the low word's, and that address.
    pub memory: Option<(usize, Addr)>,
}

impl Pair {
    /// The segment and first of the four bytes it accesses, where it names one.
    pub fn span(&self) -> Option<(i64, i64)> {
        let (_, addr) = self.memory?;
        (addr.space == Space::Segment).then_some((addr.index, addr.disp))
    }
}

/// BC's two pairs: `lift`'s numbering, low word first.
const WORDS: [(Register, Register); 2] = [(Register::AX, Register::DX), (Register::CX, Register::BX)];

/// Every pair in `blocks`, by its first node's address.
pub fn found(
    blocks: &[Block],
    nodes: &IndexMap<i64, Arc<Node>>,
) -> BTreeMap<i64, Pair> {
    let mut out = BTreeMap::new();
    for block in blocks {
        let run: Vec<&Node> =
            block.insns.iter().filter_map(|insn| nodes.get(&(insn.at as i64))).map(|one| &**one).collect();
        let mut index = 0;
        while index < run.len() {
            match at(&run[index..]) {
                Some(pair) => {
                    index += pair.nodes.len();
                    out.insert(pair.nodes[0], pair);
                }
                None => index += 1,
            }
        }
    }
    out
}

/// The pair `run` starts with.
fn at(run: &[&Node]) -> Option<Pair> {
    let ats: Vec<i64> = run.iter().take(3).map(|node| span(node).0 as i64).collect();
    if let [first, second, third, ..] = run {
        if let Some((low, high)) = negate(first, second, third) {
            return Some(Pair { shape: Shape::Neg, low, high, nodes: ats, memory: None });
        }
    }
    let [first, second, ..] = run else { return None };
    let nodes = ats[..2].to_vec();
    if let Some((memory, value)) = constants(first, second) {
        return Some(Pair {
            shape: Shape::Constant(value),
            low: Register::None,
            high: Register::None,
            nodes,
            memory: Some(memory),
        });
    }
    if let Some((source, memory)) = pushes(first, second) {
        return Some(Pair { shape: Shape::Push(source), low: Register::None, high: Register::None, nodes, memory });
    }
    let (Node::Long(a), Node::Long(b)) = (first, second) else { return None };
    let (x, y) = (&a.decoded, &b.decoded);
    if x.pair != y.pair || x.half == y.half {
        return None;
    }
    let (low, high) = WORDS[x.pair];
    let pair = |shape, memory| Some(Pair { shape, low, high, nodes: nodes.clone(), memory });
    let ordered = x.half == 0;
    match (x.kind, y.kind) {
        (Lifted::Load, Lifted::Load) | (Lifted::Store, Lifted::Store) => {
            let (index, lower, upper) = if ordered { (0, x, y) } else { (1, y, x) };
            let memory = adjacent(lower, upper)?;
            // The second load's address must not read what the first wrote.
            let written = if ordered { low } else { high };
            if x.kind == Lifted::Load && memory.base == written {
                return None;
            }
            pair(if x.kind == Lifted::Load { Shape::Load } else { Shape::Store }, Some((index, memory)))
        }
        (Lifted::Alu, Lifted::Alu) if ordered => {
            let &(partner, name) = lift::PAIRED.get(&x.alu?)?;
            (y.alu? == partner).then_some(())?;
            pair(Shape::Alu(name, Source::Memory), Some((0, adjacent(x, y)?)))
        }
        (Lifted::RegAlu, Lifted::RegAlu) if ordered && x.src_pair == y.src_pair => {
            let &(partner, name) = lift::PAIRED.get(&x.alu?)?;
            (y.alu? == partner).then_some(())?;
            let (low, high) = WORDS[x.src_pair];
            pair(Shape::Alu(name, Source::Pair(low, high)), None)
        }
        (Lifted::AluImm, Lifted::AluImm) if ordered => {
            let name = *lift::IMM_FAMILY.get(&x.alu?)?;
            lift::IMM_HIGH_FAMILY.get(name)?.contains(&y.alu?).then_some(())?;
            let value = ((y.imm? as u32 & 0xFFFF) << 16) | (x.imm? as u32 & 0xFFFF);
            pair(Shape::Alu(name, Source::Immediate(value)), None)
        }
        (Lifted::Not, Lifted::Not) => pair(Shape::Not, None),
        _ => None,
    }
}

/// The low half's address, where the high half's names the two bytes above it.
fn adjacent(
    low: &Decoded,
    high: &Decoded,
) -> Option<Addr> {
    let (low, high) = (low.mem?, high.mem?);
    (low.plus(2) == high).then_some(low)
}

/// `neg lo / adc hi,0 / neg hi`: its words.
fn negate(
    first: &Node,
    second: &Node,
    third: &Node,
) -> Option<(Register, Register)> {
    let negated = |node: &Node| match node {
        Node::Opaque(one) if one.insn.code() == Code::Neg_rm16 && !one.insn.reads_memory(0) => {
            Some(one.insn.register(0))
        }
        _ => None,
    };
    let (low, high) = (negated(first)?, negated(third)?);
    let Node::Long(Long { decoded, .. }) = second else { return None };
    let carried = decoded.kind == Lifted::AluImm
        && decoded.imm == Some(0)
        && lift::IMM_HIGH_FAMILY["add"].contains(&decoded.alu?);
    (carried && WORDS[decoded.pair] == (low, high)).then_some((low, high))
}

/// Two word constants stored to adjacent bytes, low first: the address and the
/// long.
fn constants(
    first: &Node,
    second: &Node,
) -> Option<((usize, Addr), u32)> {
    let stored = |node: &Node| match node {
        Node::Opaque(one)
            if one.insn.code() == Code::Mov_rm16_imm16
                && one.effects.stores.len() == 1
                && one.effects.stores[0].width == 2 =>
        {
            Some((one.effects.stores[0].addr?, one.insn.insn.immediate16() as u32))
        }
        _ => None,
    };
    let ((low, lower), (high, upper)) = (stored(first)?, stored(second)?);
    (low.plus(2) == high).then_some(((0, low), (upper << 16) | lower))
}

/// `push hi / push lo` of one long: where it is, and the low word's address.
fn pushes(
    first: &Node,
    second: &Node,
) -> Option<(Source, Option<(usize, Addr)>)> {
    let (Node::Opaque(high), Node::Opaque(low)) = (first, second) else { return None };
    match (high.insn.code(), low.insn.code()) {
        (Code::Push_r16, Code::Push_r16) => {
            let words = (low.insn.register(0), high.insn.register(0));
            WORDS.contains(&words).then_some((Source::Pair(words.0, words.1), None))
        }
        (Code::Push_rm16, Code::Push_rm16) if high.effects.loads.len() == 1 && low.effects.loads.len() == 1 => {
            let (upper, lower) = (high.effects.loads[0].addr?, low.effects.loads[0].addr?);
            (lower.plus(2) == upper).then_some((Source::Memory, Some((1, lower))))
        }
        (Code::Push_imm16 | Code::Pushw_imm8, Code::Push_imm16 | Code::Pushw_imm8) => {
            // A relocated immediate is an address, not a long's word.
            let word = |node: &Node| match node.semantics().sources.first() {
                Some(Loc::Imm(imm)) if imm.address.is_none() => Some(imm.value as u32 & 0xFFFF),
                _ => None,
            };
            Some((Source::Immediate(word(first)? << 16 | word(second)?), None))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use llrm_x86_bcmachine::frontends::bc::blocks::Ends;
    use llrm_x86_bcmachine::frontends::bc::declen::decode;
    use llrm_x86_bcmachine::model::ir::Effects;
    use llrm_x86_bcmachine::objectfile::module::literal_only;

    use super::*;

    /// The pairs in one block of these instructions.
    fn found_in(code: &[u8]) -> Vec<Shape> {
        let (mut insns, mut nodes, mut at) = (Vec::new(), IndexMap::default(), 0);
        while at < code.len() {
            let insn = decode(code, at).expect("an instruction");
            let decoded = lift::classify_with(&insn, &literal_only).expect("a long's half");
            nodes.insert(at as i64, Arc::new(Node::Long(Long::new(insn.clone(), decoded, Effects::no_effect()))));
            at = insn.end();
            insns.push(insn);
        }
        let block = Block { at: 0, end: at, insns, ends: Ends::FallsThrough, succ: Vec::new() };
        found(&[block], &nodes).into_values().map(|pair| pair.shape).collect()
    }

    /// `mov bx,[bx+2] / mov cx,[bx]` reads its low word through the BX it
    /// just loaded: not the four bytes at [bx], which one load would read.
    #[test]
    fn a_high_word_that_moves_the_base_is_no_pair() {
        assert_eq!(found_in(&[0x8B, 0x5F, 0x02, 0x8B, 0x4F, 0x00]), []);
        assert_eq!(found_in(&[0x8B, 0x4F, 0x00, 0x8B, 0x5F, 0x02]), [Shape::Load]);
        assert_eq!(found_in(&[0x8B, 0x57, 0x02, 0x8B, 0x47, 0x00]), [Shape::Load]);
    }
}
