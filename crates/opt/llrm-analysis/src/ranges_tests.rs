//! Ports of `tests/test_ranges.py`, `tests/test_edge_ranges.py` and
//! `tests/test_unsigned_edge_ranges.py`.
//! `test_range_alias_checks_cover_width_and_wrap` is `regions`'
//! `overlapping_range_covering_respects_width_wrap_and_each_fact_map`.

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, ValueId};
use llrm_support::hash::IndexMap;

use super::{_computed, _recurrence_span, Interval, covering, dominated_edges, edges_solved, on_edge, singletons};
use crate::cfg;
use crate::memory::{MemRef, Unit};
use crate::num::Num as BigInt;
use crate::ranges::Intervals;
use crate::testing::{DOS, block, function, layout, parsed, value};

fn interval(
    low: i64,
    high: i64,
    width: u32,
) -> Interval {
    Interval { low: low.into(), high: high.into(), width }
}

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

    fn unit(&self) -> Unit<'_> {
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }

    fn value(
        &self,
        name: &str,
    ) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn block(
        &self,
        name: &str,
    ) -> llrm_mir::module::BlockId {
        block(function(&self.module, "f"), name)
    }

    /// The instruction defining `%name`.
    fn made(
        &self,
        name: &str,
    ) -> InstId {
        let f = function(&self.module, "f");
        let wanted = self.value(name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    }
}

/// `%c = icmp <predicate> i<width> %x, <bound>` branching to `%yes` or `%no`.
fn compare(
    predicate: &str,
    width: u32,
    bound: i64,
) -> Parsed {
    Parsed::new(&format!(
        "define void @f(i{width} %x) {{
b0:
  %c = icmp {predicate} i{width} %x, {bound}
  br i1 %c, label %yes, label %no

yes:
  ret void

no:
  ret void
}}
"
    ))
}

#[test]
fn test_signed_comparison_edges() {
    for (predicate, taken, (low, high)) in [
        ("slt", true, (0, 3)),
        ("slt", false, (4, 9)),
        ("sle", true, (0, 4)),
        ("sgt", true, (5, 9)),
        ("sge", true, (4, 9)),
        ("eq", true, (4, 4)),
        ("ne", false, (4, 4)),
        ("ne", true, (0, 9)),
    ] {
        let parsed = compare(predicate, 16, 4);
        let x = parsed.value("x");
        let known = Intervals::from_iter([(x, interval(0, 9, 16))]);
        let successor = parsed.block(if taken { "yes" } else { "no" });

        let result = on_edge(&parsed.unit(), parsed.block("b0"), successor, &known, None).unwrap().unwrap();

        assert_eq!(result[&x], interval(low, high, 16), "{predicate} {taken}");
    }
}

#[test]
fn an_edge_the_facts_rule_out_is_impossible() {
    let parsed = compare("slt", 16, 4);
    let known = Intervals::from_iter([(parsed.value("x"), interval(5, 9, 16))]);
    assert_eq!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("yes"), &known, None).unwrap(), None);
    assert!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("no"), &known, None).unwrap().is_some());
}

#[test]
fn an_edge_to_a_block_that_is_no_successor_is_refused() {
    let parsed = compare("slt", 16, 4);
    assert!(on_edge(&parsed.unit(), parsed.block("yes"), parsed.block("no"), &Intervals::default(), None).is_err());
}

#[test]
fn an_unconditional_edge_says_nothing_new() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  br label %b1

b1:
  ret void
}
",
    );
    let known = Intervals::from_iter([(parsed.value("x"), interval(0, 9, 16))]);
    assert_eq!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("b1"), &known, None).unwrap(), Some(known));
}

/// Only counted loops were scoped, so `x < 8` under `x < 8` in
/// straight-line code kept both branches.
#[test]
fn test_a_compare_under_the_same_compare_is_decided_outside_any_loop() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 8
  br i1 %c, label %b1, label %b3

b1:
  %d = icmp slt i16 %x, 8
  br i1 %d, label %b2, label %b4

b2:
  ret void

b3:
  ret void

b4:
  ret void
}
",
    );
    let unit = parsed.unit();
    let scoped = dominated_edges(&unit).unwrap();
    let inside = &scoped[&cfg::id(parsed.block("b1"))];
    assert_eq!(on_edge(&unit, parsed.block("b1"), parsed.block("b4"), inside, None).unwrap(), None);
}

#[test]
fn a_guard_does_not_bound_a_join_another_path_reaches() {
    let parsed = Parsed::new(
        "define void @f(i16 %x, i1 %p) {
b0:
  br i1 %p, label %check, label %join

check:
  %c = icmp slt i16 %x, 8
  br i1 %c, label %join, label %out

join:
  %y = add i16 %x, 1
  ret void

out:
  ret void
}
",
    );
    let scoped = dominated_edges(&parsed.unit()).unwrap();
    assert!(scoped.get(&cfg::id(parsed.block("join"))).is_none_or(|facts| !facts.contains_key(&parsed.value("x"))));
}

/// `tests/test_edge_ranges.py`'s guard outside a loop: `0 <= i < 4`
/// bounds a word-array offset to 0..6.
#[test]
fn a_guarded_block_bounds_what_it_computes_from_the_guarded_value() {
    let parsed = Parsed::new(
        "define void @f(i16 %x) {
b0:
  %c = icmp slt i16 %x, 4
  br i1 %c, label %b1, label %b2

b1:
  %lo = icmp sge i16 %x, 0
  br i1 %lo, label %b3, label %b2

b3:
  %y = mul i16 %x, 2
  ret void

b2:
  ret void
}
",
    );
    let scoped = dominated_edges(&parsed.unit()).unwrap();
    let facts = &scoped[&cfg::id(parsed.block("b3"))];
    assert_eq!(facts[&parsed.value("x")], interval(0, 3, 16));
    assert_eq!(facts[&parsed.value("y")], interval(0, 6, 16));
    assert!(!scoped.contains_key(&cfg::id(parsed.block("b2"))));
}

fn computed(
    body: &str,
    name: &str,
    known: &[(&str, Interval)],
) -> Option<Interval> {
    let parsed = Parsed::new(body);
    let known = known.iter().map(|(name, one)| (parsed.value(name), one.clone())).collect::<Intervals>();
    _computed(&parsed.unit(), parsed.made(name), &known, &IndexMap::default())
}

fn unary(
    operation: &str,
    width: u32,
) -> String {
    format!(
        "define void @f(i16 %x) {{
b0:
  %y = {operation}
  ret void
}}
"
    )
    .replace("WIDTH", &width.to_string())
}

#[test]
fn test_unit_steps_require_nonwrapping_intervals() {
    for (operation, low, high, expected) in [
        ("sub i16 %x, 1", 1, 3, Some(interval(0, 2, 16))),
        ("add i16 %x, 1", -3, -1, Some(interval(-2, 0, 16))),
        ("sub i16 %x, 1", -32768, 0, None),
        ("add i16 %x, 1", 0, 32767, None),
    ] {
        assert_eq!(computed(&unary(operation, 16), "y", &[("x", interval(low, high, 16))]), expected, "{operation}");
    }
}

/// ADDRM's bounded 1..20 counter lost its interval when converted to a long.
#[test]
fn test_signed_widening_keeps_the_numeric_range() {
    for (low, high) in [(1, 20), (-32768, -1), (-10, 10)] {
        assert_eq!(
            computed(&unary("sext i16 %x to i32", 32), "y", &[("x", interval(low, high, 16))]),
            Some(interval(low, high, 32))
        );
    }
}

/// nbody's `x[i]` index was `zext i16 %i to i32` with `%i` in 0..3: no interval
/// came out, `covering` refused the access, and the hoist left x[i] and y[i]
/// loaded on every trip of the inner loop.
#[test]
fn test_zero_extending_a_non_negative_interval_keeps_the_numeric_range() {
    for (low, high) in [(0, 3), (1, 20), (0, 32767)] {
        assert_eq!(
            computed(&unary("zext i16 %x to i32", 32), "y", &[("x", interval(low, high, 16))]),
            Some(interval(low, high, 32))
        );
    }
}

#[test]
fn a_zero_extension_is_not_taken_for_a_sign_extension() {
    assert_eq!(computed(&unary("zext i16 %x to i32", 32), "y", &[("x", interval(-1, 1, 16))]), None);
}

/// `x AND 511` had no interval unless `x` had one, so a masked subscript was
/// never known to fit.
#[test]
fn test_a_nonnegative_mask_bounds_an_unknown_operand() {
    assert_eq!(computed(&unary("and i16 %x, 511", 16), "y", &[]), Some(interval(0, 511, 16)));
    assert_eq!(computed(&unary("and i16 %x, -2", 16), "y", &[]), None);
}

#[test]
fn test_secondary_recurrence_bounds_reject_wrap() {
    for (start, step, advances, expected) in [
        (0, 4, 5, Some(interval(0, 20, 16))),
        (20, -4, 5, Some(interval(0, 20, 16))),
        (7, 0, 5, Some(interval(7, 7, 16))),
        (32760, 4, 1, None),
        (-32760, -4, 2, None),
        (0, 16384, 4, None),
        (0, 4, -1, None),
    ] {
        let found = _recurrence_span(&BigInt::from(start), &BigInt::from(step), &BigInt::from(advances), 16);
        assert_eq!(found, expected, "{start} {step} {advances}");
    }
}

#[test]
fn test_shift_ranges_refuse_wraparound() {
    for (low, high, count, expected) in [
        (0, 5, 2, Some(interval(0, 20, 16))),
        (-5, -1, 2, Some(interval(-20, -4, 16))),
        (0, 16384, 1, None),
        (-32768, -1, 1, None),
        (0, 5, 32, None),
    ] {
        let operation = format!("shl i16 %x, {count}");
        assert_eq!(
            computed(&unary(&operation, 16), "y", &[("x", interval(low, high, 16))]),
            expected,
            "{low} {high} {count}"
        );
    }
}

#[test]
fn a_sum_of_intervals_that_could_wrap_has_none() {
    let body = "define void @f(i16 %x, i16 %z) {
b0:
  %y = add i16 %x, %z
  ret void
}
";
    assert_eq!(computed(body, "y", &[("x", interval(0, 20000, 16)), ("z", interval(0, 20000, 16))]), None);
    assert_eq!(
        computed(body, "y", &[("x", interval(0, 10, 16)), ("z", interval(-5, 5, 16))]),
        Some(interval(-5, 15, 16))
    );
    assert_eq!(computed(body, "y", &[("x", interval(0, 10, 16))]), None);
}

#[test]
fn test_unsigned_edge_never_removes_a_possible_selector() {
    for predicate in ["ugt", "uge", "ult", "ule"] {
        for span in [(1, 3), (255, 255), (256, 260), (-3, -1), (-2, 2)] {
            for taken in [true, false] {
                for width in [16, 32] {
                    let parsed = compare(predicate, width, 255);
                    let known = Intervals::from_iter([(parsed.value("x"), interval(span.0, span.1, width))]);
                    let answer = |value: i64| match predicate {
                        "ugt" => value > 255,
                        "uge" => value >= 255,
                        "ult" => value < 255,
                        _ => value <= 255,
                    };
                    let possible = (span.0..=span.1).any(|number| answer(number & ((1_i64 << width) - 1)) == taken);
                    let successor = parsed.block(if taken { "yes" } else { "no" });
                    let result = on_edge(&parsed.unit(), parsed.block("b0"), successor, &known, None).unwrap();
                    assert_eq!(result.is_some(), possible, "{predicate} {span:?} {taken} {width}");
                }
            }
        }
    }
}

#[test]
fn a_phi_of_one_constant_is_that_singleton_and_of_two_is_none() {
    let parsed = Parsed::new(
        "define void @f(i1 %p) {
b0:
  br i1 %p, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %same = phi i16 [ 5, %b1 ], [ 5, %b2 ]
  %either = phi i16 [ 5, %b1 ], [ 6, %b2 ]
  %twice = shl i16 %same, 1
  ret void
}
",
    );
    let known = singletons(&parsed.unit());
    assert_eq!(known.get(&parsed.value("same")), Some(&interval(5, 5, 16)));
    assert_eq!(known.get(&parsed.value("twice")), Some(&interval(10, 10, 16)));
    assert_eq!(known.get(&parsed.value("either")), None);
}

#[test]
fn covering_turns_a_bounded_index_into_the_bytes_it_reaches_and_refuses_a_wrap() {
    let parsed = Parsed::new(
        "@g = global [64 x i16] zeroinitializer

define void @f(i16 %i) {
b0:
  %e = getelementptr i16, ptr @g, i16 %i
  %a = getelementptr i8, ptr %e, i16 4
  store i16 0, ptr %a
  ret void
}
",
    );
    let unit = parsed.unit();
    let f = function(&parsed.module, "f");
    let store = f.walk().map(|(_, inst)| inst).find_map(|inst| MemRef::of(&unit, inst)).unwrap();
    let i = parsed.value("i");

    let covered = covering(&store, &BTreeMap::from([(i, interval(0, 3, 16))]));
    assert_eq!((covered.base, covered.disp, covered.width), (None, 4, 8));
    for outside in [interval(-3, 3, 16), interval(0, 40000, 16), interval(0, 3, 32)] {
        assert_eq!(covering(&store, &BTreeMap::from([(i, outside)])).base, Some(i));
    }
}

/// `tests/test_edge_ranges.py:guarded_loop`: `for i = 0 while i < 10: if i < 4:
/// off = i * 2`, the header also computing `header`.
fn guarded_loop(header: &str) -> Parsed {
    guarded_loop_of(16, header)
}

fn guarded_loop_of(
    width: u32,
    header: &str,
) -> Parsed {
    Parsed::new(&format!(
        "define void @f() {{
b0:
  br label %b10

b10:
  %i = phi i{width} [ 0, %b0 ], [ %next, %b40 ]
  {header}
  %c = icmp slt i{width} %i, 10
  br i1 %c, label %b20, label %b50

b20:
  %g = icmp slt i{width} %i, 4
  br i1 %g, label %b30, label %b40

b30:
  %off = mul i{width} %i, 2
  br label %b40

b40:
  %next = add i{width} %i, 1
  br label %b10

b50:
  ret void
}}
"
    ))
}

/// A `range` a callee states of its result, or a parameter states of itself,
/// bounded nothing: no analysis read it. `LEN` (0 to 32767), `INSTR`, the
/// runtime's counts and every frontend's stated range were dropped on the way
/// to the pass that would have used them.
#[test]
fn a_stated_range_bounds_a_parameter_and_a_call_result() {
    let parsed = Parsed::new(
        "define range(i16 0, 100) i16 @len(ptr %p) {
b0:
  ret i16 0
}

declare i16 @plain(ptr)

define i16 @f(i16 range(i16 -5, 6) %x, ptr %p) {
b0:
  %n = call i16 @len(ptr %p)
  %m = call i16 @plain(ptr %p)
  %o = call range(i16 1, 3) i16 @plain(ptr %p)
  %c = icmp slt i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %n

b2:
  ret i16 %m
}
",
    );
    let unit = parsed.unit();
    let scoped = dominated_edges(&unit).unwrap();
    let at = &scoped[&cfg::id(parsed.block("b2"))];
    assert_eq!(at.get(&parsed.value("x")), Some(&interval(0, 5, 16)), "{at:?}");
    assert_eq!(at.get(&parsed.value("n")), Some(&interval(0, 99, 16)), "{at:?}");
    assert_eq!(at.get(&parsed.value("o")), Some(&interval(1, 2, 16)), "{at:?}");
    assert_eq!(at.get(&parsed.value("m")), None, "a call with no range says nothing");
}

/// What a block assumes bounds a value in the blocks it dominates, as the
/// branch of a check does, in loops and out; not in its own block.
#[test]
fn an_assume_bounds_a_value_below_its_block() {
    let parsed = Parsed::new(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %x, i1 %c) {
b0:
  %low = icmp sge i16 %x, 0
  %high = icmp slt i16 %x, 10
  call void @llvm.assume(i1 %low)
  call void @llvm.assume(i1 %high)
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let scoped = dominated_edges(&unit).unwrap();
    for name in ["b1", "b2"] {
        assert_eq!(scoped[&cfg::id(parsed.block(name))].get(&parsed.value("x")), Some(&interval(0, 9, 16)), "{name}");
    }
    assert_eq!(
        scoped.get(&cfg::id(parsed.block("b0"))).and_then(|at| at.get(&parsed.value("x"))),
        None,
        "not in its own block"
    );
}

/// A condition a frontend states with `Op::Assume` bounds the value in the code
/// that follows it: HIR through its lowering to the reader, no hand-written
/// MIR.
#[test]
fn an_assume_a_frontend_states_bounds_a_value_below_it() {
    use llrm_hir::model::{
        Block, Dialect, Function, Instruction, Module as HirModule, Op, Operand as HirOperand, Program, RuntimeProfile,
        Terminator, TerminatorKind, Type, TypeKind, Value,
    };
    let mut integer = Type::new(1, "integer", TypeKind::Integer, 2);
    integer.signed = Some(true);
    let types = vec![Type::new(0, "void", TypeKind::Void, 0), integer];
    // v1 is the parameter; v2 = v1 < 10; assume v2; then a block that returns
    // v1.
    let values = vec![Value { id: 1, r#type: 1 }, Value { id: 2, r#type: 1 }];
    let less = Instruction::new(1, Op::Lt, vec![2], vec![HirOperand::value_ref(1), HirOperand::constant(1, 10)]);
    let assume = Instruction::new(2, Op::Assume, vec![], vec![HirOperand::value_ref(2)]);
    let first = Block::new(1, vec![less, assume], Terminator::new(TerminatorKind::Jump, Vec::new(), vec![2]));
    let second =
        Block::new(2, Vec::new(), Terminator::new(TerminatorKind::Return, vec![HirOperand::value_ref(1)], Vec::new()));
    let mut function = Function::new(1, "F%", 1, values, Vec::new(), vec![first, second], 1);
    function.parameters = vec![1];
    let program =
        Program::new(Dialect::Qb45, RuntimeProfile::Qb45, vec![HirModule::new(1, "m", types, vec![function])]);
    let emitted = llrm_hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    // The assumption is made on the comparison's own `i1`, before any pass.
    let module = emitted.module;
    let layout = llrm_mir::datalayout::DataLayout::default();
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("F%")).expect("F%").2;
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
    let below = &dominated_edges(&unit).unwrap()[&cfg::id(function.layout()[1])];
    let parameter = function.parameters()[0];
    assert_eq!(below.get(&parameter), Some(&Interval { low: (-32768).into(), high: 9.into(), width: 16 }), "{below:?}");
}

/// What `!range` says of a load or a call bounded nothing: an enum's tag,
/// `LEN`, every value a frontend states of an instruction was written to the IR
/// and never read, though a parameter's `range` was.
#[test]
fn a_range_in_metadata_bounds_a_load_and_a_call_result() {
    let parsed = Parsed::new(
        "declare i16 @count(ptr)

define i16 @f(ptr %p, i1 %c) {
b0:
  %tag = load i16, ptr %p, !range !0
  %n = call i16 @count(ptr %p), !range !1
  %free = load i16, ptr %p
  br i1 %c, label %b1, label %b2

b1:
  ret i16 %tag

b2:
  ret i16 %n
}

!0 = !{i16 0, i16 3}
!1 = !{i16 1, i16 100}
",
    );
    let unit = parsed.unit();
    let scoped = dominated_edges(&unit).unwrap();
    let at = &scoped[&cfg::id(parsed.block("b1"))];
    assert_eq!(at.get(&parsed.value("tag")), Some(&interval(0, 2, 16)), "{at:?}");
    assert_eq!(at.get(&parsed.value("n")), Some(&interval(1, 99, 16)), "{at:?}");
    assert_eq!(at.get(&parsed.value("free")), None, "a load with no range says nothing");
}

/// A frontend's `Fact::Range` of an instruction reaches the reader: HIR to its
/// lowering to `!range` to the interval.
#[test]
fn a_range_a_frontend_states_of_an_instruction_bounds_its_result() {
    use llrm_hir::facts::{Builder, Subject};
    use llrm_hir::model::{
        Block, Dialect, Function, Instruction, Module as HirModule, Op, Operand as HirOperand, Program, RuntimeProfile,
        Terminator, TerminatorKind, Type, TypeKind, Value,
    };
    let mut integer = Type::new(1, "integer", TypeKind::Integer, 2);
    integer.signed = Some(true);
    let types = vec![Type::new(0, "void", TypeKind::Void, 0), integer];
    let values = vec![Value { id: 1, r#type: 1 }, Value { id: 2, r#type: 1 }];
    let sum = Instruction::new(1, Op::Add, vec![2], vec![HirOperand::value_ref(1), HirOperand::value_ref(1)]);
    let first = Block::new(1, vec![sum], Terminator::new(TerminatorKind::Jump, Vec::new(), vec![2]));
    let second =
        Block::new(2, Vec::new(), Terminator::new(TerminatorKind::Return, vec![HirOperand::value_ref(2)], Vec::new()));
    let mut function = Function::new(1, "F%", 1, values, Vec::new(), vec![first, second], 1);
    function.parameters = vec![1];
    let mut program =
        Program::new(Dialect::Qb45, RuntimeProfile::Qb45, vec![HirModule::new(1, "m", types, vec![function])]);
    let mut facts = Builder::new("test");
    facts.state(
        Subject::Instruction { function: 1, id: 1 },
        llrm_hir::facts::Fact::Range(llrm_mir::facts::Bounds { lo: 0, hi: 7 }),
    );
    program.modules[0].facts = facts.finish();
    let emitted = llrm_hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0);
    assert_eq!(emitted.refused, Vec::<(String, String)>::new());
    let module = emitted.module;
    let layout = llrm_mir::datalayout::DataLayout::default();
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("F%")).expect("F%").2;
    let unit = crate::testing::with_registers(Unit::of(&module, &layout, function));
    let below = &dominated_edges(&unit).unwrap()[&cfg::id(function.layout()[1])];
    let result = function
        .instruction(function.block(function.layout()[0]).first().expect("a block with an instruction"))
        .result
        .expect("a sum");
    assert_eq!(below.get(&result), Some(&interval(0, 7, 16)), "{below:?}");
}

fn fixed(operation: &str) -> String {
    format!(
        "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define void @f(i32 %x, i32 %z) {{
b0:
  %y = {operation}
  ret void
}}
"
    )
}

/// `smul.fix` had no interval, so no fixed-point product was ever known to be
/// small.
#[test]
fn a_fixed_product_of_bounded_operands_is_bounded_with_its_floor() {
    let body = fixed("call i32 @llvm.smul.fix.i32(i32 %x, i32 %z, i32 1)");
    assert_eq!(
        computed(&body, "y", &[("x", interval(-3, 2, 32)), ("z", interval(1, 1, 32))]),
        Some(interval(-2, 1, 32))
    );
}

/// The wide product wraps when stored: 2^20 * 2^20 >> 8 is 2^32.
#[test]
fn a_fixed_product_whose_result_wraps_has_no_interval() {
    let body = fixed("call i32 @llvm.smul.fix.i32(i32 %x, i32 %z, i32 8)");
    assert_eq!(computed(&body, "y", &[("x", interval(0, 1 << 20, 32)), ("z", interval(0, 1 << 20, 32))]), None);
}

/// x * x over [-5, 3] came out [-15, 25]: the corners treat the two x as
/// independent.
#[test]
fn a_square_is_never_negative() {
    let body = fixed("call i32 @llvm.smul.fix.i32(i32 %x, i32 %x, i32 0)");
    assert_eq!(computed(&body, "y", &[("x", interval(-5, 3, 32))]), Some(interval(0, 25, 32)));
    let body = fixed("mul i32 %x, %x");
    assert_eq!(computed(&body, "y", &[("x", interval(-5, 3, 32))]), Some(interval(0, 25, 32)));
    assert_eq!(computed(&body, "y", &[("x", interval(2, 3, 32))]), Some(interval(4, 9, 32)));
}

/// `%s` computed from `x` and `z`, then compared against 1024: `no` is the edge
/// where `%s <= 1024`.
fn tested(computation: &str) -> Parsed {
    Parsed::new(&format!(
        "declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)

define void @f(i32 %x, i32 %z) {{
b0:
  {computation}
  %c = icmp sgt i32 %s, 1024
  br i1 %c, label %yes, label %no

yes:
  ret void

no:
  ret void
}}
"
    ))
}

fn after_no(
    parsed: &Parsed,
    known: &[(&str, Interval)],
) -> Option<Intervals> {
    let known = known.iter().map(|(name, one)| (parsed.value(name), one.clone())).collect::<Intervals>();
    on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("no"), &known, None).unwrap()
}

/// `a + b <= 1024` left `a` unbounded above, though `b >= 0` caps it.
#[test]
fn a_bounded_sum_bounds_its_addends() {
    let parsed = tested("%s = add i32 %x, %z");
    let result = after_no(&parsed, &[("x", interval(0, 5000, 32)), ("z", interval(3, 9, 32))]).unwrap();
    assert_eq!(result[&parsed.value("x")], interval(0, 1021, 32));
    assert_eq!(result[&parsed.value("z")], interval(3, 9, 32));
}

/// Without an interval for each addend the sum may have wrapped, and says
/// nothing of them.
#[test]
fn a_sum_of_an_unbounded_addend_bounds_nothing() {
    let parsed = tested("%s = add i32 %x, %z");
    let result = after_no(&parsed, &[("z", interval(3, 9, 32))]).unwrap();
    assert!(!result.contains_key(&parsed.value("x")));
}

/// `xx + yy <= 1024` with squares `xx = x*x >> 8` bounds `x` to 512: the exit
/// test of a Mandelbrot loop.
#[test]
fn a_bounded_square_bounds_its_root() {
    let parsed = tested("%s = call i32 @llvm.smul.fix.i32(i32 %x, i32 %x, i32 8)");
    let result = after_no(&parsed, &[("x", interval(-4096, 4095, 32))]).unwrap();
    assert_eq!(result[&parsed.value("x")], interval(-512, 512, 32));
}

/// An unbounded `x` may have wrapped `x*x >> 8` below 1024; its root is no
/// smaller for that.
#[test]
fn a_square_of_an_unbounded_value_bounds_nothing() {
    let parsed = tested("%s = call i32 @llvm.smul.fix.i32(i32 %x, i32 %x, i32 8)");
    let result = after_no(&parsed, &[]).unwrap();
    assert!(!result.contains_key(&parsed.value("x")));
}

/// A range scope narrowed by each edge above a block copied every interval it
/// knew once per edge (200 sequential loops: 301 s in `analysis annotated`,
/// #556). An edge's effect is the intervals it sets, and `on_edge` is that
/// effect on the known ones.
#[test]
fn test_an_edge_sets_the_intervals_it_narrows_and_copies_none_of_the_rest() {
    let parsed = compare("slt", 16, 4);
    let x = parsed.value("x");
    let unit = parsed.unit();
    let mut known: Intervals = (1000..2000).map(|at| (ValueId(at), interval(0, 1, 16))).collect();
    known.insert(x, interval(0, 9, 16));
    let delta = super::edge_delta(&unit, parsed.block("b0"), parsed.block("yes"), &known, None).unwrap().unwrap();
    assert_eq!(delta.len(), 1, "an edge on one comparison sets one interval");
    assert_eq!(delta[&x], interval(0, 3, 16));
    let whole = on_edge(&unit, parsed.block("b0"), parsed.block("yes"), &known, None).unwrap().unwrap();
    assert_eq!(whole.len(), known.len());
    assert_eq!(whole[&x], delta[&x]);
}

/// An update of the edges' facts built the graph, its predecessors, every
/// dominator and the order of the blocks again before it asked what changed:
/// 15 M of its 36 M on d_faces -O1. A change that left the CFG as it was left
/// the order as it was.
#[test]
fn an_update_of_the_edges_reads_the_order_it_had() {
    let parsed = guarded_loop("");
    let unit = parsed.unit();
    let facts = unit.registers();
    let states = edges_solved(&unit, &facts, None).unwrap();
    let found = super::orders_found();
    let reached = std::collections::BTreeSet::from([parsed.block("b30")]);
    let again = edges_solved(&unit, &facts, Some((&states, &reached))).unwrap();
    assert_eq!(super::orders_found(), found, "the order of the blocks was found again");
    assert_eq!(again, states);
}
