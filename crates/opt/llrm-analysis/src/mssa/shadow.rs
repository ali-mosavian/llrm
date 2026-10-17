//! Shadow of the translated build against today's graph
//! (`memoryssa::structure_of`), for the tests and for the replay driver:
//! nothing in the pipeline calls it.

use std::collections::BTreeSet;

use llrm_mir::module::InstId;

use super::build::Effect;
use super::verify::Level;
use super::{Kind, LIVE_ON_ENTRY, Mssa};
use crate::cfg;
use crate::consts::Calls;
use crate::memory::Unit;
use crate::memoryssa::{self, Accesses, Kind as Old};

pub fn effect_of<'a>(accesses: &'a Accesses) -> impl Fn(InstId) -> Effect + 'a {
    move |inst| Effect {
        reads: accesses.reads(inst).is_none_or(|reads| !reads.is_empty()),
        writes: accesses.writes(inst).is_none_or(|writes| !writes.is_empty()),
        ordered: false,
    }
}

pub fn translated(unit: &Unit) -> (Mssa, Accesses) {
    let accesses = Accesses::plain(unit, &Calls::default());
    let shape = unit.shape();
    let mssa = Mssa::build(unit.function, &shape.dominance, &effect_of(&accesses));
    (mssa, accesses)
}

/// What an access is, without the numbers of either graph: the live state, a
/// phi of a block, or the instruction.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Name {
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
        if (mssa.access(new).kind == Kind::Def) != (access.kind == Old::Def) {
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
        .filter(|one| mssa.access(*one).kind == Kind::Phi && resolved(&mssa, *one) == *one)
        .map(|one| name_new(&mssa, one))
        .collect();
    let today: BTreeSet<Name> = old.phis.keys().map(|at| Name::Phi(*at)).collect();
    if phis != today {
        return Err(format!("the phis: {phis:?} against {today:?}"));
    }
    Ok(())
}
