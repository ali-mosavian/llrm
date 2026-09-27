//! Port of `tests/test_floatfacts.py`, and the facts MIR functions answer.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Module, ValueId};
use num_bigint::BigInt;

use super::*;
use crate::testing::{DOS, function, layout, parsed, value};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{body}");
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, function(&self.module, "f"))
    }

    fn value(&self, name: &str) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    /// What `known` says of `%name`.
    fn fact(&self, name: &str) -> Option<Finite> {
        known(&self.unit(), &Calls::default(), None).get(&self.value(name)).cloned()
    }
}

fn finite(value: Fraction) -> Finite {
    Finite::new(value, false)
}

fn integer(n: i64) -> Finite {
    finite(Fraction::from_integer(n))
}

#[test]
fn test_unary_facts_preserve_negation_absolute_value_and_zero_sign() {
    for (operation, number, negative_zero, expected, expected_negative_zero) in [
        (Operation::Neg, 2, false, -2, false),
        (Operation::Abs, 2, false, 2, false),
        (Operation::Neg, -2, false, 2, false),
        (Operation::Abs, -2, false, 2, false),
        (Operation::Neg, 0, false, 0, true),
        (Operation::Neg, 0, true, 0, false),
        (Operation::Abs, 0, true, 0, false),
    ] {
        let rule = Rule::new(operation, &[Format::Binary64], Format::Binary64);
        let fact = evaluated(&rule, &[Finite::new(Fraction::from_integer(number), negative_zero)]);
        assert_eq!(fact, Some(Finite::new(Fraction::from_integer(expected), expected_negative_zero)));
    }
}

#[test]
fn test_single_bit_patterns_decode_without_host_float() {
    for (bits, expected, negative_zero) in [
        (0x4000_0000_u32, Fraction::from_integer(2), false),
        (0x4080_0000, Fraction::from_integer(4), false),
        (0x3f40_0000, Fraction::new(3, 4), false),
        (0xc040_0000, Fraction::from_integer(-3), false),
        (0, Fraction::from_integer(0), false),
        (0x8000_0000, Fraction::from_integer(0), true),
    ] {
        let fact = decoded(&BigInt::from(bits), Format::Binary32).unwrap();
        assert!(fact.value == expected && fact.negative_zero == negative_zero);
        assert_eq!(encoded(&fact, Format::Binary32), Some(BigInt::from(bits)));
    }
}

#[test]
fn test_subnormals_infinities_and_nans_are_not_exception_free_inputs() {
    for bits in [1_u32, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0x7f80_0001] {
        assert_eq!(decoded(&BigInt::from(bits), Format::Binary32), None);
    }
}

/// Old `Precision::Dynamic` was 24 bits, a `float`'s.
#[test]
fn test_exact_arithmetic_respects_precision_and_zero_sign() {
    for (operation, left, right, expected) in [
        (Operation::Add, 2, 4, Some(Fraction::from_integer(6))),
        (Operation::Mul, 6, 8, Some(Fraction::from_integer(48))),
        (Operation::Div, 6, 8, Some(Fraction::new(3, 4))),
        (Operation::Div, 1, 3, None),
        (Operation::Div, 1, 0, None),
        (Operation::Add, 1 << 24, 1, None),
        (Operation::Sub, 1, 1, None),
    ] {
        let rule = Rule::new(operation, &[Format::Binary32, Format::Binary32], Format::Binary32);
        let result = evaluated(&rule, &[integer(left), integer(right)]);
        assert_eq!(result.map(|result| result.value), expected, "{operation:?} {left} {right}");
    }
}

#[test]
fn test_nsz_lets_a_cancellation_be_positive_zero() {
    let rule = Rule { nsz: true, ..Rule::new(Operation::Sub, &[Format::Binary64; 2], Format::Binary64) };
    assert_eq!(evaluated(&rule, &[integer(1), integer(1)]), Some(integer(0)));
}

/// Zeros of one sign add to that sign in every rounding mode.
#[test]
fn test_zeros_of_one_sign_keep_it() {
    let rule = Rule::new(Operation::Add, &[Format::Binary64; 2], Format::Binary64);
    let negative = Finite::new(Fraction::from_integer(0), true);
    assert_eq!(evaluated(&rule, &[negative.clone(), negative.clone()]), Some(negative.clone()));
    assert_eq!(evaluated(&rule, &[negative, integer(0)]), None);
}

#[test]
fn test_single_store_does_not_keep_a_wider_intermediate() {
    let rule = Rule::new(Operation::Convert, &[Format::Binary64], Format::Binary32);
    assert_eq!(evaluated(&rule, &[integer((1 << 24) + 1)]), None);
    assert_eq!(evaluated(&rule, &[integer(1 << 24)]), Some(integer(1 << 24)));
}

#[test]
fn test_sqrt_facts_require_an_exact_rational_square() {
    let rule = Rule::new(Operation::Sqrt, &[Format::Binary64], Format::Binary64);
    for (number, negative_zero, expected) in [
        (Fraction::from_integer(1_048_576), false, Some(integer(1024))),
        (Fraction::new(9, 16), false, Some(finite(Fraction::new(3, 4)))),
        (Fraction::from_integer(0), true, Some(Finite::new(Fraction::from_integer(0), true))),
        (Fraction::from_integer(2), false, None),
        (Fraction::from_integer(-1), false, None),
    ] {
        assert_eq!(evaluated(&rule, &[Finite::new(number, negative_zero)]), expected);
    }
}

#[test]
fn test_integer_results_must_be_in_range() {
    let truncate = |result| Rule::new(Operation::Truncate, &[Format::Binary64], result);
    assert_eq!(evaluated(&truncate(Format::Signed(16)), &[finite(Fraction::new(-5, 2))]), Some(integer(-2)));
    assert_eq!(evaluated(&truncate(Format::Signed(16)), &[integer(32768)]), None);
    assert_eq!(evaluated(&truncate(Format::Unsigned(16)), &[integer(-1)]), None);
    assert_eq!(evaluated(&truncate(Format::Unsigned(16)), &[finite(Fraction::new(-1, 2))]), Some(integer(0)));
    // Rounding is the environment's: only an integral value converts.
    let rounded = Rule::new(Operation::Convert, &[Format::Binary64], Format::Signed(32));
    assert_eq!(evaluated(&rounded, &[finite(Fraction::new(5, 2))]), None);
    assert_eq!(evaluated(&rounded, &[integer(-7)]), Some(integer(-7)));
}

/// FPCSE's 2+4, product 48 and quotient 0.75, from memory and constants.
#[test]
fn test_known_inputs_reach_float_computations() {
    let parsed = Parsed::new(
        "@m = global [12 x i8] zeroinitializer

define double @f() {
b0:
  store float 2.0, ptr @m
  store double 4.0, ptr getelementptr (i8, ptr @m, i16 4)
  %a = load float, ptr @m
  %b = load double, ptr getelementptr (i8, ptr @m, i16 4)
  %wide = fpext float %a to double
  %sum = fadd double %wide, %b
  %product = fmul double %sum, 8.0
  %quotient = fdiv double %sum, 8.0
  %third = fdiv double 1.0, 3.0
  %inf = fdiv double 1.0, 0.0
  %zero = fsub double %b, %b
  %signless = fsub nsz double %b, %b
  %negated = fneg double 0.0
  %i = sitofp i16 -3 to float
  %root = call double @llvm.sqrt.f64(double 2.25e+00)
  ret double %sum
}

declare double @llvm.sqrt.f64(double)
",
    );
    assert_eq!(parsed.fact("wide"), Some(integer(2)));
    assert_eq!(parsed.fact("sum"), Some(integer(6)));
    assert_eq!(parsed.fact("product"), Some(integer(48)));
    assert_eq!(parsed.fact("quotient"), Some(finite(Fraction::new(3, 4))));
    assert_eq!(parsed.fact("third"), None);
    assert_eq!(parsed.fact("inf"), None);
    assert_eq!(parsed.fact("zero"), None);
    assert_eq!(parsed.fact("signless"), Some(integer(0)));
    assert_eq!(parsed.fact("negated"), Some(Finite::new(Fraction::from_integer(0), true)));
    assert_eq!(parsed.fact("i"), Some(integer(-3)));
    assert_eq!(parsed.fact("root"), Some(finite(Fraction::new(3, 2))));
}

#[test]
fn test_unknown_or_volatile_inputs_are_no_fact() {
    let parsed = Parsed::new(
        "@g = global double 0.0

define void @f(double %x) {
b0:
  store double 1.0, ptr @g
  %read = load volatile double, ptr @g
  %sum = fadd double %x, 1.0
  %next = fadd double %read, 1.0
  ret void
}
",
    );
    assert_eq!(parsed.fact("read"), None);
    assert_eq!(parsed.fact("sum"), None);
    assert_eq!(parsed.fact("next"), None);
}

#[test]
fn test_a_join_is_known_where_every_path_agrees() {
    let parsed = Parsed::new(
        "define void @f(i1 %c) {
b0:
  %one = fadd double 0.5, 0.5
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %same = phi double [ 1.0, %b1 ], [ %one, %b2 ]
  %differ = phi double [ 1.0, %b1 ], [ 2.0, %b2 ]
  ret void
}
",
    );
    assert_eq!(parsed.fact("same"), Some(integer(1)));
    assert_eq!(parsed.fact("differ"), None);
}

/// A computed float store's bits are memory an integer load reads.
#[test]
fn test_an_exact_store_writes_its_bits() {
    let parsed = Parsed::new(
        "@g = global float 0.0

define void @f() {
b0:
  %x = fmul float 1.5, 4.0
  store float %x, ptr @g
  %bits = load i32, ptr @g
  %back = load float, ptr @g
  ret void
}
",
    );
    let unit = parsed.unit();
    let memory = cells(&unit, &Calls::default());
    let load = function(&parsed.module, "f").walk().map(|(_, inst)| inst).find(|&inst| unit.function.instruction(inst).result == Some(parsed.value("bits"))).unwrap();
    let reference = MemRef::of(&unit, load).unwrap();
    assert_eq!(consts::_cell(&memory[&load], &reference), Some(Known::new(0x40c0_0000, 32)));
    assert_eq!(parsed.fact("back"), Some(integer(6)));
}

#[test]
fn test_conversions_answer_their_integers() {
    let parsed = Parsed::new(
        "define void @f() {
b0:
  %x = fdiv double 7.0, 2.0
  %t = fptosi double %x to i16
  %u = fptoui double %x to i8
  %negative = fneg double %x
  %wrapped = fptoui double %negative to i8
  %r = call i32 @llvm.lrint.i32.f64(double %x)
  %whole = call i32 @llvm.lrint.i32.f64(double 4.0)
  ret void
}

declare i32 @llvm.lrint.i32.f64(double)
",
    );
    let found = converted(&parsed.unit(), &Calls::default(), None);
    assert_eq!(found.get(&parsed.value("t")), Some(&Known::new(3, 16)));
    assert_eq!(found.get(&parsed.value("u")), Some(&Known::new(3, 8)));
    assert_eq!(found.get(&parsed.value("wrapped")), None);
    assert_eq!(found.get(&parsed.value("r")), None);
    assert_eq!(found.get(&parsed.value("whole")), Some(&Known::new(4, 32)));
}

/// Its three cells are one global's: consts keeps no cell of one global
/// across a store to another.
const LOOP: &str = "@m = global [12 x i8] zeroinitializer

define void @f() {
b0:
  store float 0.0, ptr @m
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %c = icmp slt i16 %i, 10
  br i1 %c, label %b2, label %b3

b2:
  %v = load float, ptr @m
  %w = fadd float %v, STEP
  store float %w, ptr @m
  store float 48.0, ptr getelementptr (i8, ptr @m, i16 4)
  %d = fdiv float 3.0, 4.0
  store float %d, ptr getelementptr (i8, ptr @m, i16 8)
  %n = add i16 %i, 1
  br label %b1

b3:
  %after = load i32, ptr @m
  ret void
}
";

/// `_MemoryQueries` keyed a reference by its address, and `repeated` built a
/// fresh store every iteration, so a freed store's address answered for the
/// next one: fpcse's D exited at 145.5 (three iterations) for 487.5.
#[test]
fn test_a_loop_exit_repeats_its_stores_every_iteration() {
    let parsed = Parsed::new(&LOOP.replace("STEP", "48.75"));
    let unit = parsed.unit();
    let exits = loop_exits(&unit, &Calls::default());
    assert_eq!(exits.len(), 1);
    assert_eq!(exits[0].count, BigInt::from(10));
    let stored = exits[0].stores.iter().map(|(_, fact)| fact.n.clone()).collect::<Vec<_>>();
    assert_eq!(stored, [0x43f3_c000, 0x4240_0000, 0x3f40_0000].map(BigInt::from));
    let edges = exit_cells(&unit, &Calls::default());
    let after = consts::known(&unit, Some(&Calls::default()), Some(&edges), None);
    assert_eq!(after.get(&parsed.value("after")), Some(&Known::new(0x43f3_c000, 32)));
}

/// A trip whose result no float holds proves no exit.
#[test]
fn test_an_inexact_trip_proves_no_exit() {
    let parsed = Parsed::new(&LOOP.replace("fadd float %v, STEP", "fdiv float 1.0, 3.0"));
    assert_eq!(loop_exits(&parsed.unit(), &Calls::default()), Vec::new());
    // 0.1f is a float, but three of them need 26 bits.
    let parsed = Parsed::new(&LOOP.replace("STEP", "0x3FB99999A0000000"));
    assert_eq!(loop_exits(&parsed.unit(), &Calls::default()), Vec::new());
}
