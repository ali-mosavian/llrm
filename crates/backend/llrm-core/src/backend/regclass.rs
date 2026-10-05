//! The register class of each value: the one place that states which registers a value may be in.
//!
//! Owned here, read by the spiller (`ssaspill`) and the allocator (`allocate`), `coalesce` and `constrain`: one answer
//! (#441). The target's own sets (`target::ADDRESSING`, `target::BYTE`, `Segments::selectors`) are the vocabulary; what
//! an operand needs of them comes from the instruction.

use std::collections::BTreeSet;

use iced_x86::Register;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::{_clobbered, _masks, _unread_move};
use crate::backend::target::{self, Segments};
use crate::model::ir::{Held, Loc, Operation, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

pub type Classes = IndexMap<u32, BTreeSet<Register>>;

fn _restrict(out: &mut Classes, value: u32, choices: &BTreeSet<Register>) {
    let now: BTreeSet<Register> = match out.get(&value) {
        Some(had) => had.intersection(choices).copied().collect(),
        None => choices.clone(),
    };
    out.insert(value, now);
}

/// The register class each value is confined to, where it is confined: a word address pair's base is confined to the
/// base registers and its index to the index registers, as the operand spells them (`AddressRoles` has chosen).
pub fn classes(body: &LirBody, segments: &Segments) -> Classes {
    collected(body, segments, false, Pairs::Positional, &mut Vec::new())
}

/// `classes` for a body whose word address pairs have not been given roles yet: both registers of a pair may be in
/// either the base or the index registers, which of the two is open.
pub fn open_classes(body: &LirBody, segments: &Segments) -> Classes {
    collected(body, segments, false, Pairs::Open, &mut Vec::new())
}

/// `classes` or `open_classes`; `optimistic` is the checker's reading of a body the coalescer has not merged yet: the
/// two sides of a phi edge's copy also join, and address classes are read through webs. An allocator cannot take
/// that, the two sides being values of their own, and SsaSpill does not price it yet.
pub fn classes_with(body: &LirBody, segments: &Segments, optimistic: bool, open: bool) -> Classes {
    collected(body, segments, optimistic, if open { Pairs::Open } else { Pairs::Positional }, &mut Vec::new())
}

/// The classes the operands force, with every word address pair's registers left unconfined: what `AddressRoles` may
/// choose among.
pub fn forced_classes(body: &LirBody, segments: &Segments) -> Classes {
    collected(body, segments, false, Pairs::Free, &mut Vec::new())
}

/// What `collected` does with a word address pair.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Pairs {
    /// The operand's own base and index.
    Positional,
    /// Either register may be the base or the index.
    Open,
    /// No restriction from the pair.
    Free,
}

/// How an instruction names a value that confines it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Byte,
    Base,
    Index,
}

/// One read of a value that confines it: where it is read, and the registers that read allows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Use {
    pub block: usize,
    pub insn: usize,
    pub value: u32,
    pub class: BTreeSet<Register>,
    pub role: Role,
    /// The instruction writes the value there, rather than reads it.
    pub defining: bool,
}

/// Every read that confines a value on its own, a byte operand or an address base or index. A value whose uses
/// share no register is one the classes leave with none.
pub fn confining_uses(body: &LirBody, segments: &Segments) -> Vec<Use> {
    let mut uses = Vec::new();
    collected(body, segments, false, Pairs::Positional, &mut uses);
    uses
}

fn collected(body: &LirBody, segments: &Segments, optimistic: bool, pairs: Pairs, uses: &mut Vec<Use>) -> Classes {
    let mut out: Classes = IndexMap::default();
    let mut selecting: BTreeSet<u32> = BTreeSet::new();
    let mut numeric: BTreeSet<u32> = BTreeSet::new();
    let either: BTreeSet<Register> = target::WORD_BASES.union(&target::WORD_INDEXES).copied().collect();
    let bytes: BTreeSet<Register> = BTreeSet::from([Register::AX, Register::BX, Register::CX, Register::DX]);

    for (at, block) in body.blocks.iter().enumerate() {
        for (position, one) in block.insns.iter().enumerate() {
            let Some(what) = &one.what else {
                continue;
            };
            let mut restrict = |out: &mut Classes, value: u32, choices: &BTreeSet<Register>, role: Role, defining: bool| {
                uses.push(Use { block: at, insn: position, value, class: choices.clone(), role, defining });
                _restrict(out, value, choices);
            };
            // A string op's segment operands are selectors, as a far access's are.
            let segments: &[Loc] = match (what.op, what.sources.len()) {
                (Operation::Copy, 4 | 5) => &what.sources[what.sources.len() - 2..],
                (Operation::Fill, 3 | 4) => &what.sources[what.sources.len() - 1..],
                _ => &[],
            };
            for place in segments {
                if let Loc::Held(held) = place {
                    selecting.insert(held.value);
                }
            }
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        selecting.insert(selector.value);
                    }
                    if let Some(base) = cell.base {
                        numeric.insert(base.value);
                    }
                }
                if let Loc::Held(held) = place {
                    if (held.width != 2 || !_SEGMENT_OPERANDS.contains(&what.op)) && !segments.iter().any(|one| matches!(one, Loc::Held(other) if other.value == held.value)) {
                        numeric.insert(held.value);
                    }
                }
                if let Loc::Mem(cell) = place {
                    if let (Some(base), None) = (cell.base, cell.index) {
                        if base.width == 2 {
                            let registers = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                                &*target::WORD_INDEXES
                            } else {
                                &*target::ADDRESSING
                            };
                            restrict(&mut out, base.value, registers, Role::Base, false);
                        }
                    }
                    if let Some(index) = cell.index {
                        numeric.insert(index.value);
                        if index.width == 2 {
                            let pair = cell.base.filter(|base| base.width == 2 && cell.scale == 1);
                            match (pair, pairs) {
                                (Some(_), Pairs::Free) => {}
                                (Some(base), Pairs::Open) => {
                                    restrict(&mut out, index.value, &either, Role::Index, false);
                                    restrict(&mut out, base.value, &either, Role::Base, false);
                                }
                                _ => {
                                    restrict(&mut out, index.value, &target::WORD_INDEXES, Role::Index, false);
                                    if let Some(base) = cell.base {
                                        restrict(&mut out, base.value, &target::WORD_BASES, Role::Base, false);
                                    }
                                }
                            }
                        }
                    }
                }
                if let Loc::Held(held) = place {
                    if held.width == 1 {
                        restrict(&mut out, held.value, &bytes, Role::Byte, what.dests.contains(place));
                    }
                }
            }
        }
    }
    let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
    for value in selecting.difference(&numeric) {
        _restrict(&mut out, *value, &selectors);
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            if target::far_load(what) {
                if let Loc::Held(held) = &what.dests[1] {
                    _restrict(&mut out, held.value, &selectors);
                }
            }
        }
    }
    _through_webs(body, &selecting, &numeric, &selectors, optimistic, &mut out);
    out
}

/// A phi's result and arguments, and the two sides of a phi edge's copy, are one value once the coalescer has merged
/// them: each takes the class of the web. A value that only a phi or such a copy reads, as one a loop reads through
/// its header phi is, has no class of its own.
fn _through_webs(body: &LirBody, selecting: &BTreeSet<u32>, numeric: &BTreeSet<u32>, selectors: &BTreeSet<Register>, optimistic: bool, out: &mut Classes) {
    let mut web: IndexMap<u32, u32> = IndexMap::default();
    fn find(web: &mut IndexMap<u32, u32>, value: u32) -> u32 {
        let up = *web.entry(value).or_insert(value);
        if up == value {
            return value;
        }
        let top = find(web, up);
        web.insert(value, top);
        top
    }
    for phi in body.blocks.iter().flat_map(|block| &block.phis) {
        for (_, value) in &phi.incoming {
            let (one, other) = (find(&mut web, phi.result), find(&mut web, *value));
            web.insert(one, other);
        }
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns).filter(|one| optimistic && one.group.is_some()) {
        if let ([into], [from]) = (one.defines.as_slice(), one.uses.as_slice()) {
            let (a, b) = (find(&mut web, *into), find(&mut web, *from));
            web.insert(a, b);
        }
    }
    let mut members: IndexMap<u32, Vec<u32>> = IndexMap::default();
    for value in web.keys().copied().collect::<Vec<_>>() {
        let top = find(&mut web, value);
        members.entry(top).or_default().push(value);
    }
    for list in members.values().filter(|list| list.len() > 1) {
        // The web has a class where its members agree; a member that has one of its own keeps it, as the allocator
        // copies between members of different classes.
        let mut agreed: Option<BTreeSet<Register>> = None;
        let mut agree = true;
        for value in list {
            if let Some(mine) = out.get(value) {
                match &agreed {
                    Some(so_far) if so_far != mine => agree = false,
                    Some(_) => {}
                    None => agreed = Some(mine.clone()),
                }
            }
        }
        if agree && agreed.is_none() && list.iter().any(|value| selecting.contains(value)) && !list.iter().any(|value| numeric.contains(value)) {
            agreed = Some(selectors.clone());
        }
        // Address classes are the checker's reading only: SsaSpill holds fewer values where it sees them, which the
        // allocator's own spills do not make up for (deedlines COPPER -Os: 112k reloads, 140k with them).
        let wanted = optimistic || agreed.as_ref().is_some_and(|class| class.iter().all(|register| selectors.contains(register)));
        if let (true, true, Some(class)) = (agree, wanted, agreed) {
            let bare: Vec<u32> = list.iter().copied().filter(|value| !out.contains_key(value)).collect();
            for value in bare {
                out.insert(value, class.clone());
            }
        }
    }
}


pub(crate) const _SEGMENT_OPERANDS: [Operation; 3] = [Operation::Move, Operation::Push, Operation::Pop];


/// A point of a body at which the values live cannot all sit in registers their classes allow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub block: i64,
    pub position: usize,
    pub file: &'static str,
    pub why: Why,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Why {
    /// More values live than the file has registers.
    Crowded { live: usize, registers: usize },
    /// Values an instruction acts on, which no assignment gives distinct registers of their classes.
    Unmatched(Vec<u32>),
}

/// Whether every value of `wanted` can take a distinct register of its own set: a bipartite matching.
fn matched(wanted: &[(u32, BTreeSet<Register>)]) -> bool {
    fn place(at: usize, wanted: &[(u32, BTreeSet<Register>)], taken: &mut IndexMap<Register, usize>, seen: &mut BTreeSet<Register>) -> bool {
        for register in &wanted[at].1 {
            if !seen.insert(*register) {
                continue;
            }
            let holder = taken.get(register).copied();
            if holder.is_none_or(|other| place(other, wanted, taken, seen)) {
                taken.insert(*register, at);
                return true;
            }
        }
        false
    }
    let mut taken: IndexMap<Register, usize> = IndexMap::default();
    (0..wanted.len()).all(|at| place(at, wanted, &mut taken, &mut BTreeSet::new()))
}

/// The points of `body` at which the values live do not fit the registers their classes allow, per register file:
/// the general registers and the segment registers. A body with none can be coloured, with copies where a value
/// waits in another class's register; one with some cannot, whatever the allocator does. `skip` names the values
/// another pass places (x87, pinned, inputs).
pub fn violations(body: &LirBody, segments: &Segments, skip: &BTreeSet<u32>, open: bool) -> Vec<Violation> {
    let confined = classes_with(body, segments, true, open);
    let (_, live_out) = crate::backend::allocate::live(body);
    let general: BTreeSet<Register> = target::AVAILABLE.iter().map(|one| crate::backend::allocate::_whole(*one)).collect();
    let selectors: BTreeSet<Register> = segments.selectors.iter().copied().collect();
    let files: [(&'static str, &BTreeSet<Register>); 2] = [("general", &general), ("selector", &selectors)];
    let in_file = |value: u32, file: &BTreeSet<Register>, general_file: bool| match confined.get(&value) {
        Some(class) => class.iter().any(|one| file.contains(&crate::backend::allocate::_whole(*one))),
        None => general_file,
    };
    let class_in = |value: u32, file: &BTreeSet<Register>| -> BTreeSet<Register> {
        match confined.get(&value) {
            Some(class) => class.iter().map(|one| crate::backend::allocate::_whole(*one)).filter(|one| file.contains(one)).collect(),
            None => file.clone(),
        }
    };
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut live: BTreeSet<u32> = live_out[&block.at].iter().copied().filter(|value| !skip.contains(value)).collect();
        for (position, one) in block.insns.iter().enumerate().rev() {
            // Two states per instruction: after it (what is live, its results among them) and before it (what it reads
            // is live, its results not yet): a result takes the register of an operand that dies.
            // The copies of one phi edge are one parallel copy: a point at its ends, not between its copies.
            if one.defines.is_empty() && one.uses.is_empty() && one.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty)) {
                continue;
            }
            // An instruction that does nothing (a placeholder left where a copy was made unnecessary) does not end the group.
            let real = |other: &&std::sync::Arc<crate::model::lir::Insn>| !(other.defines.is_empty() && other.uses.is_empty() && other.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty)));
            let grouped = |other: Option<&std::sync::Arc<crate::model::lir::Insn>>| one.group.is_some() && other.is_some_and(|other| other.group == one.group);
            let next = block.insns.iter().skip(position + 1).find(real);
            let previous = block.insns.iter().take(position).rev().find(real);
            let (inside_after, inside_before) = (grouped(next), grouped(previous));
            let after = live.clone();
            let mut before = live.clone();
            for value in &one.defines {
                before.remove(value);
            }
            before.extend(one.uses.iter().copied().filter(|value| !skip.contains(value)));
            for (state, acting_values, inside) in [(&after, &one.defines, inside_after), (&before, &one.uses, inside_before)] {
                if inside {
                    continue;
                }
                for (name, file) in files {
                    let members: Vec<u32> = state.iter().copied().filter(|value| in_file(*value, file, name == "general")).collect();
                    if members.len() > file.len() {
                        out.push(Violation { block: block.at, position, file: name, why: Why::Crowded { live: members.len(), registers: file.len() } });
                        continue;
                    }
                    let acting: Vec<(u32, BTreeSet<Register>)> = acting_values
                        .iter()
                        .copied()
                        .filter(|value| members.contains(value))
                        .collect::<BTreeSet<u32>>()
                        .into_iter()
                        .map(|value| (value, class_in(value, file)))
                        .collect();
                    if !matched(&acting) {
                        out.push(Violation { block: block.at, position, file: name, why: Why::Unmatched(acting.iter().map(|(value, _)| *value).collect()) });
                    }
                }
            }
            live = before;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Addr, Mem, Semantics};
    use crate::model::lir::{LirBlock, Phi};

    /// A far pointer's segment that a loop reads only through its header phi: FADETOCOLOR's selectors had no class of
    /// their own after SsaSpill and counted among the general registers (3 over, 63 points after PhiElimination)
    /// while they sat in selector registers.
    #[test]
    fn test_a_segment_only_a_phi_reads_has_the_class_of_its_web() {
        let (entry_segment, segment, base, loaded) = (1, 2, 3, 4);
        let cell = Mem {
            base: Some(Held { value: base, width: 2 }),
            selector: Some(Held { value: segment, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 2)
        };
        let semantics = |op, name: &str, dests, sources, target| Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let make = |at, defines: Vec<u32>, uses: Vec<u32>, what| Arc::new(Insn::new(at, None, Some(what), defines, uses));
        let entry = LirBlock {
            at: 0,
            insns: vec![
                make(1, vec![entry_segment], vec![], semantics(Operation::Move, "mov", vec![held(entry_segment)], vec![Loc::Imm(crate::model::ir::Imm { value: 1, width: 2, address: None })], None)),
                make(2, vec![base], vec![], semantics(Operation::Move, "mov", vec![held(base)], vec![Loc::Imm(crate::model::ir::Imm { value: 2, width: 2, address: None })], None)),
                make(3, vec![], vec![], semantics(Operation::Jump, "jmp", vec![], vec![], Some(1))),
            ],
            succ: vec![1],
            phis: vec![],
            cold: false,
        };
        let looped = LirBlock {
            at: 1,
            insns: vec![
                make(4, vec![loaded], vec![base, segment], semantics(Operation::Move, "mov", vec![held(loaded)], vec![Loc::Mem(cell)], None)),
                make(5, vec![], vec![], semantics(Operation::Branch, "jne", vec![], vec![], Some(1))),
            ],
            succ: vec![1, 2],
            phis: vec![Phi { result: segment, incoming: vec![(0, entry_segment), (1, segment)] }],
            cold: false,
        };
        let exit = LirBlock { at: 2, insns: vec![make(6, vec![], vec![loaded], semantics(Operation::Return, "ret", vec![], vec![], None))], succ: vec![], phis: vec![], cold: false };
        let body = LirBody::new("f", 0, vec![entry, looped, exit], IndexMap::default(), IndexMap::default());
        let segments = &target::BUILT_IN;
        let webs = classes(&body, segments);
        assert_eq!(webs.get(&entry_segment), webs.get(&segment));
        assert!(webs.get(&entry_segment).is_some());
    }
}
