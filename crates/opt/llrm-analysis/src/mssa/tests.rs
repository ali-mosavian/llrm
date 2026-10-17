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
    let load = unit.function.block(join).instructions()[0];
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

/// The clobbering access of each memory instruction of `body`'s `@f` that has a location, by the
/// translated walker at its default caps, each checked with `check_clobber_sanity`.
fn clobbers_of(body: &str) -> Vec<(llrm_mir::module::InstId, Option<llrm_mir::module::InstId>, Kind)> {
    use super::alias::AliasQuery;
    let parsed = Parsed::new(body);
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &crate::consts::Calls::default());
    let (mut mssa, _) = {
        let mssa = Mssa::build(unit.function, &unit.shape().dominance, &shadow::effect_of(&accesses));
        (mssa, ())
    };
    let aa = AliasQuery::new(&unit, &accesses);
    let mut out = Vec::new();
    for (_, inst) in unit.function.walk() {
        let Some(access) = mssa.get_memory_access(inst) else { continue };
        let Some(loc) = aa.location_of(inst) else { continue };
        let clobber = mssa.get_clobbering_memory_access(&aa, access);
        let start = mssa.get_defining_access(access).expect("a defining access");
        // A use's optimized access is its defining access now; the walk started from the one it had.
        let _ = start;
        mssa.check_clobber_sanity(&aa, mssa.access(access).defining.expect("defining"), clobber, Some(&loc), Some(inst))
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

/// The walk skips the store to another cell and stops at the store to the load's own.
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

/// Both arms store to another cell: the phi at the join is optimized away and the store above the
/// diamond clobbers the load.
#[test]
fn test_a_phi_is_optimized_away_when_no_arm_writes_the_cell() {
    let found = clobbers_of(DIAMOND_OF_OTHER_CELL);
    assert_eq!(found.last().expect("the load").1, Some(found[0].0), "{found:?}");
}

/// Both arms store to the load's cell: the phi stays the clobber (no instruction).
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
