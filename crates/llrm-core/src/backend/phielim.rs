//! Port of `qbopt/backend/phielim.py`: the pass that takes LIR out of SSA.
//!
//! Each phi becomes a copy at the end of every predecessor it names. A copy
//! on a critical edge goes in a block of its own, labelled above physical
//! offsets in a namespace per body entry. LLVM's `PHIElimination`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::support::hash::IndexMap;

use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;

pub struct PhiElimination;

impl LIRTransform for PhiElimination {
    fn class_name(&self) -> &'static str {
        "PhiElimination"
    }

    fn name(&self) -> &str {
        "phielim"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        eliminated(&body)
    }
}

/// What `_split_edges` places on one edge: phi `(result, value)` pairs, or
/// transfers already selected (`selected=True`).
#[derive(Clone, Debug)]
pub enum Split {
    Pairs(Vec<(u32, u32)>),
    Selected(Vec<Arc<Insn>>),
}

fn move_of(into: Held, out_of: Held) -> Semantics {
    Semantics {
        name: Some("mov".to_owned()),
        dests: vec![Loc::Held(into)],
        sources: vec![Loc::Held(out_of)],
        ..Semantics::new(Operation::Move)
    }
}

fn jump_to(target: i64) -> Semantics {
    Semantics {
        name: Some("jmp".to_owned()),
        target: Some(target),
        ..Semantics::new(Operation::Jump)
    }
}

/// `body` with every phi it can lower replaced by copies.
pub fn eliminated(body: &LirBody) -> Result<LirBody, String> {
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let successors: IndexMap<i64, usize> = body.blocks.iter().map(|block| (block.at, block.succ.len())).collect();
    let widths = _widths(body);

    let once = _read_once(body);
    // One per predecessor->successor edge. Every move a phi becomes on
    // that edge is simultaneous with the others and with none outside it.
    let mut groups: IndexMap<(i64, i64), i64> = IndexMap::default();
    let mut edge_group = |where_: i64, into: i64| -> i64 {
        let next = groups.len() as i64 + 1;
        *groups.entry((where_, into)).or_insert(next)
    };

    let mut split: IndexMap<(i64, i64), Split> = IndexMap::default();
    let mut copies: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut rename: IndexMap<u32, u32> = IndexMap::default();
    let mut kept: IndexMap<i64, Vec<Phi>> = IndexMap::default();
    for block in &body.blocks {
        let mut stays = Vec::new();
        let mut crossing: IndexMap<i64, Vec<(u32, u32)>> = IndexMap::default();
        for phi in &block.phis {
            let edges: Vec<(i64, u32)> =
                phi.incoming.iter().copied().filter(|(where_, _)| at_of.contains_key(where_)).collect();
            if edges.len() != phi.incoming.len() {
                stays.push(phi.clone()); // an edge from outside this body
                continue;
            }
            // A one-input phi computes its input. Unify the identities before
            // allocation instead of creating a copy.
            if edges.len() == 1 {
                rename.insert(phi.result, edges[0].1);
                continue;
            }
            if edges.iter().any(|(where_, _)| successors.get(where_).copied().unwrap_or(0) > 1) {
                // A critical edge. Where every incoming value is defined in
                // its own predecessor and read by nothing but this phi, having
                // it define the phi's result outright says what the phi said.
                if edges.iter().all(|&(w, v)| {
                    _defined_in(at_of[&w], v) && once.get(&v).copied() == Some(1) && !_live_after(&at_of, block.at, w, v, phi.result)
                }) {
                    for &(_where, value) in &edges {
                        rename.insert(value, phi.result);
                    }
                    continue;
                }
                // Split the edge, unless nothing on the predecessor's other
                // paths can read what the copies write (decided per edge below).
                for &(where_, value) in &edges {
                    if successors.get(&where_).copied().unwrap_or(0) > 1 {
                        crossing.entry(where_).or_default().push((phi.result, value));
                    } else {
                        let group = edge_group(where_, block.at);
                        copies.entry(where_).or_default().push(_copy(
                            at_of[&where_],
                            phi.result,
                            value,
                            Some(group),
                            widths[&phi.result],
                        ));
                    }
                }
                continue;
            }
            for &(where_, value) in &edges {
                let group = edge_group(where_, block.at);
                copies.entry(where_).or_default().push(_copy(
                    at_of[&where_],
                    phi.result,
                    value,
                    Some(group),
                    widths[&phi.result],
                ));
            }
        }
        for (&where_, pairs) in &crossing {
            // A copy may run in the predecessor only when neither end needs
            // a distinct value on its other paths. Otherwise put it on its
            // actual edge.
            if pairs.iter().any(|&(result, value)| {
                _observed(body, &at_of, where_, block.at, result) || _observed(body, &at_of, where_, block.at, value)
            }) {
                match split.entry((where_, block.at)).or_insert_with(|| Split::Pairs(Vec::new())) {
                    Split::Pairs(existing) => existing.extend(pairs.iter().copied()),
                    Split::Selected(_) => unreachable!("eliminated only splits pairs"),
                }
                continue;
            }
            for &(result, value) in pairs {
                let group = edge_group(where_, block.at);
                copies
                    .entry(where_)
                    .or_default()
                    .push(_copy(at_of[&where_], result, value, Some(group), widths[&result]));
            }
        }
        kept.insert(block.at, stays);
    }

    if copies.is_empty() && rename.is_empty() && split.is_empty() {
        return Ok(body.clone());
    }
    if !split.is_empty() {
        return _split_edges(body, &split, &copies, &rename, &kept, &widths, false);
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let placed = _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice));
        block.insns = placed.iter().map(|one| _renamed(one, &rename)).collect::<Result<_, _>>()?;
        block.phis = kept[&block.at].iter().map(|phi| _renamed_phi(phi, &rename)).collect::<Result<_, _>>()?;
    }
    Ok(out)
}

/// Whether `value` can be read after leaving `where` other than into `into`.
///
/// `into` defines it, so a path through `into` reads a new one.
pub fn _observed(body: &LirBody, at_of: &IndexMap<i64, &LirBlock>, where_: i64, into: i64, value: u32) -> bool {
    let _ = body;
    let mut pending: Vec<i64> = at_of[&where_].succ.iter().copied().filter(|&at| at != into).collect();
    // A phi reads on the incoming edge, before any instruction in its block.
    if pending.iter().any(|at| {
        at_of
            .get(at)
            .is_some_and(|block| block.phis.iter().any(|phi| phi.incoming.contains(&(where_, value))))
    }) {
        return true;
    }
    let mut seen: BTreeSet<i64> = pending.iter().copied().collect();
    while let Some(at) = pending.pop() {
        let Some(block) = at_of.get(&at) else {
            return true;
        };
        if block.insns.iter().any(|one| {
            one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value)
        }) {
            return true;
        }
        for &successor in &block.succ {
            let follower = at_of.get(&successor);
            if follower.is_some_and(|follower| {
                follower.phis.iter().any(|phi| phi.incoming.contains(&(block.at, value)))
            }) {
                return true;
            }
            if successor != into && !seen.contains(&successor) {
                seen.insert(successor);
                pending.push(successor);
            }
        }
    }
    false
}

/// How many times each value is read, phi edges included.
fn _read_once(body: &LirBody) -> IndexMap<u32, i64> {
    let mut out: IndexMap<u32, i64> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for &value in &one.uses {
                *out.entry(value).or_insert(0) += 1;
            }
        }
        for phi in &block.phis {
            for &(_where, value) in &phi.incoming {
                *out.entry(value).or_insert(0) += 1;
            }
        }
    }
    out
}

/// Whether `result`, the phi in `phi_block`'s, is still read after `value`
/// is defined in predecessor `from`: later there, by another phi on the edge
/// into `phi_block`, or down another path. Defining `result` where `value`
/// was would then overwrite what that read wants.
fn _live_after(at_of: &IndexMap<i64, &LirBlock>, phi_block: i64, from: i64, value: u32, result: u32) -> bool {
    let block = at_of[&from];
    let defined = block.insns.iter().position(|one| one.defines.contains(&value)).expect("defined here");
    if block.insns[defined + 1..].iter().any(|one| one.uses.contains(&result)) {
        return true;
    }
    let read_on_edge = |from: i64, into: i64| at_of[&into].phis.iter().any(|phi| phi.incoming.contains(&(from, result)));
    // Live into a block: read before redefined, or passed on.
    let mut seen = BTreeSet::new();
    let mut work: Vec<(i64, i64)> = block.succ.iter().map(|&into| (from, into)).collect();
    while let Some((edge_from, into)) = work.pop() {
        if read_on_edge(edge_from, into) {
            return true;
        }
        // The phi block's own phi defines `result` afresh.
        if into == phi_block || !seen.insert(into) {
            continue;
        }
        let Some(next) = at_of.get(&into) else { continue };
        if next.phis.iter().any(|phi| phi.result == result) {
            continue;
        }
        for one in &next.insns {
            if one.uses.contains(&result) {
                return true;
            }
            if one.defines.contains(&result) {
                break;
            }
        }
        if !next.insns.iter().any(|one| one.defines.contains(&result)) {
            work.extend(next.succ.iter().map(|&after| (into, after)));
        }
    }
    false
}

/// Whether exactly one instruction in this block defines the value.
fn _defined_in(block: &LirBlock, value: u32) -> bool {
    block.insns.iter().filter(|one| one.defines.contains(&value)).count() == 1
}

/// One instruction defining the phi's result where it defined its own.
fn _renamed(one: &Arc<Insn>, rename: &IndexMap<u32, u32>) -> Result<Arc<Insn>, String> {
    if rename.is_empty() {
        return Ok(Arc::clone(one));
    }
    let mut out = (**one).clone();
    if let Some(what) = &one.what {
        let mut what = what.clone();
        what.dests = what.dests.iter().map(|x| _settled(x, rename)).collect::<Result<_, _>>()?;
        what.sources = what.sources.iter().map(|x| _settled(x, rename)).collect::<Result<_, _>>()?;
        out.what = Some(what);
    }
    out.defines = one.defines.iter().map(|&v| _name(v, rename)).collect::<Result<_, _>>()?;
    out.uses = one.uses.iter().map(|&v| _name(v, rename)).collect::<Result<_, _>>()?;
    out.requires = one
        .requires
        .iter()
        .map(|(held, register)| Ok((Held { value: _name(held.value, rename)?, width: held.width }, *register)))
        .collect::<Result<_, String>>()?;
    out.delivers = one
        .delivers
        .iter()
        .map(|(held, register)| Ok((Held { value: _name(held.value, rename)?, width: held.width }, *register)))
        .collect::<Result<_, String>>()?;
    out.widths = one
        .widths
        .iter()
        .map(|&(value, width)| Ok((_name(value, rename)?, width)))
        .collect::<Result<_, String>>()?;
    Ok(Arc::new(out))
}

/// The final identity after chained trivial phis are unified.
fn _name(mut value: u32, rename: &IndexMap<u32, u32>) -> Result<u32, String> {
    let mut seen = BTreeSet::new();
    while let Some(&next) = rename.get(&value) {
        if next == value {
            break;
        }
        if seen.contains(&value) {
            return Err("cyclic phi rename".to_owned());
        }
        seen.insert(value);
        value = next;
    }
    Ok(value)
}

/// One operand with every value it names put through the rename.
///
/// Through `ir::mapped` rather than a case per operand shape: a cell names
/// the value that computed its address.
pub fn _settled(where_: &Loc, rename: &IndexMap<u32, u32>) -> Result<Loc, String> {
    let mut error = None;
    let out = ir::mapped(where_, |one| match _name(one.value, rename) {
        Ok(value) => Held { value, width: one.width },
        Err(refused) => {
            error.get_or_insert(refused);
            *one
        }
    });
    error.map_or(Ok(out), Err)
}

/// A surviving phi with every trivial-phi identity made final.
fn _renamed_phi(phi: &Phi, rename: &IndexMap<u32, u32>) -> Result<Phi, String> {
    Ok(Phi {
        result: _name(phi.result, rename)?,
        incoming: phi
            .incoming
            .iter()
            .map(|&(where_, value)| Ok((where_, _name(value, rename)?)))
            .collect::<Result<_, String>>()?,
    })
}

fn _widths(body: &LirBody) -> IndexMap<u32, u32> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for op in &block.insns {
            let mut operands: Vec<Held> = op.requires.iter().chain(&op.delivers).map(|(held, _)| *held).collect();
            if let Some(what) = &op.what {
                operands.extend(what.dests.iter().chain(&what.sources).flat_map(ir::values));
            }
            for (value, width) in op
                .widths
                .iter()
                .copied()
                .chain(operands.iter().map(|held| (held.value, held.width)))
            {
                let known = widths.get(&value).copied().unwrap_or(0);
                widths.insert(value, known.max(width));
            }
        }
    }
    let phis: Vec<&Phi> = body.blocks.iter().flat_map(|block| &block.phis).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for phi in &phis {
            let values: Vec<u32> =
                std::iter::once(phi.result).chain(phi.incoming.iter().map(|&(_, value)| value)).collect();
            let width = values
                .iter()
                .map(|value| widths.get(value).copied().unwrap_or(2))
                .max()
                .expect("a phi has a result");
            for value in values {
                if widths.get(&value).copied() != Some(width) {
                    widths.insert(value, width);
                    changing = true;
                }
            }
        }
    }
    widths
}

/// The move a phi becomes, at the end of the block it arrives from.
///
/// Placed on the predecessor's last instruction's address and claiming none
/// of its bytes.
fn _copy(where_: &LirBlock, into: u32, out_of: u32, group: Option<i64>, width: u32) -> Arc<Insn> {
    let last = where_.insns.last();
    let at = last.map_or(where_.at, |last| last.at);
    let edge = _nothing(last.map(|last| &**last), at);
    let mut one = Insn::new(
        at,
        Some(edge),
        Some(move_of(Held { value: into, width }, Held { value: out_of, width })),
        vec![into],
        vec![out_of],
    );
    one.group = group;
    one.op = last.and_then(|last| last.op.clone());
    Arc::new(one)
}

/// An empty span: an inserted instruction claims no original bytes.
///
/// Never None: that would mean "ask the node", whose bytes the neighbour
/// already claims.
fn _nothing(beside: Option<&Insn>, at: i64) -> (i64, i64) {
    let start = beside.and_then(|beside| beside.covers).map_or(at, |covers| covers.0);
    (start, start)
}

/// The copies at the end of the block, but ahead of what leaves it.
fn _before_the_terminator(block: &LirBlock, added: &[Arc<Insn>]) -> Vec<Arc<Insn>> {
    if added.is_empty() {
        return block.insns.clone();
    }
    let insns = &block.insns;
    let mut cut = insns.len();
    while cut > 0 && _leaves(&insns[cut - 1]) {
        cut -= 1;
    }
    insns[..cut].iter().chain(added).chain(&insns[cut..]).cloned().collect()
}

/// Whether this instruction ends the block.
fn _leaves(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch | Operation::Return))
}

/// Place already selected parallel transfers on their exact CFG edges.
pub fn placed_on_edges(body: &LirBody, transfers: &IndexMap<(i64, i64), Vec<Arc<Insn>>>) -> LirBody {
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut copies: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut split: IndexMap<(i64, i64), Split> = IndexMap::default();
    for (&(where_, into), insns) in transfers {
        if at_of[&where_].succ.len() > 1 {
            split.insert((where_, into), Split::Selected(insns.clone()));
        } else {
            copies.entry(where_).or_default().extend(insns.iter().cloned());
        }
    }
    if split.is_empty() {
        let mut out = body.clone();
        for block in &mut out.blocks {
            block.insns = _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice));
        }
        return out;
    }
    let kept: IndexMap<i64, Vec<Phi>> = body.blocks.iter().map(|block| (block.at, block.phis.clone())).collect();
    _split_edges(body, &split, &copies, &IndexMap::default(), &kept, &IndexMap::default(), true)
        .expect("an empty rename cannot cycle")
}

/// A block of its own on each critical edge, holding that edge's copies.
///
/// Synthetic labels are above physical offsets, in a namespace per body
/// entry. Layout emits the blocks out of line and each jumps to its
/// successor.
pub fn _split_edges(
    body: &LirBody,
    split: &IndexMap<(i64, i64), Split>,
    copies: &IndexMap<i64, Vec<Arc<Insn>>>,
    rename: &IndexMap<u32, u32>,
    kept: &IndexMap<i64, Vec<Phi>>,
    widths: &IndexMap<u32, u32>,
    selected: bool,
) -> Result<LirBody, String> {
    let at_of: IndexMap<i64, &LirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut made: IndexMap<(i64, i64), LirBlock> = IndexMap::default();
    let mut landing: IndexMap<(i64, i64), i64> = IndexMap::default();
    let mut edges: Vec<(&(i64, i64), &Split)> = split.iter().collect();
    edges.sort_by_key(|(key, _)| **key);
    let highest = at_of.keys().copied().max().expect("a body has a block");
    for (number, (&(where_, into), pairs)) in (1_i64..).zip(edges) {
        let at = ((body.entry + 1) << 32).max(highest) + number;
        landing.insert((where_, into), at);
        let beside = &at_of[&where_].insns[at_of[&where_].insns.len() - 1];
        let mut insns: Vec<Arc<Insn>> = match (selected, pairs) {
            (true, Split::Selected(transfers)) => transfers
                .iter()
                .map(|one| {
                    let mut placed = (**one).clone();
                    placed.at = at;
                    placed.covers = Some((at, at));
                    Arc::new(placed)
                })
                .collect(),
            (false, Split::Pairs(pairs)) => pairs
                .iter()
                .map(|&(a, b)| {
                    let mut copy = (*_made(
                        beside,
                        at,
                        move_of(Held { value: a, width: widths[&a] }, Held { value: b, width: widths[&a] }),
                        vec![a],
                        vec![b],
                    ))
                    .clone();
                    copy.group = Some(number);
                    Arc::new(copy)
                })
                .collect(),
            _ => unreachable!("`selected` says which kind the split holds"),
        };
        insns.push(_made(beside, at, jump_to(into), vec![], vec![]));
        // These instructions live outside `body.blocks` until the end, so
        // the ordinary rename walk below cannot see them.
        made.insert(
            (where_, into),
            LirBlock {
                at,
                insns: insns.iter().map(|one| _renamed(one, rename)).collect::<Result<_, _>>()?,
                succ: vec![into],
                phis: vec![],
                cold: false,
            },
        );
    }

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let succ: Vec<i64> = block
            .succ
            .iter()
            .map(|&one| landing.get(&(block.at, one)).copied().unwrap_or(one))
            .collect();
        let mut insns: Vec<Arc<Insn>> =
            _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice))
                .iter()
                .map(|one| _retargeted(one, &landing, block.at))
                .collect();
        let last = block.insns.last();
        if let Some(last) = last {
            if let Some(what) = last.what.as_ref().filter(|what| what.op == Operation::Branch) {
                let fallthrough = block.succ.iter().copied().find(|&into| Some(into) != what.target);
                if let Some(&edge) = fallthrough.and_then(|fallthrough| landing.get(&(block.at, fallthrough))) {
                    insns.push(_made(last, last.at, jump_to(edge), vec![], vec![]));
                }
            }
        }
        blocks.push(LirBlock {
            at: block.at,
            insns: insns.iter().map(|one| _renamed(one, rename)).collect::<Result<_, _>>()?,
            succ,
            phis: kept[&block.at]
                .iter()
                .map(|phi| {
                    _renamed_phi(
                        &Phi {
                            result: phi.result,
                            incoming: phi
                                .incoming
                                .iter()
                                .map(|&(where_, value)| {
                                    (landing.get(&(where_, block.at)).copied().unwrap_or(where_), value)
                                })
                                .collect(),
                        },
                        rename,
                    )
                })
                .collect::<Result<_, _>>()?,
            cold: block.cold,
        });
    }
    let mut out = body.clone();
    blocks.extend(made.into_values());
    out.blocks = blocks;
    Ok(out)
}

/// One instruction in a split block, claiming none of BC's own bytes.
fn _made(beside: &Insn, at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>) -> Arc<Insn> {
    let mut one = Insn::new(at, Some((at, at)), Some(what), defines, uses);
    one.op = beside.op.clone();
    Arc::new(one)
}

/// A branch or jump pointing at the split block instead of the successor.
fn _retargeted(one: &Arc<Insn>, landing: &IndexMap<(i64, i64), i64>, here: i64) -> Arc<Insn> {
    let Some(what) = &one.what else {
        return Arc::clone(one);
    };
    let Some(target) = what.target else {
        return Arc::clone(one);
    };
    let Some(&at) = landing.get(&(here, target)) else {
        return Arc::clone(one);
    };
    let mut out = (**one).clone();
    out.what = Some(Semantics {
        target: Some(at),
        ..what.clone()
    });
    Arc::new(out)
}

/// A split edge whose copies all went away is the edge again.
///
/// Coalescing leaves the block `_split_edges` made holding only its jump,
/// and that jump, placed out of line, was taken on every pass.
#[must_use]
pub fn unsplit(body: &LirBody) -> LirBody {
    let floor = (body.entry + 1) << 32;
    let mut bypass: IndexMap<i64, i64> = IndexMap::default();
    for block in &body.blocks {
        if block.at < floor || !block.phis.is_empty() || block.succ.len() != 1 {
            continue;
        }
        let live: Vec<&Arc<Insn>> = block
            .insns
            .iter()
            .filter(|one| {
                !(one.what.as_ref().is_some_and(|what| {
                    what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)
                }) && one.defines.is_empty()
                    && one.uses.is_empty())
            })
            .collect();
        if live.len() == 1
            && live[0]
                .what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Jump && what.target == Some(block.succ[0]))
        {
            bypass.insert(block.at, block.succ[0]);
        }
    }
    if bypass.is_empty() {
        return body.clone();
    }

    let where_ = |mut at: i64| -> i64 {
        let mut seen = BTreeSet::new();
        while let Some(&next) = bypass.get(&at) {
            if seen.contains(&at) {
                break;
            }
            seen.insert(at);
            at = next;
        }
        at
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        if bypass.contains_key(&block.at) {
            continue;
        }
        let insns = block
            .insns
            .iter()
            .map(|one| match &one.what {
                Some(what) if what.target.is_some_and(|target| bypass.contains_key(&target)) => {
                    let mut out = (**one).clone();
                    out.what = Some(Semantics {
                        target: what.target.map(where_),
                        ..what.clone()
                    });
                    Arc::new(out)
                }
                _ => Arc::clone(one),
            })
            .collect();
        let phis = block
            .phis
            .iter()
            .map(|phi| Phi {
                result: phi.result,
                incoming: phi.incoming.iter().map(|&(at, value)| (where_(at), value)).collect(),
            })
            .collect();
        blocks.push(LirBlock {
            at: block.at,
            insns,
            succ: block.succ.iter().map(|&at| where_(at)).collect(),
            phis,
            cold: block.cold,
        });
    }
    let mut out = body.clone();
    out.blocks = blocks;
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::{Split, _observed, _settled, _split_edges, eliminated, unsplit};
    use crate::backend::verify;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody, Phi};

    fn what(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) })
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>, phis: Vec<Phi>) -> LirBlock {
        LirBlock { at, insns, succ, phis, cold: false }
    }

    fn body(name: &str, blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new(name, 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn held(value: u32, width: u32) -> Loc {
        Loc::Held(Held { value, width })
    }

    fn imm(value: i64, width: u32) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn branch(at: i64, covers: Option<(i64, i64)>, name: &str, target: i64) -> Arc<Insn> {
        Arc::new(Insn::new(at, covers, what(Operation::Branch, name, vec![], vec![], Some(target)), vec![], vec![]))
    }

    fn values(done: &LirBody, pick: fn(&Insn) -> &Vec<u32>) -> BTreeSet<u32> {
        done.insns().iter().flat_map(|one| pick(one).clone()).collect()
    }

    #[test]
    fn test_unsplit_keeps_virtual_edge_copy_anchors() {
        // qcport's snd_mix lost value#259 when unsplit treated two no-byte
        // edge-copy anchors as empty and bypassed the blocks defining the phi.
        let jump_branch = branch(1, None, "je", 1 << 33);
        let anchor = Arc::new(Insn::new(2, None, what(Operation::Nothing, "", vec![], vec![], None), vec![259], vec![85]));
        let jump = Arc::new(Insn::new(3, None, what(Operation::Jump, "jmp", vec![], vec![], Some(33)), vec![], vec![]));
        let compare = Arc::new(Insn::new(
            4,
            None,
            what(
                Operation::Compare,
                "cmp",
                vec![],
                vec![Loc::Reg(Reg { register: Register::CX, width: 2 }), imm(8, 2)],
                None,
            ),
            vec![],
            vec![259],
        ));
        let mut anchored = body(
            "anchored-split-edge",
            vec![
                block(0, vec![jump_branch], vec![1 << 32, 1 << 33], vec![]),
                block(1 << 32, vec![Arc::clone(&anchor), Arc::clone(&jump)], vec![33], vec![]),
                block(1 << 33, vec![anchor, jump], vec![33], vec![]),
                block(33, vec![compare], vec![], vec![]),
            ],
        );
        anchored.inputs = BTreeSet::from([85]);

        let done = unsplit(&anchored);

        assert_eq!(verify::verify(&done, false), Vec::<String>::new());
    }

    #[test]
    fn test_split_fallthrough_gets_an_explicit_jump() {
        // EVTRAP's split exit edge was emitted unreachable after the handler.
        let jz = branch(0, Some((0, 2)), "jz", 20);
        let edge_body = body("edge", vec![block(0, vec![Arc::clone(&jz)], vec![10, 20], vec![])]);
        let done = _split_edges(
            &edge_body,
            &IndexMap::from_iter([((0, 10), Split::Pairs(vec![(2, 1)]))]),
            &IndexMap::default(),
            &IndexMap::default(),
            &IndexMap::from_iter([(0, vec![])]),
            &IndexMap::from_iter([(2, 2)]),
            false,
        )
        .unwrap();
        let edge = &done.blocks[done.blocks.len() - 1];
        let last = &done.blocks[0].insns[done.blocks[0].insns.len() - 1];
        let last_what = last.what.as_ref().unwrap();
        assert_eq!(last_what.op, Operation::Jump);
        assert_eq!(last_what.target, Some(edge.at));
        assert_eq!(last.at, jz.at);
    }

    #[test]
    fn test_a_phi_read_by_its_sibling_is_not_renamed_into_the_latch() {
        // runtime.nib's print_q4 hung once simplifycfg made its loop's entry
        // edge critical: `b` took the latch's `b + 1` as its own register, so
        // `a`'s copy on the back edge read the new `b`, not the old one.
        let mov = |at: i64, value: u32, n: i64| {
            Arc::new(Insn::new(at, Some((at, at + 1)), what(Operation::Move, "mov", vec![held(value, 2)], vec![imm(n, 2)], None), vec![value], vec![]))
        };
        let test = Arc::new(Insn::new(10, Some((10, 11)), what(Operation::Compare, "cmp", vec![], vec![held(4, 2), imm(9, 2)], None), vec![], vec![4]));
        let step = Arc::new(Insn::new(20, Some((20, 21)), what(Operation::Binary, "add", vec![held(5, 2)], vec![held(4, 2), imm(1, 2)], None), vec![5], vec![4]));
        let looped = body(
            "looped",
            vec![
                block(0, vec![mov(0, 1, 0), mov(1, 2, 1)], vec![10, 30], vec![]),
                block(
                    10,
                    vec![test],
                    vec![20, 30],
                    vec![Phi { result: 3, incoming: vec![(0, 1), (20, 4)] }, Phi { result: 4, incoming: vec![(0, 2), (20, 5)] }],
                ),
                block(20, vec![step], vec![10], vec![]),
                block(30, vec![], vec![], vec![]),
            ],
        );
        let done = eliminated(&looped).unwrap();
        let latch = done.blocks.iter().find(|one| one.at == 20).unwrap();
        let redefined = latch.insns.iter().position(|one| one.defines.contains(&4));
        let copied = latch.insns.iter().position(|one| one.defines.contains(&3) && one.uses.contains(&4));
        assert!(copied.is_some() && redefined.is_none_or(|at| at > copied.unwrap()), "{:?}", latch.insns);
    }

    #[test]
    fn test_phi_elimination_copies_the_whole_scalar() {
        // VBDOS nbody printed PX0=285219921 for 1258: phi copies truncated 32-bit accumulators.
        for critical in [false, true] {
            let load = |at: i64, value: u32| {
                Arc::new(Insn::new(
                    at,
                    Some((at, at + 1)),
                    what(Operation::Move, "mov", vec![held(value, 4)], vec![imm(0x1234_5678, 4)], None),
                    vec![value],
                    vec![],
                ))
            };
            let read = Arc::new(Insn::new(
                2,
                Some((2, 3)),
                what(Operation::Binary, "add", vec![held(4, 4)], vec![held(3, 4), held(1, 4)], None),
                vec![4],
                vec![3, 1],
            ));
            let wide = body(
                "wide",
                vec![
                    block(0, vec![load(0, 1)], if critical { vec![2, 1] } else { vec![2] }, vec![]),
                    block(1, vec![load(1, 2)], vec![2], vec![]),
                    block(2, vec![read], vec![], vec![Phi { result: 3, incoming: vec![(0, 1), (1, 2)] }]),
                ],
            );
            let done = eliminated(&wide).unwrap();
            let copies: Vec<Arc<Insn>> = done.insns().into_iter().filter(|op| op.group.is_some()).collect();
            assert_eq!(copies.len(), 2, "critical={critical}");
            assert!(copies.iter().all(|op| {
                let what = op.what.as_ref().unwrap();
                what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Held(Held { width: 4, .. })))
            }));
        }
    }

    #[test]
    fn test_phi_on_a_single_predecessor_exit_does_not_split_the_edge() {
        // HARR gained an empty jump trampoline after LCSSA closed its loop exit.
        let mut use_ = Insn::new(2, Some((2, 3)), what(Operation::Move, "mov", vec![], vec![held(3, 2)], None), vec![], vec![3]);
        use_.widths = vec![(3, 2)];
        let exit = body(
            "exit",
            vec![
                block(0, vec![branch(0, Some((0, 2)), "jz", 2)], vec![1, 2], vec![]),
                block(1, vec![], vec![], vec![]),
                block(2, vec![Arc::new(use_)], vec![], vec![Phi { result: 3, incoming: vec![(0, 1)] }]),
            ],
        );

        let done = eliminated(&exit).unwrap();

        assert_eq!(done.blocks.len(), exit.blocks.len());
        let exit_block = done.blocks.iter().find(|block| block.at == 2).unwrap();
        assert!(exit_block.phis.is_empty());
        assert!(done.insns().iter().all(|insn| insn.group.is_none()));
        assert_eq!(exit_block.insns[0].uses, vec![1]);
        assert_eq!(exit_block.insns[0].widths, vec![(1, 2)]);
    }

    #[test]
    fn test_phi_source_live_on_the_other_branch_gets_an_edge_copy() {
        // nbody spilled its inner counter after exit copies extended both accumulators.
        let push = |at: i64, value: u32| {
            Arc::new(Insn::new(at, Some((at, at + 1)), what(Operation::Push, "push", vec![], vec![held(value, 2)], None), vec![], vec![value]))
        };
        let live = body(
            "live-source",
            vec![
                block(0, vec![branch(0, Some((0, 2)), "jz", 2)], vec![1, 2], vec![]),
                block(1, vec![push(1, 1)], vec![], vec![]),
                block(2, vec![push(2, 3)], vec![], vec![Phi { result: 3, incoming: vec![(0, 1), (4, 5)] }]),
                block(4, vec![], vec![2], vec![]),
            ],
        );

        let done = eliminated(&live).unwrap();

        assert_eq!(done.blocks.len(), live.blocks.len() + 1);
        let edge = &done.blocks[done.blocks.len() - 1];
        assert!(edge.insns[0].group.is_some());
        assert_eq!(edge.insns[0].defines, vec![3]);
        assert_eq!(edge.insns[0].uses, vec![1]);
    }

    #[test]
    fn test_critical_edge_copy_uses_the_final_trivial_phi_name() {
        // C crc32 returned -1141145971 instead of 778214622: the synthetic
        // edge copy read the removed intermediate phi result.
        let move_ = |at: i64, result: u32, source: Loc| {
            let uses = match &source {
                Loc::Held(held) => vec![held.value],
                _ => vec![],
            };
            Arc::new(Insn::new(at, Some((at, at + 1)), what(Operation::Move, "mov", vec![held(result, 4)], vec![source], None), vec![result], uses))
        };
        let trivial = body(
            "trivial-before-critical",
            vec![
                block(0, vec![move_(0, 1, imm(7, 4))], vec![1], vec![]),
                block(1, vec![branch(1, Some((1, 2)), "jz", 3)], vec![2, 3], vec![Phi { result: 2, incoming: vec![(0, 1)] }]),
                // Reading the source down the other arm requires a real edge copy.
                block(2, vec![move_(2, 4, held(2, 4))], vec![], vec![]),
                block(3, vec![move_(3, 7, held(6, 4))], vec![], vec![Phi { result: 6, incoming: vec![(1, 2), (4, 5)] }]),
                block(4, vec![move_(4, 5, imm(9, 4))], vec![3], vec![]),
            ],
        );

        let done = eliminated(&trivial).unwrap();

        let edge = done.blocks.iter().find(|block| block.at > 4).unwrap();
        let copy = &edge.insns[0];
        assert_eq!(copy.uses, vec![1]);
        assert_eq!(copy.what.as_ref().unwrap().sources, vec![held(1, 4)]);
        let defined = values(&done, |one| &one.defines);
        let used = values(&done, |one| &one.uses);
        assert!(used.is_subset(&defined), "phi elimination left undefined values {:?}", used.difference(&defined));
    }

    #[test]
    fn test_phi_observation_on_an_immediate_alternate_edge_is_visible() {
        // nbody's latch passed an accumulator to phis on both of its edges.
        let alternate = body(
            "alternate-phi",
            vec![
                block(0, vec![], vec![1, 2], vec![]),
                block(1, vec![], vec![], vec![Phi { result: 3, incoming: vec![(0, 7)] }]),
                block(2, vec![], vec![], vec![]),
            ],
        );
        let at_of: IndexMap<i64, &LirBlock> = alternate.blocks.iter().map(|block| (block.at, block)).collect();
        assert!(_observed(&alternate, &at_of, 0, 2, 7));
    }

    #[test]
    fn test_phi_elimination_renames_a_value_a_cell_is_reached_by() {
        // Port of tests/test_lir.py: `_settled` looked for a Held in
        // `Mem.through`, which is a register now, so a based cell kept the old id.
        let where_ = Addr { base: Register::SI, ..Addr::new(Space::Segment, 0x10) };
        let cell = Mem {
            through: Register::None,
            offset: 0,
            disp_width: 2,
            base: Some(Held { value: 21, width: 2 }),
            ..Mem::new(Some(where_), 2)
        };
        let Loc::Mem(got) = _settled(&Loc::Mem(cell.clone()), &IndexMap::from_iter([(21, 99)])).unwrap() else {
            panic!("a cell stays a cell");
        };
        assert_eq!(got.base, Some(Held { value: 99, width: 2 }), "the cell kept {:?}", got.base);
        assert_eq!(got.through, Register::None, "the rename placed it");
        assert_eq!(got.addr, cell.addr);
        assert_eq!(got.width, cell.width);
        assert_eq!(got.offset, cell.offset);
        assert_eq!(got.disp_width, cell.disp_width);
    }
}
