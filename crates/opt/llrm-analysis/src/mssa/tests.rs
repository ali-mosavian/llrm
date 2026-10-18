//! The translated build against LLVM's own invariants (`verify_memory_ssa`) and
//! against the graph `memoryssa::structure_of` makes of the same bodies.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use super::verify::Level;
use super::{Kind, LIVE_ON_ENTRY, Mssa, shadow};
use crate::memory::Unit;
use crate::memoryssa::Accesses;
use crate::testing::{DOS, function, layout, parsed};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module =
            parsed(&format!("{DOS}@g = global [256 x i8] zeroinitializer\ndeclare void @anything()\n\n{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }
}

fn translated(unit: &Unit) -> (Mssa, Accesses) {
    shadow::translated(unit)
}

fn same_as_today(body: &str) {
    let parsed = Parsed::new(body);
    shadow::compare(&parsed.unit()).unwrap();
}

/// A join of two arms that store: one phi, at the join; the load after it uses
/// the phi.
#[test]
fn test_a_diamond_gets_one_phi_at_the_join() {
    let text = format!(
        "define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}}
"
    );
    let parsed = Parsed::new(&text);
    let unit = parsed.unit();
    let (mssa, _) = translated(&unit);
    mssa.verify_memory_ssa(unit.function, Level::Full).unwrap();
    let join = crate::testing::block(unit.function, "b3");
    let phi = mssa.get_memory_phi(join).expect("a phi at the join");
    assert_eq!(mssa.access(phi).incoming.len(), 2);
    let load = unit.function.block(join).first().expect("a block with an instruction");
    let access = mssa.get_memory_access(load).expect("an access");
    assert_eq!(mssa.access(access).kind, Kind::Use);
    assert_eq!(mssa.get_defining_access(access), Some(phi));
    same_as_today(&text);
}

/// A store in a loop: a phi at the header takes live on entry from outside and
/// the store from the back edge.
#[test]
fn test_a_loop_gets_a_phi_at_its_header() {
    same_as_today(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  store i16 %i, ptr getelementptr (i8, ptr @g, i16 32)
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  %r = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %r
}
",
    );
}

/// Reads before any store use live on entry; instructions that touch no memory
/// have no access.
#[test]
fn test_uses_before_any_def_read_live_on_entry() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %x) {
b0:
  %a = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  %b = add i16 %a, %x
  %c = load i16, ptr getelementptr (i8, ptr @g, i16 48)
  ret i16 %b
}
",
    );
    let unit = parsed.unit();
    let (mssa, _) = translated(&unit);
    mssa.verify_memory_ssa(unit.function, Level::Full).unwrap();
    let block = unit.function.layout()[0];
    let insts = unit.function.block(block).instructions();
    assert!(mssa.get_memory_access(insts[1]).is_none());
    for at in [0, 2] {
        let access = mssa.get_memory_access(insts[at]).expect("an access");
        assert_eq!(mssa.get_defining_access(access), Some(LIVE_ON_ENTRY));
    }
}

/// A nest of two loops around a store, with a call after: the phis match
/// today's graph.
#[test]
fn test_a_nest_matches_the_graph_today() {
    same_as_today(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b4 ]
  br label %b2

b2:
  %k = phi i16 [ 0, %b1 ], [ %l, %b2 ]
  store i16 %k, ptr getelementptr (i8, ptr @g, i16 32)
  %l = add i16 %k, 1
  %c = icmp slt i16 %l, %n
  br i1 %c, label %b2, label %b3

b3:
  call void @anything()
  br label %b4

b4:
  %j = add i16 %i, 1
  %d = icmp slt i16 %j, %n
  br i1 %d, label %b1, label %b5

b5:
  %r = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %r
}
",
    );
}

/// The clobbering access of each memory instruction of `body`'s `@f` that has a
/// location, by the translated walker at its default caps, each checked with
/// `check_clobber_sanity`.
fn clobbers_of(body: &str) -> Vec<(llrm_mir::module::InstId, Option<llrm_mir::module::InstId>, Kind)> {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(body);
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let (mut mssa, _) = {
        let mssa = Mssa::build(unit.function, &unit.shape().dominance, &shadow::effect_of(&accesses, unit.function));
        (mssa, ())
    };
    let aa = AliasQuery::new(&unit, &accesses);
    let mut out = Vec::new();
    for (_, inst) in unit.function.walk() {
        let Some(access) = mssa.get_memory_access(inst) else { continue };
        let Some(loc) = aa.location_of(inst) else { continue };
        let clobber = mssa.get_clobbering_memory_access(&aa, access);
        let start = mssa.get_defining_access(access).expect("a defining access");
        // A use's optimized access is its defining access now; the walk started
        // from the one it had.
        let _ = start;
        mssa.check_clobber_sanity(
            &aa,
            mssa.access(access).defining.expect("defining"),
            clobber,
            Some(&loc),
            Some(inst),
        )
        .unwrap_or_else(|why| panic!("{inst:?}: {why}"));
        out.push((inst, mssa.access(clobber).inst, mssa.access(clobber).kind));
    }
    out
}

const SAME_CELL_AFTER_OTHER_STORE: &str = "define i16 @f() {
b0:
  store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
  store i16 2, ptr getelementptr (i8, ptr @g, i16 48)
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}
";

/// The walk skips the store to another cell and stops at the store to the
/// load's own.
#[test]
fn test_a_load_is_clobbered_by_the_store_of_its_cell_not_by_another_cells() {
    let found = clobbers_of(SAME_CELL_AFTER_OTHER_STORE);
    let load = found.last().expect("the load");
    let first_store = found[0].0;
    assert_eq!(load.1, Some(first_store), "{found:?}");
}

/// Nothing writes the cell: live on entry clobbers it.
#[test]
fn test_a_load_nothing_writes_is_clobbered_by_live_on_entry() {
    let found = clobbers_of(
        "define i16 @f() {
b0:
  store i16 2, ptr getelementptr (i8, ptr @g, i16 48)
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}
",
    );
    let load = found.last().expect("the load");
    assert!(load.1.is_none() && load.2 == Kind::Def, "{found:?}");
}

const DIAMOND_OF_OTHER_CELL: &str = "define i16 @f(i1 %c) {
b0:
  store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
  br i1 %c, label %b1, label %b2

b1:
  store i16 2, ptr getelementptr (i8, ptr @g, i16 48)
  br label %b3

b2:
  store i16 3, ptr getelementptr (i8, ptr @g, i16 48)
  br label %b3

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}
";

/// Both arms store to another cell: the phi at the join is optimized away and
/// the store above the diamond clobbers the load.
#[test]
fn test_a_phi_is_optimized_away_when_no_arm_writes_the_cell() {
    let found = clobbers_of(DIAMOND_OF_OTHER_CELL);
    assert_eq!(found.last().expect("the load").1, Some(found[0].0), "{found:?}");
}

/// Both arms store to the load's cell: the phi stays the clobber (no
/// instruction).
#[test]
fn test_a_join_of_two_stores_to_the_cell_is_clobbered_by_its_phi() {
    let found = clobbers_of(
        "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  store i16 3, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x
}
",
    );
    assert_eq!(found.last().expect("the load").2, Kind::Phi, "{found:?}");
}

/// A volatile load is ordered against the other defs, so it is a def, not a use
/// (`isOrdered`,
/// [`MemorySSA.cpp#L1748`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1748)).
/// Taken as a use it would move across a store.
#[test]
fn test_a_volatile_load_is_a_def() {
    let parsed = Parsed::new(
        "define i16 @f() {
b0:
  %a = load volatile i16, ptr getelementptr (i8, ptr @g, i16 32)
  %b = load i16, ptr getelementptr (i8, ptr @g, i16 48)
  ret i16 %b
}
",
    );
    let unit = parsed.unit();
    let (mssa, _) = translated(&unit);
    mssa.verify_memory_ssa(unit.function, Level::Full).unwrap();
    let block = unit.function.layout()[0];
    let insts = unit.function.block(block).instructions();
    assert_eq!(mssa.access(mssa.get_memory_access(insts[0]).expect("an access")).kind, Kind::Def);
    assert_eq!(mssa.access(mssa.get_memory_access(insts[1]).expect("an access")).kind, Kind::Use);
}

/// Two volatile loads are never reordered (`areLoadsReorderable`): the second
/// is clobbered by the first. A plain load of another cell after them is
/// clobbered by neither.
#[test]
fn test_a_volatile_load_clobbers_the_next_volatile_load_only() {
    let found = clobbers_of(
        "define i16 @f() {
b0:
  %a = load volatile i16, ptr getelementptr (i8, ptr @g, i16 32)
  %b = load volatile i16, ptr getelementptr (i8, ptr @g, i16 48)
  %c = load i16, ptr getelementptr (i8, ptr @g, i16 64)
  ret i16 %c
}
",
    );
    assert_eq!(found[1].1, Some(found[0].0), "the second volatile load: {found:?}");
    assert!(found[2].1.is_none() && found[2].2 == Kind::Def, "the plain load: {found:?}");
}

/// Moving uses off a crowded defining access keeps every other user's place in
/// its use list (`Use::removeFromList` is O(1); a scan of the list made a body
/// of N loads cost N^2 to build: loop 1600, 28 M instructions of 305 M).
#[test]
fn test_leaving_a_crowded_defining_access_keeps_the_other_users_in_its_list() {
    let loads: String =
        (0..64).map(|n| format!("  %x{n} = load i16, ptr getelementptr (i8, ptr @g, i16 {})\n", 2 * n)).collect();
    let parsed = Parsed::new(&format!("define i16 @f() {{\nb0:\n{loads}  ret i16 %x0\n}}\n"));
    let unit = parsed.unit();
    let (mut mssa, _) = translated(&unit);
    let live = mssa.live_on_entry_def();
    // Every other load leaves, to live on entry again, then back.
    let uses: Vec<_> = (1..mssa.len() as u32).filter(|a| mssa.access(*a).kind == Kind::Use).collect();
    for &one in uses.iter().step_by(2) {
        mssa.set_defining_access(one, None);
    }
    for &one in uses.iter().step_by(2) {
        mssa.set_defining_access(one, Some(live));
    }
    mssa.verify_memory_ssa(unit.function, Level::Full).unwrap();
}

/// The loads of a body of `n` loads and one store, in `@f`.
fn loads_and_a_store(n: usize) -> String {
    let loads: String = (0..n)
        .map(|k| format!("  %x{k} = load i16, ptr getelementptr (i8, ptr @g, i16 {})\n", 2 * (k % 120)))
        .collect();
    format!(
        "define i16 @f() {{\nb0:\n  store i16 1, ptr getelementptr (i8, ptr @g, i16 250)\n{loads}  ret i16 %x0\n}}\n"
    )
}

/// Taking a use off its defining access looks at a constant number of
/// use-list entries, whatever the list holds (`Use::removeFromList`). A scan of
/// the list made it N^2: the 1,600-statement `loop` ladder body was 1.9x
/// today's walk (256 loads: 16,512 entries looked at for 256 removals, here at
/// most 256).
#[test]
fn test_taking_a_use_off_a_crowded_defining_access_is_constant_work() {
    for n in [256, 1024] {
        let parsed = Parsed::new(&loads_and_a_store(n));
        let unit = parsed.unit();
        let (mut mssa, _) = translated(&unit);
        let live = mssa.live_on_entry_def();
        let uses: Vec<_> = (1..mssa.len() as u32).filter(|a| mssa.access(*a).kind == Kind::Use).collect();
        let before = mssa.use_list_steps();
        for &one in &uses {
            mssa.set_defining_access(one, None);
            mssa.set_defining_access(one, Some(live));
        }
        let steps = mssa.use_list_steps() - before;
        assert!(steps <= uses.len() as u64, "{n} loads: {steps} entries looked at for {} removals", uses.len());
    }
}

/// An alias question looks at one cache entry, however many locations have been
/// asked of the def: the cache is a map, as `AAQueryInfo::AliasCache`. A list
/// per def made 1,024 locations cost 1,024^2 / 2 entries looked at.
#[test]
fn test_an_alias_question_looks_at_one_cache_entry() {
    use super::alias::AliasQuery;
    for n in [256, 1024] {
        let parsed = Parsed::new(&loads_and_a_store(n));
        let unit = parsed.unit();
        let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
        let aa = AliasQuery::new(&unit, &accesses);
        let store = unit
            .function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| accesses.writes(inst).is_some_and(|w| !w.is_empty()))
            .expect("the store");
        let locations: Vec<_> = unit.function.walk().filter_map(|(_, inst)| aa.location_of(inst)).collect();
        for _ in 0..2 {
            for loc in &locations {
                aa.mod_set_loc(store, loc);
            }
        }
        let (asked, _) = aa.questions();
        assert!(aa.cache_steps() <= asked as u64, "{n} locations: {} entries for {asked} questions", aa.cache_steps());
    }
}

/// EarlyCSE's x_hash: one arm of a diamond stores into the table, the other
/// does not, and two loads of the same slot follow the join. The walker answers
/// ONE access for both, the join's phi (a set of the nearest stores made the
/// second load a different question from the first, and its CSE was missed:
/// 524 B instead of 515).
#[test]
fn test_loads_after_a_diamond_with_a_store_in_one_arm_are_clobbered_by_the_one_phi() {
    let parsed = Parsed::new(
        "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr @g
  br label %b3

b2:
  br label %b3

b3:
  %x = load i16, ptr @g
  %y = load i16, ptr @g
  ret i16 %y
}
",
    );
    let unit = parsed.unit();
    let (mssa, accesses) = translated(&unit);
    let view = super::view::Translated::new(&unit, &accesses, mssa);
    let loads: Vec<_> = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .collect();
    let first = view.clobbering_access(loads[0]).expect("an answer");
    let second = view.clobbering_access(loads[1]).expect("an answer");
    assert_eq!(first, second, "both loads have the same one clobber");
    assert!(view.is_phi(first), "the clobbers diverge, so it is the join's phi");
}
