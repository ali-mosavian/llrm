//! The register class of each value: the one place that states which registers
//! a value may be in.
//!
//! Owned here, read by the spiller (`ssaspill`) and the allocator (`allocate`),
//! `coalesce` and `constrain`: one answer (#441). The target's own sets
//! (`target::ADDRESSING`, `target::BYTE`, `Segments::selectors`) are the
//! vocabulary; what an operand needs of them comes from the instruction.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::{_clobbered, _masks};
use crate::backend::classes::RegisterClasses;
use crate::backend::target::{self, Segments};
use crate::model::ir::{Loc, Operation, Space};
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

pub type Classes = IndexMap<u32, BTreeSet<RegId>>;

fn _restrict(
    out: &mut Classes,
    value: u32,
    choices: &BTreeSet<RegId>,
) {
    let now: BTreeSet<RegId> = match out.get(&value) {
        Some(had) => had.intersection(choices).copied().collect(),
        None => choices.clone(),
    };
    out.insert(value, now);
}

/// The register class each value is confined to, where it is confined.
pub fn classes(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
) -> Classes {
    classes_with(body, prefer_indexes, segments, registers, false)
}

/// `classes`; `optimistic` is the checker's reading of a body the coalescer has
/// not merged yet: the two sides of a phi edge's copy also join, and address
/// classes are read through webs. An allocator cannot take that, the two sides
/// being values of their own, and SsaSpill does not price it yet.
pub fn classes_with(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    optimistic: bool,
) -> Classes {
    collected(body, prefer_indexes, segments, registers, optimistic, None, None)
}

/// What a caller that has already found the body's intervals and clobber masks
/// gives `classes`, which reads them for its `[word+word]` roles and would
/// number the body and find both again.
pub struct Found<'a> {
    pub live: &'a dyn crate::backend::live::Ranges,
    pub masks: &'a crate::backend::allocate::Masks,
}

/// `classes`, given the body's intervals and masks.
pub fn classes_given(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    found: &Found,
) -> Classes {
    collected(body, prefer_indexes, segments, registers, false, None, Some(found))
}

/// How an instruction names a value that confines it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Byte,
    Base,
    Index,
}

/// One read of a value that confines it: where it is read, and the registers
/// that read allows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Use {
    pub block: usize,
    pub insn: usize,
    pub value: u32,
    pub class: BTreeSet<RegId>,
    pub role: Role,
    /// The instruction writes the value there, rather than reads it.
    pub defining: bool,
}

/// Every read that confines a value on its own, a byte operand or an address
/// base or index. A value whose uses share no register is one the classes leave
/// with none.
pub fn confining_uses(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> Vec<Use> {
    let mut uses = Vec::new();
    collected(body, &BTreeSet::new(), segments, registers, false, Some(&mut uses), None);
    uses
}

/// Which of the sets of registers an operand confines a value to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Kind {
    WordIndexes,
    Addressing,
    WordBases,
    Bytes,
}

impl Kind {
    const ALL: [Kind; 4] = [Kind::WordIndexes, Kind::Addressing, Kind::WordBases, Kind::Bytes];

    fn registers<'a>(
        self,
        registers: &'a RegisterClasses,
    ) -> &'a BTreeSet<RegId> {
        match self {
            Kind::WordIndexes => &registers.word_indexes,
            Kind::Addressing => &registers.addressing,
            Kind::WordBases => &registers.word_bases,
            Kind::Bytes => &registers.byte_words,
        }
    }
}

/// What one instruction says of the values it names, apart from the others.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Item {
    Restrict {
        value: u32,
        kind: Kind,
        role: Role,
        defining: bool,
    },
    /// Read as a number, not as a segment selector.
    Numeric(u32),
    /// Read as a segment selector.
    Selecting(u32),
    /// A base and an index that may be either way round.
    Pair(u32, u32),
    /// The selector of a far load.
    Far(u32),
}

/// The items of `one`, appended.
pub fn contribution(
    regs: Regs,
    one: &crate::model::lir::Insn,
    items: &mut Vec<Item>,
) {
    let Some(what) = &one.what else {
        return;
    };
    // A string op's segment operands are selectors, as a far access's are.
    let segments: &[Loc] = match (what.op, what.sources.len()) {
        (Operation::Copy, 4 | 5) => &what.sources[what.sources.len() - 2..],
        (Operation::Fill, 3 | 4) => &what.sources[what.sources.len() - 1..],
        _ => &[],
    };
    for place in segments {
        if let Loc::Held(held) = place {
            items.push(Item::Selecting(held.value));
        }
    }
    for place in what.dests.iter().chain(&what.sources) {
        if let Loc::Mem(cell) = place {
            if let Some(selector) = cell.selector {
                items.push(Item::Selecting(selector.value));
            }
            if let Some(base) = cell.base {
                items.push(Item::Numeric(base.value));
            }
        }
        if let Loc::Held(held) = place {
            if (held.width != 2 || !_SEGMENT_OPERANDS.contains(&what.op))
                && !segments.iter().any(|one| matches!(one, Loc::Held(other) if other.value == held.value))
            {
                items.push(Item::Numeric(held.value));
            }
        }
        if let Loc::Mem(cell) = place {
            if let (Some(base), None) = (cell.base, cell.index) {
                if base.width == 2 {
                    let kind = if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                        Kind::WordIndexes
                    } else {
                        Kind::Addressing
                    };
                    items.push(Item::Restrict { value: base.value, kind, role: Role::Base, defining: false });
                }
            }
            if let Some(index) = cell.index {
                items.push(Item::Numeric(index.value));
                if index.width == 2 {
                    if cell.base.is_some_and(|base| base.width == 2) && cell.scale == 1 {
                        items.push(Item::Pair(cell.base.expect("checked").value, index.value));
                    } else {
                        items.push(Item::Restrict {
                            value: index.value,
                            kind: Kind::WordIndexes,
                            role: Role::Index,
                            defining: false,
                        });
                        if let Some(base) = cell.base {
                            items.push(Item::Restrict {
                                value: base.value,
                                kind: Kind::WordBases,
                                role: Role::Base,
                                defining: false,
                            });
                        }
                    }
                }
            }
        }
        if let Loc::Held(held) = place {
            if held.width == 1 {
                items.push(Item::Restrict {
                    value: held.value,
                    kind: Kind::Bytes,
                    role: Role::Byte,
                    defining: what.dests.contains(place),
                });
            }
        }
    }
    if target::far_load(regs, what) {
        if let Loc::Held(held) = &what.dests[1] {
            items.push(Item::Far(held.value));
        }
    }
}

/// What the instructions of a body say of its values, counted, so that a body
/// made from another by a rewrite takes the counts of the instructions it
/// lost and gained and works out the class of the values they name only.
#[derive(Clone, Default)]
pub struct Scan {
    counts: crate::support::hash::HashMap<u32, Counts>,
    pairs: crate::support::hash::HashMap<(u32, u32), u32>,
    /// Each value's class from its counts, before the roles of word addresses
    /// and the webs.
    pre: Classes,
}

#[derive(Clone, Copy, Default)]
struct Counts {
    kinds: [u32; 4],
    selecting: u32,
    numeric: u32,
    far: u32,
}

impl Counts {
    fn is_empty(&self) -> bool {
        self.kinds == [0; 4] && self.selecting == 0 && self.numeric == 0 && self.far == 0
    }
}

impl Scan {
    /// The counts of `body`'s instructions.
    pub fn of(
        body: &LirBody,
        registers: &RegisterClasses,
        segments: &Segments,
    ) -> Self {
        let regs = body.regs();
        let mut scan = Scan::default();
        let mut touched = Vec::new();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            scan.count(regs, one, true, &mut touched);
        }
        scan.classify(touched, registers, segments);
        scan
    }

    /// These counts with the instructions in `gone` taken out and those in
    /// `added` put in.
    pub fn after(
        &self,
        gone: &[std::sync::Arc<crate::model::lir::Insn>],
        added: &[std::sync::Arc<crate::model::lir::Insn>],
        registers: &RegisterClasses,
        segments: &Segments,
    ) -> Self {
        let regs = segments.registers;
        let mut scan = self.clone();
        let mut touched = Vec::new();
        for one in gone {
            scan.count(regs, one, false, &mut touched);
        }
        for one in added {
            scan.count(regs, one, true, &mut touched);
        }
        scan.classify(touched, registers, segments);
        scan
    }

    fn count(
        &mut self,
        regs: Regs,
        one: &crate::model::lir::Insn,
        put: bool,
        touched: &mut Vec<u32>,
    ) {
        let mut items = Vec::new();
        contribution(regs, one, &mut items);
        for item in items {
            let (value, slot): (u32, fn(&mut Counts) -> &mut u32) = match item {
                Item::Restrict { value, kind, .. } => (
                    value,
                    match kind {
                        Kind::WordIndexes => |c| &mut c.kinds[0],
                        Kind::Addressing => |c| &mut c.kinds[1],
                        Kind::WordBases => |c| &mut c.kinds[2],
                        Kind::Bytes => |c| &mut c.kinds[3],
                    },
                ),
                Item::Numeric(value) => (value, |c| &mut c.numeric),
                Item::Selecting(value) => (value, |c| &mut c.selecting),
                Item::Far(value) => (value, |c| &mut c.far),
                Item::Pair(base, index) => {
                    let count = self.pairs.entry((base, index)).or_insert(0);
                    if put {
                        *count += 1;
                    } else {
                        *count -= 1;
                        if *count == 0 {
                            self.pairs.remove(&(base, index));
                        }
                    }
                    continue;
                }
            };
            let counts = self.counts.entry(value).or_default();
            if put {
                *slot(counts) += 1;
            } else {
                *slot(counts) -= 1;
            }
            touched.push(value);
        }
    }

    fn classify(
        &mut self,
        mut touched: Vec<u32>,
        registers: &RegisterClasses,
        segments: &Segments,
    ) {
        touched.sort_unstable();
        touched.dedup();
        let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
        for value in touched {
            let counts = self.counts[&value];
            if counts.is_empty() {
                self.counts.remove(&value);
            }
            let mut sets: Vec<&BTreeSet<RegId>> = Kind::ALL
                .iter()
                .zip(counts.kinds)
                .filter(|(_, n)| *n > 0)
                .map(|(kind, _)| kind.registers(registers))
                .collect();
            if (counts.selecting > 0 && counts.numeric == 0) || counts.far > 0 {
                sets.push(&selectors);
            }
            match sets.split_first() {
                None => {
                    self.pre.swap_remove(&value);
                }
                Some((first, rest)) => {
                    let class: BTreeSet<RegId> =
                        first.iter().copied().filter(|one| rest.iter().all(|set| set.contains(one))).collect();
                    self.pre.insert(value, class);
                }
            }
        }
    }

    /// `classes_given` of the body these counts are of.
    pub fn classes(
        &self,
        body: &LirBody,
        prefer_indexes: &BTreeSet<u32>,
        segments: &Segments,
        registers: &RegisterClasses,
        found: &Found,
    ) -> Classes {
        let mut out = self.pre.clone();
        let mut pairs: Vec<(u32, u32)> = self.pairs.keys().copied().collect();
        pairs.sort_unstable();
        _word_address_roles(&pairs, &mut out, body, prefer_indexes, segments, registers, Some(found));
        let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
        let (selecting, numeric) = (
            |value: u32| self.counts.get(&value).is_some_and(|c| c.selecting > 0),
            |value: u32| self.counts.get(&value).is_some_and(|c| c.numeric > 0),
        );
        _through_webs(body, &selecting, &numeric, &selectors, false, &mut out);
        out
    }
}

fn collected(
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    optimistic: bool,
    mut uses: Option<&mut Vec<Use>>,
    found: Option<&Found>,
) -> Classes {
    let regs = body.regs();
    let mut out: Classes = IndexMap::default();
    let mut selecting: BTreeSet<u32> = BTreeSet::new();
    let mut numeric: BTreeSet<u32> = BTreeSet::new();
    let mut word_pairs: Vec<(u32, u32)> = Vec::new();
    let _scan = llrm_support::debug::span("classes scan");
    let mut items = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        for (position, one) in block.insns.iter().enumerate() {
            items.clear();
            contribution(regs, one, &mut items);
            for item in &items {
                match *item {
                    Item::Restrict { value, kind, role, defining } => {
                        let choices = kind.registers(registers);
                        // The reads are kept only for a caller that asks for
                        // them.
                        if let Some(uses) = uses.as_deref_mut() {
                            uses.push(Use { block: at, insn: position, value, class: choices.clone(), role, defining });
                        }
                        _restrict(&mut out, value, choices);
                    }
                    Item::Numeric(value) => {
                        numeric.insert(value);
                    }
                    Item::Selecting(value) => {
                        selecting.insert(value);
                    }
                    Item::Pair(base, index) => word_pairs.push((base, index)),
                    Item::Far(_) => {}
                }
            }
        }
    }
    drop(_scan);
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    for value in selecting.difference(&numeric) {
        _restrict(&mut out, *value, &selectors);
    }
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let Some(what) = &one.what {
            if target::far_load(regs, what) {
                if let Loc::Held(held) = &what.dests[1] {
                    _restrict(&mut out, held.value, &selectors);
                }
            }
        }
    }
    llrm_support::debug::timed("classes word roles", || {
        _word_address_roles(&word_pairs, &mut out, body, prefer_indexes, segments, registers, found)
    });
    llrm_support::debug::timed("classes webs", || {
        _through_webs(
            body,
            &|value| selecting.contains(&value),
            &|value| numeric.contains(&value),
            &selectors,
            optimistic,
            &mut out,
        )
    });
    out
}

/// A phi's result and arguments, and the two sides of a phi edge's copy, are
/// one value once the coalescer has merged them: each takes the class of the
/// web. A value that only a phi or such a copy reads, as one a loop reads
/// through its header phi is, has no class of its own.
fn _through_webs(
    body: &LirBody,
    selecting: &dyn Fn(u32) -> bool,
    numeric: &dyn Fn(u32) -> bool,
    selectors: &BTreeSet<RegId>,
    optimistic: bool,
    out: &mut Classes,
) {
    let mut web: IndexMap<u32, u32> = IndexMap::default();
    fn find(
        web: &mut IndexMap<u32, u32>,
        value: u32,
    ) -> u32 {
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
        // The web has a class where its members agree; a member that has one of
        // its own keeps it, as the allocator copies between members of
        // different classes.
        let mut agreed: Option<BTreeSet<RegId>> = None;
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
        if agree
            && agreed.is_none()
            && list.iter().any(|value| selecting(*value))
            && !list.iter().any(|value| numeric(*value))
        {
            agreed = Some(selectors.clone());
        }
        // Address classes are the checker's reading only: SsaSpill holds fewer
        // values where it sees them, which the allocator's own spills
        // do not make up for (deedlines COPPER -Os: 112k reloads, 140k with
        // them).
        let wanted = optimistic
            || agreed.as_ref().is_some_and(|class| class.iter().all(|register| selectors.contains(register)));
        if let (true, true, Some(class)) = (agree, wanted, agreed) {
            let bare: Vec<u32> = list.iter().copied().filter(|value| !out.contains_key(value)).collect();
            for value in bare {
                out.insert(value, class.clone());
            }
        }
    }
}

/// Choose BX versus SI/DI for commutative `[word+word]` graphs.
fn _word_address_roles(
    pairs: &[(u32, u32)],
    confined: &mut Classes,
    body: &LirBody,
    prefer_indexes: &BTreeSet<u32>,
    segments: &Segments,
    registers: &RegisterClasses,
    found: Option<&Found>,
) {
    // No pair, no component: nothing below would run, and it numbers the body,
    // finds every interval and builds the masks first.
    if pairs.is_empty() {
        return;
    }
    let mut adjacent: IndexMap<u32, BTreeSet<u32>> = IndexMap::default();
    for (base, index) in pairs {
        adjacent.entry(*base).or_default().insert(*index);
        adjacent.entry(*index).or_default().insert(*base);
    }
    let mut unseen: BTreeSet<u32> = adjacent.keys().copied().collect();
    let (own_live, own_masks);
    let (live, masks) = match found {
        Some(found) => (found.live, found.masks),
        None => {
            let numbered = ranges::indexed(body);
            own_live = ranges::intervals(body, Some(&numbered));
            own_masks = _masks(body, &numbered, segments);
            (&own_live as &dyn crate::backend::live::Ranges, &own_masks)
        }
    };
    let word_base = *registers.word_bases.iter().next().expect("one word base");

    let base_penalty = |values: &BTreeSet<u32>| -> i64 {
        values
            .iter()
            .filter_map(|value| live.range(*value))
            .map(|interval| i64::from(_clobbered(interval, word_base, &masks, 2)))
            .sum()
    };

    let allowed = |confined: &Classes, values: &BTreeSet<u32>, choices: &BTreeSet<RegId>| -> bool {
        values
            .iter()
            .all(
                |value| match confined.get(value) {
                    Some(had) => had.intersection(choices).next().is_some(),
                    None => !choices.is_empty(),
                },
            )
    };

    let restrict = |confined: &mut Classes, values: &BTreeSet<u32>, choices: &BTreeSet<RegId>| {
        for value in values {
            _restrict(confined, *value, choices);
        }
    };

    while let Some(&seed) = unseen.iter().next() {
        let mut colors: IndexMap<u32, u8> = IndexMap::from_iter([(seed, 0)]);
        let mut work = vec![seed];
        let mut bipartite = true;
        while let Some(value) = work.pop() {
            for other in &adjacent[&value] {
                match colors.get(other) {
                    None => {
                        let color = 1 - colors[&value];
                        colors.insert(*other, color);
                        work.push(*other);
                    }
                    Some(color) if *color == colors[&value] => bipartite = false,
                    Some(_) => {}
                }
            }
        }
        let component: BTreeSet<u32> = colors.keys().copied().collect();
        for value in &component {
            unseen.remove(value);
        }
        let source_spelling = |confined: &mut Classes| {
            for (base, index) in pairs {
                if component.contains(base) {
                    restrict(confined, &BTreeSet::from([*base]), &registers.word_bases);
                    restrict(confined, &BTreeSet::from([*index]), &registers.word_indexes);
                }
            }
        };
        if !bipartite {
            source_spelling(confined);
            continue;
        }
        let sides: (BTreeSet<u32>, BTreeSet<u32>) = (
            colors.iter().filter(|(_value, color)| **color == 0).map(|(value, _)| *value).collect(),
            colors.iter().filter(|(_value, color)| **color != 0).map(|(value, _)| *value).collect(),
        );
        let options: Vec<(&BTreeSet<u32>, &BTreeSet<u32>)> = [(&sides.0, &sides.1), (&sides.1, &sides.0)]
            .into_iter()
            .filter(|(left, right)| {
                allowed(confined, left, &registers.word_bases) && allowed(confined, right, &registers.word_indexes)
            })
            .collect();
        if options.is_empty() {
            source_spelling(confined);
            continue;
        }
        let key = |option: &(&BTreeSet<u32>, &BTreeSet<u32>)| {
            (
                base_penalty(option.0),
                option.0.intersection(prefer_indexes).count(),
                option.0.len(),
                option.0.iter().copied().collect::<Vec<u32>>(),
            )
        };
        // `min` keeps the first of equal keys.
        let mut best = options[0];
        for option in &options[1..] {
            if key(option) < key(&best) {
                best = *option;
            }
        }
        let (bases, indexes) = (best.0.clone(), best.1.clone());
        restrict(confined, &bases, &registers.word_bases);
        restrict(confined, &indexes, &registers.word_indexes);
    }
}

pub(crate) const _SEGMENT_OPERANDS: [Operation; 3] = [Operation::Move, Operation::Push, Operation::Pop];

/// A point of a body at which the values live cannot all sit in registers their
/// classes allow.
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
    /// Values an instruction acts on, which no assignment gives distinct
    /// registers of their classes.
    Unmatched(Vec<u32>),
}

/// Whether every value of `wanted` can take a distinct register of its own set:
/// a bipartite matching.
fn matched(wanted: &[(u32, BTreeSet<RegId>)]) -> bool {
    fn place(
        at: usize,
        wanted: &[(u32, BTreeSet<RegId>)],
        taken: &mut IndexMap<RegId, usize>,
        seen: &mut BTreeSet<RegId>,
    ) -> bool {
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
    let mut taken: IndexMap<RegId, usize> = IndexMap::default();
    (0..wanted.len()).all(|at| place(at, wanted, &mut taken, &mut BTreeSet::new()))
}

/// The points of `body` at which the values live do not fit the registers their
/// classes allow, per register file: the general registers and the segment
/// registers. A body with none can be coloured, with copies where a value waits
/// in another class's register; one with some cannot, whatever the allocator
/// does. `skip` names the values another pass places (x87, pinned, inputs).
pub fn violations(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
    skip: &BTreeSet<u32>,
) -> Vec<Violation> {
    let confined = classes_with(body, &BTreeSet::new(), segments, registers, true);
    let (_, live_out) = crate::backend::allocate::live(body);
    let general: BTreeSet<RegId> =
        registers.available.iter().map(|one| crate::backend::allocate::_whole(*one)).collect();
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    let files: [(&'static str, &BTreeSet<RegId>); 2] = [("general", &general), ("selector", &selectors)];
    let in_file = |value: u32, file: &BTreeSet<RegId>, general_file: bool| match confined.get(&value) {
        Some(class) => class.iter().any(|one| file.contains(&crate::backend::allocate::_whole(*one))),
        None => general_file,
    };
    let class_in = |value: u32, file: &BTreeSet<RegId>| -> BTreeSet<RegId> {
        match confined.get(&value) {
            Some(class) => class
                .iter()
                .map(|one| crate::backend::allocate::_whole(*one))
                .filter(|one| file.contains(one))
                .collect(),
            None => file.clone(),
        }
    };
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut live: BTreeSet<u32> =
            live_out[&block.at].iter().copied().filter(|value| !skip.contains(value)).collect();
        for (position, one) in block.insns.iter().enumerate().rev() {
            // Two states per instruction: after it (what is live, its results
            // among them) and before it (what it reads is live, its
            // results not yet): a result takes the register of an operand that
            // dies. The copies of one phi edge are one parallel
            // copy: a point at its ends, not between its copies.
            if one.defines.is_empty()
                && one.uses.is_empty()
                && one.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty))
            {
                continue;
            }
            // An instruction that does nothing (a placeholder left where a copy
            // was made unnecessary) does not end the group.
            let real = |other: &&std::sync::Arc<crate::model::lir::Insn>| {
                !(other.defines.is_empty()
                    && other.uses.is_empty()
                    && other.what.as_ref().is_none_or(|what| what.name.as_deref().is_none_or(str::is_empty)))
            };
            let grouped = |other: Option<&std::sync::Arc<crate::model::lir::Insn>>| {
                one.group.is_some() && other.is_some_and(|other| other.group == one.group)
            };
            let next = block.insns.iter().skip(position + 1).find(real);
            let previous = block.insns.iter().take(position).rev().find(real);
            let (inside_after, inside_before) = (grouped(next), grouped(previous));
            let after = live.clone();
            let mut before = live.clone();
            for value in &one.defines {
                before.remove(value);
            }
            before.extend(one.uses.iter().copied().filter(|value| !skip.contains(value)));
            for (state, acting_values, inside) in
                [(&after, &one.defines, inside_after), (&before, &one.uses, inside_before)]
            {
                if inside {
                    continue;
                }
                for (name, file) in files {
                    let members: Vec<u32> =
                        state.iter().copied().filter(|value| in_file(*value, file, name == "general")).collect();
                    if members.len() > file.len() {
                        out.push(Violation {
                            block: block.at,
                            position,
                            file: name,
                            why: Why::Crowded { live: members.len(), registers: file.len() },
                        });
                        continue;
                    }
                    let acting: Vec<(u32, BTreeSet<RegId>)> = acting_values
                        .iter()
                        .copied()
                        .filter(|value| members.contains(value))
                        .collect::<BTreeSet<u32>>()
                        .into_iter()
                        .map(|value| (value, class_in(value, file)))
                        .collect();
                    if !matched(&acting) {
                        out.push(Violation {
                            block: block.at,
                            position,
                            file: name,
                            why: Why::Unmatched(acting.iter().map(|(value, _)| *value).collect()),
                        });
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
    use crate::model::ir::{Addr, Held, Mem, Semantics};
    use crate::model::lir::{Insn, LirBlock, Phi};

    /// A far pointer's segment that a loop reads only through its header phi:
    /// FADETOCOLOR's selectors had no class of their own after SsaSpill and
    /// counted among the general registers (3 over, 63 points after
    /// PhiElimination) while they sat in selector registers.
    #[test]
    fn test_a_segment_only_a_phi_reads_has_the_class_of_its_web() {
        let (entry_segment, segment, base, loaded) = (1, 2, 3, 4);
        let cell = Mem {
            base: Some(Held { value: base, width: 2 }),
            selector: Some(Held { value: segment, width: 2 }),
            ..Mem::new(Some(Addr { segment: (RegId::ES).iced(), ..Addr::new(Space::Far, 0) }), 2)
        };
        let semantics = |op, name: &str, dests, sources, target| Semantics {
            name: Some(name.to_owned()),
            dests,
            sources,
            target,
            ..Semantics::new(op)
        };
        let held = |value| Loc::Held(Held { value, width: 2 });
        let make =
            |at, defines: Vec<u32>, uses: Vec<u32>, what| Arc::new(Insn::new(at, None, Some(what), defines, uses));
        let entry = LirBlock {
            at: 0,
            insns: vec![
                make(
                    1,
                    vec![entry_segment],
                    vec![],
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![held(entry_segment)],
                        vec![Loc::Imm(crate::model::ir::Imm { value: 1, width: 2, address: None })],
                        None,
                    ),
                ),
                make(
                    2,
                    vec![base],
                    vec![],
                    semantics(
                        Operation::Move,
                        "mov",
                        vec![held(base)],
                        vec![Loc::Imm(crate::model::ir::Imm { value: 2, width: 2, address: None })],
                        None,
                    ),
                ),
                make(3, vec![], vec![], semantics(Operation::Jump, "jmp", vec![], vec![], Some(1))),
            ]
            .into(),
            succ: vec![1],
            phis: vec![],
            cold: false,
        };
        let looped = LirBlock {
            at: 1,
            insns: vec![
                make(
                    4,
                    vec![loaded],
                    vec![base, segment],
                    semantics(Operation::Move, "mov", vec![held(loaded)], vec![Loc::Mem(cell)], None),
                ),
                make(5, vec![], vec![], semantics(Operation::Branch, "jne", vec![], vec![], Some(1))),
            ]
            .into(),
            succ: vec![1, 2],
            phis: vec![Phi { result: segment, incoming: vec![(0, entry_segment), (1, segment)] }],
            cold: false,
        };
        let exit = LirBlock {
            at: 2,
            insns: vec![make(6, vec![], vec![loaded], semantics(Operation::Return, "ret", vec![], vec![], None))]
                .into(),
            succ: vec![],
            phis: vec![],
            cold: false,
        };
        let body = LirBody::new("f", 0, vec![entry, looped, exit], IndexMap::default(), IndexMap::default());
        let segments = &target::BUILT_IN;
        let webs = classes(&body, &BTreeSet::new(), segments, &crate::backend::classes::RegisterClasses::m16());
        assert_eq!(webs.get(&entry_segment), webs.get(&segment));
        assert!(webs.get(&entry_segment).is_some());
    }
}
