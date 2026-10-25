//! Shadow of the translated build against today's graph
//! (`memoryssa::structure_of`), for the tests and for the replay driver:
//! nothing in the pipeline calls it.

use std::collections::BTreeSet;

use llrm_mir::module::InstId;

use super::build::Effect;
use super::verify::Level;
use super::{Kind, LIVE_ON_ENTRY, Mssa};
use crate::cfg;
use crate::memory::Unit;
use crate::memoryssa::{self, Accesses, Kind as Old};

pub fn effect_of<'a>(
    accesses: &'a Accesses,
    function: &'a llrm_mir::module::Function,
) -> impl Fn(InstId) -> Effect + 'a {
    move |inst| Effect {
        reads: accesses.reads(inst).is_none_or(|reads| !reads.is_empty()),
        writes: accesses.writes(inst).is_none_or(|writes| !writes.is_empty()),
        // `isOrdered`: a volatile access is a def, so it is ordered against the
        // other defs.
        ordered: matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { volatile: true, .. }
                | llrm_mir::opcode::Opcode::Store { volatile: true, .. }
        ),
    }
}

/// [`effect_of`] for accesses that may be lazy: asked of `unit`.
pub fn effect_of_in<'a>(
    accesses: &'a Accesses,
    unit: &'a Unit<'a>,
) -> impl Fn(InstId) -> Effect + 'a {
    let function = unit.function;
    move |inst| Effect {
        reads: accesses.reads_in(unit, inst).is_none_or(|reads| !reads.is_empty()),
        writes: accesses.writes_in(unit, inst).is_none_or(|writes| !writes.is_empty()),
        // `isOrdered`: a volatile access is a def, so it is ordered against the
        // other defs.
        ordered: matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { volatile: true, .. }
                | llrm_mir::opcode::Opcode::Store { volatile: true, .. }
        ),
    }
}

/// `effect_of` for `Accesses::plain(unit, calls)`, without making it: what an
/// instruction reads and writes follows from its opcode (a load reads, a
/// store writes: every load and store has a reference) and, for a call, from
/// its attributes and `may_write` (the footprint held is not empty, or none
/// is).
pub fn effect_from_opcodes<'a>(
    unit: &'a Unit<'a>,
    may_write: impl Fn(InstId) -> bool + 'a,
) -> impl Fn(InstId) -> Effect + 'a {
    move |inst| {
        let function = unit.function;
        let opcode = &function.instruction(inst).opcode;
        let ordered = matches!(
            opcode,
            llrm_mir::opcode::Opcode::Load { volatile: true, .. }
                | llrm_mir::opcode::Opcode::Store { volatile: true, .. }
        );
        if let Some(own) = crate::memory::own_bytes(opcode) {
            return Effect { reads: own.reads, writes: own.writes, ordered };
        }
        let (reads, writes) = match opcode {
            llrm_mir::opcode::Opcode::Call(info) | llrm_mir::opcode::Opcode::Invoke(info) => {
                let callee = llrm_mir::memory::callee(unit.context, function, inst)
                    .and_then(|one| unit.globals.get(one.0 as usize))
                    .and_then(llrm_mir::module::GlobalValue::function);
                let reading = llrm_mir::memory::stated(&info.attrs).reads
                    && callee.is_none_or(|one| llrm_mir::memory::stated(&one.attrs).reads);
                let writing = crate::memory::unmodeled_write(unit, inst) && may_write(inst);
                (reading, writing)
            }
            _ => (false, false),
        };
        Effect { reads, writes, ordered }
    }
}

/// What the build reads of each instruction, as `buildMemorySSA` has it: a load
/// reads and a store writes (every load and store has a reference), a call as
/// the AA stack's `getMemoryEffects` says (`getModRefInfo(Call, nullptr)`:
/// attributes, `GlobalsModRef`), and a volatile access is ordered. No access
/// list is read, nor the calls' write footprints: over the corpus the two
/// agree on every call.
pub fn effect_from_aa<'a>(
    unit: &'a Unit<'a>,
    stack: crate::aggregate::AggregateAA<'a>,
) -> impl Fn(InstId) -> Effect + 'a {
    let stack = std::cell::RefCell::new(stack);
    move |inst| {
        use crate::aa::{AAQueryInfo, AAResults};
        let opcode = &unit.function.instruction(inst).opcode;
        let ordered = matches!(
            opcode,
            llrm_mir::opcode::Opcode::Load { volatile: true, .. }
                | llrm_mir::opcode::Opcode::Store { volatile: true, .. }
        );
        if let Some(own) = crate::memory::own_bytes(opcode) {
            return Effect { reads: own.reads, writes: own.writes, ordered };
        }
        if matches!(
            opcode,
            llrm_mir::opcode::Opcode::Call(_) | llrm_mir::opcode::Opcode::Invoke(_)
        ) {
            let asked = stack.borrow_mut().get_memory_effects(inst, &mut AAQueryInfo::default()).overall();
            return Effect { reads: asked.is_ref_set(), writes: asked.is_mod_set(), ordered };
        }
        Effect { reads: false, writes: false, ordered }
    }
}

pub fn translated(unit: &Unit) -> (Mssa, Accesses) {
    let accesses = Accesses::plain(unit, &Default::default());
    let shape = unit.shape();
    let mssa = Mssa::build(unit.function, &shape.dominance, &effect_of(&accesses, unit.function));
    (mssa, accesses)
}

/// What an access is, without the numbers of either graph: the live state, a
/// phi of a block, or the instruction.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Name {
    Live,
    Phi(i64),
    Site(InstId),
}

/// `access`, with a phi whose incoming values are all one value (or itself)
/// replaced by that value, as `tryRemoveTrivialPhi` would: today's graph keeps
/// no such phi.
fn resolved(
    mssa: &Mssa,
    access: u32,
) -> u32 {
    let mut at = access;
    loop {
        let one = mssa.access(at);
        if one.kind != Kind::Phi {
            return at;
        }
        let values: BTreeSet<u32> =
            one.incoming.iter().map(|(_, value)| resolved_value(mssa, *value, at)).filter(|v| *v != at).collect();
        match values.iter().copied().collect::<Vec<_>>()[..] {
            [only] => at = only,
            [] => return LIVE_ON_ENTRY,
            _ => return at,
        }
    }
}

fn resolved_value(
    mssa: &Mssa,
    value: u32,
    except: u32,
) -> u32 {
    let mut seen = vec![except];
    let mut at = value;
    loop {
        let one = mssa.access(at);
        if one.kind != Kind::Phi || seen.contains(&at) {
            return at;
        }
        seen.push(at);
        let values: BTreeSet<u32> = one.incoming.iter().map(|(_, v)| *v).filter(|v| !seen.contains(v)).collect();
        match values.iter().copied().collect::<Vec<_>>()[..] {
            [only] => at = only,
            _ => return at,
        }
    }
}

fn name_new(
    mssa: &Mssa,
    access: u32,
) -> Name {
    let one = mssa.access(access);
    match one.kind {
        Kind::Phi => Name::Phi(cfg::id(one.block)),
        _ if access == LIVE_ON_ENTRY => Name::Live,
        _ => Name::Site(one.inst.expect("an instruction")),
    }
}

fn name_old(
    old: &memoryssa::Structure,
    access: usize,
) -> Name {
    let one = old.accesses.iter().find(|one| one.id == access).expect("an access");
    match one.kind {
        Old::Live => Name::Live,
        Old::Phi => Name::Phi(one.block.expect("a block")),
        _ => Name::Site(one.site.expect("a site")),
    }
}

/// The translated graph verifies, and gives each memory instruction the
/// defining access today's graph gives it; its non-trivial phis are today's
/// phis.
pub fn compare(unit: &Unit) -> Result<(), String> {
    let (mssa, accesses) = translated(unit);
    mssa.verify_memory_ssa(unit.function, Level::Full)?;
    let old = memoryssa::built(unit, &accesses);
    for (site, access) in &old.sites {
        let Some(new) = mssa.get_memory_access(*site) else { return Err(format!("no access for {site:?}")) };
        // Today's graph takes a volatile load for a use; LLVM's `isOrdered`
        // makes it a def.
        let volatile =
            matches!(
                unit.function.instruction(*site).opcode,
                llrm_mir::opcode::Opcode::Load { volatile: true, .. }
            );
        if !volatile && (mssa.access(new).kind == Kind::Def) != (access.kind == Old::Def) {
            return Err(format!("{site:?} is a def in one graph and a use in the other"));
        }
        let defining = resolved(&mssa, mssa.access(new).defining.expect("a defining access"));
        let (new_name, old_name) =
            (name_new(&mssa, defining), name_old(&old, access.defining.expect("a defining access")));
        if new_name != old_name {
            return Err(format!("the defining access of {site:?}: {new_name:?} against {old_name:?}"));
        }
    }
    let phis: BTreeSet<Name> = (0..mssa.accesses.len() as u32)
        .filter(|one| !mssa.access(*one).dead && mssa.access(*one).kind == Kind::Phi && resolved(&mssa, *one) == *one)
        .map(|one| name_new(&mssa, one))
        .collect();
    let today: BTreeSet<Name> = old.phis.keys().map(|at| Name::Phi(*at)).collect();
    if phis != today {
        return Err(format!("the phis: {phis:?} against {today:?}"));
    }
    Ok(())
}

/// What the translated walker did over every load and store of a function, and
/// how its answers stand against today's walker.
#[derive(Debug, Default)]
pub struct WalkReport {
    pub queries: usize,
    /// Accesses stepped over (`MaxCheckLimit` less what was left).
    pub steps: u64,
    pub asked: usize,
    pub worked: usize,
    /// The clobber is one of today's nearest writes (or live on entry where
    /// today's walk reaches it).
    pub in_today: usize,
    /// The clobber is a phi.
    pub phi: usize,
    /// Neither: the translated clobber is a def today's walk did not name.
    pub elsewhere: usize,
}

/// `getClobberingMemoryAccess` of every load and store, each checked with
/// `check_clobber_sanity`.
pub fn walk(unit: &Unit) -> Result<WalkReport, String> {
    use super::alias::AliasQuery;
    use super::walker::MAX_CHECK_LIMIT;
    let accesses = Accesses::plain(unit, &Default::default());
    let shape = unit.shape();
    let mut mssa = Mssa::build(unit.function, &shape.dominance, &effect_of(&accesses, unit.function));
    let old = memoryssa::built(unit, &accesses);
    let aa = AliasQuery::new(unit, &accesses);
    let mut report = WalkReport::default();
    for (_, inst) in unit.function.walk() {
        let Some(access) = mssa.get_memory_access(inst) else { continue };
        let Some(loc) = aa.location_of(inst) else { continue };
        let started = mssa.access(access).defining.expect("a defining access");
        let mut limit = MAX_CHECK_LIMIT;
        let clobber = mssa.get_clobbering_memory_access_base(&aa, access, false, &mut limit);
        report.queries += 1;
        report.steps += u64::from(MAX_CHECK_LIMIT - limit);
        mssa.check_clobber_sanity(&aa, started, clobber, Some(&loc), Some(inst))
            .map_err(|why| format!("{inst:?}: {why}"))?;
        if mssa.access(clobber).kind == Kind::Phi {
            report.phi += 1;
            continue;
        }
        let near = old.clobbers(inst, &accesses.references[&inst]);
        let named = |id: usize| old.access(id).site;
        let site = mssa.access(clobber).inst;
        if (clobber == LIVE_ON_ENTRY && near.iter().any(|id| old.access(*id).kind == Old::Live))
            || (site.is_some() && near.iter().any(|id| named(*id) == site))
        {
            report.in_today += 1;
        } else {
            report.elsewhere += 1;
        }
    }
    let (asked, worked) = aa.questions();
    report.asked = asked;
    report.worked = worked;
    Ok(report)
}

/// What a graph says, in names that do not depend on how it was made: for each
/// memory instruction, whether it is a def and the access it reads, a trivial
/// phi taken for what it stands for; and for each phi that stands, the access
/// each of its edges reads.
#[derive(Debug, PartialEq)]
pub struct Said {
    pub instructions: llrm_mir::dense::IdMap<InstId, (bool, Name)>,
    pub phis: std::collections::BTreeMap<Name, Vec<(i64, Name)>>,
}

/// The instructions whose use the walker moved to an access further up
/// (`MemoryUse::isOptimized`): a graph made afresh has the nearest def there,
/// and LLVM's `verifyMemorySSA` does not compare the two either.
pub fn optimized_uses(mssa: &Mssa) -> Vec<InstId> {
    (0..mssa.accesses.len() as u32)
        .filter(|&at| at != LIVE_ON_ENTRY && !mssa.access(at).dead && mssa.access(at).kind == Kind::Use)
        .filter(|&at| mssa.is_optimized(at))
        .filter_map(|at| mssa.access(at).inst)
        .collect()
}

pub fn said(mssa: &Mssa) -> Said {
    let mut instructions = llrm_mir::dense::IdMap::new();
    let mut phis = std::collections::BTreeMap::new();
    for at in 0..mssa.accesses.len() as u32 {
        let one = mssa.access(at);
        if one.dead || at == LIVE_ON_ENTRY {
            continue;
        }
        if let Some(inst) = one.inst {
            let defining = resolved(mssa, one.defining.expect("a defining access"));
            instructions.insert(inst, (one.kind == Kind::Def, name_new(mssa, defining)));
        } else if one.kind == Kind::Phi && resolved(mssa, at) == at {
            let mut edges: Vec<(i64, Name)> = one
                .incoming
                .iter()
                .map(|&(from, value)| (cfg::id(from), name_new(mssa, resolved_value(mssa, value, at))))
                .collect();
            edges.sort();
            phis.insert(name_new(mssa, at), edges);
        }
    }
    Said { instructions, phis }
}

/// `updated` says what a graph made afresh of `function` says: the check of the
/// updater, edit by edit.
/// The walker has answered for every load and store of both first: a use's
/// defining access is what the walker found, so a graph the walker has been
/// asked of is compared with one it has been asked of.
pub fn same_as_fresh(
    updated: &mut Mssa,
    function: &llrm_mir::module::Function,
    dominance: &crate::cfg::Dominance,
    effect: &dyn Fn(InstId) -> Effect,
    aa: &super::alias::AliasQuery,
) -> Result<(), String> {
    updated.verify_memory_ssa(function, Level::Full)?;
    let mut fresh = Mssa::build(function, dominance, effect);
    walked(updated, aa, function);
    walked(&mut fresh, aa, function);
    let updated = &*updated;
    let (left, right) = (said(updated), said(&fresh));
    if left == right {
        return Ok(());
    }
    let wrong = left.instructions.iter().find(|(inst, said)| right.instructions.get(inst) != Some(said));
    Err(format!(
        "the updated graph differs from a fresh one: instruction {wrong:?} against {:?}; phis {:?} against {:?}",
        wrong.and_then(|(inst, _)| right.instructions.get(&inst)),
        left.phis,
        right.phis
    ))
}

/// Whether a kept graph's answer `kept` is safe against `fresh`'s walk `now`:
/// the same access, or one `now` dominates. A use keeps the answer it had
/// before an edit took a clobber away, later than a fresh walk's; an earlier
/// one would let a pass move past a clobber. The pin returns a cached
/// optimized access without walking again (MemorySSA.cpp:2556), and resets it
/// only for the uses an edit may have given a new clobber
/// (MemorySSAUpdater.cpp:1253, 1441; MemorySSA.h:337-346), so a cached answer
/// is older: nearer the use than a fresh walk, never past a clobber.
pub fn answer_is_safe(
    fresh: &Mssa,
    kept: &Name,
    now: &Name,
) -> bool {
    if kept == now {
        return true;
    }
    let find = |name: &Name| match name {
        Name::Live => Some(fresh.live_on_entry_def()),
        Name::Phi(at) => fresh.get_memory_phi(cfg::block(*at)),
        Name::Site(inst) => fresh.get_memory_access(*inst),
    };
    matches!(
        (find(now), find(kept)),
        (Some(now), Some(kept)) if fresh.dominates(now, kept)
    )
}

/// What the walker answers for every load and store of `function`, in names:
/// the clobbering access of each, a trivial phi taken for what it stands for.
pub fn walked(
    mssa: &mut Mssa,
    aa: &super::alias::AliasQuery,
    function: &llrm_mir::module::Function,
) -> llrm_mir::dense::IdMap<InstId, Name> {
    let mut answers = llrm_mir::dense::IdMap::new();
    for (_, inst) in function.walk() {
        let Some(access) = mssa.get_memory_access(inst) else { continue };
        if aa.location_of(inst).is_none() {
            continue;
        }
        let clobber = mssa.get_clobbering_memory_access(aa, access);
        answers.insert(inst, name_new(mssa, resolved(mssa, clobber)));
    }
    answers
}
