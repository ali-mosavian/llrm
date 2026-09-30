//! llrm-core's `induction_tests.rs` and `counted_loops_tests.rs`, and what
//! they leave untested, as loops the interpreter runs.
//!
//! Skipped: the BC and OMF fixture tests (NDARR, MATRIX, HARR, NESTED),
//! the `lower`, `ssa::constructed`, CSE and `dead` tests filed here; the
//! `or i, i` zero tests (a flags idiom); width-preserving copies (no copy);
//! loopexit's and rotate's shapes, which wait for those ports.

use std::collections::BTreeSet;

use crate::graph::loops::{self, Loop};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::{Val, run};
use llrm_mir::module::{Function, InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, IntPredicate};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use super::*;
use crate::cfg;
use crate::consts;
use crate::consts::Known;
use crate::memory::Unit;
use crate::testing::{DOS, corpus, function, layout, parsed, value};

pub(crate) struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    pub fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{body}");
        let layout = layout(&module);
        Self { module, layout }
    }

    pub fn function(&self) -> &Function {
        function(&self.module, "f")
    }

    pub fn unit(&self) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, self.function())
    }

    pub fn value(&self, name: &str) -> ValueId {
        value(self.function(), name)
    }

    /// The instruction defining `%name`.
    pub fn made(&self, name: &str) -> InstId {
        let wanted = self.value(name);
        self.function().walk().map(|(_, inst)| inst).find(|&inst| self.function().instruction(inst).result == Some(wanted)).expect("defined")
    }

    pub fn only_loop(&self) -> Loop {
        let found = loops::loops(&cfg::graph(self.function()), None);
        let [one] = &found[..] else { panic!("one loop") };
        one.clone()
    }

    pub fn counted(&self, inbounds: bool) -> Vec<CountedLoop> {
        counted(&self.unit(), &self.only_loop(), None, inbounds)
    }

    pub fn derived(&self) -> Vec<Derived> {
        derived(&self.unit(), &self.only_loop(), None)
    }

    pub fn basics(&self) -> IndexMap<ValueId, Affine> {
        basics(&self.unit(), &self.only_loop())
    }

    /// `@f(arguments)`'s integer result.
    pub fn run(&self, arguments: &[(i64, u32)], fuel: u64) -> Option<u128> {
        let arguments = arguments.iter().map(|&(n, width)| Val::Int { bits: n as u128 & llrm_mir::context::mask(width), width }).collect();
        match run(&self.module, "f", arguments, fuel) {
            Ok(Val::Int { bits, .. }) => Some(bits),
            Ok(other) => panic!("{other:?}"),
            Err(_) => None,
        }
    }
}

fn spelled(test: IntPredicate) -> &'static str {
    llrm_mir::opcode::spelling(&llrm_mir::opcode::INT_PREDICATE, test)
}

fn signed(n: i64, width: u32) -> i64 {
    llrm_mir::context::signed(n as u128 & llrm_mir::context::mask(width), width) as i64
}

fn constant(n: i64, width: u32) -> AffineOperand {
    AffineOperand::constant(n, width)
}

#[derive(Clone, Copy)]
pub(crate) struct Shape {
    pub posttested: bool,
    pub stepped: bool,
    pub mirrored: bool,
    pub split: bool,
    pub width: u32,
    /// The step's flags: `nsw`, `nuw` or none.
    pub flags: &'static str,
}

pub(crate) fn shaped(shape: &str, width: u32) -> Shape {
    Shape { posttested: shape != "pre", stepped: shape == "post-stepped", mirrored: false, split: false, width, flags: "" }
}

/// `i = start` stepping by `step` while `i test bound`: a start of None is
/// `%g`, a bound of None `%n`. It returns `trips << 32 | seen << 16 | left`:
/// the trips, the last trip's `i` (`start` for none), and the `i` the
/// loop leaves with (the stepped one for a post-tested loop).
pub(crate) fn looped(start: Option<i64>, bound: Option<i64>, test: IntPredicate, step: i64, shape: Shape) -> Parsed {
    let w = shape.width;
    let start = start.map_or("%g".to_owned(), |n| signed(n, w).to_string());
    let bound = bound.map_or("%n".to_owned(), |n| signed(n, w).to_string());
    let flags = shape.flags;
    let step = signed(step, w);
    let compare = |tested: &str, test: IntPredicate| {
        if shape.mirrored {
            format!("icmp {} i{w} {bound}, {tested}", spelled(test.swapped()))
        } else {
            format!("icmp {} i{w} {tested}, {bound}", spelled(test))
        }
    };
    let finish = |exit: &str, from: &str, trips: &str, seen: &str, left: &str| {
        format!(
            "{exit}:
  %t = phi i32 [ {trips}, %{from} ]
  %s = phi i{w} [ {seen}, %{from} ]
  %l = phi i{w} [ {left}, %{from} ]
  %t64 = zext i32 %t to i64
  %s64 = zext i{w} %s to i64
  %l64 = zext i{w} %l to i64
  %hi = shl i64 %t64, 32
  %mid = shl i64 %s64, 16
  %r0 = or i64 %hi, %mid
  %r = or i64 %r0, %l64
  ret i64 %r
}}
"
        )
    };
    let head = format!("define i64 @f(i{w} %g, i{w} %n) {{\nb0:\n  br label %b1\n\n");
    let text = if !shape.posttested {
        // Leaves on the test's negation, as a guarded loop does.
        format!(
            "{head}b1:
  %i = phi i{w} [ {start}, %b0 ], [ %next, %b2 ]
  %trips = phi i32 [ 0, %b0 ], [ %up, %b2 ]
  %seen = phi i{w} [ {start}, %b0 ], [ %i, %b2 ]
  %c = {}
  br i1 %c, label %b3, label %b2

b2:
  %up = add i32 %trips, 1
  %next = add {flags} i{w} %i, {step}
  br label %b1

{}",
            compare("%i", test.inverse()),
            finish("b3", "b1", "%trips", "%seen", "%i")
        )
    } else {
        let tested = if shape.stepped { "%next" } else { "%i" };
        let (latch, jump) = if shape.split { ("b4", "  br label %b4\n\nb4:\n") } else { ("b1", "") };
        format!(
            "{head}b1:
  %i = phi i{w} [ {start}, %b0 ], [ %next, %{latch} ]
  %trips = phi i32 [ 0, %b0 ], [ %up, %{latch} ]
  %up = add i32 %trips, 1
  %next = add {flags} i{w} %i, {step}
{jump}  %c = {}
  br i1 %c, label %b1, label %b2

{}",
            compare(tested, test),
            finish("b2", latch, "%up", "%i", "%next")
        )
    };
    Parsed::new(&text)
}

/// `(trips, seen, left)` of `looped`'s result, `seen` and `left` signed.
pub(crate) fn decoded(result: u128, width: u32) -> (BigInt, i64, i64) {
    let lane = |shift: u32| signed(((result >> shift) & 0xFFFF) as i64, width);
    (BigInt::from(result >> 32), lane(16), lane(0))
}

const TESTS: [IntPredicate; 9] = [
    IntPredicate::Slt,
    IntPredicate::Sle,
    IntPredicate::Ult,
    IntPredicate::Ule,
    IntPredicate::Sgt,
    IntPredicate::Sge,
    IntPredicate::Ugt,
    IntPredicate::Uge,
    IntPredicate::Ne,
];
const ENDS: [i64; 6] = [0, 1, 0x7F, 0x80, 0xFE, 0xFF];

/// The proof, over every byte loop of these ends and tests, against running it.
///
/// A loop the interpreter sees end has its exact count, or no proof; one
/// that never ends has no proof. The last trip's counter is `last`
/// wherever given, and a pre-tested loop leaves with its `exit_value`.
#[test]
fn test_every_counted_loop_runs_its_proved_trips() {
    for (shape, split) in [("pre", false), ("post", false), ("post-stepped", false), ("post", true), ("post-stepped", true)] {
        for step in [1, -1, 3, -3] {
            let mut proved = BTreeSet::new();
            for test in TESTS {
                for start in ENDS {
                    for bound in ENDS {
                        for mirrored in [false, true] {
                            let parsed = looped(Some(start), Some(bound), test, step, Shape { mirrored, split, ..shaped(shape, 8) });
                            let proofs = parsed.counted(false);
                            let where_ = (shape, split, step, test, start, bound, mirrored);
                            let Some(result) = parsed.run(&[(0, 8), (0, 8)], 4_000) else {
                                assert!(proofs.is_empty(), "{where_:?}");
                                continue;
                            };
                            let [proof] = &proofs[..] else {
                                assert!(proofs.is_empty(), "{where_:?}");
                                continue;
                            };
                            let (trips, seen, left) = decoded(result, 8);
                            assert!(proof.count.as_ref() == Some(&trips) && proof.test == test, "{where_:?} {trips}");
                            if proof.last.is_some() {
                                assert_eq!((proof.first.clone(), proof.last.clone()), (Some(signed(start, 8).into()), Some(seen.into())), "{where_:?}");
                            }
                            if !proof.posttested && trips > BigInt::from(0) {
                                let exit = exit_value(proof, &mut |_, _| panic!("constant ends place nothing"));
                                assert_eq!(exit, Some(constant(left, 8)), "{where_:?}");
                            }
                            proved.insert(spelled(test));
                        }
                    }
                }
            }
            // Every test whose direction the step can end is proved somewhere.
            let ending = TESTS.into_iter().filter(|&test| test == IntPredicate::Ne || _ascending(test) == (step > 0)).map(spelled).collect::<BTreeSet<_>>();
            assert_eq!(proved, ending, "{shape} {split} {step}");
        }
    }
}

/// `i = 0; while i <= 32767` was proved to run 32768 trips: `i + 1` wraps and it never ends.
#[test]
fn test_an_inclusive_test_at_its_types_maximum_is_not_counted() {
    let cases: [(i64, i64, IntPredicate, Option<i64>); 5] = [
        (0, 0x7FFF, IntPredicate::Sle, None), // signed <= its maximum never fails
        (0, 0x7FFE, IntPredicate::Sle, Some(0x7FFF)),
        (0, 0xFFFF, IntPredicate::Ule, None), // unsigned <= its maximum never fails
        (1, 0xFFFE, IntPredicate::Ule, Some(0xFFFE)),
        (-3, 2, IntPredicate::Slt, Some(5)),
    ];
    for (start, bound, test, trips) in cases {
        let parsed = looped(Some(start), Some(bound), test, 1, shaped("pre", 16));
        let maxima = parsed.counted(false).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        let expected = trips.map(|trips| vec![Some(BigInt::from(trips))]).unwrap_or_default();
        assert_eq!(maxima, expected, "{start} {bound} {test:?}");
    }
}

#[test]
fn test_affine_map_carries_the_modular_injectivity_proof() {
    let source = Affine { value: ValueId(1), start: constant(0, 16), step: constant(1, 16), header: 1 };
    let byte_offset = Affine { value: ValueId(2), start: constant(0, 16), step: constant(16, 16), header: 1 };

    let mapping = relation(&source, &byte_offset, &IndexMap::default()).unwrap();

    assert_eq!(mapping, AffineMap { scale: BigInt::from(16), offset: BigInt::from(0), width: 16 });
    assert!(mapping.injective(&BigInt::from(0), &BigInt::from(5)));
    assert!(!mapping.injective(&BigInt::from(0), &BigInt::from(4096)));
}

#[test]
fn test_posttested_counter_has_an_exact_fixed_trip_count() {
    let parsed = looped(Some(0), Some(4), IntPredicate::Ult, 1, shaped("post-stepped", 16));
    let facts = consts::known(&parsed.unit(), None, None, None);
    assert_eq!(trip_count(&parsed.unit(), &parsed.only_loop(), &facts), Some(BigInt::from(4)));
    assert_eq!(decoded(parsed.run(&[(0, 16), (0, 16)], 1_000).unwrap(), 16).0, BigInt::from(4));
}

#[test]
fn test_posttested_symbolic_sentinel_keeps_its_exact_trip_count() {
    let parsed = Parsed::new(
        "define i32 @f(i32 %start) {
b0:
  %end = add i32 %start, 768
  br label %b1

b1:
  %i = phi i32 [ %start, %b0 ], [ %next, %b2 ]
  %trips = phi i32 [ 0, %b0 ], [ %up, %b2 ]
  br label %b2

b2:
  %up = add i32 %trips, 1
  %next = add i32 %i, 24
  %c = icmp ne i32 %next, %end
  br i1 %c, label %b1, label %b3

b3:
  %t = phi i32 [ %up, %b2 ]
  ret i32 %t
}
",
    );
    let facts = consts::known(&parsed.unit(), None, None, None);
    assert_eq!(trip_count(&parsed.unit(), &parsed.only_loop(), &facts), Some(BigInt::from(32)));
    for start in [0, -5, 0x7FFF_FFF0] {
        assert_eq!(parsed.run(&[(start, 32)], 10_000), Some(32), "{start}");
    }
}

/// A counter, `%i`, stepped once a trip, and a multiply of `%x`.
fn stepped(step: &str, multiply: &str) -> Parsed {
    Parsed::new(&format!(
        "define void @f(i16 %x, i1 %c) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %next, %b1 ]
  %m = {multiply}
  %next = {step}
  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
    ))
}

#[test]
fn test_a_multiply_of_another_value_is_not_derived() {
    let other = stepped("add i16 %i, 1", "mul i16 %x, 2");
    assert!(!other.basics().is_empty());
    assert!(other.derived().iter().all(|one| one.op != other.made("m")));
    let counter = stepped("add i16 %i, 1", "mul i16 %i, 2");
    let found = counter.derived();
    assert_eq!(found.iter().find(|one| one.op == counter.made("m")).map(|one| one.by.clone()), Some(constant(2, 16)));
}

#[test]
fn test_the_backedge_must_step_the_exact_phi_value() {
    assert!(stepped("add i16 %x, 1", "mul i16 %i, 2").basics().is_empty());
}

#[test]
fn test_a_step_is_an_invariant_or_constant_added_to_the_phi() {
    for (step, expected) in [
        ("add i16 %i, 1", Some(constant(1, 16))),
        ("add i16 1, %i", Some(constant(1, 16))),
        ("sub i16 %i, 3", Some(constant(-3, 16))),
        ("add i16 %i, %x", Some(AffineOperand::Value(ValueId(0), 16))),
        ("add i16 %i, %m", None),     // changes in the loop
        ("sub i16 %i, %x", None),     // a variable subtracted
        ("sub i16 3, %i", None),      // the phi negated
        ("mul i16 %i, 1", None),
    ] {
        let parsed = stepped(step, "add i16 %x, 5");
        let expected = expected.map(|step| match step {
            AffineOperand::Value(..) => AffineOperand::Value(parsed.value("x"), 16),
            other => other,
        });
        assert_eq!(parsed.basics().values().next().map(|one| one.step.clone()), expected, "{step}");
    }
}

#[test]
fn test_long_recurrence_keeps_its_width() {
    let parsed = Parsed::new(
        "define void @f(i32 %x, i1 %c) {
b0:
  br label %b1

b1:
  %i = phi i32 [ %x, %b0 ], [ %next, %b1 ]
  %next = add i32 %i, 1
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
    );
    let found = parsed.basics();
    let recurrence = &found[&parsed.value("i")];
    assert_eq!(recurrence.start, AffineOperand::Value(parsed.value("x"), 32));
    assert_eq!(recurrence.step, constant(1, 32));
}

/// `(i * factor + i) << 1`, the factor a constant only the facts know.
#[test]
fn test_composed_word_address_has_one_recurrence() {
    for (factor, expected) in [(20, 42), (32767, 0)] {
        let parsed = Parsed::new(&format!(
            "define void @f(i16 %x, i1 %c) {{
b0:
  %factor = add i16 0, {factor}
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %m = mul i16 %i, %factor
  %sum = add i16 %m, %i
  %address = shl i16 %sum, 1
  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
        ));
        let found = parsed.derived();
        let formula = found.iter().find(|one| one.op == parsed.made("address")).unwrap();
        assert_eq!(formula.by, constant(expected, 16), "{factor}");
        assert_eq!(formula.of.value, parsed.value("i"));
    }
}

#[test]
fn test_composed_offset_can_carry_an_invariant_pointer() {
    for (element, by, offset) in [("i8", 2, 1), ("i16", 4, 2)] {
        let parsed = Parsed::new(&format!(
            "define void @f(ptr %base, i16 %x, i1 %c) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %offset = mul i16 %i, 2
  %displaced = add i16 %offset, 6
  %p = getelementptr {element}, ptr %base, i16 %displaced
  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
        ));
        let found = parsed.derived();
        let carried = found.iter().find(|one| one.op == parsed.made("p")).unwrap();
        assert_eq!(carried.pointer, Some(Operand::Value(parsed.value("base"))));
        assert_eq!(carried.of.value, parsed.value("i"));
        assert_eq!(carried.by, constant(by, 16));
        assert_eq!(carried.offsets, vec![(constant(6, 16), BigInt::from(offset))]);
    }
}

/// A struct field's constant bytes are an offset of the address.
#[test]
fn test_a_field_address_carries_its_constant_bytes() {
    let parsed = Parsed::new(
        "%pair = type { i16, i16 }

define void @f(ptr %base, i16 %x, i1 %c) {
b0:
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %p = getelementptr %pair, ptr %base, i16 %i, i32 1
  %q = getelementptr i16, ptr %p, i16 %i
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
    );
    let found = parsed.derived();
    let field = found.iter().find(|one| one.op == parsed.made("p")).unwrap();
    assert_eq!((field.by.clone(), field.offsets.clone()), (constant(4, 16), vec![(constant(2, 16), BigInt::from(1))]));
    // Off an address of the counter: one formula off `%base`.
    let chained = found.iter().find(|one| one.op == parsed.made("q")).unwrap();
    assert_eq!((chained.by.clone(), chained.offsets.clone()), (constant(6, 16), vec![(constant(2, 16), BigInt::from(1))]));
}

#[test]
fn test_every_incoming_path_agrees_on_the_recurrence() {
    for mismatch in ["start", "step", "unchanged", "none"] {
        let second_start = if mismatch == "start" { "%y" } else { "%x" };
        let second_step = if mismatch == "step" { "sub i16 %i, 1" } else { "add i16 %i, 1" };
        let second_next = if mismatch == "unchanged" { "%i" } else { "%other" };
        let parsed = Parsed::new(&format!(
            "define void @f(i16 %x, i16 %y, i1 %c, i1 %d) {{
b0:
  br i1 %d, label %b1, label %b4

b4:
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ {second_start}, %b4 ], [ %next, %b1 ], [ {second_next}, %b3 ]
  %next = add i16 %i, 1
  br i1 %c, label %b1, label %b3

b3:
  %other = {second_step}
  br i1 %d, label %b1, label %b2

b2:
  ret void
}}
"
        ));
        assert_eq!(!parsed.basics().is_empty(), mismatch == "none", "{mismatch}");
    }
}

#[test]
fn test_a_shift_recurrence_requires_a_constant_count() {
    for (shift, expected) in [("shl i16 %i, %x", None), ("shl i16 3, %i", None), ("shl i16 %i, 3", Some(8)), ("shl i16 %i, 16", None)] {
        let parsed = stepped("add i16 %i, 1", shift);
        let found = parsed.derived();
        let by = found.iter().find(|one| one.op == parsed.made("m")).map(|one| one.by.clone());
        assert_eq!(by, expected.map(|by| constant(by, 16)), "{shift}");
    }
}

/// Strength reduction started `a[i].x` at `n + (m * 12 + 600)` and `a[i].y`
/// at `n + (m * 12 + 606)`: no single root, so merged PARTICLE kept six
/// pointers and spilled four.
#[test]
fn test_starts_whose_terms_agree_are_a_constant_apart() {
    let parsed = Parsed::new(
        "define void @f(i16 %n, i16 %m) {
b0:
  %scaled = mul i16 %m, 12
  %inner0 = add i16 %scaled, 600
  %start0 = add i16 %n, %inner0
  %inner1 = add i16 %scaled, 606
  %start1 = add i16 %n, %inner1
  ret void
}
",
    );
    let at = |name: &str| AffineOperand::Value(parsed.value(name), 16);
    assert_eq!(distance(&parsed.unit(), &at("start1"), &at("start0"), 16), Some(BigInt::from(6)));
    assert_eq!(distance(&parsed.unit(), &at("start1"), &at("n"), 16), None);
}

/// `counted` reached `derived` for a memory bound, whose quotient rule asked `counted`: unbounded recursion.
#[test]
fn test_a_loop_dividing_its_counter_is_counted_without_asking_itself() {
    let parsed = Parsed::new(
        "define void @f(i16 %g) {
b0:
  br label %b1

b1:
  %i = phi i16 [ %g, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 5
  br i1 %c, label %b2, label %b3

b2:
  %q = sdiv i16 %i, 1
  %next = add i16 %i, 1
  br label %b1

b3:
  ret void
}
",
    );
    let proofs = parsed.counted(true);
    let [proof] = &proofs[..] else { panic!("one proof") };
    assert_eq!(proof.count, None);
}

/// IVARM lost its ten-trip proof when IndVarSimplify changed `<= 10` to `!= 37`.
#[test]
fn test_a_not_equal_loop_knows_its_last_trip_only_without_wrapping() {
    let cases: [(i64, i64, i64, Option<i64>); 8] = [
        (7, 3, 37, Some(34)),
        (37, -3, 7, Some(10)),
        (0, 1, 32767, Some(32766)),
        (0, -1, -32768, Some(-32767)),
        (7, 3, 38, None), // reached only after wrapping
        (7, -3, 37, None),
        (7, 3, 7, None),          // no trip
        (32767, 1, -32768, None), // its exit value wraps
    ];
    for (start, step, bound, last) in cases {
        let parsed = looped(Some(start), Some(bound), IntPredicate::Ne, step, shaped("pre", 16));
        let proofs = parsed.counted(false);
        let [proof] = &proofs[..] else { panic!("one proof") };
        assert_eq!(proof.last, last.map(BigInt::from), "{start} {step} {bound}");
        if last.is_some() {
            let (_, seen, _) = decoded(parsed.run(&[(0, 16), (0, 16)], 1_000_000).unwrap(), 16);
            assert_eq!(Some(seen), last, "{start} {step} {bound}");
        }
    }
}

/// `for i = 0 to n: load a[i]` over an unknown `n`: only the access can bound its trips.
fn indexing(inbounds: &str) -> Parsed {
    Parsed::new(&format!(
        "@a = global [100 x i8] zeroinitializer
@n = global i16 0

define void @f() {{
b0:
  %limit = load i16, ptr @n
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp ugt i16 %i, %limit
  br i1 %c, label %b3, label %b2

b2:
  %p = getelementptr {inbounds} i8, ptr @a, i16 %i
  %x = load i8, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  ret void
}}
"
    ))
}

/// Raised BC's `a[i]` may wrap its 16-bit offset, yet it bounded `i <= n` as if it could not.
#[test]
fn test_only_a_promised_access_bounds_an_inclusive_loop() {
    for (inbounds, maximum) in [("inbounds", Some(0x10000)), ("", None)] {
        let maxima = indexing(inbounds).counted(true).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        assert_eq!(maxima, maximum.map(|maximum| vec![Some(BigInt::from(maximum))]).unwrap_or_default(), "{inbounds}");
        // Not asked to read the accesses: no proof.
        assert!(indexing(inbounds).counted(false).is_empty());
    }
}

/// A symbolic proof's trips, skip test and exit value, placed for each
/// start, against running the loop from it.
#[test]
fn test_a_symbolic_count_agrees_with_running_it() {
    for (test, step, bound) in [
        (IntPredicate::Slt, 1, 10),
        (IntPredicate::Ult, 1, 10),
        (IntPredicate::Sgt, -1, -4),
        (IntPredicate::Ugt, -1, 4),
        (IntPredicate::Ne, 1, 0),
        (IntPredicate::Ne, -1, 0),
    ] {
        let parsed = looped(None, Some(bound), test, step, shaped("pre", 16));
        let proofs = parsed.counted(false);
        let [proof] = &proofs[..] else { panic!("{test:?}: one proof") };
        assert_eq!(proof.count, None, "{test:?}");
        let g = parsed.value("g");
        for start in [-30000_i64, -5, -1, 0, 1, 3, 9, 10, 11, 1000] {
            let number = |one: &AffineOperand| match one {
                AffineOperand::Const(known) => known.n.clone(),
                AffineOperand::Value(value, _) if *value == g => BigInt::from(start),
                other => panic!("{other:?}"),
            };
            let mut computed = |op: BinaryOp, args: Vec<AffineOperand>| {
                let (left, right) = (number(&args[0]), number(&args[1]));
                let n = match op {
                    BinaryOp::Add => left + right,
                    BinaryOp::Sub => left - right,
                    other => panic!("{other:?}"),
                };
                AffineOperand::constant(n, 16)
            };
            let Some(result) = parsed.run(&[(start, 16), (0, 16)], 2_000_000) else { continue };
            let (ran, _, left) = decoded(result, 16);
            let ((ahead, behind), skip) = skipped(proof).unwrap();
            let never = compared(skip, &number(&ahead), &number(&behind), 16);
            assert_eq!(never, ran == BigInt::from(0), "{test:?} {start}");
            if never {
                continue;
            }
            let trips = trips(proof, &mut computed).unwrap();
            assert_eq!(trips, AffineOperand::Const(Known::new(masked(&ran, 16), 16)), "{test:?} {start}");
            assert_eq!(exit_value(proof, &mut computed), Some(constant(left, 16)), "{test:?} {start}");
        }
    }
}

/// `left test right` on `width`-bit integers.
fn compared(test: IntPredicate, left: &BigInt, right: &BigInt, width: u32) -> bool {
    let signed = |n: &BigInt| _as_signed(&masked(n, width), width);
    let (ul, ur) = (masked(left, width), masked(right, width));
    let (sl, sr) = (signed(left), signed(right));
    match test {
        IntPredicate::Eq => ul == ur,
        IntPredicate::Ne => ul != ur,
        IntPredicate::Ult => ul < ur,
        IntPredicate::Ule => ul <= ur,
        IntPredicate::Ugt => ul > ur,
        IntPredicate::Uge => ul >= ur,
        IntPredicate::Slt => sl < sr,
        IntPredicate::Sle => sl <= sr,
        IntPredicate::Sgt => sl > sr,
        IntPredicate::Sge => sl >= sr,
    }
}

#[test]
fn test_a_variable_step_or_a_changing_bound_is_not_counted() {
    let variable = Parsed::new(
        "define void @f(i16 %s) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 100
  br i1 %c, label %b2, label %b3

b2:
  %next = add i16 %i, %s
  br label %b1

b3:
  ret void
}
",
    );
    assert_eq!(variable.basics().len(), 1, "a recurrence, stepping by %s");
    assert!(variable.counted(true).is_empty());
    let changing = Parsed::new(
        "define void @f() {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %n = phi i16 [ 100, %b0 ], [ %less, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  %less = add i16 %n, -1
  br label %b1

b3:
  ret void
}
",
    );
    assert!(changing.counted(true).is_empty());
}

/// A second way out stops the count, unless it stops the program.
#[test]
fn test_a_second_exit_is_counted_only_where_it_stops_the_program() {
    for (elsewhere, stops) in [("ret void", None), ("unreachable", Some(10))] {
        let parsed = Parsed::new(&format!(
            "define void @f(i1 %d) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b4, label %b3

b4:
  br i1 %d, label %b5, label %b2

b5:
  {elsewhere}

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  ret void
}}
"
        ));
        let (unit, loop_) = (parsed.unit(), parsed.only_loop());
        assert!(counted(&unit, &loop_, None, false).is_empty(), "{elsewhere}");
        let facts = consts::known(&unit, None, None, None);
        assert_eq!(trips_unless_stopped(&unit, &loop_, &facts), stops.map(BigInt::from), "{elsewhere}");
    }
}

/// `i <= n` from a runtime start ends only where `i + 1` may not wrap.
#[test]
fn test_a_step_promised_not_to_wrap_ends_an_inclusive_symbolic_loop() {
    for (test, flags, maximum) in [
        (IntPredicate::Sle, "nsw", Some(0x10000)),
        (IntPredicate::Sle, "nuw", None),
        (IntPredicate::Sle, "", None),
        (IntPredicate::Ule, "nuw", Some(0x10000)),
        (IntPredicate::Ule, "nsw", None),
    ] {
        let parsed = looped(None, None, test, 1, Shape { flags, ..shaped("pre", 16) });
        let maxima = parsed.counted(false).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        assert_eq!(maxima, maximum.map(|one| vec![Some(BigInt::from(one))]).unwrap_or_default(), "{test:?} {flags}");
    }
}

/// `i < n` from a known start cannot pass the width's end: `lsr` needs
/// that bound to prove a recurrence reaches zero no earlier, and
/// found none for a runtime `n` (the old body read it off `n`'s range).
#[test]
fn an_exclusive_test_is_bounded_by_its_widths_end() {
    for (test, start, maximum) in [(IntPredicate::Ult, 0, Some(0xFFFF)), (IntPredicate::Slt, 0, Some(0x7FFF)), (IntPredicate::Ult, 5, Some(0xFFFA)), (IntPredicate::Ule, 0, None)] {
        let parsed = looped(Some(start), None, test, 1, shaped("pre", 16));
        let maxima = parsed.counted(false).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        assert_eq!(maxima, maximum.map(|one| vec![Some(BigInt::from(one))]).unwrap_or_default(), "{test:?} {start}");
    }
}

/// `sext` or `zext` of a counted byte counter, times 3.
fn extended(cast: &str, start: i64, bound: i64) -> Parsed {
    Parsed::new(&format!(
        "define void @f() {{
b0:
  br label %b1

b1:
  %i = phi i8 [ {start}, %b0 ], [ %next, %b2 ]
  %c = icmp ult i8 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  %wide = {cast} i8 %i to i16
  %m = mul i16 %wide, 3
  %next = add i8 %i, 1
  br label %b1

b3:
  ret void
}}
",
        start = signed(start, 8),
        bound = signed(bound, 8)
    ))
}

#[test]
fn test_an_extension_of_a_counter_that_cannot_wrap_is_a_wide_recurrence() {
    for (cast, start, bound, wide_start) in [
        ("sext", 0, 100, Some(0)),
        ("sext", 100, 200, None), // crosses the sign
        ("zext", 100, 200, Some(100)),
        ("sext", 0xF0, 0xFF, Some(-16)),
        ("zext", 0xF0, 0xFF, Some(0xF0)),
    ] {
        let parsed = extended(cast, start, bound);
        let found = parsed.derived();
        let product = found.iter().find(|one| one.op == parsed.made("m"));
        let expected = wide_start.map(|n| Affine { value: parsed.value("wide"), start: constant(n, 16), step: constant(1, 16), header: parsed.only_loop().header });
        assert_eq!(product.map(|one| one.of.clone()), expected, "{cast} {start} {bound}");
    }
}

#[test]
fn test_an_exact_quotient_of_a_counter_is_a_recurrence() {
    for (step, divisor, expected) in [(4, 2, Some(2)), (4, -2, Some(-2)), (3, 2, None), (4, 0, None)] {
        let parsed = Parsed::new(&format!(
            "define void @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 100
  br i1 %c, label %b2, label %b3

b2:
  %q = sdiv i16 %i, {divisor}
  %next = add i16 %i, {step}
  br label %b1

b3:
  ret void
}}
"
        ));
        let found = parsed.derived();
        let quotient = found.iter().find(|one| one.op == parsed.made("q"));
        assert_eq!(quotient.map(|one| one.of.step.clone()), expected.map(|n| constant(n, 16)), "{step} {divisor}");
    }
}

#[test]
fn test_advances_are_each_values_change_per_trip() {
    let parsed = stepped("add i16 %i, 2", "mul i16 %i, -3");
    let found = advances(&parsed.unit(), &parsed.only_loop());
    let expected = [("i", 2), ("m", -6), ("next", 2)].map(|(name, n)| (parsed.value(name), BigInt::from(n)));
    assert_eq!(found, IndexMap::from_iter(expected));
}

#[test]
fn test_a_derived_formula_maps_the_counter_by_constants() {
    let parsed = stepped("add i16 %i, 1", "mul i16 %i, 3");
    let formula = parsed.derived().into_iter().find(|one| one.op == parsed.made("m")).unwrap();
    let facts = consts::known(&parsed.unit(), None, None, None);
    assert_eq!(derived_map(&formula, &facts), Some(AffineMap { scale: BigInt::from(3), offset: BigInt::from(0), width: 16 }));
}

/// A pre-tested two-block loop testing `%i` by `compare`, which `read` may read too.
fn replaceable(read: &str, compare: &str) -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(ptr %p, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp {compare}
  br i1 %c, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  {read}
  br label %b1

b3:
  %left = phi i16 [ %i, %b1 ]
  ret i16 %left
}}
"
    ))
}

#[test]
fn test_a_counter_is_replaceable_only_where_every_other_read_is_covered() {
    let parsed = replaceable("store i16 %i, ptr %p", "slt i16 %i, 10");
    let (unit, loop_) = (parsed.unit(), parsed.only_loop());
    let proofs = counted(&unit, &loop_, None, false);
    let [proof] = &proofs[..] else { panic!("one proof") };
    let store = parsed.function().walk().map(|(_, inst)| inst).find(|&inst| matches!(parsed.function().instruction(inst).opcode, llrm_mir::Opcode::Store { .. })).unwrap();
    assert_eq!(control_replacement(&unit, &loop_, proof, &BTreeSet::new()), None);
    let replacement = control_replacement(&unit, &loop_, proof, &BTreeSet::from([store])).expect("covered");
    assert_eq!((replacement.update, replacement.exits), (parsed.value("next"), vec![parsed.made("left")]));

    let other = replaceable("%twice = add i16 %next, %next", "slt i16 %i, 10");
    let (unit, loop_) = (other.unit(), other.only_loop());
    let proofs = counted(&unit, &loop_, None, false);
    assert_eq!(control_replacement(&unit, &loop_, &proofs[0], &BTreeSet::new()), None, "the update is read");
}

#[test]
fn test_a_candidate_ends_control_only_within_its_period() {
    for (bound, step, accepted) in [("10", 2, true), ("%n", 2, false), ("%n", 1, true)] {
        let parsed = replaceable("", &format!("ne i16 %i, {bound}"));
        let (unit, loop_) = (parsed.unit(), parsed.only_loop());
        let proofs = counted(&unit, &loop_, None, false);
        let [proof] = &proofs[..] else { panic!("one proof") };
        let candidate = Affine { step: constant(step, 16), ..proof.counter.clone() };
        let found = zero_terminating_control(&unit, &loop_, proof, &candidate, &BTreeSet::new(), None);
        assert_eq!(found.is_some(), accepted, "{bound} {step}");
    }
}

/// Every count the corpus proves, against stepping its counter through its test.
///
/// The test is run by hand, not by `_ordered_after` or `_equal_after`.
#[test]
fn test_every_corpus_count_is_where_its_test_first_fails() {
    let (mut proofs, mut counts) = (0, 0);
    for (name, module) in corpus() {
        let layout = layout(&module);
        for (_, global, function) in module.functions() {
            if function.entry().is_none() {
                continue;
            }
            let unit = Unit::of(&module, &layout, function);
            let facts = consts::known(&unit, None, None, None);
            for loop_ in loops::loops(&cfg::graph(function), None) {
                for proof in counted_unless_stopped(&unit, &loop_, Some(&facts), false) {
                    proofs += 1;
                    let (Some(count), AffineOperand::Const(start), AffineOperand::Const(bound)) = (&proof.count, &proof.start, &proof.bound) else { continue };
                    counts += 1;
                    let width = proof.width();
                    let mut i = start.n.clone();
                    let mut ran = BigInt::from(0);
                    loop {
                        let tested = if proof.posttested && proof.stepped { &i + &proof.step } else { i.clone() };
                        if !proof.posttested && !compared(proof.test, &tested, &bound.n, width) {
                            break;
                        }
                        ran += 1;
                        if proof.posttested && !compared(proof.test, &tested, &bound.n, width) {
                            break;
                        }
                        i = masked(&(&i + &proof.step), width);
                        assert!(ran <= BigInt::from(1) << width, "{name}/@{:?}: endless", global.name);
                    }
                    assert_eq!(&ran, count, "{name}/@{:?} at {}", global.name, loop_.header);
                }
            }
        }
    }
    // The corpus's FOR loops are counted: none found means recognition broke.
    assert!(counts > 100 && proofs > counts, "{counts} constant counts of {proofs} proofs");
}

#[test]
fn test_every_corpus_derived_formula_names_a_counter_of_its_loop() {
    let mut found = 0;
    for (name, module) in corpus() {
        let layout = layout(&module);
        for (_, _, function) in module.functions().filter(|(_, _, one)| one.entry().is_some()) {
            let unit = Unit::of(&module, &layout, function);
            for (loop_, counters, formulas) in of(&unit) {
                for formula in formulas {
                    found += 1;
                    let block = cfg::id(function.parent(formula.op).expect("placed"));
                    assert!(loop_.body.contains(&block) && formula.of.header == loop_.header, "{name}");
                    assert!(counters.contains_key(&formula.of.value) || unit.defining(Operand::Value(formula.of.value)).is_some(), "{name}");
                }
            }
        }
    }
    assert!(found > 100, "{found} formulas");
}

/// An inner loop from `%s`, a phi the outer loop carries: its entry value,
/// then the counter as it left plus `rewind`. Rewound by the 8 it advanced,
/// `%s` is always `%start` and each run takes 4 trips; by 6, later runs
/// take 3, so no count holds.
#[test]
fn a_start_rewound_to_its_entry_value_is_counted_from_it() {
    for (rewind, count, trips) in [(-8, Some(4), 12), (-6, None, 10)] {
        let parsed = Parsed::new(&format!(
            "define i32 @f(i32 %x) {{
b0:
  %start = add i32 %x, 5
  %bound = add i32 %start, 8
  br label %b1

b1:
  %s = phi i32 [ %start, %b0 ], [ %reset, %b4 ]
  %o = phi i16 [ 0, %b0 ], [ %onext, %b4 ]
  %t = phi i32 [ 0, %b0 ], [ %n, %b4 ]
  br label %b2

b2:
  %cur = phi i32 [ %s, %b1 ], [ %fol, %b3 ]
  %n = phi i32 [ %t, %b1 ], [ %n1, %b3 ]
  %done = icmp eq i32 %cur, %bound
  br i1 %done, label %b4, label %b3

b3:
  %fol = add i32 %cur, 2
  %n1 = add i32 %n, 1
  br label %b2

b4:
  %reset = add i32 %cur, {rewind}
  %onext = add i16 %o, 1
  %again = icmp slt i16 %onext, 3
  br i1 %again, label %b1, label %b5

b5:
  ret i32 %n
}}
"
        ));
        let inner = loops::loops(&cfg::graph(parsed.function()), None).into_iter().min_by_key(|one| one.body.len()).unwrap();
        let facts = consts::known(&parsed.unit(), None, None, None);
        assert_eq!(trip_count(&parsed.unit(), &inner, &facts), count.map(BigInt::from), "{rewind}");
        assert_eq!(parsed.run(&[(0, 32)], 1_000), Some(trips), "{rewind}");
    }
}

/// Mandelbrot's `cx = (long)px * 24 + x`: the sign extension of the word
/// counter is a recurrence of its own, so no use reads the word counter to
/// extend it. As a use it kept the word counter beside `cx`, a register
/// and an add a trip.
#[test]
fn test_an_extended_counter_is_a_recurrence_not_a_use() {
    let parsed = Parsed::new(
        "define i32 @f(i32 %x) {
b0:
  br label %b1

b1:
  %px = phi i16 [ -16, %b0 ], [ %next, %b2 ]
  %s = phi i32 [ 0, %b0 ], [ %s.next, %b2 ]
  %go = icmp slt i16 %px, 16
  br i1 %go, label %b2, label %b3

b2:
  %w = sext i16 %px to i32
  %m = mul nsw i32 %w, 24
  %cx = add nsw i32 %m, %x
  %s.next = add i32 %s, %cx
  %next = add nsw i16 %px, 1
  br label %b1

b3:
  ret i32 %s
}
",
    );
    let unit = parsed.unit();
    let loop_ = parsed.only_loop();
    let counters = basics(&unit, &loop_);
    let found = users(&unit, &loop_, &counters, &derived(&unit, &loop_, Some(&counters)));
    let extension = parsed.made("w");
    assert!(found.web.contains(&extension), "{:?}", found.uses);
    assert!(found.uses.iter().all(|one| one.user != extension), "{:?}", found.uses);
    let of = &found.values[&parsed.value("w")];
    assert_eq!((of.start.known(), of.step.known()), (Some(BigInt::from(-16)), Some(BigInt::from(1))));
}
