//! llrm-core's `induction_tests.rs` and `counted_loops_tests.rs`, and what
//! they leave untested, as loops the interpreter runs.
//!
//! Skipped: the BC and OMF fixture tests (NDARR, MATRIX, HARR, NESTED),
//! the `lower`, `ssa::constructed`, CSE and `dead` tests filed here; the
//! `or i, i` zero tests (a flags idiom); width-preserving copies (no copy);
//! loopexit's and rotate's shapes, which wait for those ports.

use std::collections::BTreeSet;

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
use crate::graph::loops::{self, Loop};
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
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, self.function()))
    }

    pub fn value(
        &self,
        name: &str,
    ) -> ValueId {
        value(self.function(), name)
    }

    /// The instruction defining `%name`.
    pub fn made(
        &self,
        name: &str,
    ) -> InstId {
        let wanted = self.value(name);
        self.function()
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| self.function().instruction(inst).result == Some(wanted))
            .expect("defined")
    }

    pub fn only_loop(&self) -> Loop {
        let found = loops::loops(&cfg::graph(self.function()), None);
        let [one] = &found[..] else { panic!("one loop") };
        one.clone()
    }

    pub fn counted(
        &self,
        inbounds: bool,
    ) -> Vec<CountedLoop> {
        counted(&self.unit(), &self.only_loop(), None, inbounds)
    }

    pub fn recurrence(
        &self,
        name: &str,
    ) -> Option<Recurrence> {
        recurrences_of(self).remove(&self.value(name))
    }

    pub fn basics(&self) -> IndexMap<ValueId, Affine> {
        basics(&self.unit(), &self.only_loop())
    }

    /// `@f(arguments)`'s integer result.
    pub fn run(
        &self,
        arguments: &[(i64, u32)],
        fuel: u64,
    ) -> Option<u128> {
        let arguments = arguments
            .iter()
            .map(|&(n, width)| Val::Int { bits: n as u128 & llrm_mir::context::mask(width), width })
            .collect();
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

fn signed(
    n: i64,
    width: u32,
) -> i64 {
    llrm_mir::context::signed(n as u128 & llrm_mir::context::mask(width), width) as i64
}

fn constant(
    n: i64,
    width: u32,
) -> AffineOperand {
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

pub(crate) fn shaped(
    shape: &str,
    width: u32,
) -> Shape {
    Shape {
        posttested: shape != "pre",
        stepped: shape == "post-stepped",
        mirrored: false,
        split: false,
        width,
        flags: "",
    }
}

/// `i = start` stepping by `step` while `i test bound`: a start of None is
/// `%g`, a bound of None `%n`. It returns `trips << 32 | seen << 16 | left`:
/// the trips, the last trip's `i` (`start` for none), and the `i` the
/// loop leaves with (the stepped one for a post-tested loop).
pub(crate) fn looped(
    start: Option<i64>,
    bound: Option<i64>,
    test: IntPredicate,
    step: i64,
    shape: Shape,
) -> Parsed {
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
pub(crate) fn decoded(
    result: u128,
    width: u32,
) -> (BigInt, i64, i64) {
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
    for (shape, split) in
        [("pre", false), ("post", false), ("post-stepped", false), ("post", true), ("post-stepped", true)]
    {
        for step in [1, -1, 3, -3] {
            let mut proved = BTreeSet::new();
            for test in TESTS {
                for start in ENDS {
                    for bound in ENDS {
                        for mirrored in [false, true] {
                            let parsed = looped(
                                Some(start),
                                Some(bound),
                                test,
                                step,
                                Shape { mirrored, split, ..shaped(shape, 8) },
                            );
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
                                assert_eq!(
                                    (proof.first.clone(), proof.last.clone()),
                                    (Some(signed(start, 8).into()), Some(seen.into())),
                                    "{where_:?}"
                                );
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
            let ending = TESTS
                .into_iter()
                .filter(|&test| test == IntPredicate::Ne || _ascending(test) == (step > 0))
                .map(spelled)
                .collect::<BTreeSet<_>>();
            assert_eq!(proved, ending, "{shape} {split} {step}");
        }
    }
}

/// `i = 0; while i <= 32767` was proved to run 32768 trips: `i + 1` wraps and
/// it never ends.
#[test]
fn test_an_inclusive_test_at_its_types_maximum_is_not_counted() {
    let cases: [(i64, i64, IntPredicate, Option<i64>); 5] = [
        // signed <= its maximum never fails
        (0, 0x7FFF, IntPredicate::Sle, None),
        (0, 0x7FFE, IntPredicate::Sle, Some(0x7FFF)),
        // unsigned <= its maximum never fails
        (0, 0xFFFF, IntPredicate::Ule, None),
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
fn stepped(
    step: &str,
    multiply: &str,
) -> Parsed {
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
    assert_eq!(other.recurrence("m"), None);
    let counter = stepped("add i16 %i, 1", "mul i16 %i, 2");
    let x = Scev::unknown(counter.value("x"), 16);
    assert_eq!(
        counter.recurrence("m"),
        Some(Recurrence { pointer: None, start: x.times(&BigInt::from(2)), step: Scev::constant(2, 16) })
    );
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
        ("add i16 %i, %m", None), // changes in the loop
        ("sub i16 %i, %x", None), // a variable subtracted
        ("sub i16 3, %i", None),  // the phi negated
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
        let x = Scev::unknown(parsed.value("x"), 16);
        let expected =
            Recurrence { pointer: None, start: x.times(&BigInt::from(expected)), step: Scev::constant(expected, 16) };
        assert_eq!(parsed.recurrence("address"), Some(expected), "{factor}");
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
        let x = Scev::unknown(parsed.value("x"), 16);
        let start = x.times(&BigInt::from(by)).plus(&Scev::constant(6 * offset, 16));
        let carried =
            Recurrence { pointer: Some(Operand::Value(parsed.value("base"))), start, step: Scev::constant(by, 16) };
        assert_eq!(parsed.recurrence("p"), Some(carried));
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
    let (base, x) = (Operand::Value(parsed.value("base")), Scev::unknown(parsed.value("x"), 16));
    let at = |scale: i64, bytes: i64| Recurrence {
        pointer: Some(base),
        start: x.times(&BigInt::from(scale)).plus(&Scev::constant(bytes, 16)),
        step: Scev::constant(scale, 16),
    };
    assert_eq!(parsed.recurrence("p"), Some(at(4, 2)));
    // Off an address of the counter: one recurrence off `%base`.
    assert_eq!(parsed.recurrence("q"), Some(at(6, 2)));
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
    for (shift, expected) in
        [("shl i16 %i, %x", None), ("shl i16 3, %i", None), ("shl i16 %i, 3", Some(8)), ("shl i16 %i, 16", None)]
    {
        let parsed = stepped("add i16 %i, 1", shift);
        let step = parsed.recurrence("m").map(|of| of.step);
        assert_eq!(step, expected.map(|by| Scev::constant(by, 16)), "{shift}");
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

/// `counted` reached `derived` for a memory bound, whose quotient rule asked
/// `counted`: unbounded recursion.
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

/// IVARM lost its ten-trip proof when IndVarSimplify changed `<= 10` to `!=
/// 37`.
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

/// `for i = 0 to n: load a[i]` over an unknown `n`: only the access can bound
/// its trips.
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

/// Raised BC's `a[i]` may wrap its 16-bit offset, yet it bounded `i <= n` as if
/// it could not.
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
fn compared(
    test: IntPredicate,
    left: &BigInt,
    right: &BigInt,
    width: u32,
) -> bool {
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
    for (test, start, maximum) in [
        (IntPredicate::Ult, 0, Some(0xFFFF)),
        (IntPredicate::Slt, 0, Some(0x7FFF)),
        (IntPredicate::Ult, 5, Some(0xFFFA)),
        (IntPredicate::Ule, 0, None),
    ] {
        let parsed = looped(Some(start), None, test, 1, shaped("pre", 16));
        let maxima = parsed.counted(false).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
        assert_eq!(maxima, maximum.map(|one| vec![Some(BigInt::from(one))]).unwrap_or_default(), "{test:?} {start}");
    }
}

/// `sext` or `zext` of a counted byte counter, times 3.
fn extended(
    cast: &str,
    start: i64,
    bound: i64,
) -> Parsed {
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
        let wide = parsed.recurrence("wide").map(|of| (of.start, of.step));
        let expected = wide_start.map(|n| (Scev::constant(n, 16), Scev::constant(1, 16)));
        assert_eq!(wide, expected, "{cast} {start} {bound}");
        assert_eq!(parsed.recurrence("m").is_some(), expected.is_some(), "{cast} {start} {bound}");
    }
}

/// C's `for (unsigned char i = 0; i < 9; ++i) a[i]`: the header reads `zext i`
/// too, once more than the body, on the trip that leaves. It was refused
/// whatever the count, so the address was no recurrence and a counted loop
/// kept a conversion per trip on m32 (crc, 832 executed against 496).
#[test]
fn test_an_extension_in_the_header_is_a_recurrence_where_the_last_trip_fits() {
    for (start, bound, cast, fits) in [(0, 9, "zext", true), (0, 255, "zext", true), (1, 0, "zext", false)] {
        let test = if bound == 0 { "ne" } else { "ult" };
        let parsed = Parsed::new(&format!(
            "define void @f() {{
b0:
  br label %b1

b1:
  %i = phi i8 [ {start}, %b0 ], [ %next, %b2 ]
  %h = {cast} i8 %i to i16
  %m = mul i16 %h, 3
  %c = icmp {test} i8 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  %next = add i8 %i, 1
  br label %b1

b3:
  ret void
}}
"
        ));
        assert_eq!(
            parsed.recurrence("h").map(|of| (of.start, of.step)),
            fits.then(|| (Scev::constant(start, 16), Scev::constant(1, 16))),
            "{start} {bound}"
        );
    }
}

/// `for (unsigned short i = 0; i < n; ++i)`: no count is known, but the test
/// ends the loop before the counter passes the width's largest, so its
/// zero extension is a wide counter. A signed test or a larger step does
/// not say so.
#[test]
fn test_a_symbolic_ult_counter_extends_to_a_wide_recurrence() {
    for (test, step, cast, wide) in
        [("ult", 1, "zext", true), ("ult", 2, "zext", false), ("slt", 1, "zext", false), ("ult", 1, "sext", false)]
    {
        let parsed = Parsed::new(&format!(
            "define void @f(i8 %n) {{
b0:
  br label %b1

b1:
  %i = phi i8 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp {test} i8 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %w = {cast} i8 %i to i16
  %m = mul i16 %w, 3
  %next = add i8 %i, {step}
  br label %b1

b3:
  ret void
}}
"
        ));
        assert_eq!(parsed.recurrence("w").is_some(), wide, "{test} {step} {cast}");
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
        let quotient = parsed.recurrence("q");
        assert_eq!(quotient.map(|of| of.step), expected.map(|n| Scev::constant(n, 16)), "{step} {divisor}");
    }
}

#[test]
fn test_advances_are_each_values_change_per_trip() {
    let parsed = stepped("add i16 %i, 2", "mul i16 %i, -3");
    let found = advances(&parsed.unit(), &parsed.only_loop());
    let expected = [("i", 2), ("m", -6), ("next", 2)].map(|(name, n)| (parsed.value(name), BigInt::from(n)));
    assert_eq!(found, IndexMap::from_iter(expected));
}

/// A pre-tested two-block loop testing `%i` by `compare`, which `read` may read
/// too.
fn replaceable(
    read: &str,
    compare: &str,
) -> Parsed {
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
    let store = parsed
        .function()
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            parsed.function().instruction(inst).opcode,
            llrm_mir::Opcode::Store { .. }
        ))
        .unwrap();
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

/// Every count the corpus proves, against stepping its counter through its
/// test.
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
            let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
            let facts = consts::known(&unit, None, None, None);
            for loop_ in loops::loops(&cfg::graph(function), None) {
                for proof in counted_unless_stopped(&unit, &loop_, Some(&facts), false) {
                    proofs += 1;
                    let (Some(count), AffineOperand::Const(start), AffineOperand::Const(bound)) =
                        (&proof.count, &proof.start, &proof.bound)
                    else {
                        continue;
                    };
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
fn test_every_corpus_recurrence_is_computed_inside_its_loop() {
    let mut found = 0;
    for (name, module) in corpus() {
        let layout = layout(&module);
        for (_, _, function) in module.functions().filter(|(_, _, one)| one.entry().is_some()) {
            let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
            for loop_ in unit.shape().loops.iter() {
                let counters = basics(&unit, loop_);
                for inst in recurrences(&unit, loop_, &counters).web {
                    found += 1;
                    let block = cfg::id(function.parent(inst).expect("placed"));
                    assert!(loop_.body.contains(&block), "{name}");
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
        let inner =
            loops::loops(&cfg::graph(parsed.function()), None).into_iter().min_by_key(|one| one.body.len()).unwrap();
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
    let found = users(&unit, &loop_, &counters);
    let extension = parsed.made("w");
    assert!(found.web.contains(&extension), "{:?}", found.uses);
    assert!(found.uses.iter().all(|one| one.user != extension), "{:?}", found.uses);
    let of = &found.values[&parsed.value("w")];
    assert_eq!((of.start.known(), of.step.known()), (Some(BigInt::from(-16)), Some(BigInt::from(1))));
}

/// `zip(a, b)`: the index tested against each slice's length, one exit
/// at the header and one in the body. No single-exit proof counted it.
const ZIPPED: &str = "define i16 @f(i16 %la, i16 %lb) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %ina = icmp ult i16 %i, %la
  br i1 %ina, label %check, label %done
check:
  %inb = icmp ult i16 %i, %lb
  br i1 %inb, label %body, label %done
body:
  %t = add i16 %s, %i
  %j = add nuw i16 %i, 1
  br label %head
done:
  %r = phi i16 [ %s, %head ], [ %s, %check ]
  ret i16 %r
}
";

/// Each exit the latch follows is counted by its own compare, and the
/// loop takes the least of their counts: `la` and `lb` backedges.
#[test]
fn test_every_exit_is_counted_and_the_loop_takes_the_least() {
    let parsed = Parsed::new(ZIPPED);
    assert!(counted(&parsed.unit(), &parsed.only_loop(), None, false).is_empty());
    let found = exits(&parsed.unit(), &parsed.only_loop(), None, false);
    let (la, lb) = (parsed.value("la"), parsed.value("lb"));
    let taken = found.iter().map(|one| one.taken.clone()).collect::<Vec<_>>();
    assert_eq!(
        taken,
        [
            Some(vec![Scev::of(&AffineOperand::Value(la, 16), 16)]),
            Some(vec![Scev::of(&AffineOperand::Value(lb, 16), 16)])
        ],
        "{found:?}"
    );
    assert_eq!(backedges(&found).map(|least| least.len()), Some(2));
}

/// A branch leaving as soon as either of two compares fails takes the
/// least of their counts; the same compares under an `or`, leaving only
/// when both fail, count only where they agree.
#[test]
fn test_a_branch_on_two_compares_takes_the_least_of_their_counts() {
    let text = "define i16 @f(i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %head ]
  %j = add nuw i16 %i, 1
  %a = icmp ult i16 %j, 10
  %b = icmp ult i16 %j, 7
  %c = and i1 %a, %b
  br i1 %c, label %head, label %done
done:
  ret i16 %j
}
";
    let parsed = Parsed::new(text);
    let found = exits(&parsed.unit(), &parsed.only_loop(), None, false);
    let taken = found[0].taken.clone().expect("counted");
    assert_eq!(taken.iter().filter_map(Scev::known).min(), Some(BigInt::from(6)), "{found:?}");
    let either = Parsed::new(&text.replace("and i1", "or i1"));
    assert_eq!(exits(&either.unit(), &either.only_loop(), None, false)[0].taken, None);
}

/// A loop to `i != n` reading `a[i]`, two-byte elements, in bounds: at
/// most 32768 trips, as for `i < n`. The in-bounds bound was skipped for
/// `!=`, and Nib's `zip` of two slices, its exits made one `!=`, could
/// not count down to zero by two.
#[test]
fn test_an_inequality_loop_is_bounded_by_its_in_bounds_accesses() {
    let text = "define i16 @f(ptr addrspace(1) %a, i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i16 [ 0, %entry ], [ %t, %body ]
  %more = icmp ne i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %o = mul i16 %i, 2
  %p = getelementptr inbounds i8, ptr addrspace(1) %a, i16 %o
  %v = load i16, ptr addrspace(1) %p
  %t = add i16 %s, %v
  %j = add i16 %i, 1
  br label %head
done:
  ret i16 %s
}
";
    let parsed = Parsed::new(text);
    let proofs = counted(&parsed.unit(), &parsed.only_loop(), None, true);
    assert_eq!(proofs.first().and_then(|proof| proof.maximum.clone()), Some(BigInt::from(32768)), "{proofs:?}");
}

/// `if (n > 0) do { ... } while (++i < n)`: the first test is proved by
/// the branch over the loop, so it runs `n` trips, and the proof says so
/// where it could say nothing of a loop tested after its trips.
fn guarded_do(guard: &str) -> String {
    format!(
        "define i16 @f(i16 %n) {{
entry:
  %ok = icmp sgt i16 %n, {guard}
  br i1 %ok, label %pre, label %done
pre:
  br label %body
body:
  %i = phi i16 [ 0, %pre ], [ %j, %body ]
  %j = add nsw i16 %i, 1
  %c = icmp slt i16 %j, %n
  br i1 %c, label %body, label %after
after:
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %j, %after ]
  ret i16 %r
}}
"
    )
}

#[test]
fn test_a_loop_tested_after_its_trips_is_counted_where_entry_proves_the_first() {
    let proven = Parsed::new(&guarded_do("0"));
    let proofs = counted(&proven.unit(), &proven.only_loop(), None, false);
    let [proof] = &proofs[..] else { panic!("{proofs:?}") };
    assert!(proof.posttested && proof.entry_guarded && proof.count.is_none(), "{proof:?}");
    let n = Scev::of(&AffineOperand::Value(proven.value("n"), 16), 16);
    assert_eq!(proof.trips_linear(), Some(n));
    // Entered where `n > -5`, the first test is not proved: no count.
    let unproven = Parsed::new(&guarded_do("-5"));
    assert!(counted(&unproven.unit(), &unproven.only_loop(), None, false).is_empty());
}

/// `a[i + 8]` checked against the length before the loop's own test:
/// the check is an exit of its own, counted `len - 8` where the compare
/// reads `i + 8` rather than the counter.
#[test]
fn test_an_exit_testing_a_counter_plus_a_constant_is_counted() {
    let text = "define i16 @f(i16 %n, i16 %len) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %x = add i16 %i, 8
  %inside = icmp ult i16 %x, %len
  br i1 %inside, label %check, label %bad
check:
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %j = add nsw i16 %i, 1
  br label %head
done:
  ret i16 %i
bad:
  unreachable
}
";
    let parsed = Parsed::new(text);
    let found = exits(&parsed.unit(), &parsed.only_loop(), None, false);
    let len = Scev::of(&AffineOperand::Value(parsed.value("len"), 16), 16);
    let eight = Scev::constant(8, 16);
    assert_eq!(found[0].taken, Some(vec![len.minus(&eight)]), "{found:?}");
}

/// The exit compares the stepped value in the body, and the way back is a
/// block that only jumps, split from a critical edge. Neither the header
/// nor the latch branched, so no proof counted it.
#[test]
fn test_an_exit_behind_a_forwarding_latch_is_counted() {
    let text = "define i16 @f(i16 %n) {
entry:
  %ok = icmp sgt i16 %n, 0
  br i1 %ok, label %pre, label %done
pre:
  br label %head
head:
  %i = phi i16 [ 0, %pre ], [ %j, %back ]
  br label %body
body:
  %j = add nsw i16 %i, 1
  %c = icmp sge i16 %j, %n
  br i1 %c, label %after, label %back
back:
  br label %head
after:
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %j, %after ]
  ret i16 %r
}
";
    let parsed = Parsed::new(text);
    let proofs = counted(&parsed.unit(), &parsed.only_loop(), None, false);
    let [proof] = &proofs[..] else { panic!("{proofs:?}") };
    assert!(proof.posttested && proof.stepped && proof.entry_guarded, "{proof:?}");
}

/// A single block holds the trip and tests the stepped value: it is tested
/// after the trip whichever block the branch sits in, and as before where it
/// reads the phi.
#[test]
fn test_a_header_holding_its_whole_trip_is_tested_after_it() {
    let text = "define i16 @f(i16 %n) {
entry:
  %ok = icmp sgt i16 %n, 0
  br i1 %ok, label %pre, label %done
pre:
  br label %head
head:
  %i = phi i16 [ 0, %pre ], [ %j, %back ]
  %j = add nsw i16 %i, 1
  %c = icmp sge i16 %j, %n
  br i1 %c, label %after, label %back
back:
  br label %head
after:
  br label %done
done:
  %r = phi i16 [ 0, %entry ], [ %j, %after ]
  ret i16 %r
}
";
    let parsed = Parsed::new(text);
    let proofs = counted(&parsed.unit(), &parsed.only_loop(), None, false);
    let [proof] = &proofs[..] else { panic!("{proofs:?}") };
    assert!(proof.posttested && proof.stepped, "{proof:?}");
    // Reading the phi, the same block tests before the trip: no change there.
    let before = Parsed::new(&text.replace("icmp sge i16 %j, %n", "icmp sge i16 %i, %n"));
    let proofs = counted(&before.unit(), &before.only_loop(), None, false);
    assert!(proofs.iter().all(|proof| !proof.posttested), "{proofs:?}");
}

/// Every value of the loop `parsed` holds that is a recurrence, by form.
fn recurrences_of(parsed: &Parsed) -> std::collections::BTreeMap<ValueId, Recurrence> {
    let (unit, loop_) = (parsed.unit(), parsed.only_loop());
    let counters = basics(&unit, &loop_);
    recurrences(&unit, &loop_, &counters).values
}

/// `sum` at the values `env` gives its unknowns, modulo its width.
fn evaluated(
    sum: &Scev,
    env: &dyn Fn(ValueId) -> BigInt,
) -> BigInt {
    let mut total = sum.constant.clone();
    for (product, factor) in &sum.terms {
        total += product.values().iter().fold(factor.clone(), |so_far, value| so_far * env(*value));
    }
    masked(&total, sum.width)
}

/// Values the generated loops make that are affine in the counter but
/// that have no recurrence: the form's reach, counted so it can only grow.
const REFUSED_BASELINE: usize = 0;

/// A recurrence's form at trip `k` is what the interpreter computes there,
/// wrapping included, for random loops of sums, products, shifts and
/// extensions. A wrong form here was a wrong address or exit value.
#[test]
fn test_generated_recurrences_match_the_interpreter() {
    use crate::generated::{Inputs, Rng, case, observed, seeds};
    let (mut affine, mut found, mut checked) = (0, 0, 0);
    for seed in seeds() {
        let case = case(seed);
        if std::env::var("SCEV_SEED").is_ok() {
            eprintln!("{}", case.text);
        }
        let parsed = Parsed::new(&case.text);
        let forms = recurrences_of(&parsed);
        let mut rng = Rng::new(seed ^ 0xabcd);
        for (which, one) in case.tracked.iter().enumerate() {
            let value = parsed.value(&one.name);
            affine += usize::from(one.affine());
            let Some(of) = forms.get(&value).filter(|of| of.pointer.is_none()) else { continue };
            found += usize::from(one.affine());
            assert_eq!(of.width(), one.width, "seed {seed}: %{} is {} bits\n{}", one.name, one.width, case.text);
            for _ in 0..4 {
                let inputs = Inputs::random(&mut rng);
                let trip = rng.below(40) as u16;
                let env = |term: ValueId| {
                    let name = parsed.function().value(term).name.clone().expect("a named unknown");
                    let n = inputs.named(&name).map(u128::from).unwrap_or_else(|| {
                        let at = case.tracked.iter().position(|one| one.name == name).expect("a tracked unknown");
                        observed(&parsed.module, &inputs, at, 0).unwrap()
                    });
                    BigInt::from(n)
                };
                let (start, step) = (evaluated(&of.start, &env), evaluated(&of.step, &env));
                let expected = masked(&(start + step * trip), of.width());
                let got = masked(&BigInt::from(observed(&parsed.module, &inputs, which, trip).unwrap()), of.width());
                assert_eq!(got, expected, "seed {seed} %{} at trip {trip}, {inputs:?}\n{}", one.name, case.text);
                checked += 1;
            }
        }
    }
    eprintln!("affine values {affine}, with a recurrence {found}, refused {}, checked {checked}", affine - found);
    assert!(affine - found <= REFUSED_BASELINE, "refusals grew");
}

fn random_sum(
    rng: &mut crate::generated::Rng,
    width: u32,
) -> Scev {
    let mut sum = Scev::constant(rng.word(), width);
    for _ in 0..rng.below(4) {
        sum = sum.plus(
            &Scev::of(&AffineOperand::Value(ValueId(rng.below(4) as u32), width), width)
                .times(&BigInt::from(rng.word() as i16)),
        );
    }
    sum
}

/// `one * other`, where the form defines it.
fn product_of(
    one: &Scev,
    other: &Scev,
) -> Option<Scev> {
    one.product(other).into()
}

/// The algebra the form promises: add and mul commute and associate, mul
/// distributes over add, 0 and 1 are identities, `x - x` is 0, and
/// arithmetic wraps at the width with truncation commuting with both.
/// Equal values must be one canonical form whatever order built them: a
/// miss made two equal recurrences compare unequal and cost a counter.
#[test]
fn test_form_obeys_the_ring_laws() {
    use crate::generated::Rng;
    let mut products = 0;
    for seed in 0..400 {
        let mut rng = Rng::new(seed);
        let width = [16, 32][rng.below(2) as usize];
        let (x, y, z) = (random_sum(&mut rng, width), random_sum(&mut rng, width), random_sum(&mut rng, width));
        let (zero, one) = (Scev::constant(0, width), Scev::constant(1, width));
        assert_eq!(x.plus(&y), y.plus(&x), "seed {seed}");
        assert_eq!(x.plus(&y).plus(&z), x.plus(&y.plus(&z)), "seed {seed}");
        assert_eq!(x.plus(&zero), x, "seed {seed}");
        assert!(x.minus(&x).is_zero(), "seed {seed}");
        assert_eq!(x.minus(&y), x.plus(&y.times(&BigInt::from(-1))), "seed {seed}");
        let narrow = width / 2;
        assert_eq!(x.plus(&y).truncated(narrow), x.truncated(narrow).plus(&y.truncated(narrow)), "seed {seed}");
        let k = BigInt::from(rng.word() as i16);
        assert_eq!(x.plus(&y).times(&k), x.times(&k).plus(&y.times(&k)), "seed {seed}");
        let constant = Scev::constant(k.clone(), width);
        if let (Some(a), Some(b)) = (product_of(&x, &constant), product_of(&constant, &x)) {
            assert_eq!(a, b, "seed {seed}");
            assert_eq!(a, x.times(&k), "seed {seed}");
        }
        let Some(xy) = product_of(&x, &y) else { continue };
        let (Some(yx), Some(xz), Some(yz)) = (product_of(&y, &x), product_of(&x, &z), product_of(&y, &z)) else {
            continue;
        };
        let (Some(xy_z), Some(x_yz)) = (product_of(&xy, &z), product_of(&x, &yz)) else { continue };
        products += 1;
        assert_eq!(xy, yx, "seed {seed}");
        assert_eq!(xy_z, x_yz, "seed {seed}");
        assert_eq!(product_of(&x, &y.plus(&z)), Some(xy.plus(&xz)), "seed {seed}");
        assert_eq!(product_of(&x, &one), Some(x.clone()), "seed {seed}");
        assert!(product_of(&x, &zero).is_some_and(|zero| zero.is_zero()), "seed {seed}");
        assert_eq!(
            xy.truncated(narrow),
            product_of(&x.truncated(narrow), &y.truncated(narrow)).unwrap(),
            "seed {seed}"
        );
    }
    eprintln!("products defined {products} of 400");
    assert!(products > 0);
}

/// `%i` from `%x` by 1, and `body`, which `%v` names: the loop and its
/// recurrence.
fn symbolic_product(body: &str) -> (Parsed, Option<Recurrence>) {
    let parsed = Parsed::new(&format!(
        "define void @f(i16 %x, i16 %k, i16 %m, i16 %w, i16 %a, i16 %b, i1 %c) {{
b0:
  %p = mul i16 %a, %b
  br label %b1

b1:
  %i = phi i16 [ %x, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
{body}
  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
    ));
    let of = parsed.recurrence("v");
    (parsed, of)
}

fn unknown_of(
    parsed: &Parsed,
    name: &str,
) -> Scev {
    Scev::unknown(parsed.value(name), 16)
}

fn product(of: &[Scev]) -> Scev {
    of.iter().skip(1).fold(of[0].clone(), |so_far, next| so_far.product(next).expect("within the cap"))
}

/// `i*m` from a symbolic start was no recurrence: a value times a symbolic
/// start or step was not linear, so every `a[i*m]` and exit value off it
/// kept a multiply a trip.
#[test]
fn test_a_counter_from_a_symbol_times_an_invariant_is_a_recurrence() {
    let (parsed, of) = symbolic_product("  %v = mul i16 %i, %m");
    let (x, m) = (unknown_of(&parsed, "x"), unknown_of(&parsed, "m"));
    assert_eq!(of, Some(Recurrence { pointer: None, start: product(&[x, m.clone()]), step: m }));
}

/// `(i + k) * m`: the sum is a recurrence, and so is its scale.
#[test]
fn test_a_sum_with_an_invariant_times_an_invariant_is_a_recurrence() {
    let (parsed, of) = symbolic_product("  %s = add i16 %i, %k\n  %v = mul i16 %s, %m");
    let (x, k, m) = (unknown_of(&parsed, "x"), unknown_of(&parsed, "k"), unknown_of(&parsed, "m"));
    assert_eq!(of, Some(Recurrence { pointer: None, start: product(&[x.plus(&k), m.clone()]), step: m }));
}

/// `i*m*w`: a product of two invariants scales the counter, one monomial
/// of three unknowns for the start.
#[test]
fn test_a_counter_times_two_invariants_is_one_monomial() {
    let (parsed, of) = symbolic_product("  %t = mul i16 %i, %m\n  %v = mul i16 %t, %w");
    let of = of.expect("a recurrence");
    let (x, m, w) = (unknown_of(&parsed, "x"), unknown_of(&parsed, "m"), unknown_of(&parsed, "w"));
    assert_eq!(of.start, product(&[x.clone(), m.clone(), w.clone()]));
    assert_eq!(of.step, product(&[m, w]));
    assert_eq!(of.start.terms.len(), 1);
    assert_eq!(of.start.terms.keys().next().unwrap().values().len(), 3);
}

/// A counter from `a*b`, times `m`: a product of a product of two
/// symbols, and `m`, as the start.
#[test]
fn test_a_start_that_is_a_product_of_two_symbols_scales() {
    let parsed = Parsed::new(
        "define void @f(i16 %a, i16 %b, i16 %m, i1 %c) {
b0:
  %p = mul i16 %a, %b
  br label %b1

b1:
  %i = phi i16 [ %p, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %v = mul i16 %i, %m
  br i1 %c, label %b1, label %b2

b2:
  ret void
}
",
    );
    let (p, m) = (unknown_of(&parsed, "p"), unknown_of(&parsed, "m"));
    assert_eq!(parsed.recurrence("v"), Some(Recurrence { pointer: None, start: product(&[p, m.clone()]), step: m }));
}

/// Equal values built in another order are one recurrence.
#[test]
fn test_recurrences_built_in_another_order_are_equal() {
    let (_, first) = symbolic_product("  %t = mul i16 %i, %m\n  %v = mul i16 %t, %w");
    let (_, second) = symbolic_product("  %t = mul i16 %w, %i\n  %v = mul i16 %m, %t");
    assert!(first.is_some());
    assert_eq!(first, second);
}

/// An 8-bit counter from `start` by `step`, ended by `!=` against `bound`,
/// tested `shape` (pre, post or post-stepped); the function returns its trips.
fn tested_for_equality(
    shape: &str,
    step: i64,
    scale: u32,
) -> Parsed {
    let (start, bound) =
        if scale == 0 { ("%a".to_owned(), "%b".to_owned()) } else { ("%s".to_owned(), "%e".to_owned()) };
    let lead =
        if scale == 0 { String::new() } else { format!("  %s = shl i8 %a, {scale}\n  %e = shl i8 %b, {scale}\n") };
    let body = match shape {
        "pre" => format!(
            "b1:
  %i = phi i8 [ {start}, %b0 ], [ %next, %b2 ]
  %n = phi i8 [ 0, %b0 ], [ %n1, %b2 ]
  %c = icmp ne i8 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  %next = add i8 %i, {step}
  %n1 = add i8 %n, 1
  br label %b1

b3:
  ret i8 %n
"
        ),
        _ => {
            let tested = if shape == "post-stepped" { "%next" } else { "%i" };
            format!(
                "b1:
  %i = phi i8 [ {start}, %b0 ], [ %next, %b1 ]
  %n = phi i8 [ 0, %b0 ], [ %n1, %b1 ]
  %next = add i8 %i, {step}
  %n1 = add i8 %n, 1
  %c = icmp ne i8 {tested}, {bound}
  br i1 %c, label %b1, label %b2

b2:
  ret i8 %n1
"
            )
        }
    };
    Parsed::new(&format!("define i8 @f(i8 %a, i8 %b) {{\nb0:\n{lead}  br label %b1\n\n{body}}}\n"))
}

/// `trips` of `proof` at `a` and `b`, evaluated: the instructions it would
/// place, run on numbers.
fn evaluated_trips(
    parsed: &Parsed,
    proof: &CountedLoop,
    a: u128,
    b: u128,
    scale: u32,
) -> Option<u128> {
    let env = |name: &str| -> u128 {
        match name {
            "a" => a,
            "b" => b,
            "s" => (a << scale) & 0xFF,
            "e" => (b << scale) & 0xFF,
            other => panic!("%{other}"),
        }
    };
    let number = |one: &AffineOperand| match one {
        AffineOperand::Const(known) => known.n.clone(),
        AffineOperand::Value(value, _) => {
            BigInt::from(env(parsed.function().value(*value).name.as_deref().expect("a named value")))
        }
    };
    let mut computed = |kind: BinaryOp, args: Vec<AffineOperand>| {
        let (x, y) = (number(&args[0]), number(&args[1]));
        let result = match kind {
            BinaryOp::Add => x + y,
            BinaryOp::Sub => x - y,
            BinaryOp::Mul => x * y,
            BinaryOp::And => x & y,
            BinaryOp::LShr => x >> usize::try_from(y).unwrap(),
            other => panic!("{other:?}"),
        };
        AffineOperand::constant(result, 8)
    };
    match trips(proof, &mut computed)? {
        AffineOperand::Const(known) => u128::try_from(known.n).ok(),
        value @ AffineOperand::Value(..) => u128::try_from(number(&value)).ok(),
    }
}

/// A loop ended by `!=` against an invariant, with a symbolic start or bound,
/// any step, and any shape was uncounted: after lsr every `for x in xs` is
/// `iv.next != 0` from `-2 * len`. Its count is where `start + k * step` first
/// meets the bound, solved modulo the width and proved by what the low bits of
/// the distance are, and it is what the loop makes for every input the loop
/// ends on.
#[test]
fn test_a_loop_tested_for_equality_is_counted_by_solving_for_the_bound() {
    let mut checked = 0;
    for shape in ["pre", "post", "post-stepped"] {
        for (step, scale) in [(1, 0), (-1, 0), (3, 0), (-5, 0), (2, 1), (-2, 1), (4, 2), (6, 1), (-12, 2)] {
            let parsed = tested_for_equality(shape, step, scale);
            let proofs = parsed.counted(false);
            let proof = proofs
                .iter()
                .find(|one| one.test == IntPredicate::Ne)
                .unwrap_or_else(|| panic!("{shape} step {step} scale {scale}: no proof"));
            assert_eq!((proof.posttested, proof.stepped), (shape != "pre", shape == "post-stepped"), "{shape}");
            for a in (0..256).step_by(7) {
                for b in (0..256).step_by(5) {
                    // Where the loop never ends there is nothing to count.
                    let Some(actual) = parsed.run(&[(a, 8), (b, 8)], 3_000) else { continue };
                    let counted =
                        evaluated_trips(&parsed, proof, a as u128, b as u128, scale).expect("trips are placed");
                    assert_eq!(counted & 0xFF, actual & 0xFF, "{shape} step {step} scale {scale} a {a} b {b}");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 1_000, "{checked} runs");
}

/// Where the step is even and nothing says the distance is a multiple of it,
/// the loop may never end: no count.
#[test]
fn test_a_step_the_distance_may_not_divide_has_no_equality_count() {
    for shape in ["pre", "post", "post-stepped"] {
        let parsed = tested_for_equality(shape, 2, 0);
        assert!(parsed.counted(false).iter().all(|one| one.test != IntPredicate::Ne), "{shape}");
    }
}

/// `for x in xs` after lsr, as the pipeline leaves it: `iv` from `-(len << 1)`
/// by 2, `iv.next != 0` after each trip. The loop makes `len` trips.
#[test]
fn test_the_loop_lsr_leaves_for_a_slice_makes_its_length_in_trips() {
    let parsed = Parsed::new(
        "define i16 @f(ptr %p, i16 %len) {
b1:
  %shifted = shl i16 %len, 1
  %start = sub i16 0, %shifted
  %empty = icmp ule i16 %len, 0
  br i1 %empty, label %b5, label %b3

b3:
  %sum = phi i16 [ %next_sum, %b3 ], [ 0, %b1 ]
  %iv = phi i16 [ %next, %b3 ], [ %start, %b1 ]
  %at = getelementptr i8, ptr %p, i16 %iv
  %v = load i16, ptr %at
  %next_sum = add i16 %sum, %v
  %next = add i16 %iv, 2
  %more = icmp ne i16 %next, 0
  br i1 %more, label %b3, label %b5

b5:
  %r = phi i16 [ 0, %b1 ], [ %next_sum, %b3 ]
  ret i16 %r
}
",
    );
    let proofs = parsed.counted(false);
    let [proof] = &proofs[..] else { panic!("one proof: {proofs:?}") };
    assert!(proof.posttested && proof.stepped && proof.test == IntPredicate::Ne);
    let len = parsed.value("len");
    for n in [1_i128, 2, 7, 1000, 32767] {
        let number = |one: &AffineOperand| match one {
            AffineOperand::Const(known) => known.n.clone(),
            AffineOperand::Value(value, _) if *value == len => BigInt::from(n),
            AffineOperand::Value(value, _) if *value == parsed.value("start") => {
                BigInt::from(masked(&BigInt::from(-2 * n), 16))
            }
            other => panic!("{other:?}"),
        };
        let mut computed = |kind: BinaryOp, args: Vec<AffineOperand>| {
            let (x, y) = (number(&args[0]), number(&args[1]));
            let result = match kind {
                BinaryOp::Add => x + y,
                BinaryOp::Sub => x - y,
                BinaryOp::And => x & y,
                BinaryOp::LShr => x >> usize::try_from(y).unwrap(),
                BinaryOp::Mul => x * y,
                other => panic!("{other:?}"),
            };
            AffineOperand::constant(result, 16)
        };
        let count = match trips(proof, &mut computed).expect("a count") {
            AffineOperand::Const(known) => known.n,
            other => panic!("{other:?}"),
        };
        assert_eq!(count, BigInt::from(n), "len {n}");
    }
}

/// Every loop of the optimized corpus in `LOOPS_CORPUS` (a directory of
/// `optimized/*.ll`), sorted by how far its trips are known: the table the
/// counting is measured by. `cargo test -- --ignored loop_count_table
/// --nocapture`.
#[test]
#[ignore = "a measurement, not a check: it reads a corpus directory"]
fn loop_count_table() {
    let Ok(root) = std::env::var("LOOPS_CORPUS") else { return };
    let mut paths: Vec<_> = std::fs::read_dir(std::path::Path::new(&root).join("optimized"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let (mut constant, mut symbolic, mut uncounted, mut no_counter, mut programs) = (0, 0, 0, 0, 0);
    for path in paths {
        let Ok(module) = llrm_mir::parse::module(&std::fs::read_to_string(&path).unwrap()) else { continue };
        programs += 1;
        let layout = layout(&module);
        for (_, global, function) in module.functions().filter(|(_, _, one)| one.entry().is_some()) {
            let gname = global.name.clone();
            let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
            for loop_ in unit.shape().loops.iter() {
                let proofs = counted_unless_stopped(&unit, loop_, None, false);
                let placed = |proof: &CountedLoop| {
                    proof.count.is_some() || trips(proof, &mut |_, args| args[0].clone()).is_some()
                };
                if proofs.iter().any(|proof| proof.count.is_some()) {
                    constant += 1;
                } else if proofs.iter().any(placed) {
                    symbolic += 1;
                } else if !basics(&unit, loop_).is_empty() || !pointers(&unit, loop_).is_empty() {
                    uncounted += 1;
                    if std::env::var("LOOPS_SHOW").is_ok() {
                        let counters = basics(&unit, loop_)
                            .values()
                            .map(|one| format!("{:?}+{:?}", one.start, one.step))
                            .collect::<Vec<_>>();
                        eprintln!(
                            "uncounted {} {}: {}",
                            path.file_stem().unwrap().to_string_lossy(),
                            gname.as_deref().unwrap_or("?"),
                            counters.join(" ")
                        );
                    }
                } else {
                    no_counter += 1;
                }
            }
        }
    }
    eprintln!(
        "loops in {programs} programs: constant {constant}, symbolic {symbolic}, counter but uncounted {uncounted}, no counter {no_counter}"
    );
}

/// An 8-bit counter from `%a` by `step` while `%i test %b`, its step promised
/// not to wrap, tested `shape` (pre or post-stepped behind a guard on the
/// entry); it returns its trips.
fn tested_for_order(
    shape: &str,
    test: IntPredicate,
    step: i64,
) -> Parsed {
    let flag = if matches!(
        test,
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
    ) {
        "nuw"
    } else {
        "nsw"
    };
    let word = spelled(test);
    let advance = if step > 0 { format!("add {flag} i8 %i, {step}") } else { format!("sub {flag} i8 %i, {}", -step) };
    let text = if shape == "pre" {
        format!(
            "define i8 @f(i8 %a, i8 %b) {{
b0:
  br label %b1

b1:
  %i = phi i8 [ %a, %b0 ], [ %next, %b2 ]
  %n = phi i8 [ 0, %b0 ], [ %n1, %b2 ]
  %c = icmp {word} i8 %i, %b
  br i1 %c, label %b2, label %b3

b2:
  %next = {advance}
  %n1 = add i8 %n, 1
  br label %b1

b3:
  ret i8 %n
}}
"
        )
    } else {
        format!(
            "define i8 @f(i8 %a, i8 %b) {{
b0:
  %g = icmp {word} i8 %a, %b
  br i1 %g, label %b8, label %b9

b8:
  br label %b1

b1:
  %i = phi i8 [ %a, %b8 ], [ %next, %b1 ]
  %n = phi i8 [ 0, %b8 ], [ %n1, %b1 ]
  %next = {advance}
  %n1 = add i8 %n, 1
  %c = icmp {word} i8 %next, %b
  br i1 %c, label %b1, label %b2

b2:
  ret i8 %n1

b9:
  ret i8 0
}}
"
        )
    };
    Parsed::new(&text)
}

/// A loop ordered against an invariant by a step of more than one, promised not
/// to wrap, was uncounted unless the bound was a number: its count is the
/// distance to the bound divided by the step and rounded up, on every input the
/// loop is entered with.
#[test]
fn test_an_ordered_loop_by_a_longer_step_is_counted_by_dividing_the_distance() {
    let mut checked = 0;
    for shape in ["pre", "post-stepped"] {
        for (test, steps) in [
            (IntPredicate::Ult, [2, 3, 5]),
            (IntPredicate::Ule, [2, 3, 5]),
            (IntPredicate::Slt, [2, 3, 5]),
            (IntPredicate::Sle, [2, 3, 5]),
            (IntPredicate::Ugt, [-2, -3, -5]),
            (IntPredicate::Uge, [-2, -3, -5]),
            (IntPredicate::Sgt, [-2, -3, -5]),
            (IntPredicate::Sge, [-2, -3, -5]),
        ] {
            for step in steps {
                let parsed = tested_for_order(shape, test, step);
                let proofs = parsed.counted(false);
                let proof = proofs
                    .iter()
                    .find(|one| one.test == test)
                    .unwrap_or_else(|| panic!("{shape} {test:?} step {step}: no proof"));
                for a in (0..256).step_by(3) {
                    for b in (0..256).step_by(5) {
                        // An input the loop is not entered with has no count to
                        // check; one that wraps is poison.
                        let Some(actual) = parsed.run(&[(a, 8), (b, 8)], 3_000).filter(|&actual| actual != 0) else {
                            continue;
                        };
                        let mut computed = |kind: BinaryOp, args: Vec<AffineOperand>| {
                            let number = |one: &AffineOperand| match one {
                                AffineOperand::Const(known) => known.n.clone(),
                                AffineOperand::Value(value, _) => {
                                    BigInt::from(match parsed.function().value(*value).name.as_deref() {
                                        Some("a") => a,
                                        Some("b") => b,
                                        other => panic!("{other:?}"),
                                    } as u128)
                                }
                            };
                            let (x, y) = (number(&args[0]), number(&args[1]));
                            AffineOperand::constant(
                                match kind {
                                    BinaryOp::Add => x + y,
                                    BinaryOp::Sub => x - y,
                                    BinaryOp::UDiv => x / y,
                                    other => panic!("{other:?}"),
                                },
                                8,
                            )
                        };
                        let AffineOperand::Const(counted) = trips(proof, &mut computed).expect("placed") else {
                            panic!("a number")
                        };
                        assert_eq!(counted.n, BigInt::from(actual), "{shape} {test:?} step {step} a {a} b {b}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 2_000, "{checked} runs");
}

/// Without the promise a longer step may jump the bound and wrap: no count.
#[test]
fn test_a_longer_step_that_may_wrap_has_no_ordered_count() {
    let text = tested_for_order("pre", IntPredicate::Ult, 3).function().clone();
    let _ = text;
    let parsed = Parsed::new(
        "define i8 @f(i8 %a, i8 %b) {
b0:
  br label %b1

b1:
  %i = phi i8 [ %a, %b0 ], [ %next, %b2 ]
  %c = icmp ult i8 %i, %b
  br i1 %c, label %b2, label %b3

b2:
  %next = add i8 %i, 3
  br label %b1

b3:
  ret i8 0
}
",
    );
    assert!(parsed.counted(false).is_empty());
}

/// `i = start; while i ule n { i = add FLAGS i, 1 }`, 8 bits.
fn climbing(
    start: &str,
    flags: &str,
) -> Parsed {
    Parsed::new(&format!(
        "define i8 @f(i8 %a, i8 %n) {{
b0:
  br label %b1

b1:
  %i = phi i8 [ {start}, %b0 ], [ %next, %b2 ]
  %c = icmp ule i8 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %next = add {flags} i8 %i, 1
  br label %b1

b3:
  ret i8 %i
}}
"
    ))
}

/// A counter that starts at or above zero and climbs without a signed wrap
/// cannot wrap unsigned either, whatever it is compared with: a signed FOR
/// counter from 0 by 1 is `nuw`, and an unsigned `<=` against it counts.
/// Without the inference an inclusive unsigned test of a counter that may wrap
/// past the maximum may never end, so no count.
#[test]
fn test_a_signed_counter_from_zero_climbing_is_also_unsigned_nowrap() {
    assert_eq!(climbing("0", "nsw").counted(false).len(), 1, "nsw from 0");
    assert_eq!(climbing("0", "nuw").counted(false).len(), 1, "nuw from 0 (as before)");
    assert_eq!(climbing("0", "").counted(false).len(), 0, "no promise");
    assert_eq!(climbing("%a", "nsw").counted(false).len(), 0, "nsw from a value that may be negative");
    assert_eq!(climbing("-1", "nsw").counted(false).len(), 0, "nsw from below zero");
}

/// `i < (n & 3)` from zero runs at most three trips: a bound's range bounds
/// the trips where no constant does. `window` priced such a loop at an
/// estimated ten trips and split it into windows that cost more than the
/// carries three trips save.
#[test]
fn test_a_bound_known_by_its_range_bounds_the_trips() {
    let parsed = Parsed::new(
        "define i16 @f(i16 %n) {
b0:
  %m = and i16 %n, 3
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, %m
  br i1 %c, label %b2, label %b3

b2:
  %next = add nsw i16 %i, 1
  br label %b1

b3:
  ret i16 %i
}
",
    );
    let maxima = parsed.counted(false).into_iter().map(|proof| proof.maximum).collect::<Vec<_>>();
    assert_eq!(maxima, vec![Some(BigInt::from(3))]);
}

/// quicksort's partition: `i` starts where the counter `j` does and goes
/// up by one on the ways that swap (`step`), by `by` there.
fn partition(
    by: u32,
    flags: &str,
) -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(i16 %lo, i16 %hi, i16 %pivot) {{
entry:
  br label %head
head:
  %j = phi i16 [ %lo, %entry ], [ %next, %join ]
  %i = phi i16 [ %lo, %entry ], [ %kept, %join ]
  %more = icmp slt i16 %j, %hi
  br i1 %more, label %body, label %done
body:
  %small = icmp slt i16 %j, %pivot
  br i1 %small, label %step, label %keep
step:
  %up = add i16 %i, {by}
  br label %join
keep:
  br label %join
join:
  %kept = phi i16 [ %up, %step ], [ %i, %keep ]
  %next = add {flags} i16 %j, 1
  br label %head
done:
  ret i16 %i
}}
"
    ))
}

/// The follower `i` of the counter `j`, which starts at `lo` too, stays
/// between them: the check `a[i]` after `a[j]` had was kept for it.
#[test]
fn test_a_phi_that_follows_the_counter_stays_between_its_start_and_it() {
    let parsed = partition(1, "nsw");
    let found = followers(&parsed.unit(), &parsed.only_loop());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].0, found[0].1), (parsed.value("i"), parsed.value("j")));
}

/// A counter that may wrap, or a follower that may pass it, proves no order.
#[test]
fn test_a_follower_that_may_pass_its_counter_proves_nothing() {
    let wrapping = partition(1, "");
    assert!(followers(&wrapping.unit(), &wrapping.only_loop()).is_empty(), "j + 1 may wrap");
    let fast = partition(2, "nsw");
    assert!(followers(&fast.unit(), &fast.only_loop()).is_empty(), "i + 2 may pass j + 1");
}

/// A loop tested after its trips, entered behind `a < b` through a block of its
/// own, makes `b - a` trips: the count a pre-tested one would, materialised
/// (`trips` gave none for a posttested loop whose step is one, so a copied loop
/// test lost its symbolic count).
#[test]
fn test_a_guarded_posttested_loop_of_unit_steps_has_its_trips() {
    let parsed = Parsed::new(
        "define i8 @f(i8 %a, i8 %b) {
b0:
  %go = icmp ult i8 %a, %b
  br i1 %go, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i8 [ %a, %pre ], [ %next, %b1 ]
  %n = phi i8 [ 0, %pre ], [ %n1, %b1 ]
  %next = add i8 %i, 1
  %n1 = add i8 %n, 1
  %c = icmp ult i8 %next, %b
  br i1 %c, label %b1, label %b2

b2:
  ret i8 %n1

b3:
  ret i8 0
}
",
    );
    let proofs = parsed.counted(false);
    let [proof] = &proofs[..] else { panic!("one proof") };
    assert!(proof.posttested && proof.entry_guarded, "{proof:?}");
    let mut checked = 0;
    for a in (0..256).step_by(3) {
        for b in (0..256).step_by(5) {
            if a >= b {
                continue;
            }
            let actual = parsed.run(&[(a, 8), (b, 8)], 3_000).expect("it ends");
            let counted = evaluated_trips(&parsed, proof, a as u128, b as u128, 0).expect("trips are placed");
            assert_eq!(counted & 0xFF, actual & 0xFF, "a {a} b {b}");
            checked += 1;
        }
    }
    assert!(checked > 500, "{checked} runs");
}

/// A loop whose latch tests the counter before its step, `do { .. } while (i >=
/// 1)` with `i - 1` the step, entered behind a guard, is the counted loop `i -
/// 1 >= 0` is: one proof, the stepped form with the bound a step on. Unread,
/// the loop had none, and LSR kept both the counter and a pointer where the
/// stepped form ends on the pointer's flags (x_insertion's inner loop, `dec;
/// jl` beside `add; je`).
#[test]
fn test_a_latch_test_of_the_counter_before_its_step_is_the_stepped_test_a_step_on() {
    let parsed = Parsed::new(
        "define i32 @f(i32 %n) {
b0:
  %g = icmp slt i32 %n, 0
  br i1 %g, label %b4, label %b1

b1:
  br label %b2

b2:
  %i = phi i32 [ %n, %b1 ], [ %next, %b2 ]
  %trips = phi i32 [ 0, %b1 ], [ %up, %b2 ]
  %up = add i32 %trips, 1
  %next = sub nsw i32 %i, 1
  %c = icmp sge i32 %i, 1
  br i1 %c, label %b2, label %b3

b3:
  ret i32 %up

b4:
  ret i32 0
}
",
    );
    let proofs = parsed.counted(false);
    let [proof] = &proofs[..] else { panic!("{proofs:?}") };
    assert!(proof.posttested && proof.stepped && proof.shifted && proof.entry_guarded, "{proof:?}");
    assert_eq!(proof.bound, constant(0, 32));
    // n + 1 trips for the guarded n >= 0.
    for n in [0, 1, 5] {
        assert_eq!(parsed.run(&[(n, 32)], 10_000), Some(n as u128 + 1), "{n}");
    }
}

/// A loop that starts at `x - 1` behind a guard `x != 1` starts off its bound:
/// `a != 1` proves `a - 1 != 0` (LLVM's isKnownPredicate compares the sides'
/// difference). Unproven, hanoi's nest (`-ftree-ch`) had eight loops no pass
/// could count, each holding its counter and the step beside it.
#[test]
fn test_a_loop_started_a_step_off_its_guard_s_value_is_entered() {
    let parsed = Parsed::new(
        "define i32 @f(i32 %x) {
b0:
  %g = icmp eq i32 %x, 1
  br i1 %g, label %b4, label %b1

b1:
  %start = sub nsw i32 %x, 1
  br label %b2

b2:
  %i = phi i32 [ %start, %b1 ], [ %next, %b2 ]
  %trips = phi i32 [ 0, %b1 ], [ %up, %b2 ]
  %up = add i32 %trips, 1
  %next = sub nsw i32 %i, 1
  %c = icmp eq i32 %i, 1
  br i1 %c, label %b3, label %b2

b3:
  ret i32 %up

b4:
  ret i32 0
}
",
    );
    let proofs = parsed.counted(false);
    let [proof] = &proofs[..] else { panic!("{proofs:?}") };
    assert!(proof.shifted && proof.entry_guarded, "{proof:?}");
    // x - 1 trips for x > 1.
    for x in [2, 3, 9] {
        assert_eq!(parsed.run(&[(x, 32)], 10_000), Some(x as u128 - 1), "{x}");
    }
}
