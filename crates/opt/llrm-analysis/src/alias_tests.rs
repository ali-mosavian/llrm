//! Ports of `tests/test_mir_alias.py`, on rich MIR text.

use std::collections::BTreeSet;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::IndexMap;

use super::{
    _direct_summary, Effect, PointsTo, Procedure, Summary, UNKNOWN, annotated, calls_by_attributes, congruences,
    nonnull_by_definition, points_to, summaries,
};
use crate::memory::{
    self, Identity, MemRef, MemoryKind, MemoryObject, ObjectInterner, ObjectRef, Provenance, Slice, Unit, object_of,
};
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

    fn unit_of(
        &self,
        name: &str,
    ) -> Unit<'_> {
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, name)))
    }

    fn unit(&self) -> Unit<'_> {
        self.unit_of("f")
    }

    fn value(
        &self,
        name: &str,
    ) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn facts(&self) -> PointsTo {
        points_to(&self.unit(), None, None).unwrap()
    }

    /// The object `%name` allocates.
    fn object(
        &self,
        name: &str,
    ) -> ObjectRef {
        object_of(&self.unit(), Operand::Value(self.value(name))).expect("an object")
    }

    /// The instructions of `@f` whose opcode `is` picks.
    fn all(
        &self,
        is: impl Fn(&Opcode) -> bool,
    ) -> Vec<InstId> {
        let f = function(&self.module, "f");
        f.walk().map(|(_, inst)| inst).filter(|&inst| is(&f.instruction(inst).opcode)).collect()
    }

    /// What `@f`'s calls read and write, callees summarized by `known`.
    fn effects(
        &self,
        _known: &IndexMap<String, Summary>,
    ) -> Vec<Effect> {
        calls_by_attributes(&crate::testing::with_points(self.unit())).values().cloned().collect()
    }
}

fn object(
    kind: MemoryKind,
    identity: Identity,
    extent: Option<i64>,
) -> MemoryObject {
    MemoryObject { identity: Some(identity), extent, ..MemoryObject::new(kind) }
}

fn one(
    object: &MemoryObject,
    low: i64,
    high: i64,
) -> Provenance {
    Provenance::one_with_slice(object.clone(), low, high, 1, 1, BTreeSet::new()).unwrap()
}

fn parameter(index: i64) -> MemoryObject {
    object(MemoryKind::Parameter, Identity::Int(index), None)
}

/// An access to bytes `low..high` of `object`.
fn bytes(
    object: &(impl Into<ObjectRef> + Clone),
    low: i64,
    high: i64,
) -> MemRef {
    let slice = Slice::new(object.clone(), low, low + 1, 1, high - low).unwrap();
    MemRef::reach(
        u32::try_from(high - low).unwrap(),
        Provenance { slices: crate::memory::Sorted::from([slice]), restrict: crate::memory::Sorted::new() },
    )
}

fn overlaps(
    one: &MemRef,
    other: &MemRef,
) -> bool {
    overlapping(one, other, None, None, None).unwrap()
}

fn writes(
    effect: &Effect,
    access: &MemRef,
) -> bool {
    effect.stores.iter().any(|one| overlaps(access, one))
}

#[test]
fn test_points_to_flows_through_memory_and_a_phi() {
    // A pointer spilled on one arm and joined with a copy retains its object
    // set.
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
        let loaded = parsed.facts().values[&parsed.value("loaded")].clone();
        sender
            .send(
                loaded
                    .slices
                    .iter()
                    .any(|one| ObjectInterner::of(&parsed.module.context).object(one.object) == parameter(0)),
            )
            .unwrap();
    });
    assert!(receiver.recv_timeout(std::time::Duration::from_secs(10)).expect("points_to terminates"));
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
    assert_eq!(
        tagged.slices.iter().map(|one| one.object).collect::<BTreeSet<_>>(),
        BTreeSet::from([parsed.object("a")])
    );
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
    assert_eq!(
        summary
            .writes
            .iter()
            .map(|one| ObjectInterner::of(&parsed.module.context).object(one.object))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([parameter(0)])
    );
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
    let expected = Provenance {
        slices: UNKNOWN.slices.union(&Provenance::one(parameter(0)).slices),
        restrict: crate::memory::Sorted::new(),
    };
    assert_eq!(facts.values[&parsed.value("current")], expected);
    assert_eq!(facts.values[&parsed.value("advanced")], expected);
}

#[test]
fn test_parameter_modref_is_instantiated_at_a_call_site() {
    // A callee writing parameter zero clobbers its actual object and no
    // neighbour.
    let summary = Summary {
        writes: BTreeSet::from([Slice::new(parameter(0), 2, 4, 1, 1).unwrap()]).into(),
        ..Summary::default()
    };
    let actual = object(MemoryKind::Global, Identity::Global(7), Some(16));
    let effect = summary.instantiated(&[one(&actual, 4, 5)]).writes;
    assert_eq!(effect, BTreeSet::from([Slice::new(actual, 6, 8, 1, 1).unwrap()]).into());
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
    assert_eq!(summary.writes, BTreeSet::from([Slice::whole(parameter(0))]).into());
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
    assert!(
        effect.stores.iter().any(|one| one
            .provenance
            .as_ref()
            .unwrap()
            .slices
            .iter()
            .any(|one| one.object.kind == MemoryKind::Nonlocal))
    );
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
    let private =
        MemoryObject { captured: false, ..object(MemoryKind::Global, Identity::Str("counter".to_owned()), None) };
    let unaddressed = MemoryObject {
        addressed: false,
        captured: false,
        ..object(MemoryKind::Global, Identity::Str("total".to_owned()), None)
    };
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
    // Two fields off one pointer, each whole-object provenance, were called
    // overlapping.
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
    assert!(
        facts.nonnull(parsed.value("a")) && !facts.nonnull(parsed.value("p")) && !facts.nonnull(parsed.value("loaded"))
    );
    assert_eq!(nonnull_by_definition(&unit, parsed.value("a")), Some(true));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("p")), Some(false));
    assert_eq!(nonnull_by_definition(&unit, parsed.value("loaded")), None);
}

/// The objects `@f` lets escape when `%a`'s address meets `use_`.
fn escaped(use_: &str) -> BTreeSet<ObjectRef> {
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
    assert_eq!(escaped("%n = ptrtoint ptr %a to i16\n  %m = inttoptr i16 %n to ptr\n  ret ptr %m").len(), 1);
    assert_eq!(
        escaped("%n = ptrtoint ptr %a to i16\n  %m = add i16 %n, 2\n  %k = inttoptr i16 %m to ptr\n  ret ptr %k").len(),
        1
    );
    assert!(escaped("store ptr %a, ptr %slot\n  ret ptr null").is_empty());
    assert_eq!(escaped("store ptr %a, ptr %slot\n  ret ptr %slot").len(), 2);
}

/// PDS and VB FPDEEP: the raise left `ptrtoint` of `i`'s global and an `add` of
/// it, nothing reading either, and the global counted as escaped, so each PRINT
/// might have written `i` and its loop stayed rolled.
#[test]
fn an_integer_address_nothing_reads_escapes_nothing() {
    assert!(escaped("%n = ptrtoint ptr %a to i16\n  ret ptr null").is_empty());
    assert!(
        escaped("%n = ptrtoint ptr %a to i16\n  %m = add i16 %n, 2\n  %k = add i16 %m, 2\n  ret ptr null").is_empty()
    );
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
    // The guard `%i < 4` and the counted loop narrowed this to [4, 11) by 2
    // (`Bounded`, llrm-only); `computeConstantRange` reads neither a phi nor a
    // dominating branch, and neither does the pin's BasicAA, so the slice is
    // the whole object (AM-BD-4, pin-faithful).
    assert_eq!(tagged.slices, crate::memory::Sorted::from([Slice::new(global, 0, 64, 1, 2).unwrap()]));
}

/// A byte counter stepping by `step` for 200 trips, counting the trips
/// where `%i urem modulus != residue`.
fn wrapping(
    step: i64,
    check: Option<(i64, i64)>,
) -> String {
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
            assert_eq!(
                llrm_mir::interpret::run(&checked, "f", vec![], 100_000),
                Ok(llrm_mir::interpret::Val::Int { bits: 0, width: 16 }),
                "{step}"
            );
        }
    }
}

fn parsed_module(text: &str) -> Module {
    parsed(&format!("{DOS}{text}"))
}

/// A `noretain` call keeps nothing, but still reaches what its argument points
/// to while it runs. `ERASE` of a `$STATIC` array through a descriptor zeroes
/// the array; its write set lacked the array, so a later read of `cacheList(1)`
/// was forwarded the 9 stored before it (Q45S34).
#[test]
fn a_noretain_call_still_writes_what_its_argument_points_to() {
    let premise = Parsed::new(
        "declare void @use(ptr nocapture)

define void @f() {
b0:
  %slot = alloca [4 x i8]
  %inner = alloca [4 x i8]
  store ptr %inner, ptr %slot
  call void @use(ptr %slot)
  ret void
}
",
    );
    assert!(writes(&premise.effects(&IndexMap::default())[0], &bytes(&premise.object("inner"), 0, 4)));
    let kept = Parsed::new(
        "declare void @use(ptr nocapture noretain)

define void @f() {
b0:
  %slot = alloca [4 x i8]
  %inner = alloca [4 x i8]
  store ptr %inner, ptr %slot
  call void @use(ptr %slot)
  ret void
}
",
    );
    assert!(writes(&kept.effects(&IndexMap::default())[0], &bytes(&kept.object("inner"), 0, 4)));
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

const TYPE_TREE: &str = "
!0 = !{!\"root\"}
!1 = !{!\"scalar\", !0, i64 0}
!2 = !{!\"int\", !1, i64 0}
!3 = !{!\"long\", !2, i64 0}
!4 = !{!2, !2, i64 0}
!5 = !{!3, !3, i64 0}
";

/// With no type tree in the unit, each access built the module's whole tree
/// from all its metadata (`Tbaa::of`) to read its own tag's ancestors:
/// `Summaries` did that for every access of every procedure in every round, 41%
/// of deedlines' compile (#394).
#[test]
fn an_access_in_a_unit_with_no_type_tree_builds_none() {
    let stores: String = (0..20).map(|at| format!("  store i16 {at}, ptr %p, !tbaa !{}\n", 4 + at % 2)).collect();
    let parsed = Parsed::new(&format!("define void @f(ptr %p) {{\n{stores}  ret void\n}}\n{TYPE_TREE}"));
    let before = llrm_mir::tbaa::built();
    let references = annotated(&parsed.unit()).expect("annotates");
    assert_eq!(references.len(), 20);
    assert_eq!(llrm_mir::tbaa::built() - before, 0, "a type tree was built for an access");
}

/// Reading a tag's ancestors without the tree gives what the tree gives, for
/// every tag.
#[test]
fn a_tags_ancestors_are_the_same_with_and_without_the_type_tree() {
    let parsed = Parsed::new(&format!("define void @f() {{\n  ret void\n}}\n{TYPE_TREE}"));
    let metadata = &parsed.module.metadata;
    let tree = llrm_mir::tbaa::Tbaa::of(metadata);
    for at in 0..metadata.len() as u32 {
        let tag = llrm_mir::module::MetadataId(at);
        assert_eq!(llrm_mir::tbaa::Tbaa::chain(metadata, tag), tree.of_tag(metadata, tag), "tag !{at}");
    }
    assert_eq!(llrm_mir::tbaa::Tbaa::chain(metadata, llrm_mir::module::MetadataId(5)), ["int", "scalar", "root"]);
}

/// A body with no call was solved for what escapes twice by `summaries`: once
/// for its direct summary and once for its visit, which finds the same with no
/// call to give other captures (2 of 4 solves of d_faces' escape phase, which
/// is 2.7% of the compile). The visit reuses the direct summary's facts.
#[test]
fn a_body_with_no_call_is_solved_for_what_escapes_once_by_summaries() {
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures = IndexMap::from_iter([("callee".to_owned(), Procedure::of(parsed.unit_of("callee")))]);
    let before = crate::alias::escape_solves();
    summaries(&procedures, None).unwrap();
    assert_eq!(crate::alias::escape_solves() - before, 1, "a call-free body was solved for escapes twice");
}

/// What a body does on its own was made again for every body in every round of
/// the fixed point: 7000 times in deedlines, 47 s of 101 (#394). It is made
/// once per body, however many rounds.
#[test]
fn a_direct_summary_is_made_once_per_body_however_many_rounds() {
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures =
        IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
    let before = super::direct_runs();
    summaries(&procedures, None).unwrap();
    assert_eq!(super::direct_runs() - before, 2);
}

/// Every round of the fixed point visited every body: a chain of calls with no
/// cycle in it took as many rounds as it had links, and each visit ran
/// `points_to` (#394). A body in no cycle is visited once, callees first; a
/// cycle iterates, and only it.
#[test]
fn a_body_in_no_cycle_of_calls_is_visited_once() {
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures =
        IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
    let before = super::visits();
    summaries(&procedures, None).unwrap();
    assert_eq!(super::visits() - before, 2);

    let recursive =
        Parsed::new("define void @f(ptr %p) {\nb0:\n  store i8 0, ptr %p\n  call void @f(ptr %p)\n  ret void\n}\n");
    let procedures = IndexMap::from_iter([("f".to_owned(), Procedure::of(recursive.unit()))]);
    let before = super::visits();
    summaries(&procedures, None).unwrap();
    assert!(super::visits() - before >= 2, "a cycle is visited until nothing changes");
}

/// Every inline splice worked the summaries of the whole module out again
/// (host.c: 72 runs, 20.8 s of 35.8 s). After one body is edited only it and
/// what reads it, as far as a summary came out other than it was, are visited,
/// and the result is what a whole run makes.
#[test]
fn an_edit_visits_the_edited_body_and_its_callers_only() {
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures =
        IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
    let mut memo = super::SummaryMemo::default();
    let whole = super::summaries_updating(&procedures, None, &mut memo, None).unwrap();
    assert_eq!(whole, summaries(&procedures, None).unwrap());
    // `f` calls `callee`: editing `f` leaves `callee` as it was.
    let (visits, direct) = (super::visits(), super::direct_runs());
    let again =
        super::summaries_updating(&procedures, None, &mut memo, Some(&BTreeSet::from(["f".to_owned()]))).unwrap();
    assert_eq!((super::visits() - visits, super::direct_runs() - direct), (1, 1), "only f is made again");
    assert_eq!(again, whole);
    // `callee` is edited and comes out as it was: `f`, which reads it, is not
    // visited.
    let (visits, direct) = (super::visits(), super::direct_runs());
    let again =
        super::summaries_updating(&procedures, None, &mut memo, Some(&BTreeSet::from(["callee".to_owned()]))).unwrap();
    assert_eq!((super::visits() - visits, super::direct_runs() - direct), (1, 1), "callee alone");
    assert_eq!(again, whole);
}

/// A run in which every body was edited updated what it kept, which was
/// nothing: waking the readers of each body and comparing what changed cost 2.7
/// and 5.2 Minstr more than the run from nothing on the `joins` axis at N=64
/// (-O2), twice of six runs. It is the run from nothing, and the result is the
/// same.
#[test]
fn a_run_in_which_every_body_is_edited_is_worked_out_from_nothing() {
    let parsed = Parsed::new(CALLEE_WRITES_ITS_PARAMETER);
    let procedures =
        IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
    let mut memo = super::SummaryMemo::default();
    let whole = super::summaries_updating(&procedures, None, &mut memo, None).unwrap();
    let before = super::whole_runs();
    let one = super::summaries_updating(&procedures, None, &mut memo, Some(&BTreeSet::from(["f".to_owned()]))).unwrap();
    assert_eq!((super::whole_runs() - before, &one), (0, &whole), "one body edited is an update");
    let all = BTreeSet::from(["f".to_owned(), "callee".to_owned()]);
    let again = super::summaries_updating(&procedures, None, &mut memo, Some(&all)).unwrap();
    assert_eq!((super::whole_runs() - before, &again), (1, &whole), "every body edited is a run from nothing");
}

/// The facts a visit found are of the body as it was: an edited body is visited
/// from its own, and the summaries are what a whole run of the edited module
/// makes (host.c: one function differed before).
#[test]
fn an_edited_body_is_summarized_from_its_new_facts() {
    // The pointer passed is an instruction's, found by the body's points-to
    // facts, not a constant.
    let text = |at: u32| {
        CALLEE_WRITES_ITS_PARAMETER.replace(
            "  call void @callee(ptr getelementptr (i8, ptr @g, i16 4))",
            &format!("  %x = getelementptr i8, ptr @g, i16 {at}\n  call void @callee(ptr %x)"),
        )
    };
    let before = Parsed::new(&text(4));
    let after = Parsed::new(&text(8));
    fn made(parsed: &Parsed) -> IndexMap<String, Procedure<'_>> {
        IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))))
    }
    let mut memo = super::SummaryMemo::default();
    let first = super::summaries_updating(&made(&before), None, &mut memo, None).unwrap();
    let edited = made(&after);
    let again = super::summaries_updating(&edited, None, &mut memo, Some(&BTreeSet::from(["f".to_owned()]))).unwrap();
    assert_eq!(again, summaries(&edited, None).unwrap());
    assert_ne!(again, first, "premise: the edit changes the summary");
}

/// Each call to something unknown built what it may do besides what its
/// arguments reach (every tracked global, the callbacks) afresh:
/// `_unknown_visible` was 28% of compiling deedlines, and `analysis summaries`
/// 12 s of it (#558). Calls to one routine with the same escaped objects are
/// worked out once.
#[test]
fn test_calls_alike_to_something_unknown_are_worked_out_once() {
    let calls: String = (0..12).map(|_| "  call void @ext(ptr %p)\n".to_owned()).collect();
    let parsed =
        Parsed::new(&format!("declare void @ext(ptr)\n\ndefine void @f(ptr %p) {{\nb0:\n{calls}  ret void\n}}\n"));
    let procedures = IndexMap::from_iter([("f".to_owned(), Procedure::of(parsed.unit()))]);
    let before = super::other_runs();
    summaries(&procedures, None).unwrap();
    assert_eq!(super::other_runs() - before, 1);
}

/// Allocations the calling thread made, for the tests below.
#[cfg(test)]
mod counted {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static MADE: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(
            &self,
            layout: Layout,
        ) -> *mut u8 {
            MADE.with(|made| made.set(made.get() + 1));
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(
            &self,
            pointer: *mut u8,
            layout: Layout,
        ) {
            unsafe { System.dealloc(pointer, layout) }
        }
        unsafe fn realloc(
            &self,
            pointer: *mut u8,
            layout: Layout,
            size: usize,
        ) -> *mut u8 {
            MADE.with(|made| made.set(made.get() + 1));
            unsafe { System.realloc(pointer, layout, size) }
        }
    }

    pub fn made() -> usize {
        MADE.with(Cell::get)
    }
}

#[cfg(test)]
#[global_allocator]
static COUNTING: counted::Counting = counted::Counting;

/// A `MemRef` copied its type name and every ancestor's as strings, a vector,
/// and a set: 5 allocations for a reference of a three-deep type, 8% of all the
/// compile's allocations and ~3% of its instructions on QCport. The names are
/// shared.
#[test]
fn a_typed_reference_is_copied_without_allocating() {
    let parsed = Parsed::new(&format!(
        "define void @f(ptr %p) {{\n  store i16 1, ptr %p, !tbaa !5\n  ret void\n}}\n{TYPE_TREE}"
    ));
    let unit = parsed.unit();
    let store = parsed.all(|op| matches!(op, Opcode::Store { .. }))[0];
    let tree = llrm_mir::tbaa::Tbaa::of(&parsed.module.metadata);
    let unit = Unit { tbaa: Some(&tree), ..unit };
    let reference = MemRef::of(&unit, store).expect("a store");
    assert_eq!(reference.lineage.len(), 3);
    let before = counted::made();
    let copies: Vec<MemRef> = Vec::new();
    let copy = reference.clone();
    assert_eq!(counted::made() - before, 0, "a copy of a reference allocated");
    drop((copy, copies));
}

/// A held access was asked its `!tbaa` name and lineage again at every hit
/// (9 M instructions on d_faces). Unchanged metadata is not asked twice.
#[test]
fn a_held_access_is_not_asked_its_type_again() {
    let parsed = Parsed::new(&format!(
        "define void @f(ptr %p) {{\n  store i16 1, ptr %p, !tbaa !5\n  ret void\n}}\n{TYPE_TREE}"
    ));
    let store = parsed.all(|op| matches!(op, Opcode::Store { .. }))[0];
    let tree = llrm_mir::tbaa::Tbaa::of(&parsed.module.metadata);
    for tree in [None, Some(&tree)] {
        let unit = Unit { tbaa: tree, ..parsed.unit() };
        let first = MemRef::of(&unit, store).expect("a store");
        let before = crate::memory::TYPE_READS.with(std::cell::Cell::get);
        for _ in 0..20 {
            assert_eq!(MemRef::of(&unit, store).as_ref(), Some(&first));
        }
        let reads = crate::memory::TYPE_READS.with(std::cell::Cell::get) - before;
        assert_eq!(reads, 0, "a held access was asked its type {reads} times in 20 hits");
    }
}

/// A reference of a unit with no type tree was given the names of its type and
/// every ancestor as new strings, for each reference: 7% of the compile's
/// allocations (QCport). The names are read once for each type of a module.
#[test]
fn a_reference_of_a_unit_with_no_type_tree_reads_its_type_names_once() {
    let parsed = Parsed::new(&format!(
        "define void @f(ptr %p) {{\n  store i16 1, ptr %p, !tbaa !5\n  store i16 2, ptr %p, !tbaa !5\n  ret void\n}}\n{TYPE_TREE}"
    ));
    let unit = parsed.unit();
    assert!(unit.tbaa.is_none(), "premise: the unit has no tree");
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    let first = crate::memory::lineage(&unit, stores[0]);
    assert_eq!(first.len(), 3);
    crate::memory::typed(&unit, stores[0]).expect("a name");
    let before = counted::made();
    let second = crate::memory::lineage(&unit, stores[1]);
    let named = crate::memory::typed(&unit, stores[1]);
    assert_eq!(counted::made() - before, 0, "the names of a type already read were read again");
    assert_eq!(second, first);
    assert!(named.is_some());
}

/// Picking the buckets a write reaches made a vector for the objects, one for
/// the classes and one grown a few times for the answer: 7% of the compile's
/// allocations (QCport). One vector, sized once, is the answer.
#[test]
fn picking_the_buckets_of_a_write_allocates_once() {
    let global =
        MemoryObject { identity: Some(Identity::Global(1)), extent: Some(16), ..MemoryObject::new(MemoryKind::Global) };
    let reference = MemRef { provenance: Some(one(&global, 0, 4)), ..MemRef::reach(4, one(&global, 0, 4)) };
    let parts = crate::regions::OverlapParts::default();
    let before = counted::made();
    let reached = crate::regions::overlap_buckets(&reference, &parts);
    assert_eq!(counted::made() - before, 1, "the picking allocated more than its answer");
    drop(reached);
}

/// `may_alias` copied both references' provenance, two sets of slices, for each
/// pair it was asked of, though nothing narrowed them (`regions::refined`: 4.6%
/// of the compile's allocations and 15% of its bytes, QCport). It reads them
/// where they are.
#[test]
fn asking_whether_two_references_may_alias_copies_no_provenance() {
    let global =
        MemoryObject { identity: Some(Identity::Global(1)), extent: Some(16), ..MemoryObject::new(MemoryKind::Global) };
    let other =
        MemoryObject { identity: Some(Identity::Global(2)), extent: Some(16), ..MemoryObject::new(MemoryKind::Global) };
    let write = MemRef { provenance: Some(one(&global, 0, 4)), ..MemRef::reach(4, one(&global, 0, 4)) };
    let read = MemRef { provenance: Some(one(&other, 0, 4)), ..MemRef::reach(4, one(&other, 0, 4)) };
    let before = counted::made();
    let answer = crate::regions::may_alias(&write, &read, None, None, None).unwrap();
    assert_eq!(counted::made() - before, 0, "the provenance of a pair was copied");
    assert!(!answer);
}

/// A callee was looked up among every global by name at every call of every
/// visit (`_summary`: 12.7 G of host.c's 94 G instructions in `summaries
/// visit`, a quarter of it that scan). The procedure knows once which of its
/// callees another definition may replace.
#[test]
fn a_procedure_knows_which_callees_a_definition_elsewhere_may_replace() {
    let text = |linkage: &str| {
        CALLEE_WRITES_ITS_PARAMETER.replace("define void @callee", &format!("define {linkage}void @callee"))
    };
    for (linkage, replaceable) in [("", false), ("weak ", true), ("linkonce_odr ", true), ("internal ", false)] {
        let parsed = Parsed::new(&text(linkage));
        let procedure = Procedure::of(parsed.unit_of("f"));
        assert_eq!(procedure.replaceable.contains("callee"), replaceable, "linkage [{linkage}]");
        let procedures =
            IndexMap::from_iter(["f", "callee"].map(|name| (name.to_owned(), Procedure::of(parsed.unit_of(name)))));
        let found = summaries(&procedures, None).unwrap();
        // No summary describes a call that may be replaced: f then writes what
        // an unknown callee may.
        assert_eq!(found["f"].writes.is_empty(), false, "f writes something, replaceable callee or not");
    }
}

/// `initialized` made two points-to solves and the effects of every call for
/// each body of the module, 45% of `mir interprocedural` over QCport at -O1.
/// What it states is bytes a body writes into a pointer parameter before
/// reading them: a body with no pointer parameter, or with no store or call,
/// states none, and is not solved.

/// `initialized` solved where every pointer points three times (plain, with the
/// call arguments, with arguments and captures), the escape phase being the
/// only part that differs: 3,908 + 1,954 solves over QCport's front half at
/// -O1. Once a body.

/// `initialized` asked the escape phase twice more (with the call arguments,
/// and plain) for provenance alone: `.values` of each is the value solve's, and
/// 20 of d_faces' 252 escape phases at -O2 were these. It asks the one of
/// `calls_annotated`, whose effects it reads.

/// A loop the shape does not know of: the shape was taken before a copy loop
/// was spliced in, so its dominance says no block dominates a latch and no back
/// edge widens the pointers that walk it. Each round adds an offset to them
/// (`p`, `p+1`, `p+2`, ...) and the value solve never settles: qb-runtime's
/// i8out.c compiled for minutes in a GlobalsAA that read a stale shape. Past
/// gcc's `max-fields-for-field-sensitive` a fact is taken whole, and the solve
/// ends in a few hundred rounds.
#[test]
fn test_a_pointer_that_walks_a_loop_the_shape_does_not_know_settles() {
    let text = "define internal void @copy(ptr %a, ptr %b, i16 %n) {
entry:
  br label %head

head:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %p = phi ptr [ %a, %entry ], [ %p1, %body ]
  %q = phi ptr [ %b, %entry ], [ %q1, %body ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %body, label %exit

body:
  %v = load i8, ptr %q
  store i8 %v, ptr %p
  %p1 = getelementptr inbounds i8, ptr %p, i16 1
  %q1 = getelementptr inbounds i8, ptr %q, i16 1
  %next = add i16 %i, 1
  br label %head

exit:
  ret void
}

define void @f(ptr %x, ptr %y, i16 %m) {
entry:
  call void @copy(ptr %x, ptr %y, i16 %m)
  ret void
}
";
    let mut module = crate::testing::parsed(text);
    let layout = crate::testing::layout(&module);
    let (copy, f) = (module.named("copy").unwrap(), module.named("f").unwrap());
    let body = module.global(copy).function().unwrap().clone();
    let stale = crate::cfg::Shape::of(module.global(f).function().unwrap());
    let call = {
        let function = module.global(f).function().unwrap();
        function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| llrm_mir::memory::callee(&module.context, function, inst) == Some(copy))
            .unwrap()
    };
    let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[f.0 as usize].kind else {
        unreachable!()
    };
    llrm_mir::splice::splice(&mut module.context, function, call, &body);
    let function = module.global(f).function().unwrap();
    let known = crate::cfg::Shape::of(function);
    let exposed = std::collections::BTreeSet::new();
    let solve = |shape: &crate::cfg::Shape| {
        let unit = crate::memory::Unit::of(&module, &layout, function).with_shape(shape).with_exposed(&exposed);
        let (before, collapsed) = (crate::alias::value_rounds(), crate::alias::limit_collapses());
        crate::alias::point_values(&unit).unwrap();
        (crate::alias::value_rounds() - before, crate::alias::limit_collapses() - collapsed)
    };
    // The premise: with the shape of the spliced body the loop is known,
    // nothing grows without bound, and nothing needs the limit.
    let (settled, collapsed) = solve(&known);
    assert_eq!(collapsed, 0, "the solve with the right shape reached the limit");
    assert!(settled < 200, "{settled} visits with the right shape");
    // The stale shape: the pointers walk past the limit and are taken whole.
    let (rounds, collapsed) = solve(&stale);
    assert!(collapsed > 0, "the premise: the pointers walked past the limit");
    assert!(rounds < 10 * crate::alias::MAX_FIELDS_FOR_FIELD_SENSITIVE, "{rounds} visits");
}

/// An entry whose summary shrank made the callbacks add every entry up again:
/// 549 of GORILLA's 1,175 changes at -O1, 545 of the `summaries callbacks`
/// row's 1,039 Minstr. A slice has its owners counted, so a change costs the
/// slices it moved.
#[test]
fn an_entry_that_loses_a_slice_others_hold_moves_that_slice_and_not_every_entry() {
    let entries = 24;
    let held: Vec<Slice> =
        (0..40).map(|k| Slice::new(object(MemoryKind::Global, Identity::Int(k), None), 0, 4, 1, 4).unwrap()).collect();
    let summary = |reads: &[Slice]| Summary {
        reads: reads.iter().copied().collect::<BTreeSet<_>>().into(),
        ..Summary::default()
    };
    let parts: IndexMap<String, Summary> = (0..entries).map(|k| (format!("e{k}"), summary(&held))).collect();
    let mut back = super::CalledBack::from_parts(parts.clone());
    let before = super::callback_slices_moved();
    for k in 0..entries {
        // The entry loses a slice the others hold, then gets it back.
        back.changed(&format!("e{k}"), &summary(&held[1..]));
        assert_eq!(back.summary().unwrap().reads, summary(&held).reads, "the others still hold it");
        back.changed(&format!("e{k}"), &summary(&held));
    }
    let moved = super::callback_slices_moved() - before;
    assert!(moved <= 4 * entries, "{moved} slices moved for {} changes of one slice", 2 * entries);
    // and the sum is what adding the entries up gives, after entries drop what
    // only they held
    let own = Slice::new(object(MemoryKind::Global, Identity::Int(99), None), 0, 4, 1, 4).unwrap();
    back.changed("e0", &summary(&[held.as_slice(), &[own]].concat()));
    assert!(back.summary().unwrap().reads.contains(&own));
    back.changed("e0", &summary(&held));
    assert!(!back.summary().unwrap().reads.contains(&own), "the last owner dropped it");
}

/// The bodies of a run, as `Bodies` that count what a run asks of them, and
/// say they are the bodies of the run before (a layout) when told to.
struct Counting<'a> {
    inner: &'a IndexMap<String, Procedure<'a>>,
    asked: std::cell::Cell<usize>,
    layout: u64,
}

impl super::Bodies for Counting<'_> {
    fn len(&self) -> usize {
        self.inner.len()
    }

    fn name(
        &self,
        at: usize,
    ) -> &str {
        self.asked.set(self.asked.get() + 1);
        super::Bodies::name(self.inner, at)
    }

    fn position(
        &self,
        name: &str,
    ) -> Option<usize> {
        self.asked.set(self.asked.get() + 1);
        super::Bodies::position(self.inner, name)
    }

    fn facts(
        &self,
        at: usize,
    ) -> &std::rc::Rc<super::CallFacts> {
        self.asked.set(self.asked.get() + 1);
        super::Bodies::facts(self.inner, at)
    }

    fn procedure(
        &self,
        at: usize,
    ) -> Procedure<'_> {
        self.asked.set(self.asked.get() + 1);
        super::Bodies::procedure(self.inner, at)
    }

    fn globals(&self) -> Option<&crate::globalsaa::Globals> {
        super::Bodies::globals(self.inner)
    }

    fn layout(&self) -> u64 {
        self.layout
    }
}

/// Every run over a chain of N bodies asked each of them for its name and its
/// calls to find the ones that moved and to build what it kept (chain N=2048:
/// 74 runs of 2,048 bodies, `analysis summaries` 3.2x per doubling, gcc's 1.0).
/// Told they are the bodies it had, a run asks only for the edited one's.
#[test]
fn a_run_over_the_bodies_it_had_asks_for_the_edited_one_alone() {
    let asked_for = |n: usize| {
        let mut text = String::from("define i16 @h0(i16 %x) {\nb0:\n  ret i16 %x\n}\n");
        for k in 1..n {
            text +=
                &format!("define i16 @h{k}(i16 %x) {{\nb0:\n  %y = call i16 @h{}(i16 %x)\n  ret i16 %y\n}}\n", k - 1);
        }
        let parsed = Parsed::new(&text);
        let names: Vec<String> = (0..n).map(|k| format!("h{k}")).collect();
        let procedures =
            IndexMap::from_iter(names.iter().map(|name| (name.clone(), Procedure::of(parsed.unit_of(name)))));
        let mut memo = super::SummaryMemo::default();
        let bodies = Counting { inner: &procedures, asked: 0.into(), layout: 7 };
        super::summaries_updating(&bodies, None, &mut memo, None).unwrap();
        bodies.asked.set(0);
        let dirty = BTreeSet::from(["h0".to_owned()]);
        let again = super::summaries_updating(&bodies, None, &mut memo, Some(&dirty)).unwrap();
        assert_eq!(again, summaries(&procedures, None).unwrap());
        bodies.asked.get()
    };
    let (small, large) = (asked_for(16), asked_for(64));
    assert!(large <= small + 8, "a run over 4x the bodies asked {large} times where {small} sufficed");
}

/// Whether `@f`'s two stores, through `%p` and `%q` as `restricted` makes
/// them, may overlap.
fn restricted_overlap(restricted: &str) -> bool {
    let parsed = Parsed::new(&format!(
        "declare ptr @llrm.noalias.p0(ptr)

define void @f(ptr %p, ptr %q) {{
b0:
{restricted}  store i16 0, ptr %pp
  store i16 0, ptr %qq
  ret void
}}
"
    ));
    let tagged = annotated(&parsed.unit()).unwrap();
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    overlaps(&tagged[&stores[0]], &tagged[&stores[1]])
}

/// Two pointers each made restrict by the pass that tested their ranges are
/// apart; one alone is not, nor is one made so twice.
#[test]
fn pointers_made_restrict_are_apart_and_a_lone_one_is_not() {
    let both = "  %pp = call ptr @llrm.noalias.p0(ptr %p)\n  %qq = call ptr @llrm.noalias.p0(ptr %q)\n";
    let first = "  %pp = call ptr @llrm.noalias.p0(ptr %p)\n  %qq = getelementptr i8, ptr %q, i16 0\n";
    let none = "  %pp = getelementptr i8, ptr %p, i16 0\n  %qq = getelementptr i8, ptr %q, i16 0\n";
    assert!(!restricted_overlap(both));
    assert!(restricted_overlap(first));
    assert!(restricted_overlap(none));
}

/// The value solve visited every instruction of every block each round (a
/// chain of N diamonds spilling and reloading a pointer: d_faces, 178 solves of
/// 0.3 G for 1.2 G); it visits an instruction again only when a value or an
/// object's fields it reads changed, so a body of N pointer instructions costs
/// a small multiple of N visits.
#[test]
fn the_value_solve_visits_each_pointer_instruction_a_few_times() {
    let mut text = String::from(
        "define void @f(i1 %c, ptr %p, ptr %q) {\nb0:\n  %slot = alloca ptr\n  store ptr %p, ptr %slot\n  br label %d0\n",
    );
    let n = 60;
    for k in 0..n {
        text += &format!(
            "d{k}:\n  %l{k} = load ptr, ptr %slot\n  %g{k} = getelementptr i8, ptr %l{k}, i16 {k}\n  store ptr %g{k}, ptr %slot\n  br i1 %c, label %a{k}, label %d{next}\na{k}:\n  store ptr %q, ptr %slot\n  br label %d{next}\n",
            next = k + 1
        );
    }
    text += &format!("d{n}:\n  ret void\n}}\n");
    let parsed = Parsed::new(&text);
    let before = crate::alias::value_rounds();
    crate::alias::point_values(&parsed.unit()).unwrap();
    let visits = crate::alias::value_rounds() - before;
    let pointer_instructions = 5 * n + 2;
    assert!(visits <= 4 * pointer_instructions, "{visits} visits for {pointer_instructions} pointer instructions");
}

/// A pointer carried around a loop through a spill slot, with no phi to widen
/// it, grew a slice a trip until the limit: qcport/cmd -O1 took 7.3 G
/// instructions where 2.6 G was (a 170-instruction body solved in 4,691
/// visits). A fact a store after the load carries back is widened as a
/// loop-carried phi is.
#[test]
fn a_pointer_walking_a_loop_through_a_slot_settles_in_a_few_visits() {
    let parsed = Parsed::new(
        "define void @f(i1 %c, ptr %base) {
b0:
  %slot = alloca ptr
  store ptr %base, ptr %slot
  br label %loop

loop:
  %p = load ptr, ptr %slot
  %q = getelementptr i8, ptr %p, i16 1
  store ptr %q, ptr %slot
  br i1 %c, label %loop, label %out

out:
  ret void
}
",
    );
    let before = crate::alias::value_rounds();
    crate::alias::point_values(&parsed.unit()).unwrap();
    let visits = crate::alias::value_rounds() - before;
    assert!(visits < 40, "{visits} visits for a loop of three pointer instructions");
}

/// Every solve worked out what had escaped before each call, load and store
/// (a bit set cloned for each: 54 M of d_faces' 425 M escape cost, answers
/// nothing reads). A solve works out none; an answer is found when asked, once.
#[test]
fn what_escaped_before_an_instruction_is_found_when_asked_and_once() {
    let parsed = Parsed::new(
        "declare void @keep(ptr)

define void @f(ptr %p) {
b0:
  %a = alloca i16
  %b = alloca i16
  store i16 1, ptr %a
  %x = load i16, ptr %b
  call void @keep(ptr %a)
  store i16 2, ptr %a
  ret void
}
",
    );
    let before = crate::alias::before_answers();
    let facts = parsed.facts();
    assert_eq!(crate::alias::before_answers() - before, 0, "the solve answered what nobody asked");
    let call = parsed.all(|op| matches!(op, Opcode::Call(_)))[0];
    let first = facts.escaped_before.get(&call).expect("a call is asked of");
    let again = facts.escaped_before.get(&call).expect("a call is asked of");
    assert_eq!(first, again);
    assert_eq!(crate::alias::before_answers() - before, 1, "an answer was worked out twice");
    let a = parsed.object("a");
    assert!(first.contains(&a), "a kept argument has escaped by the call that keeps it: {first:?}");
}

/// `apart` found what escaped before the instruction (the block-entry dataflow
/// of the whole body) before it looked at the two references: 4,503 asks over
/// d_faces -O2, nearly all of an access that reaches more than what escaped.
/// It asks only for an access that reaches only what escaped, to a frame cell,
/// and then once.
#[test]
fn apart_builds_what_escaped_only_for_an_access_that_reaches_only_what_escaped() {
    let parsed = Parsed::new(
        "define void @f(ptr %p) {\nb0:\n  %a = alloca i16\n  store i16 1, ptr %a\n  store i16 2, ptr %p\n  ret void\n}\n",
    );
    let facts = parsed.facts();
    let unit = parsed.unit();
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    let held = |store| {
        let reference = MemRef::of(&unit, store).expect("a store");
        MemRef { provenance: facts.reference(&unit, &reference), ..reference }
    };
    let (own, through) = (held(stores[0]), MemRef::of(&unit, stores[1]).expect("a store"));
    assert!(own.provenance.is_some() && through.provenance.is_none(), "premise");
    let before = crate::alias::structures_built();
    assert!(!facts.escaped_before.apart(stores[1], &own, &own), "an access to a frame object reaches it");
    assert_eq!(crate::alias::structures_built() - before, 0, "built for an access that reaches more");
    assert!(facts.escaped_before.apart(stores[1], &through, &own), "an unplaced pointer cannot reach a frame cell");
    assert!(facts.escaped_before.apart(stores[1], &through, &own));
    assert_eq!(crate::alias::structures_built() - before, 1, "built once, when asked");
}

/// An access was held for each version of its function, and a type tag
/// attached afterwards logs no change: the held access kept no type, so a
/// store tagged `!tbaa` after its first ask was apart from nothing.
#[test]
fn a_type_attached_after_the_access_was_first_asked_is_seen() {
    let mut parsed =
        Parsed::new(&format!("define void @f(ptr %p) {{\n  store i16 1, ptr %p\n  ret void\n}}\n{TYPE_TREE}"));
    let store = parsed.all(|op| matches!(op, Opcode::Store { .. }))[0];
    assert!(MemRef::of(&parsed.unit(), store).expect("a store").typed.is_none(), "premise: no type yet");
    let (_, function) = parsed.module.function_mut("f").expect("f");
    function.annotate(store, "tbaa", llrm_mir::module::MetadataId(5));
    let reference = MemRef::of(&parsed.unit(), store).expect("a store");
    assert!(reference.typed.is_some(), "the type attached later was not seen");
    assert_eq!(reference.lineage.len(), 3);
}

/// The live sets of each instruction of a block were B-trees, a node for every
/// eleven values live: a hundred values live at the top of a block cost twenty
/// allocations, twice, for every instruction `live_points` listed (1.6% of the
/// compile of d_faces in the tree's operations alone). They are bit sets: one
/// allocation each.
#[test]
fn the_live_points_of_a_block_allocate_a_set_each_not_a_node_for_eleven_values() {
    let values = 100;
    let mut body = String::from("define i16 @f(i16 %a) {\nb0:\n");
    for k in 0..values {
        body += &format!("  %v{k} = add i16 %a, {}\n", k + 1);
    }
    body += "  %w0 = add i16 %v0, 1\n";
    for k in 1..values {
        body += &format!("  %w{k} = xor i16 %w{}, %v{k}\n", k - 1);
    }
    body += &format!("  ret i16 %w{}\n}}\n", values - 1);
    let module = crate::testing::parsed(&body);
    let function = crate::testing::function(&module, "f");
    let found = crate::liveness::live(function);
    let block = function.layout()[0];
    let before = counted::made();
    let points = crate::liveness::live_points(function, &found, block);
    let made = counted::made() - before;
    assert!(points.len() > 2 * values - 2);
    assert!(made <= 4 * points.len(), "{} points of up to {values} live values made {made} allocations", points.len());
}

/// Two arguments of a call, one a constant global and one a pointer nothing
/// places, replaced by one value the analysis knows nothing of: the effect kept
/// past the edit lists the global beside the unknown, a fresh run lists the
/// unknown alone. `covers` took that for a contradiction and the replay check
/// (`LLRM_CHECK_REPLAY`) stopped QCport's `ls` at -O2 once two arguments were
/// merged. The kept effect has every reference of the fresh one.
#[test]
fn an_effect_with_a_global_beside_the_unknown_covers_the_unknown_alone() {
    let global = MemoryObject {
        identity: Some(Identity::Global(12)),
        extent: Some(2),
        constant: true,
        ..MemoryObject::new(MemoryKind::Global)
    };
    let other =
        MemoryObject { identity: Some(Identity::Global(13)), extent: Some(2), ..MemoryObject::new(MemoryKind::Global) };
    let reference =
        |provenance: Provenance| MemRef { provenance: Some(provenance.clone()), ..MemRef::reach(1, provenance) };
    let effect = |loads: Vec<MemRef>| Effect { loads: loads.into(), ..Effect::default() };
    let placed = reference(one(&global, 0, 2));
    let unknown = reference(UNKNOWN.clone());
    assert!(
        effect(vec![unknown.clone(), placed.clone()]).covers(&effect(vec![unknown])),
        "the global is within the unknown"
    );
    assert!(
        !effect(vec![placed.clone()]).covers(&effect(vec![reference(one(&other, 0, 2))])),
        "another global is a new read"
    );
    let unknown = reference(UNKNOWN.clone());
    assert!(!effect(vec![placed]).covers(&effect(vec![unknown])), "a kept global does not cover a fresh unknown");
}

/// A fresh run of a call states a slice of an escaped object (a parameter the
/// body stored away) where the kept effect has the unknown object's whole range
/// alone: the unknown reaches what escaped, so nothing is new (greeting.nib and
/// pipeline.nib at -O1: 'the result kept past 56 sound edits contradicts what
/// deriving it afresh gives'). The other way is still flagged: a kept effect
/// that lists the parameter alone does not cover a fresh unknown, and the
/// unknown does not cover an object that never escaped.
#[test]
fn an_effect_of_the_unknown_covers_a_fresh_slice_of_an_escaped_object_and_not_the_reverse() {
    let escaped = MemoryObject {
        identity: Some(Identity::Int(3)),
        captured: true,
        addressed: true,
        ..MemoryObject::new(MemoryKind::Parameter)
    };
    let private = MemoryObject {
        identity: Some(Identity::Value(9)),
        extent: Some(4),
        addressed: false,
        captured: false,
        ..MemoryObject::new(MemoryKind::Frame)
    };
    let reference =
        |provenance: Provenance| MemRef { provenance: Some(provenance.clone()), ..MemRef::reach(1, provenance) };
    let effect = |loads: Vec<MemRef>| Effect { loads: loads.into(), ..Effect::default() };
    let unknown = reference(UNKNOWN.clone());
    let parameter = reference(one(&escaped, 0, 1));
    assert!(
        effect(vec![unknown.clone()]).covers(&effect(vec![unknown.clone(), parameter.clone()])),
        "the unknown reaches what escaped"
    );
    assert!(
        !effect(vec![parameter.clone()]).covers(&effect(vec![parameter.clone(), unknown.clone()])),
        "a kept parameter does not cover a fresh unknown"
    );
    assert!(
        !effect(vec![unknown]).covers(&effect(vec![reference(one(&private, 0, 2))])),
        "the unknown does not reach an object that never escaped"
    );
}

/// A fresh run of a call states a slice of a global the program took the
/// address of where the kept effect has what a callee may reach (`Nonlocal`),
/// whole: that reaches every escaped object, a frame one aside, as
/// `objects_may_alias` says. `covers` took the global for a new read, and
/// `LLRM_CHECK_REPLAY` stopped arrprm.bas at -O1 and -O2 (m16) once -O1
/// inlined: 'call-effects: the result kept past 4 sound edits contradicts what
/// deriving it afresh gives' (no object changed: the kept effect was the less
/// precise, both sound).
#[test]
fn an_effect_of_what_a_callee_reaches_covers_a_fresh_slice_of_an_escaped_global() {
    let escaped = MemoryObject {
        identity: Some(Identity::Global(4)),
        extent: Some(8),
        captured: true,
        addressed: true,
        ..MemoryObject::new(MemoryKind::Global)
    };
    let private = MemoryObject {
        identity: Some(Identity::Global(5)),
        extent: Some(8),
        captured: false,
        addressed: true,
        ..MemoryObject::new(MemoryKind::Global)
    };
    let frame = MemoryObject {
        identity: Some(Identity::Value(9)),
        extent: Some(4),
        captured: true,
        addressed: true,
        ..MemoryObject::new(MemoryKind::Frame)
    };
    let reference =
        |provenance: Provenance| MemRef { provenance: Some(provenance.clone()), ..MemRef::reach(1, provenance) };
    let effect = |loads: Vec<MemRef>| Effect { loads: loads.into(), ..Effect::default() };
    let nonlocal = reference(Provenance::one(ObjectRef::NONLOCAL));
    assert!(
        effect(vec![nonlocal.clone()]).covers(&effect(vec![nonlocal.clone(), reference(one(&escaped, 0, 8))])),
        "a callee's reach includes an escaped global"
    );
    assert!(
        !effect(vec![nonlocal.clone()]).covers(&effect(vec![reference(one(&private, 0, 8))])),
        "not a global whose address never escaped"
    );
    assert!(
        !effect(vec![nonlocal.clone()]).covers(&effect(vec![reference(one(&frame, 0, 4))])),
        "not a frame object, escaped or not"
    );
    // `objects_may_alias` asks both objects addressed before it asks captured.
    let unaddressed = MemoryObject {
        identity: Some(Identity::Global(6)),
        extent: Some(8),
        captured: true,
        addressed: false,
        ..MemoryObject::new(MemoryKind::Global)
    };
    assert!(
        !effect(vec![nonlocal]).covers(&effect(vec![reference(one(&unaddressed, 0, 8))])),
        "not a global nothing computes the address of, captured or not"
    );
    let unknown = reference(UNKNOWN.clone());
    assert!(
        !effect(vec![unknown]).covers(&effect(vec![reference(one(&unaddressed, 0, 8))])),
        "the unknown object does not reach it either"
    );
}

/// A summary stands for the bytes its slices name. A fresh read of two bytes
/// at offset 2 of a 7-byte global lies in a kept read of its bytes [0, 7), and
/// a two-byte read starting anywhere in [0, 7) names the object's last byte
/// only as far as its end: `covers` took each for a new read
/// (`LLRM_CHECK_STALE` stopped `rotate`, `lsr` and `lcssa` on entries.nib,
/// roster.nib and flat_containers.nib).
#[test]
fn a_summary_covers_a_read_by_the_bytes_it_names_not_by_its_shape() {
    let global =
        MemoryObject { identity: Some(Identity::Global(7)), extent: Some(7), ..MemoryObject::new(MemoryKind::Global) };
    let reads = |slices: &[(i64, i64, i64)]| Summary {
        reads: slices
            .iter()
            .map(|&(low, high, width)| Slice::new(global.clone(), low, high, 1, width).unwrap())
            .collect::<BTreeSet<_>>()
            .into(),
        ..Summary::default()
    };
    let kept = reads(&[(0, 7, 1)]);
    assert!(kept.covers(&reads(&[(0, 7, 1), (2, 3, 2), (4, 5, 2)])), "bytes 2..4 and 4..6 are in [0, 7)");
    assert!(kept.covers(&reads(&[(0, 7, 2)])), "the byte past the end is no byte of the object");
    assert!(!reads(&[(0, 3, 1)]).covers(&reads(&[(2, 5, 1)])), "bytes the kept read does not name");
    // The clip is the object's end, not the kept slice's: in a larger object
    // the byte after the kept ones is a new read.
    let large = MemoryObject { extent: Some(20), ..global.clone() };
    let in_large = |low, high, width| Summary {
        reads: BTreeSet::from([Slice::new(large.clone(), low, high, 1, width).unwrap()]).into(),
        ..Summary::default()
    };
    assert!(!in_large(0, 7, 1).covers(&in_large(5, 8, 1)), "byte 7 of a 20-byte object is a new read");
    assert!(!in_large(0, 7, 1).covers(&in_large(0, 7, 2)), "so is the byte after a two-byte read from 6");
}

/// A local published only after a call is still reached by its unknown writes:
/// the footprint names the unknown object, which may be any object whose
/// address leaves the function anywhere (`ObjectRef::captured`), not before
/// the call as the pin's `isNotCapturedBefore` has it. The old effect lists
/// (`a_pointer_published_after_a_call_is_not_reached_by_it`) were flow
/// sensitive; the precision is row AM-CA-8. A local that is never published
/// stays out.
#[test]
fn a_calls_footprint_reaches_a_local_published_later_but_not_one_never_published() {
    let text = PUBLISHED.replace(
        "  store ptr %escaped, ptr @g\n  call void @external()\n",
        "  call void @external()\n  store ptr %escaped, ptr @g\n",
    );
    let parsed = Parsed::new(&text);
    let unit = parsed.unit();
    let found = crate::alias::calls_by_attributes(&unit);
    let effect = found.iter().next().map(|(_, effect)| effect).expect("the call");
    assert!(writes(effect, &bytes(&parsed.object("escaped"), 0, 4)), "published later: reached (AM-CA-8)");
    assert!(!writes(effect, &bytes(&parsed.object("private"), 0, 4)), "never published: not reached");
}

/// What a callee reaches elsewhere depends on the module analyses alone: made
/// under one proxy it is worked out once for the callee, not again for each
/// body state (76 runs of the footprints on d_faces -O2, 6.5 M and 3.8 M Ir of
/// `reaches`).
#[test]
fn what_a_callee_reaches_elsewhere_is_worked_out_once_under_one_proxy() {
    let module = crate::testing::parsed(
        "@g = internal global i16 0\n\ndeclare void @external()\n\ndefine void @f() {\nb0:\n  call void @external()\n  ret void\n}\n",
    );
    let layout = crate::testing::layout(&module);
    let f = crate::testing::function(&module, "f");
    let outer = llrm_mir::passes::Outer::of(&module, None);
    let footprints = || {
        let before = crate::alias::reaches_made();
        let unit = crate::memory::Unit::within(&module.context, &layout, f, &outer);
        crate::alias::calls_by_attributes(&unit);
        crate::alias::reaches_made() - before
    };
    assert_eq!(footprints(), 1, "the first body state works it out");
    assert_eq!(footprints(), 0, "the next asks the proxy");
}

/// A byval parameter is the callee's own copy: the call writes nothing of the
/// object it passes, and `initializes` on it fills nothing of it. Without
/// `byval` the same call writes and fills the object. Ported from
/// `a_callee_that_writes_a_byval_parameter_writes_nothing_of_the_callers_object`
/// and `a_byval_argument_that_initializes_fills_nothing_of_the_callers_object`
/// (c/byval_inline -O2 dropped the store `g.a[0] = 1`).
#[test]
fn a_byval_argument_is_neither_written_nor_filled_by_the_call() {
    let of = |attrs: &str| {
        let text = format!(
            "declare void @h(ptr {attrs}initializes((0, 4))) memory(argmem: write) nounwind\ndefine void @f() {{\nb0:\n  %a = alloca i32\n  call void @h(ptr %a)\n  ret void\n}}\n"
        );
        let parsed = Parsed::new(&text);
        let unit = parsed.unit();
        let found = crate::alias::calls_by_attributes(&unit);
        let call = unit
            .function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Call(_)))
            .unwrap();
        let accesses = crate::memoryssa::Accesses::of(&unit, IndexMap::default(), &found);
        (found[&call].stores.len(), accesses.fills(call).len())
    };
    assert_eq!(of(""), (1, 1), "premise: a plain parameter is written and filled");
    assert_eq!(of("byval(i32) "), (0, 0));
}

/// A call's footprint from its attributes alone (`calls_by_attributes`): a
/// `memset` of a constant length writes those bytes through its destination
/// and reads nothing; a callee with `memory(argmem: read)` reads through its
/// argument and writes nothing; one with no attribute may write anywhere.
#[test]
fn a_calls_footprint_follows_its_attributes() {
    let parsed = Parsed::new(
        "declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare i16 @peek(ptr nocapture) memory(argmem: read) nounwind willreturn
declare void @anything(ptr)

define void @f() {
b0:
  %s = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)
  %v = call i16 @peek(ptr %s)
  call void @anything(ptr %s)
  ret void
}
",
    );
    let unit = parsed.unit();
    let found = crate::alias::calls_by_attributes(&unit);
    let calls: Vec<InstId> = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Call(_)))
        .collect();
    let [memset, peek, anything] = calls[..] else { panic!("{calls:?}") };
    let shape = |at: InstId| (found[&at].loads.len(), found[&at].stores.len());
    assert_eq!(shape(memset), (0, 1));
    assert_eq!(found[&memset].stores[0].width, 8, "the bytes the length names");
    assert_eq!(shape(peek), (1, 0));
    assert!(found[&anything].stores.len() > 1 && found[&anything].loads.len() > 1, "unknown memory too");
}

/// A load asked its global's size through the whole object: capture state and
/// an interner lookup (750 Ir each) for a number the type gives. fjoins N=64
/// asked 63,024 times, 47 M of 991 M Ir.
#[test]
fn test_the_extent_of_a_global_is_read_without_interning_its_object() {
    let parsed = Parsed::new(
        "@g = global i32 7

define void @f() {
b0:
  store i8 0, ptr @g
  ret void
}
",
    );
    let unit = parsed.unit();
    let root = unit
        .function
        .walk()
        .flat_map(|(_, inst)| unit.function.instruction(inst).operands.iter().copied().collect::<Vec<_>>())
        .find(|operand| matches!(
            operand,
            Operand::Constant(id) if matches!(unit.context.get(*id).kind, llrm_mir::context::ConstantKind::Global(_))
        ))
        .expect("the global's address");
    let interner = ObjectInterner::of(unit.context);
    let before = interner.lookups();
    assert_eq!(memory::extent_of(&unit, root), Some(4));
    assert_eq!(interner.lookups(), before, "extent_of interned the global's object");
}
