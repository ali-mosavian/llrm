//! Port of `tests/test_floatbounds.py`, and the bounds MIR functions answer.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

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
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }

    /// The instruction defining `%name`.
    fn made(
        &self,
        name: &str,
    ) -> InstId {
        let f = function(&self.module, "f");
        let wanted = value(f, name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    }

    /// Whether `exact` proves `%name`'s instruction.
    fn exact(
        &self,
        name: &str,
    ) -> bool {
        let unit = self.unit();
        // The manager's bounds of the body, as the analysis that asks carries
        // them.
        let bounds = ranges::bounds(&unit).unwrap();
        let unit = unit.with_bounds(&bounds);
        exact(&unit, &floatfacts::known(&unit, &Calls::default(), None)).unwrap().contains(&self.made(name))
    }

    /// `_memory` of the load `%name`, its block's scope as `ranges::bounded`
    /// says.
    fn loaded(
        &self,
        name: &str,
    ) -> Option<Bounds> {
        let unit = self.unit();
        let load = self.made(name);
        let block = cfg::id(unit.function.parent(load).unwrap());
        let memory = floatfacts::cells(&unit, &Calls::default());
        let scoped = ranges::bounded(&unit).unwrap();
        _memory(&unit, load, Format::Binary32, &memory[&load], scoped.get(&block).unwrap_or(&crate::ranges::Intervals::default()))
    }
}

fn pair(
    low: i64,
    high: i64,
) -> Bounds {
    (BigInt::from(low), BigInt::from(high))
}

/// Old `Precision::Dynamic` was 24 bits, a `float`'s.
#[test]
fn test_arithmetic_requires_exactness_at_its_precision() {
    for (operation, bounds, expected) in [
        (Operation::Add, [(-32768, 32767), (-32768, 32767)], Some((-65536, 65534))),
        (Operation::Mul, [(-32768, 32767), (-32768, 32767)], None),
        (Operation::Mul, [(-100, 100), (-100, 100)], Some((-10000, 10000))),
        (Operation::Sub, [(-10, 10), (-10, 10)], Some((-20, 20))),
        (Operation::Div, [(1, 3), (1, 3)], None),
        (Operation::Add, [(1 << 24, 1 << 24), (1, 1)], None),
    ] {
        let rule = Rule::new(operation, &[Format::Binary32; 2], Format::Binary32);
        let inputs = bounds.map(|(low, high)| pair(low, high));
        assert_eq!(evaluated(&rule, &inputs), expected.map(|(low, high)| pair(low, high)), "{operation:?} {bounds:?}");
    }
    let wide = Rule::new(Operation::Mul, &[Format::Binary64; 2], Format::Binary64);
    assert_eq!(
        evaluated(&wide, &[pair(-32768, 32767), pair(-32768, 32767)]),
        Some(pair(-32768 * 32767, 32768 * 32768))
    );
}

#[test]
fn test_integer_inputs_bound_their_float_work() {
    let parsed = Parsed::new(
        "define void @f(i16 %a, i16 %b, i1 %c) {
b0:
  %x = sitofp i16 %a to float
  %y = sitofp i16 %b to float
  %sum = fadd float %x, %y
  %product = fmul float %x, %y
  %wide = sitofp i16 %a to double
  %exact = fmul double %wide, %wide
  %half = fmul float %x, 0.5
  %quotient = fdiv float %x, %y
  %back = fptosi float %sum to i32
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %joined = phi float [ %x, %b0 ], [ 1.0e+06, %b1 ]
  %big = phi float [ %x, %b0 ], [ 1.6777216e+07, %b1 ]
  %late = fadd float %joined, 1.0
  %over = fadd float %big, 1.0
  ret void
}
",
    );
    for (name, expected) in [
        ("x", true),
        ("sum", true),
        ("product", false),
        ("exact", true),
        ("half", false),
        ("quotient", false),
        ("back", true),
        ("late", true),
        ("over", false),
    ] {
        assert_eq!(parsed.exact(name), expected, "{name}");
    }
}

const ARRAY: &str = "@a = global [3 x float] zeroinitializer

define float @f(i16 %i) {
b0:
  store float 12.0, ptr @a
  store float MIDDLE, ptr getelementptr (i8, ptr @a, i16 4)
  store float 60.0, ptr getelementptr (i8, ptr @a, i16 8)
  %low = icmp sge i16 %i, 0
  br i1 %low, label %b1, label %b3

b1:
  %high = icmp sle i16 %i, HIGH
  br i1 %high, label %b2, label %b3

b2:
  %p = getelementptr float, ptr @a, i16 %i
  %x = load float, ptr %p
  %y = fmul float %x, 2.0
  ret float %y

b3:
  ret float 0.0
}
";

/// FPDEEP loaded p(i) five times despite its three initialized finite
/// elements: every element an index may reach bounds the load.
#[test]
fn test_finite_array_proof_requires_known_nonwrapping_bytes() {
    let array = |middle: &str, high: &str| Parsed::new(&ARRAY.replace("MIDDLE", middle).replace("HIGH", high));
    let parsed = array("36.0", "2");
    assert_eq!(parsed.loaded("x"), Some(pair(12, 60)));
    assert!(parsed.exact("x") && parsed.exact("y"));
    // Past the array, no byte is known.
    assert_eq!(array("36.0", "3").loaded("x"), None);
    // Not a store of the element: nothing known.
    let parsed =
        Parsed::new(&ARRAY.replace("MIDDLE", "36.0").replace("HIGH", "2").replace("  store float 12.0, ptr @a\n", ""));
    assert_eq!(parsed.loaded("x"), None);
    // An index no guard bounds.
    let parsed = Parsed::new(
        &ARRAY
            .replace("MIDDLE", "36.0")
            .replace("HIGH", "2")
            .replace("br i1 %high, label %b2, label %b3", "br label %b2"),
    );
    assert_eq!(parsed.loaded("x"), None);
}

/// An infinite, NaN, denormal or fractional p(1) is not a finite-integer proof.
#[test]
fn test_array_reuse_requires_every_element_to_have_proven_integer_bounds() {
    for middle in ["0x7FF0000000000000", "0x7FF8000000000000", "0x36A0000000000000", "0.5"] {
        let parsed = Parsed::new(&ARRAY.replace("MIDDLE", middle).replace("HIGH", "2"));
        assert_eq!(parsed.loaded("x"), None, "{middle}");
        assert!(!parsed.exact("x"), "{middle}");
    }
}
