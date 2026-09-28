//! How a group's matcher walks each block, and the window its rules see.
//!
//! The walks are the ones the hand-written passes used, kept exactly: which
//! instructions a window skips, whether a match consumes its instructions
//! or rewrites them in place, and where the walk resumes decide the output.

use std::cell::OnceCell;
use std::collections::HashSet;
use std::sync::Arc;

use crate::backend::cpu::Profile;
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{self, Counter, DeadAfter, id};
use crate::backend::{liveness, regthrash};
use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Reg, Semantics};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, IndexMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    D,
    S,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Reg,
    Mem,
    Imm,
    Held,
    Other,
    Absent,
}

/// The instructions a window steps over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    None,
    Meta,
    Inert,
    Nothing,
}

impl Skip {
    fn skips(self, one: &Insn) -> bool {
        match self {
            Skip::None => false,
            Skip::Meta => one.is_meta(),
            Skip::Inert => peephole::_skippable_nothing(one),
            Skip::Nothing => peephole::_nothing(one),
        }
    }
}

/// One rewritten instruction: what takes a matched one's place, or one to
/// drop with `lir::without`.
pub enum Out {
    Put(Arc<Insn>),
    Drop(Arc<Insn>),
}

/// What a rule makes of the `len` instructions it matched.
pub struct Rewrite {
    pub len: usize,
    pub out: Vec<Out>,
}

/// A group's automaton: `head` tests the first instruction and names the
/// state it reaches, `tail` tests the rest from there and runs the rules;
/// `cross` says whether a gap may take in another instruction.
pub struct Matcher {
    pub width: usize,
    pub head: fn(&Window) -> u32,
    pub tail: fn(&Cx, &Window, u32) -> Option<Rewrite>,
    pub cross: Option<fn(&Cx, &Window, &Arc<Insn>, u32) -> bool>,
}

/// The instructions a rule sees: the first, then those after any gap.
pub struct Window<'a> {
    insns: Vec<&'a Arc<Insn>>,
    gap: &'a [Arc<Insn>],
}

impl<'a> Window<'a> {
    pub fn len(&self) -> usize {
        self.insns.len()
    }

    pub fn slot(&self, k: usize) -> &'a Arc<Insn> {
        self.insns[k]
    }

    pub fn insn(&self, k: usize) -> &'a Arc<Insn> {
        self.insns[k]
    }

    /// What a gap stepped over.
    pub fn gap(&self) -> &'a [Arc<Insn>] {
        self.gap
    }

    pub fn what(&self, k: usize) -> Option<&'a Semantics> {
        self.insns.get(k)?.what.as_ref()
    }

    pub fn op(&self, k: usize) -> Option<Operation> {
        self.what(k).map(|what| what.op)
    }

    pub fn name(&self, k: usize) -> Option<&'a str> {
        self.what(k)?.name.as_deref()
    }

    pub fn dests(&self, k: usize) -> usize {
        self.what(k).map_or(0, |what| what.dests.len())
    }

    pub fn sources(&self, k: usize) -> usize {
        self.what(k).map_or(0, |what| what.sources.len())
    }

    pub fn loc(&self, k: usize, side: Side, index: usize) -> Option<&'a Loc> {
        let what = self.what(k)?;
        match side {
            Side::D => what.dests.get(index),
            Side::S => what.sources.get(index),
        }
    }

    pub fn kind(&self, k: usize, side: Side, index: usize) -> Kind {
        match self.loc(k, side, index) {
            None => Kind::Absent,
            Some(Loc::Reg(_)) => Kind::Reg,
            Some(Loc::Mem(_)) => Kind::Mem,
            Some(Loc::Imm(_)) => Kind::Imm,
            Some(Loc::Held(_)) => Kind::Held,
            Some(Loc::Address(_) | Loc::St(_)) => Kind::Other,
        }
    }

    pub fn width(&self, k: usize, side: Side, index: usize) -> Option<u32> {
        match self.loc(k, side, index)? {
            Loc::Reg(one) => Some(one.width),
            Loc::Mem(one) => Some(one.width),
            Loc::Imm(one) => Some(one.width),
            Loc::Held(one) => Some(one.width),
            Loc::Address(_) | Loc::St(_) => None,
        }
    }

    /// Whether an immediate carries no address.
    pub fn bare(&self, k: usize, side: Side, index: usize) -> Option<bool> {
        self.imm(k, side, index).map(|one| one.address.is_none())
    }

    pub fn value(&self, k: usize, side: Side, index: usize) -> Option<i64> {
        self.imm(k, side, index).map(|one| one.value)
    }

    pub fn reg(&self, k: usize, side: Side, index: usize) -> Option<Reg> {
        match self.loc(k, side, index)? {
            Loc::Reg(one) => Some(*one),
            _ => None,
        }
    }

    pub fn mem(&self, k: usize, side: Side, index: usize) -> Option<&'a Mem> {
        match self.loc(k, side, index)? {
            Loc::Mem(one) => Some(one),
            _ => None,
        }
    }

    pub fn imm(&self, k: usize, side: Side, index: usize) -> Option<&'a Imm> {
        match self.loc(k, side, index)? {
            Loc::Imm(one) => Some(one),
            _ => None,
        }
    }

    pub fn held(&self, k: usize, side: Side, index: usize) -> Option<Held> {
        match self.loc(k, side, index)? {
            Loc::Held(one) => Some(*one),
            _ => None,
        }
    }
}

/// What the rules may ask of the body, each computed once when first asked.
pub struct Facts<'a> {
    pub body: &'a LirBody,
    pub cpu: Option<&'a Profile>,
    exits: OnceCell<IndexMap<i64, Lanes>>,
    flags_out: OnceCell<HashMap<i64, Lanes>>,
    users: OnceCell<Counter>,
}

impl<'a> Facts<'a> {
    pub fn new(body: &'a LirBody, cpu: Option<&'a Profile>) -> Self {
        Self { body, cpu, exits: OnceCell::new(), flags_out: OnceCell::new(), users: OnceCell::new() }
    }

    /// How many instructions and phis read each value.
    pub fn users(&self) -> &Counter {
        self.users.get_or_init(|| {
            let mut users = Counter::default();
            for one in self.body.blocks.iter().flat_map(|block| &block.insns) {
                for value in &one.uses {
                    *users.entry(*value).or_insert(0) += 1;
                }
            }
            for phi in self.body.blocks.iter().flat_map(|block| &block.phis) {
                for (_, value) in &phi.incoming {
                    *users.entry(*value).or_insert(0) += 1;
                }
            }
            users
        })
    }
}

/// One block's facts, as the walk found the block.
pub struct Cx<'a> {
    pub facts: &'a Facts<'a>,
    pub block: &'a LirBlock,
    dead: OnceCell<DeadAfter>,
}

impl Cx<'_> {
    /// The register and flag lanes dead after `one`, an instruction of the block as found.
    pub fn dead_after(&self, one: &Arc<Insn>) -> Lanes {
        let dead = self.dead.get_or_init(|| {
            let exits = self.facts.exits.get_or_init(|| liveness::dead_at_exit(self.facts.body));
            regthrash::_dead_after(self.block, exits[&self.block.at])
        });
        dead[&id(one)]
    }

    /// The flag lanes something may read after the block.
    pub fn flags_out(&self) -> Lanes {
        self.facts.flags_out.get_or_init(|| peephole::_flags_live_out(self.facts.body))[&self.block.at]
    }

    pub fn cpu(&self) -> &Profile {
        self.facts.cpu.expect("a group that prices asks with a CPU")
    }
}

fn run(cx: &Cx, matcher: &Matcher, window: &Window) -> Option<Rewrite> {
    match (matcher.head)(window) {
        0 => None,
        state => (matcher.tail)(cx, window, state),
    }
}

fn blocks(body: &LirBody, facts: &Facts, mut each: impl FnMut(&Cx) -> Vec<Arc<Insn>>) -> LirBody {
    let blocks = body
        .blocks
        .iter()
        .map(|block| block.with_insns(each(&Cx { facts, block, dead: OnceCell::new() })))
        .collect();
    body.with_blocks(blocks)
}

/// Every instruction alone, replaced in place.
pub fn each(body: &LirBody, facts: &Facts, matcher: &Matcher) -> LirBody {
    blocks(body, facts, |cx| {
        cx.block
            .insns
            .iter()
            .map(|one| {
                let window = Window { insns: vec![one], gap: &[] };
                match run(cx, matcher, &window).map(|made| made.out) {
                    Some(mut out) => match out.pop() {
                        Some(Out::Put(made)) => made,
                        _ => unreachable!("an each rule puts one instruction"),
                    },
                    None => Arc::clone(one),
                }
            })
            .collect()
    })
}

/// A rewrite's instructions, those it drops given to `lir::without`.
fn spliced(out: Vec<Out>) -> Vec<Arc<Insn>> {
    let dropped: Vec<Arc<Insn>> = out.iter().filter_map(|one| if let Out::Drop(one) = one { Some(Arc::clone(one)) } else { None }).collect();
    let all: Vec<Arc<Insn>> = out.into_iter().map(|one| match one {
        Out::Put(one) | Out::Drop(one) => one,
    }).collect();
    if dropped.is_empty() {
        return all;
    }
    lir::without(&all, |one| dropped.iter().any(|dropped| Arc::ptr_eq(dropped, one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
}

/// Runs of consecutive instructions, those `skip` names stepped over; a
/// match consumes what it matched, and skipped instructions inside it
/// follow the rewrite.
pub fn window(body: &LirBody, facts: &Facts, matcher: &Matcher, skip: Skip) -> LirBody {
    blocks(body, facts, |cx| {
        let insns = &cx.block.insns;
        let code: Vec<usize> = (0..insns.len()).filter(|at| !skip.skips(&insns[*at])).collect();
        let mut out = Vec::with_capacity(insns.len());
        let (mut next, mut at) = (0, 0);
        while at < code.len() {
            out.extend(insns[next..code[at]].iter().cloned());
            let taken = &code[at..(at + matcher.width).min(code.len())];
            let window = Window { insns: taken.iter().map(|one| &insns[*one]).collect(), gap: &[] };
            match run(cx, matcher, &window) {
                Some(made) => {
                    let last = code[at + made.len - 1];
                    out.extend(spliced(made.out));
                    out.extend(insns[code[at] + 1..last].iter().filter(|one| skip.skips(one)).cloned());
                    next = last + 1;
                    at += made.len;
                }
                None => {
                    out.push(Arc::clone(&insns[code[at]]));
                    next = code[at] + 1;
                    at += 1;
                }
            }
        }
        out.extend(insns[next..].iter().cloned());
        out
    })
}

/// Windows of `width` rewritten in place, moving on `advance` after a
/// match; what a rewrite drops leaves the block through `lir::without`
/// once the walk is done.
pub fn slide(body: &LirBody, facts: &Facts, matcher: &Matcher, advance: usize) -> LirBody {
    blocks(body, facts, |cx| {
        let mut insns = cx.block.insns.clone();
        let mut removed: HashSet<usize> = HashSet::new();
        let mut at = 0;
        while at + matcher.width <= insns.len() {
            if insns[at..at + matcher.width].iter().any(|one| removed.contains(&id(one))) {
                at += 1;
                continue;
            }
            let window = Window { insns: insns[at..at + matcher.width].iter().collect(), gap: &[] };
            let Some(made) = run(cx, matcher, &window) else {
                at += 1;
                continue;
            };
            for (offset, one) in made.out.into_iter().enumerate() {
                match one {
                    Out::Put(one) => insns[at + offset] = one,
                    Out::Drop(one) => {
                        removed.insert(id(&one));
                    }
                }
            }
            at += advance;
        }
        lir::without(&insns, |one| removed.contains(&id(one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
    })
}

/// The first instruction, then each later one the gap has not stopped at,
/// with the ones after it; rewritten in place. The gap takes in an
/// instruction no rule matched at while `cross` allows. `first_original`
/// matches the first as the block had it before the walk; `resume_past`
/// goes on after the last instruction matched rather than after the first.
pub fn gap(body: &LirBody, facts: &Facts, matcher: &Matcher, skip: Skip, first_original: bool, resume_past: bool) -> LirBody {
    let cross = matcher.cross.expect("a gap group says what it may cross");
    blocks(body, facts, |cx| {
        let original = &cx.block.insns;
        let mut insns = original.clone();
        let work: Vec<usize> = (0..original.len()).filter(|at| !skip.skips(&original[*at])).collect();
        let mut at = 0;
        while at < work.len() {
            let first = if first_original { Arc::clone(&original[work[at]]) } else { Arc::clone(&insns[work[at]]) };
            let mut next = at + 1;
            let state = (matcher.head)(&Window { insns: vec![&first], gap: &[] });
            if state != 0 {
                for candidate in at + 1..work.len() {
                    let rest = &work[candidate..(candidate + matcher.width - 1).min(work.len())];
                    let window = Window {
                        insns: std::iter::once(&first).chain(rest.iter().map(|one| &insns[*one])).collect(),
                        gap: &insns[work[at] + 1..work[candidate]],
                    };
                    if let Some(made) = (matcher.tail)(cx, &window, state) {
                        let places: Vec<usize> = std::iter::once(work[at]).chain(rest.iter().copied()).collect();
                        if resume_past {
                            next = candidate + made.len - 1;
                        }
                        for (place, one) in places.into_iter().zip(made.out) {
                            let Out::Put(one) = one else { unreachable!("a gap rule puts in place") };
                            insns[place] = one;
                        }
                        break;
                    }
                    if !cross(cx, &window, &insns[work[candidate]], state) {
                        break;
                    }
                }
            }
            at = next;
        }
        insns
    })
}
