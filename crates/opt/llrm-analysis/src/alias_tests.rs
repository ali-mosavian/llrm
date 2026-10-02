//! Ports of `tests/test_mir_alias.py`, on rich MIR text.

use std::collections::BTreeSet;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::IndexMap;

use super::{_direct_summary, Effect, PointsTo, Procedure, Summary, UNKNOWN, annotated, calls_annotated, congruences, nonnull_by_definition, points_to, summaries};
use crate::memory::{self, Identity, MemRef, MemoryKind, MemoryObject, Provenance, Slice, Unit, object_of};
use crate::regions::overlapping;
use crate::testing::{DOS, function, layout, parsed, value};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit_of(&self, name: &str) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, function(&self.module, name))
    }

    fn unit(&self) -> Unit<'_> {
        self.unit_of("f")
    }

    fn value(&self, name: &str) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn facts(&self) -> PointsTo {
        points_to(&self.unit(), None, None).unwrap()
    }

    /// The object `%name` allocates.
    fn object(&self, name: &str) -> MemoryObject {
        object_of(&self.unit(), Operand::Value(self.value(name))).expect("an object")
    }

    /// The instructions of `@f` whose opcode `is` picks.
    fn all(&self, is: impl Fn(&Opcode) -> bool) -> Vec<InstId> {
        let f = function(&self.module, "f");
        f.walk().map(|(_, inst)| inst).filter(|&inst| is(&f.instruction(inst).opcode)).collect()
    }

    /// What `@f`'s calls read and write, callees summarized by `known`.
    fn effects(&self, known: &IndexMap<String, Summary>) -> Vec<Effect> {
        calls_annotated(&Procedure::of(self.unit()), known).unwrap().into_values().collect()
    }
}

fn object(kind: MemoryKind, identity: Identity, extent: Option<i64>) -> MemoryObject {
    MemoryObject { identity: Some(identity), extent, ..MemoryObject::new(kind) }
}

fn one(object: &MemoryObject, low: i64, high: i64) -> Provenance {
    Provenance::one_with_slice(object.clone(), low, high, 1, 1, BTreeSet::new()).unwrap()
}

fn parameter(index: i64) -> MemoryObject {
    object(MemoryKind::Parameter, Identity::Int(index), None)
}

/// An access to bytes `low..high` of `object`.
fn bytes(object: &MemoryObject, low: i64, high: i64) -> MemRef {
    let slice = Slice::new(object.clone(), low, low + 1, 1, high - low).unwrap();
    MemRef::reach(u32::try_from(high - low).unwrap(), Provenance { slices: BTreeSet::from([slice]), restrict: BTreeSet::new() })
}

fn overlaps(one: &MemRef, other: &MemRef) -> bool {
    overlapping(one, other, None, None, None).unwrap()
}

fn writes(effect: &Effect, access: &MemRef) -> bool {
    effect.stores.iter().any(|one| overlaps(access, one))
}

#[test]
fn test_points_to_flows_through_memory_and_a_phi() {
    // A pointer spilled on one arm and joined with a copy retains its object set.
    let parsed = Parsed::new(
        "define void @f(i1 %c) {
b0:
  %root = alloca [4 x i8]
  %slot = alloca ptr
  store ptr %root, ptr %slot
  br i1 %c, label %b10, label %b20

b10:
  %loaded = load ptr, ptr %slot
  br label %b30

b20:
  br label %b30

b30:
  %joined = phi ptr [ %loaded, %b10 ], [ %root, %b20 ]
  ret void
}
",
    );
    let facts = parsed.facts();
    let root = &facts.values[&parsed.value("root")];
    assert_eq!(&facts.values[&parsed.value("loaded")], root);
    assert_eq!(&facts.values[&parsed.value("joined")], root);
}

#[test]
fn test_a_call_writing_a_cell_leaves_its_contents_unknown() {
    // A call given &local was taken to leave the pointer stored in it, so a
    // heap pointer loaded back after it was "into local".
    let parsed = Parsed::new(
        "declare void @g(ptr)

define void @f() {
b0:
  %local = alloca ptr
  %other = alloca i16
  store ptr %other, ptr %local
  call void @g(ptr %local)
  %loaded = load ptr, ptr %local
  ret void
}
",
    );
    let loaded = &parsed.facts().values[&parsed.value("loaded")];
    assert!(loaded.slices.iter().any(|one| one.object.kind == MemoryKind::Unknown), "{loaded:?}");
}

#[test]
fn a_call_that_writes_nothing_keeps_a_spilled_pointer() {
    let parsed = Parsed::new(
        "declare void @g(ptr) memory(read)

define void @f() {
b0:
  %local = alloca ptr
  %other = alloca i16
  store ptr %other, ptr %local
  call void @g(ptr %local)
  %loaded = load ptr, ptr %local
  ret void
}
",
    );
    let facts = parsed.facts();
    assert_eq!(facts.values[&parsed.value("loaded")], facts.values[&parsed.value("other")]);
}

#[test]
fn test_a_pointer_advanced_through_memory_reaches_a_fixed_point() {
    // `*to++` in heap.c's copy_bytes: each pass stored one more offset into
    // the cell, and llrm-c never finished.
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let parsed = Parsed::new(
            "define void @f(ptr %start, i1 %c) {
b0:
  %slot = alloca ptr
  store ptr %start, ptr %slot
  br label %b10

b10:
  %loaded = load ptr, ptr %slot
  %advanced = getelementptr i8, ptr %loaded, i16 1
  store ptr %advanced, ptr %slot
  br i1 %c, label %b10, label %b20

b20:
  ret void
}
",
        );
        sender.send(parsed.facts().values[&parsed.value("loaded")].clone()).unwrap();
    });
    let loaded = receiver.recv_timeout(std::time::Duration::from_secs(10)).expect("points_to terminates");
    assert!(loaded.slices.iter().any(|one| one.object == parameter(0)));
}

#[test]
fn test_unannotated_computed_address_keeps_ssa_provenance() {
    // C nbody lost every frame-array object and stopped unrolling its hot loop.
    let parsed = Parsed::new(
        "define i64 @f(i16 %i) {
b0:
  %a = alloca [32 x i8]
  %p = getelementptr i8, ptr %a, i16 %i
  %r = load i64, ptr %p
  ret i64 %r
}
",
    );
    let load = parsed.all(|op| matches!(op, Opcode::Load { .. }))[0];
    let tagged = annotated(&parsed.unit()).unwrap()[&load].provenance.clone().expect("a derived provenance");
    assert_eq!(tagged.slices.iter().map(|one| one.object.clone()).collect::<BTreeSet<_>>(), BTreeSet::from([parsed.object("a")]));
}

#[test]
fn test_store_through_parameter_keeps_disjoint_frame_pointer_spill() {
    // ls_switch lost its saved parameter after storing field 0, so fields
    // 2/4/6 became unknown writes.
    let parsed = Parsed::new(
        "define void @f(ptr %root) {
b0:
  %slot = alloca ptr
  store ptr %root, ptr %slot
  %first = load ptr, ptr %slot
  store i16 1, ptr %first
  %second = load ptr, ptr %slot
  %field = getelementptr i8, ptr %second, i16 2
  store i16 2, ptr %field
  ret void
}
",
    );
    let facts = parsed.facts();
    let summary = _direct_summary(&parsed.unit()).unwrap();
    assert_eq!(facts.values[&parsed.value("second")], facts.values[&parsed.value("root")]);
    assert!(!summary.unknown_write);
    assert_eq!(summary.writes.iter().map(|one| one.object.clone()).collect::<BTreeSet<_>>(), BTreeSet::from([parameter(0)]));
}

#[test]
fn test_pointer_phi_with_an_unknown_arm_is_unknown() {
    // A known arm must not erase the other arm: that falsely made the joined
    // pointer disjoint from real objects.
    let parsed = Parsed::new(
        "define void @f(i1 %c, i16 %n) {
b0:
  %known = alloca [4 x i8]
  br i1 %c, label %b10, label %b20

b10:
  br label %b30

b20:
  %unknown = inttoptr i16 %n to ptr
  br label %b30

b30:
  %joined = phi ptr [ %known, %b10 ], [ %unknown, %b20 ]
  ret void
}
",
    );
    assert_eq!(parsed.facts().values[&parsed.value("joined")], *UNKNOWN);
}

#[test]
fn test_pointer_recurrence_at_a_loop_header_widens_and_terminates() {
    // qcport con_print advanced its text pointer one byte per solver round
    // instead of compiling.
    let parsed = Parsed::new(
        "define void @f(ptr %root, i1 %c) {
b0:
  br label %b10

b10:
  %current = phi ptr [ %root, %b0 ], [ %advanced, %b20 ]
  br label %b20

b20:
  %advanced = getelementptr i8, ptr %current, i16 1
  br i1 %c, label %b10, label %b30

b30:
  ret void
}
",
    );
    let facts = parsed.facts();
    let expected = Provenance { slices: UNKNOWN.slices.union(&Provenance::one(parameter(0)).slices).cloned().collect(), restrict: BTreeSet::new() };
    assert_eq!(facts.values[&parsed.value("current")], expected);
    assert_eq!(facts.values[&parsed.value("advanced")], expected);
}

#[test]
fn test_parameter_modref_is_instantiated_at_a_call_site() {
    // A callee writing parameter zero clobbers its actual object and no neighbour.
    let summary = Summary { writes: BTreeSet::from([Slice::new(parameter(0), 2, 4, 1, 1).unwrap()]), ..Summary::default() };
    let actual = object(MemoryKind::Global, Identity::Global(7), Some(16));
    let effect = summary.instantiated(&[one(&actual, 4, 5)]).writes;
    assert_eq!(effect, BTreeSet::from([Slice::new(actual, 6, 8, 1, 1).unwrap()]));
}

const CALLEE_WRITES_ITS_PARAMETER: &str = "@g = global [16 x i8] zeroinitializer

define void @callee(ptr %p) {
b0:
  %q = getelementptr i8, ptr %p, i16 2
  store i16 0, ptr %q
  ret void
}

define void @f() {
b0:
  call void @callee(ptr getelementptr (i8, ptr @g, i16 4))
  ret void
}
";

#[test]
fn test_interprocedural_modref_reaches_the_call_operation() {
    // A known callee replaces the call's catch-all effect with its actual object.
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures = IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
    let known = summaries(&procedures, None).unwrap();
    let effect = &parsed.effects(&known)[0];
    let Some(super::Actual::Provenance(passed)) = procedures["f"].arguments.values().next().map(|actual| actual[0].clone()) else { panic!("@g's slice") };
    let g = passed.slices.first().unwrap().object.clone();
    assert!(effect.loads.is_empty());
    assert!(writes(effect, &bytes(&g, 6, 8)));
    assert!(!writes(effect, &bytes(&g, 4, 6)) && !writes(effect, &bytes(&g, 8, 16)));
}

#[test]
fn test_recursive_pointer_offset_summary_widens_and_terminates() {
    // A recursive f(p + 1) grew one byte per summary round; the SCC effect is
    // the whole formal object.
    let parsed = Parsed::new(
        "define void @f(ptr %p) {
b0:
  store i8 0, ptr %p
  %next = getelementptr i8, ptr %p, i16 1
  call void @f(ptr %next)
  ret void
}
",
    );
    let procedures = IndexMap::from_iter([("f".to_owned(), Procedure::of(parsed.unit()))]);
    let summary = summaries(&procedures, None).unwrap()["f"].clone();
    assert_eq!(summary.writes, BTreeSet::from([Slice::whole(parameter(0))]));
}

#[test]
fn test_unknown_call_reaches_nonlocals_and_only_its_pointer_actual() {
    // An unknown C call does not clobber every local, but may use a local
    // whose address it receives.
    let parsed = Parsed::new(
        "declare void @external(ptr, i16)

define void @f() {
b0:
  %passed = alloca [4 x i8]
  %private = alloca [4 x i8]
  call void @external(ptr %passed, i16 0)
  ret void
}
",
    );
    let effect = &parsed.effects(&IndexMap::default())[0];
    let (passed, private) = (parsed.object("passed"), parsed.object("private"));
    assert!(writes(effect, &bytes(&passed, 0, 4)) && writes(effect, &bytes(&passed, 3, 4)));
    assert!(!writes(effect, &bytes(&private, 0, 4)));
    assert!(effect.stores.iter().any(|one| one.provenance.as_ref().unwrap().slices.iter().any(|one| one.object.kind == MemoryKind::Nonlocal)));
}

const PUBLISHED: &str = "@g = global ptr null

declare void @external()

define void @f() {
b0:
  %escaped = alloca [4 x i8]
  %private = alloca [4 x i8]
  store ptr %escaped, ptr @g
  call void @external()
  ret void
}
";

#[test]
fn test_unknown_call_reaches_a_frame_pointer_escaped_before_the_call() {
    // Storing &local outside the frame exposes that object to a later
    // unknown call, not its neighbours.
    let parsed = Parsed::new(PUBLISHED);
    let effect = &parsed.effects(&IndexMap::default())[0];
    assert!(writes(effect, &bytes(&parsed.object("escaped"), 0, 4)));
    assert!(!writes(effect, &bytes(&parsed.object("private"), 0, 4)));
}

#[test]
fn a_pointer_published_after_a_call_is_not_reached_by_it() {
    let text = PUBLISHED.replace("  store ptr %escaped, ptr @g\n  call void @external()\n", "  call void @external()\n  store ptr %escaped, ptr @g\n");
    let parsed = Parsed::new(&text);
    let effect = &parsed.effects(&IndexMap::default())[0];
    assert!(!writes(effect, &bytes(&parsed.object("escaped"), 0, 4)));
}

#[test]
fn test_known_capture_summary_controls_later_external_reach() {
    // A borrowed pointer remains private; a retained pointer exposes its
    // object to subsequent unknown calls.
    let parsed = Parsed::new(
        "declare void @known(ptr)

declare void @external()

define void @f() {
b0:
  %local = alloca [4 x i8]
  call void @known(ptr %local)
  call void @external()
  ret void
}
",
    );
    let local = bytes(&parsed.object("local"), 0, 4);
    let borrowed = parsed.effects(&IndexMap::from_iter([("known".to_owned(), Summary::default())]));
    let captured = Summary { captures: BTreeSet::from([Some(Identity::Int(0))]), ..Summary::default() };
    let captured = parsed.effects(&IndexMap::from_iter([("known".to_owned(), captured)]));
    assert!(!writes(&borrowed[1], &local));
    assert!(writes(&captured[1], &local));
}

#[test]
fn test_offsets_in_different_objects_are_never_compared() {
    // A bp slot and an sp push were called disjoint by comparing -0x16 with -2.
    let r#static = one(&object(MemoryKind::Global, Identity::Str("table".to_owned()), None), 0x16, 0x18);
    let extern_ = one(&object(MemoryKind::External, Identity::Str("shared".to_owned()), None), 2, 4);
    assert!(r#static.intersects(&extern_));
}

#[test]
fn test_capture_decides_what_nonlocal_reaches() {
    // A call's NONLOCAL reach met every global, so no call left a private
    // static in a register.
    let private = MemoryObject { captured: false, ..object(MemoryKind::Global, Identity::Str("counter".to_owned()), None) };
    let unaddressed = MemoryObject { addressed: false, captured: false, ..object(MemoryKind::Global, Identity::Str("total".to_owned()), None) };
    let nonlocal = MemoryObject::new(MemoryKind::Nonlocal);
    let unknown = MemoryObject::new(MemoryKind::Unknown);
    assert!(!memory::objects_may_alias(&nonlocal, &private));
    // A pointer no fact follows reaches only what escaped (`_lost`
    // publishes the rest); the old route's unknown x86 operand met it too.
    assert!(!memory::objects_may_alias(&unknown, &private));
    assert!(!memory::objects_may_alias(&unknown, &unaddressed));
    assert!(memory::objects_may_alias(&unknown, &MemoryObject { captured: true, ..private.clone() }));
    assert!(memory::objects_may_alias(&unaddressed, &unaddressed));
}

#[test]
fn test_one_base_value_settles_provenance_references_by_displacement() {
    // Two fields off one pointer, each whole-object provenance, were called overlapping.
    let parsed = Parsed::new(
        "define void @f(ptr %p) {
b0:
  %second = getelementptr i8, ptr %p, i16 2
  %third = getelementptr i8, ptr %p, i16 1
  store i16 0, ptr %p
  store i16 0, ptr %second
  store i16 0, ptr %third
  ret void
}
",
    );
    let unit = parsed.unit();
    let [first, second, third] = parsed
        .all(|op| matches!(op, Opcode::Store { .. }))
        .into_iter()
        .map(|inst| MemRef { provenance: Some(UNKNOWN.clone()), ..MemRef::of(&unit, inst).unwrap() })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(overlapping(&first, &second, None, None, None), Ok(false));
    assert_eq!(overlapping(&first, &third, None, None, None), Ok(true));
}

/// Whether `@f`'s two stores, through `%p` and `%q`, may overlap.
fn parameters_overlap(attributes: &str) -> bool {
    let parsed = Parsed::new(&format!(
        "define void @f(ptr {attributes} %p, ptr {attributes} %q) {{
b0:
  store i16 0, ptr %p
  store i16 0, ptr %q
  ret void
}}
"
    ));
    let tagged = annotated(&parsed.unit()).unwrap();
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    overlaps(&tagged[&stores[0]], &tagged[&stores[1]])
}

#[test]
fn noalias_parameters_are_disjoint_and_plain_ones_may_alias() {
    assert!(!parameters_overlap("noalias"));
    assert!(parameters_overlap(""));
}

#[test]
fn a_frame_object_is_nonnull_and_a_parameter_is_not() {
    let parsed = Parsed::new(
        "define void @f(ptr %p) {
b0:
  %a = alloca i16
  %slot = alloca ptr
  %loaded = load ptr, ptr %slot
  ret void
}
",
    );
    let (unit, facts) = (parsed.unit(), parsed.facts());
    assert!(facts.nonnull(parsed.value("a")) && !facts.nonnull(parsed.value("p")) && !facts.nonnull(parsed.value("loaded")));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("a")), Some(true));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("p")), Some(false));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("loaded")), None);
}

/// The objects `@f` lets escape when `%a`'s address meets `use_`.
fn escaped(use_: &str) -> BTreeSet<MemoryObject> {
    let parsed = Parsed::new(&format!(
        "define ptr @f() {{
b0:
  %a = alloca i16
  %slot = alloca ptr
  {use_}
}}
"
    ));
    parsed.facts().escaped
}

#[test]
fn a_returned_or_integer_address_escapes_and_a_frame_spill_does_not() {
    assert_eq!(escaped("ret ptr %a").len(), 1);
    assert_eq!(escaped("%n = ptrtoint ptr %a to i16\n  ret ptr null").len(), 1);
    assert!(escaped("store ptr %a, ptr %slot\n  ret ptr null").is_empty());
    assert_eq!(escaped("store ptr %a, ptr %slot\n  ret ptr %slot").len(), 2);
}

#[test]
fn annotated_narrows_a_loop_index_to_its_strided_interval() {
    // `tests/test_edge_ranges.py:guarded_loop` plus a word load at @g+4
    // indexed by the counter scaled by two.
    let parsed = Parsed::new(
        "@g = global [64 x i8] zeroinitializer

define void @f() {
b0:
  br label %b10

b10:
  %i = phi i16 [ 0, %b0 ], [ %next, %b40 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b20, label %b50

b20:
  %g = icmp slt i16 %i, 4
  br i1 %g, label %b30, label %b40

b30:
  %off = mul i16 %i, 2
  %q = getelementptr i8, ptr @g, i16 %off
  %p = getelementptr i8, ptr %q, i16 4
  %x = load i16, ptr %p
  br label %b40

b40:
  %next = add i16 %i, 1
  br label %b10

b50:
  ret void
}
",
    );
    let strides = congruences(&parsed.unit());
    assert_eq!(strides.get(&parsed.value("i")), Some(&(1.into(), 0.into())));
    assert_eq!(strides.get(&parsed.value("off")), Some(&(2.into(), 0.into())));

    let load = parsed.all(|op| matches!(op, Opcode::Load { .. }))[0];
    let tagged = annotated(&parsed.unit()).unwrap()[&load].provenance.clone().expect("a provenance");
    let root = MemRef::of(&parsed.unit(), load).and_then(|one| one.root).expect("a root");
    let global = object_of(&parsed.unit(), root).expect("an object");
    assert_eq!(tagged.slices, BTreeSet::from([Slice::new(global, 4, 11, 2, 2).unwrap()]));
}

/// A byte counter stepping by `step` for 200 trips, counting the trips
/// where `%i urem modulus != residue`.
fn wrapping(step: i64, check: Option<(i64, i64)>) -> String {
    let (modulus, residue) = check.unwrap_or((1, 0));
    format!(
        "define i16 @f() {{
b0:
  br label %b1

b1:
  %i = phi i8 [ 0, %b0 ], [ %next, %b1 ]
  %j = phi i16 [ 0, %b0 ], [ %jnext, %b1 ]
  %wrong = phi i16 [ 0, %b0 ], [ %worse, %b1 ]
  %r = urem i8 %i, {modulus}
  %off = icmp ne i8 %r, {residue}
  %z = zext i1 %off to i16
  %worse = add i16 %wrong, %z
  %next = add i8 %i, {step}
  %jnext = add i16 %j, 1
  %c = icmp slt i16 %jnext, 200
  br i1 %c, label %b1, label %b2

b2:
  %w = phi i16 [ %worse, %b1 ]
  ret i16 %w
}}
"
    )
}

/// `i += 3` on a byte was `i == 0 (mod 3)`, but 255 + 3 is 2: the stride
/// must divide the width's modulus to survive the wrap.
#[test]
fn a_congruence_holds_across_the_counters_wrap() {
    for (step, strided) in [(3, false), (4, true), (-12, true)] {
        let parsed = Parsed::new(&wrapping(step, None));
        let (modulus, residue) = congruences(&parsed.unit())[&parsed.value("i")].clone();
        if strided {
            assert!(modulus > 1.into(), "{step}: {modulus}");
        }
        let (modulus, residue) = (i64::try_from(modulus).unwrap(), i64::try_from(residue).unwrap());
        if modulus > 1 {
            let checked = parsed_module(&wrapping(step, Some((modulus, residue))));
            assert_eq!(llrm_mir::interpret::run(&checked, "f", vec![], 100_000), Ok(llrm_mir::interpret::Val::Int { bits: 0, width: 16 }), "{step}");
        }
    }
}

fn parsed_module(text: &str) -> Module {
    parsed(&format!("{DOS}{text}"))
}

/// A slot zeroed by `memset` and only later made to hold `&inner`: the
/// memset may only write through its argument, so it reads no pointer out
/// of the slot and publishes nothing, and a call between the two does not
/// reach `inner`. Lending the slot published what it would later hold, so
/// every bounds error in T028's `update` read its zeroed samples.
#[test]
fn a_write_only_argument_publishes_nothing_it_will_hold() {
    let parsed = Parsed::new(
        "declare void @external()

declare void @use(ptr)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

define void @f() {
b0:
  %slot = alloca [4 x i8]
  %inner = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %slot, i8 0, i16 4, i1 false)
  call void @external()
  store ptr %inner, ptr %slot
  call void @use(ptr %slot)
  ret void
}
",
    );
    let effects = parsed.effects(&IndexMap::default());
    assert!(!writes(&effects[1], &bytes(&parsed.object("inner"), 0, 4)));
}

/// A value nothing is known of is a multiple of 1, so `x << 1` and
/// `0 - (x << 1)` are multiples of 2 and `x * 8` of 8: the fact a trip
/// count's divisibility proof reads. Only operands with a fact gave one.
#[test]
fn test_a_shift_or_multiple_of_an_unknown_is_a_multiple() {
    let parsed = Parsed::new(
        "define void @f(i16 %x, i16 %y) {
b0:
  %s = shl i16 %x, 1
  %t = sub i16 0, %s
  %m = mul i16 %y, 8
  %u = add i16 %x, %y
  ret void
}
",
    );
    let found = congruences(&parsed.unit());
    assert_eq!(found.get(&parsed.value("s")), Some(&(2.into(), 0.into())));
    assert_eq!(found.get(&parsed.value("t")), Some(&(2.into(), 0.into())));
    assert_eq!(found.get(&parsed.value("m")), Some(&(8.into(), 0.into())));
    assert_eq!(found.get(&parsed.value("u")), None);
}

/// A parameter the language states `nonnull` is non-null at its definition;
/// the same pointer unstated may be null.
#[test]
fn a_nonnull_parameter_is_nonnull_by_definition() {
    let parsed = Parsed::new("define void @f(ptr nonnull %p, ptr %q) {\nb0:\n  ret void\n}\n");
    let unit = parsed.unit();
    assert_eq!(nonnull_by_definition(&unit, parsed.value("p")), Some(true));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("q")), Some(false));
}
