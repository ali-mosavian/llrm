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
