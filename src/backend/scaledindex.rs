//! Fold a scaled index into a 32-bit address where lowering proved it exact.
//!
//! `shl bx,1` feeding `[bx+di]` becomes `[edi+ebx*2]` behind the 67h prefix,
//! and the shift goes. The 32-bit sum names the byte the 16-bit one does only
//! where lowering proved the address exact (`Mem::exact_scale`) and both
//! roots' upper halves are zero (`upperzero`). Where the access's loop writes
//! a root only below its upper half, one `movzx` in the loop's preheader
//! makes it zero for every trip.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::intervals::_graph;
use crate::analysis::loops;
use crate::backend::cpu::{self as targets, ProfileOrName};
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{_flag_lanes, _lanes, _register_effects, id};
use crate::backend::upperzero::{self, Roots, ROOTS};
use crate::backend::{liveness, regthrash, target};
use crate::model::ir::{self, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// One shift the address can absorb, and what absorbing it needs.
struct Candidate {
    shift: usize,
    users: Vec<usize>,
    /// The shifted value, the register the users read it through, and the
    /// register and value they read it through once the shift is gone.
    scaled: u32,
    shifted: Register,
    index: Register,
    source: u32,
    scale: i64,
    /// Per user, the roots whose upper half must be zero there.
    needs: Vec<Roots>,
}

fn full32(register: Register) -> Register {
    ir::root(register)
}

/// The register, scale and input value of `shl r,k` or `add r,r`, where
/// `scales` holds the scale.
fn shift_of(one: &Insn, scales: &BTreeSet<i64>) -> Option<(Reg, i64, u32)> {
    let what = one.what.as_ref()?;
    if !one.clobbers.is_empty() || !one.clobbers_high.is_empty() || one.symbol == Some(true) || one.defines.len() != 1 {
        return None;
    }
    let [input] = *one.uses.iter().copied().collect::<BTreeSet<u32>>().into_iter().collect::<Vec<_>>().as_slice() else {
        return None;
    };
    let (register, scale) = match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Binary, Some("shl" | "sal"), [Loc::Reg(written)], [Loc::Reg(read), Loc::Imm(Imm { value: amount @ 1..=31, address: None, .. })]) if written == read => {
            (*written, 1_i64 << amount)
        }
        (Operation::Binary, Some("add"), [Loc::Reg(written)], [Loc::Reg(read), Loc::Reg(other)]) if written == read && read == other => (*written, 2),
        _ => return None,
    };
    (register.width == 2 && full32(register.register) != Register::ESP && scales.contains(&scale)).then_some((register, scale, input))
}

/// The lanes `one` reads and writes, or None when unknown.
fn effects(one: &Insn) -> Option<(Lanes, Lanes)> {
    _register_effects(one, true, true).or_else(|| liveness::_declared(one))
}

/// Whether nothing in `between` writes any lane of `register`.
fn untouched(between: &[Arc<Insn>], register: Register) -> bool {
    let lanes = _lanes(register);
    between.iter().all(|one| effects(one).is_some_and(|(_, writes)| writes.and(&lanes).is_empty()))
}

/// The base register of each cell in `one` indexed by `scaled` through
/// `shifted` with an exact `scale`, or None when `one` reads `shifted` any
/// other way.
fn indexed_by(one: &Insn, scaled: u32, shifted: Register, scale: i64) -> Option<Vec<Register>> {
    let what = one.what.as_ref()?;
    let root = full32(shifted);
    let mut bases = Vec::new();
    for operand in what.dests.iter().chain(&what.sources) {
        match operand {
            Loc::Reg(register) if full32(register.register) == root => {
                if what.sources.contains(operand) {
                    return None;
                }
            }
            Loc::Mem(cell) if [cell.through, cell.index_through].map(full32).contains(&root) => {
                // The allocator may name base and index in either order for
                // the 16-bit encoding.
                let base = if full32(cell.index_through) == root { cell.through } else { cell.index_through };
                let proper = cell.index == Some(Held { value: scaled, width: 2 })
                    && cell.scale == 1
                    && cell.exact_scale == Some(scale)
                    && cell.base.is_some_and(|base| base.width == 2)
                    && full32(base) != root
                    && base != Register::None
                    && cell.addr.is_some_and(|addr| matches!(addr.space, Space::Far | Space::Literal));
                if !proper {
                    return None;
                }
                bases.push(base);
            }
            Loc::Address(_) => return None,
            _ => {}
        }
    }
    Some(bases)
}

fn candidates(body: &LirBody, scales: &BTreeSet<i64>) -> Vec<Candidate> {
    let exits = liveness::dead_at_exit(body);
    let (_, live_out) = crate::backend::allocate::live(body);
    let nothing = Insn::new(0, None, None, Vec::new(), Vec::new());
    let flags = _flag_lanes(0xFFFF_FFFF);
    let mut found = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        'shifts: for (at, shift) in block.insns.iter().enumerate() {
            let Some((register, scale, input)) = shift_of(shift, scales) else {
                continue;
            };
            let scaled = shift.defines[0];
            let lanes = _lanes(register.register);
            let Some((_, writes)) = effects(shift) else {
                continue;
            };
            if !writes.and(&flags).minus(&dead_after[&id(shift)]).is_empty() {
                continue;
            }
            // Every reader of the shifted register, up to the next write of
            // it, addresses through it; past the last, it is dead.
            let mut users: Vec<(usize, Vec<Register>)> = Vec::new();
            let mut rewritten = false;
            for (offset, one) in block.insns[at + 1..].iter().enumerate() {
                let Some((reads, writes)) = effects(one) else {
                    continue 'shifts;
                };
                if !reads.and(&lanes).is_empty() {
                    let Some(bases) = indexed_by(one, scaled, register.register, scale).filter(|bases| !bases.is_empty()) else {
                        continue 'shifts;
                    };
                    users.push((at + 1 + offset, bases));
                }
                if !writes.and(&lanes).is_empty() {
                    rewritten = true;
                    break;
                }
            }
            let Some(&(last, _)) = users.last() else {
                continue;
            };
            if !rewritten && !lanes.minus(&dead_after[&id(&block.insns[last])]).is_empty() {
                continue;
            }
            // A shift that defines a fresh identity takes it away: nothing but
            // the users may read it. One that keeps its input's identity
            // leaves it defined where it was; the lanes above answer for the
            // register's contents.
            let others: Vec<Arc<Insn>> = block.insns[at + 1..]
                .iter()
                .enumerate()
                .filter(|(offset, _)| !users.iter().any(|(user, _)| *user == at + 1 + offset))
                .map(|(_, one)| Arc::clone(one))
                .collect();
            let outside = live_out.get(&block.at).cloned().unwrap_or_default();
            if input != scaled && crate::backend::peephole::_loses_live_definition(std::slice::from_ref(shift), &nothing, &others, &outside) {
                continue;
            }
            // Address through the register a copy read, where it still holds
            // the value: the copy then feeds nothing.
            let (mut index, mut source) = (register.register, input);
            let copy_at = block.insns[..at].iter().rposition(|one| effects(one).is_none_or(|(_, writes)| !writes.and(&lanes).is_empty()));
            if let Some(copy_at) = copy_at {
                let copy = &block.insns[copy_at];
                if let (Some(what), [original]) = (&copy.what, copy.uses.as_slice()) {
                    if let (Operation::Move, Some("mov"), [Loc::Reg(Reg { width: 2, .. })], [Loc::Reg(from @ Reg { width: 2, .. })]) =
                        (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
                    {
                        if full32(from.register) != Register::ESP && untouched(&block.insns[copy_at + 1..last], from.register) {
                            index = from.register;
                            source = *original;
                        }
                    }
                }
            }
            let needs = users
                .iter()
                .map(|(_, bases)| {
                    bases.iter().chain([&index]).fold(0, |roots, register| roots | upperzero::bit(*register).unwrap_or(0))
                })
                .collect();
            found.push(Candidate {
                shift: id(shift),
                users: users.iter().map(|(user, _)| id(&block.insns[*user])).collect(),
                scaled,
                shifted: register.register,
                index,
                source,
                scale,
                needs,
            });
        }
    }
    found
}

/// `movzx root,word` for each root in `roots`, before `block`'s terminator.
fn extended(block: &LirBlock, roots: Roots) -> LirBlock {
    let mut insns = block.insns.clone();
    let at = insns.last().map_or(block.at, |one| one.at);
    let position = if insns.last().is_some_and(|one| liveness::_terminator(one.what.as_ref())) { insns.len() - 1 } else { insns.len() };
    for (index, root) in ROOTS.iter().enumerate() {
        if roots & 1 << index == 0 {
            continue;
        }
        let word = target::named(*root, 2);
        let what = Semantics {
            name: Some("movzx".into()),
            dests: vec![Loc::Reg(Reg { register: *root, width: 4 })],
            sources: vec![Loc::Reg(Reg { register: word, width: 2 })],
            ..Semantics::new(Operation::Extend)
        };
        insns.insert(position, Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())));
    }
    block.with_insns(insns)
}

/// `body` with the upper half of each loop's `roots` zeroed on entry, where
/// every way in may take it: one successor, and those lanes dead there.
fn preheaded(body: &LirBody, wanted: &IndexMap<i64, Roots>) -> (LirBody, IndexMap<i64, Roots>) {
    let graph = _graph(&body.blocks);
    let found = loops::loops(&graph, Some(body.entry));
    let predecessors = loops::predecessors(&graph);
    let exits = liveness::dead_at_exit(body);
    let mut placed: IndexMap<i64, Roots> = IndexMap::default();
    let mut per_block: IndexMap<i64, Roots> = IndexMap::default();
    for (header, roots) in wanted {
        let Some(inside) = found.iter().find(|one| one.header == *header).map(|one| &one.body) else {
            continue;
        };
        let entries: Vec<i64> = predecessors.get(header).into_iter().flatten().copied().filter(|at| !inside.contains(at)).collect();
        let mut allowed = *roots;
        for (index, root) in ROOTS.iter().enumerate() {
            let upper = [(*root, 2), (*root, 3)];
            let takes = !entries.is_empty()
                && entries.iter().all(|at| {
                    body.blocks.iter().find(|block| block.at == *at).is_some_and(|block| block.succ == [*header])
                        && upper.iter().all(|lane| exits[at].contains(lane))
                });
            if !takes {
                allowed &= !(1 << index);
            }
        }
        if allowed == 0 {
            continue;
        }
        placed.insert(*header, allowed);
        for at in entries {
            *per_block.entry(at).or_default() |= allowed;
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match per_block.get(&block.at) {
            Some(roots) => extended(block, *roots),
            None => block.clone(),
        })
        .collect();
    (body.with_blocks(blocks), placed)
}

fn rewritten(one: &Insn, candidate: &Candidate) -> Arc<Insn> {
    let root = full32(candidate.shifted);
    let widened = |operand: &Loc| -> Loc {
        match operand {
            Loc::Mem(cell) if cell.index == Some(Held { value: candidate.scaled, width: 2 }) => {
                let base = if full32(cell.index_through) == root { cell.through } else { cell.index_through };
                Loc::Mem(Mem {
                    base: cell.base.map(|base| Held { width: 4, ..base }),
                    through: full32(base),
                    index: Some(Held { value: candidate.source, width: 4 }),
                    index_through: full32(candidate.index),
                    scale: candidate.scale,
                    ..cell.clone()
                })
            }
            _ => operand.clone(),
        }
    };
    let what = one.what.as_ref().expect("a user has semantics");
    let mut uses: Vec<u32> = one.uses.iter().map(|value| if *value == candidate.scaled { candidate.source } else { *value }).collect();
    uses.dedup();
    Arc::new(Insn {
        what: Some(Semantics {
            dests: what.dests.iter().map(widened).collect(),
            sources: what.sources.iter().map(widened).collect(),
            ..what.clone()
        }),
        uses,
        ..one.clone()
    })
}

/// Fold each exact scaled index the cost model prefers into its addresses.
pub fn scaled_indexes<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let Some(secondary) = profile.address_forms.iter().find(|form| form.secondary && form.index_width == 4) else {
        return Ok(body.clone());
    };
    let found: Vec<Candidate> = candidates(body, &secondary.scales)
        .into_iter()
        .filter(|candidate| {
            let old = profile.operations.shift;
            let new = candidate.users.len() as i64 * (secondary.use_cost + profile.partial_register_stall);
            new <= old
        })
        .collect();
    if found.is_empty() {
        return Ok(body.clone());
    }
    let block_of: HashMap<usize, i64> =
        body.blocks.iter().flat_map(|block| block.insns.iter().map(move |one| (id(one), block.at))).collect();
    let graph = _graph(&body.blocks);
    let natural = loops::loops(&graph, Some(body.entry));
    let innermost =
        |at: i64| natural.iter().filter(|one| one.body.contains(&at)).min_by_key(|one| one.body.len()).map(|one| one.header);

    // Ask each missing root of the innermost loop around the access, then
    // keep the candidates the analysis proves over the result.
    let mut accepted: Vec<&Candidate> = found.iter().collect();
    loop {
        let zero = upperzero::before(body);
        let mut wanted: IndexMap<i64, Roots> = IndexMap::default();
        let mut placeable = Vec::new();
        for candidate in &accepted {
            let mut fine = true;
            for (user, needs) in candidate.users.iter().zip(&candidate.needs) {
                let missing = needs & !zero[user];
                if missing == 0 {
                    continue;
                }
                match innermost(block_of[user]) {
                    Some(header) => *wanted.entry(header).or_default() |= missing,
                    None => fine = false,
                }
            }
            if fine {
                placeable.push(*candidate);
            }
        }
        let (extended, _) = preheaded(body, &wanted);
        let zero = upperzero::before(&extended);
        let proven: Vec<&Candidate> = placeable
            .into_iter()
            .filter(|candidate| candidate.users.iter().zip(&candidate.needs).all(|(user, needs)| needs & !zero[user] == 0))
            .collect();
        if proven.len() == accepted.len() {
            let removed: HashSet<usize> = proven.iter().map(|candidate| candidate.shift).collect();
            let by_user: HashMap<usize, &Candidate> =
                proven.iter().flat_map(|candidate| candidate.users.iter().map(move |user| (*user, *candidate))).collect();
            let blocks = extended
                .blocks
                .iter()
                .map(|block| {
                    block.with_insns(
                        block
                            .insns
                            .iter()
                            .filter(|one| !removed.contains(&id(one)))
                            .map(|one| match by_user.get(&id(one)) {
                                Some(candidate) => rewritten(one, candidate),
                                None => Arc::clone(one),
                            })
                            .collect(),
                    )
                })
                .collect();
            return Ok(extended.with_blocks(blocks));
        }
        accepted = proven;
        if accepted.is_empty() {
            return Ok(body.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;

    use super::scaled_indexes;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn reg(register: Register, width: u32) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn one(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, defines: Vec<u32>, uses: Vec<u32>) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    #[test]
    fn test_the_folded_index_names_the_value_the_shift_read() {
        // Uncoalesced, `shl bx,1` reads 20 and defines 21: the address kept
        // naming 21, which lost its only definition with the shift.
        let cell = Mem {
            addr: Some(Addr { segment: Register::FS, ..Addr::new(Space::Far, 0) }),
            // The allocator names BX first for the 16-bit encoding.
            through: Register::BX,
            base: Some(Held { value: 26, width: 2 }),
            index: Some(Held { value: 21, width: 2 }),
            index_through: Register::DI,
            exact_scale: Some(2),
            ..Mem::new(None, 2)
        };
        let insns = vec![
            one(0, Operation::Extend, "movzx", vec![reg(Register::EBX, 4)], vec![reg(Register::BX, 2)], vec![], vec![]),
            one(1, Operation::Extend, "movzx", vec![reg(Register::EDI, 4)], vec![reg(Register::DI, 2)], vec![], vec![]),
            one(2, Operation::Binary, "shl", vec![reg(Register::BX, 2)], vec![reg(Register::BX, 2), Loc::Imm(Imm { value: 1, width: 1, address: None })], vec![21], vec![20]),
            one(3, Operation::Move, "mov", vec![reg(Register::BX, 2)], vec![Loc::Mem(cell)], vec![30], vec![26, 21]),
            one(4, Operation::Compare, "cmp", vec![], vec![reg(Register::BX, 2), Loc::Imm(Imm { value: 0, width: 2, address: None })], vec![], vec![30]),
        ];
        let body = LirBody::new("index", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());

        let result = scaled_indexes(&body, "386").unwrap();

        let load = result.blocks[0].insns.iter().find(|one| one.defines == [30]).unwrap();
        assert_eq!(result.blocks[0].insns.len(), 4);
        let Loc::Mem(cell) = &load.what.as_ref().unwrap().sources[0] else { panic!("not a cell") };
        assert_eq!((cell.index, cell.scale, cell.through, cell.index_through), (Some(Held { value: 20, width: 4 }), 2, Register::EDI, Register::EBX));
        assert!(load.uses.contains(&20) && !load.uses.contains(&21));
    }
}
