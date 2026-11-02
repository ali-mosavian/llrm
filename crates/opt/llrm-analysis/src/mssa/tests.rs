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

/// The access vector is sized from the function when the graph is built: it
/// grew nine times a build (0.55 M Ir on d_faces -O2), so its capacity was a
/// doubling of what it held, not the size it was made at.
#[test]
fn test_the_accesses_are_reserved_when_the_graph_is_built() {
    let text = format!(
        "define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  store i16 %i, ptr getelementptr (i8, ptr @g, i16 32)
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  store i16 %x, ptr getelementptr (i8, ptr @g, i16 34)
  store i16 %x, ptr getelementptr (i8, ptr @g, i16 36)
  store i16 %x, ptr getelementptr (i8, ptr @g, i16 38)
  %j = add i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %x
}}
"
    );
    let parsed = Parsed::new(&text);
    let unit = parsed.unit();
    let (mssa, _) = translated(&unit);
    let reserved = unit.function.instruction_count() / 3 + unit.function.layout().len() + 1;
    assert!(mssa.accesses.len() > 4 && mssa.accesses.len() <= reserved);
    assert_eq!(mssa.accesses.capacity(), reserved, "the access vector grew while the graph was built");
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
    let insts = unit.function.iter_block(block).collect::<Vec<_>>();
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
    let insts = unit.function.iter_block(block).collect::<Vec<_>>();
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

/// Two different globals are different objects: a store to one does not
/// clobber a load of the other. (Without it the walk of any loop that stores to
/// another global ended at the header's phi, and LICM kept the load.)
#[test]
fn test_a_store_to_another_global_does_not_clobber_the_load() {
    let found = clobbers_of(
        "@h = global i16 0

define i16 @f() {
b0:
  store i16 1, ptr @h
  %x = load i16, ptr @g
  ret i16 %x
}
",
    );
    let load = found.last().expect("the load");
    assert!(load.1.is_none() && load.2 == Kind::Def, "{found:?}");
}

/// A loop that stores through a variable index into one alloca does not clobber
/// the loads of another: both are identified objects (`getUnderlyingObject`).
/// Without it the walk ended at the header's phi, and LICM kept every load of a
/// scalar slot (matmul: 12 of LLVM's 25 hoists missed).
#[test]
fn test_a_loop_storing_into_an_array_by_a_variable_index_does_not_clobber_another_slot() {
    let found = clobbers_of(
        "define i16 @f(i16 %n) {
b0:
  %s = alloca i16
  %a = alloca [8 x i16]
  store i16 1, ptr %s
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %x = load i16, ptr %s
  %o = mul i16 %i, 16
  %q = getelementptr inbounds i8, ptr %a, i16 %o
  %o2 = mul i16 %n, 2
  %p = getelementptr inbounds i8, ptr %q, i16 %o2
  store i16 %x, ptr %p
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 0
}
",
    );
    // found: the first store, the load, the store through the index.
    assert_eq!(found[1].1, Some(found[0].0), "{found:?}");
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

/// A pointer's underlying object is found once, however often it is asked: the
/// walk down its GEP chain, made again at every step of every store's upward
/// walk, was two thirds of DSE's cost on a body of 400 stores through one
/// pointer.
#[test]
fn test_an_underlying_object_is_walked_once_per_pointer() {
    use super::alias::AliasQuery;
    let mut text = String::from("define void @f(ptr %p, i32 %a) {\nb0:\n");
    for k in 0..64 {
        text += &format!("  %g{k} = getelementptr i8, ptr %p, i16 {}\n  store i32 %a, ptr %g{k}\n", 4 * k);
    }
    text += "  ret void\n}\n";
    let parsed = Parsed::new(&text);
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let locations: Vec<_> = unit.function.walk().filter_map(|(_, inst)| aa.location_of(inst)).collect();
    assert_eq!(locations.len(), 64);
    for _ in 0..8 {
        for pair in locations.windows(2) {
            aa.alias(&pair[0], &pair[1]);
        }
    }
    assert!(
        aa.bases_walked() <= locations.len() as u64,
        "{} walks for {} locations",
        aa.bases_walked(),
        locations.len()
    );
}

/// A pointer read from memory cannot point into an alloca whose address is
/// never taken, so a store through it does not clobber the alloca's load
/// (`isNonEscapingLocalObject`). Without it every loop that stores through a
/// pointer it loaded kept every load of a local (d_faces: 10 of LLVM's 88
/// promotions missed).
#[test]
fn test_a_store_through_a_loaded_pointer_does_not_clobber_an_unexposed_alloca() {
    let found = clobbers_of(
        "define i16 @f(ptr %p) {
b0:
  %s = alloca i16
  store i16 1, ptr %s
  %q = load ptr, ptr %p
  store i16 2, ptr %q
  %x = load i16, ptr %s
  ret i16 %x
}
",
    );
    // found: the store to the slot, the load of the pointer, the store through
    // it, the load of the slot.
    assert_eq!(found[3].1, Some(found[0].0), "{found:?}");
}

/// The same through a phi of pointers loaded from memory (`aliasPHI`).
#[test]
fn test_a_store_through_a_phi_of_loaded_pointers_does_not_clobber_an_unexposed_alloca() {
    let found = clobbers_of(
        "define i16 @f(ptr %p, ptr %r, i1 %c) {
b0:
  %s = alloca i16
  store i16 1, ptr %s
  %q1 = load ptr, ptr %p
  %q2 = load ptr, ptr %r
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %q = phi ptr [ %q1, %b1 ], [ %q2, %b2 ]
  store i16 2, ptr %q
  %x = load i16, ptr %s
  ret i16 %x
}
",
    );
    let (slot_store, load) = (found[0].0, found.last().expect("the load"));
    assert_eq!(load.1, Some(slot_store), "{found:?}");
}

/// A call writes anything it can reach, not an alloca whose address was never
/// given out.
#[test]
fn test_a_call_does_not_clobber_an_unexposed_alloca() {
    let found = clobbers_of(
        "define i16 @f() {
b0:
  %s = alloca i16
  store i16 1, ptr %s
  call void @anything()
  %x = load i16, ptr %s
  ret i16 %x
}
",
    );
    assert_eq!(found[1].1, Some(found[0].0), "{found:?}");
}

/// The alias sets of the loads and stores of `@f`, as (pointer count, must
/// alias, mod, ref) of each live set.
fn alias_sets_of(body: &str) -> Vec<(usize, bool, bool, bool)> {
    use super::alias::AliasQuery;
    use crate::aliassets::AliasSetTracker;
    let parsed = Parsed::new(body);
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let mut tracker = AliasSetTracker::new(&aa);
    for (_, inst) in unit.function.walk() {
        let Some(loc) = aa.location_of(inst) else { continue };
        if matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Store { .. }
        ) {
            tracker.add_store(&loc);
        } else {
            tracker.add_load(&loc);
        }
    }
    tracker
        .alias_sets()
        .map(|(_, set)| (set.get_pointers().len(), set.is_must_alias(), set.is_mod(), set.is_ref()))
        .collect()
}

/// A load and a store through one pointer are one set that must alias; a
/// pointer that may be either of two globals joins them into a set that may.
#[test]
fn test_alias_sets_join_what_may_alias_and_keep_what_must() {
    let same = alias_sets_of(
        "@h = global i16 0

define i16 @f() {
b0:
  store i16 1, ptr @h
  %x = load i16, ptr @h
  ret i16 %x
}
",
    );
    assert_eq!(same, [(1, true, true, true)]);
    let apart = alias_sets_of(
        "@h = global i16 0
@k = global i16 0

define i16 @f() {
b0:
  store i16 1, ptr @h
  %x = load i16, ptr @k
  ret i16 %x
}
",
    );
    assert_eq!(apart, [(1, true, true, false), (1, true, false, true)]);
    let joined = alias_sets_of(
        "@h = global i16 0

define i16 @f(ptr %p) {
b0:
  store i16 1, ptr @h
  %x = load i16, ptr %p
  ret i16 %x
}
",
    );
    assert_eq!(joined, [(2, false, true, true)]);
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

/// Two inbounds geps off a global and an argument, sharing a variable index,
/// with the second 4 bytes on: `aliasGEP` puts that past the global, so no
/// alias. The stand-in's own test said May, so a walk kept a store live
/// that BasicAA clears.
#[test]
fn test_an_alias_question_is_answered_by_basicaa() {
    use super::alias::{AliasQuery, AliasResult};
    let parsed = Parsed::new(
        "define i16 @f(ptr %p, i16 %i) {
b0:
  %a = getelementptr inbounds i8, ptr @g, i16 %i
  %b0 = getelementptr inbounds i8, ptr %p, i16 %i
  %b = getelementptr inbounds i8, ptr %b0, i16 4
  %x = load i32, ptr %a
  %y = load i32, ptr %b
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let loads: Vec<_> = unit.function.walk().filter_map(|(_, inst)| aa.location_of(inst)).collect();
    assert_eq!(aa.alias(&loads[0], &loads[1]), AliasResult::No);
}

/// A call that names no location may touch any alloca whose address is
/// captured, and none whose address is only compared: the stand-in read the
/// `icmp` as exposing it, so the call kept a load of the local live (MayModRef,
/// where CaptureTracking says NoModRef).
#[test]
fn test_a_call_does_not_reach_a_local_whose_address_is_only_compared() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "define i16 @f(ptr %p) {
b0:
  %a = alloca i16
  store i16 1, ptr %a
  %same = icmp eq ptr %a, %p
  call void @anything()
  %x = load i16, ptr %a
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let call = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ))
        .expect("the call");
    let load = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .expect("the load");
    let loc = aa.location_of(load).expect("a location");
    let touched = aa.get_mod_ref_info(call, &loc);
    assert!(!touched.is_mod_or_ref(), "{touched:?}");
}

/// A call that is handed a local's address without capturing it still reaches
/// the local through its argument. The stand-in asked only whether the address
/// is captured, so `look(ptr nocapture %a)` was taken to leave the slot alone
/// and a store before it was dropped: a miscompile (#2887).
#[test]
fn test_a_call_reaches_a_local_it_is_handed_without_capturing_it() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "declare void @look(ptr nocapture)
define i16 @f() {
b0:
  %a = alloca i16
  store i16 1, ptr %a
  call void @look(ptr %a)
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let insts: Vec<_> = unit.function.walk().map(|(_, inst)| inst).collect();
    let call = *insts
        .iter()
        .find(|&&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ))
        .expect("the call");
    let store = *insts
        .iter()
        .find(|&&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Store { .. }
        ))
        .expect("the store");
    let loc = aa.location_of(store).expect("a location");
    // Premise: the call's effects are unknown to the access lists.
    assert!(accesses.writes(call).is_none());
    assert!(aa.get_mod_ref_info(call, &loc).is_mod_or_ref());
    assert!(aa.mod_set_loc(call, &loc), "the call may write the slot it was handed");
}

/// A call that names no location cannot touch a local that only a later call
/// captures: the whole-function capture answer made it reach the slot, so a
/// store before the escape stayed live across the unknown call.
#[test]
fn test_a_call_before_the_escape_does_not_reach_the_local() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "declare void @keep(ptr)
define i16 @f() {
b0:
  %a = alloca i16
  store i16 1, ptr %a
  call void @anything()
  call void @keep(ptr %a)
  ret i16 0
}
",
    );
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let aa = AliasQuery::new(&unit, &accesses);
    let insts: Vec<_> = unit.function.walk().map(|(_, inst)| inst).collect();
    let is_call = |&&inst: &&llrm_mir::module::InstId| {
        matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        )
    };
    let calls: Vec<_> = insts.iter().filter(is_call).copied().collect();
    let store = *insts
        .iter()
        .find(|&&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Store { .. }
        ))
        .expect("the store");
    let loc = aa.location_of(store).expect("a location");
    // Premise: both calls are unknown to the access lists.
    assert!(calls.iter().all(|&call| accesses.writes(call).is_none()), "{calls:?}");
    assert!(!aa.get_mod_ref_info(calls[0], &loc).is_mod_or_ref(), "before the escape");
    assert!(aa.get_mod_ref_info(calls[1], &loc).is_mod_or_ref(), "the escape itself reaches it");
}

/// A call that reads unknown memory reads a local only after the local's
/// address was captured (`EarliestEscapeInfo`). The whole function was asked:
/// the `error()` on the path that never reaches the capture was taken to read
/// the slot, and DSE kept a `memset` that another path overwrote.
#[test]
fn test_unknown_reads_of_a_call_reach_a_local_only_after_its_capture() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "declare void @use(ptr)

declare void @error() noreturn

define void @f(i1 %c) {
b0:
  %s = alloca [4 x i8]
  store i16 1, ptr %s
  br i1 %c, label %b2, label %b1

b1:
  call void @use(ptr %s)
  call void @error()
  ret void

b2:
  call void @error()
  unreachable
}
",
    );
    let unit = parsed.unit();
    let references = unit.function.walk().filter_map(|(_, inst)| unit.reference(inst).map(|one| (inst, one))).collect();
    let calls: Vec<_> = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Call(_)
        ))
        .collect();
    let [_use, after_capture, before_capture] = calls[..] else { panic!("{calls:?}") };
    let accesses = Accesses::of(&unit, references, &crate::alias::calls_by_attributes(&unit));
    let aa = AliasQuery::new(&unit, &accesses);
    let store = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            unit.function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Store { .. }
        ))
        .expect("the store");
    let slot = aa.location_of(store).expect("the slot");
    assert!(aa.get_mod_ref_info(after_capture, &slot).reads, "after the capture the call may read the slot");
    assert!(!aa.get_mod_ref_info(before_capture, &slot).reads, "before any capture it cannot");
}

/// LLRM_CHECK_LICM_GRAPH accepted every kept answer once it let a later one
/// pass; an earlier one (a clobber missed) must still fail.
#[test]
fn test_a_kept_answer_is_safe_only_when_the_fresh_one_dominates_it() {
    use super::shadow::{Name, answer_is_safe};
    let parsed = Parsed::new(
        "define void @f(i1 %c) {
b0:
  br label %b1

b1:
  store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
  br i1 %c, label %b1, label %b2

b2:
  br label %b3

b3:
  store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
  br i1 %c, label %b3, label %b4

b4:
  ret void
}
",
    );
    let unit = parsed.unit();
    let (fresh, _) = translated(&unit);
    let at = |name: &str| crate::cfg::id(crate::testing::block(unit.function, name));
    let (first, second) = (Name::Phi(at("b1")), Name::Phi(at("b3")));
    let store = Name::Site(unit.function.block(crate::testing::block(unit.function, "b1")).first().expect("a store"));
    assert!(answer_is_safe(&fresh, &second, &first), "a later answer than the fresh one is safe");
    assert!(answer_is_safe(&fresh, &first, &first));
    assert!(!answer_is_safe(&fresh, &first, &second), "an earlier answer misses the second loop's clobber");
    assert!(!answer_is_safe(&fresh, &Name::Live, &store), "live on entry misses the store");
    assert!(answer_is_safe(&fresh, &store, &Name::Live));
}

/// LLRM_CHECK_PRESERVED said `earlycse@1180 left mssa held but stale` of 172
/// programs: a load the walker had moved above a store it does not alias
/// (`MemoryUse::isOptimized`) differed from the fresh graph's nearest def. A
/// graph the walker has answered for equals one made afresh.
#[test]
fn a_graph_with_walked_uses_equals_one_made_afresh() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "define i8 @f() {
b0:
  store i32 7, ptr @g
  store i8 1, ptr getelementptr (i8, ptr @g, i16 4)
  %r = load i8, ptr @g
  ret i8 %r
}
",
    );
    let unit = parsed.unit();
    let accesses = std::rc::Rc::new(Accesses::plain(&unit, &crate::consts::Calls::default()));
    let build = || Mssa::build(unit.function, &unit.shape().dominance, &shadow::effect_of(&accesses, unit.function));
    let mut held = build();
    let aa = AliasQuery::new(&unit, &accesses);
    shadow::walked(&mut held, &aa, unit.function);
    let graph = |mssa| crate::manager::MssaGraph::new(mssa, Some(std::rc::Rc::clone(&accesses)));
    assert!(graph(held) == graph(build()));
}

/// A client that holds pointers and no access names them (MemCpyOpt, made
/// with `Accesses::resolved`, 2.7 M of `Pointers::run`) asks the stack: with no
/// access list at all, a store is apart from another object and not from its
/// own, a call is no better than its attributes, and two locations alias as the
/// pin's BasicAA says.
#[test]
fn test_the_queries_of_a_pointer_need_no_access_lists() {
    use super::alias::{AliasQuery, AliasResult};
    let parsed = Parsed::new(
        "declare void @llvm.lifetime.end.p0(i64, ptr)

define i16 @f() {
b0:
  %a = alloca i16
  %b = alloca i16
  store i16 1, ptr %a
  %x = load i16, ptr %a
  %y = load i16, ptr %b
  call void @llvm.lifetime.end.p0(i64 2, ptr %b)
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let none = Accesses::default();
    let aa = AliasQuery::new(&unit, &none);
    let inst = |at: usize| unit.function.iter_block(crate::testing::block(unit.function, "b0")).nth(at).unwrap();
    let (store, load_a, load_b, ended) = (inst(2), inst(3), inst(4), inst(5));
    let pointer = |name: &str| llrm_mir::module::Operand::Value(crate::testing::value(unit.function, name));
    let (at_a, at_b) = (aa.location_at(pointer("a"), 2), aa.location_at(pointer("b"), 2));
    assert_eq!(aa.alias(&at_a, &at_b), AliasResult::No);
    assert_eq!(aa.alias(&at_a, &aa.location_at(pointer("a"), 2)), AliasResult::Must);
    assert!(aa.get_mod_ref_info(store, &at_a).writes, "a store writes its own cell");
    assert!(!aa.get_mod_ref_info(store, &at_b).writes, "and not another object's");
    assert!(!aa.get_mod_ref_info(ended, &at_a).is_mod_or_ref(), "lifetime.end of %b leaves %a alone");
    assert!(aa.get_mod_ref_info(ended, &at_b).is_mod_or_ref());
    assert!(aa.mod_ref_instructions(store, load_a), "the store and the load of %a");
    assert!(!aa.mod_ref_instructions(store, load_b), "the store to %a and the load of %b");
    assert!(!aa.mod_ref_instructions(ended, load_a) && aa.mod_ref_instructions(ended, load_b));
    assert!(!aa.mod_set_call(store, ended), "a store to %a against a call that touches %b");
}

/// MemorySSA.cpp#L298: an `invariant.start` shows up as a def that touches
/// the memory it covers, but it is a marker and clobbers no query. A call after
/// it was clobbered by it (the call's attributes against the start's).
#[test]
fn test_an_invariant_start_clobbers_no_later_call() {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(
        "declare ptr @llvm.invariant.start.p0(i64 immarg, ptr nocapture)
declare void @ext(ptr)

define void @f(ptr %q, ptr %p) {
b0:
  %s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)
  call void @ext(ptr %p)
  ret void
}
",
    );
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let mssa = Mssa::build(unit.function, &unit.shape().dominance, &shadow::effect_of(&accesses, unit.function));
    let aa = AliasQuery::new(&unit, &accesses);
    let insts: Vec<_> = unit.function.iter_block(crate::testing::block(unit.function, "b0")).collect();
    let (start, ext) = (insts[0], insts[1]);
    let def = mssa.get_memory_access(start).expect("the start is a def");
    assert!(!super::walker::instruction_clobbers_query(&mssa, &aa, def, None, Some(ext)), "a marker clobbers nothing");
}
