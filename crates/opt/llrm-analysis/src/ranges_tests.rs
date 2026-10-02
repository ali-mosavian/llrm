//! Ports of `tests/test_ranges.py`, `tests/test_edge_ranges.py` and
//! `tests/test_unsigned_edge_ranges.py`. `test_range_alias_checks_cover_width_and_wrap`
//! is `regions`' `overlapping_range_covering_respects_width_wrap_and_each_fact_map`.

use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, ValueId};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_computed, _recurrence_span, Interval, bounded, covering, dominated_edges, exact_offsets, on_edge, singletons};
use crate::cfg;
use crate::memory::{MemRef, Unit};
use crate::testing::{DOS, block, function, layout, parsed, value};

fn interval(low: i64, high: i64, width: u32) -> Interval {
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
        Unit::of(&self.module, &self.layout, function(&self.module, "f"))
    }

    fn value(&self, name: &str) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn block(&self, name: &str) -> llrm_mir::module::BlockId {
        block(function(&self.module, "f"), name)
    }

    /// The instruction defining `%name`.
    fn made(&self, name: &str) -> InstId {
        let f = function(&self.module, "f");
        let wanted = self.value(name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    }
}

/// `%c = icmp <predicate> i<width> %x, <bound>` branching to `%yes` or `%no`.
fn compare(predicate: &str, width: u32, bound: i64) -> Parsed {
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
        let known = IndexMap::from_iter([(x, interval(0, 9, 16))]);
        let successor = parsed.block(if taken { "yes" } else { "no" });

        let result = on_edge(&parsed.unit(), parsed.block("b0"), successor, &known, None).unwrap().unwrap();

        assert_eq!(result[&x], interval(low, high, 16), "{predicate} {taken}");
    }
}

#[test]
fn an_edge_the_facts_rule_out_is_impossible() {
    let parsed = compare("slt", 16, 4);
    let known = IndexMap::from_iter([(parsed.value("x"), interval(5, 9, 16))]);
    assert_eq!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("yes"), &known, None).unwrap(), None);
    assert!(on_edge(&parsed.unit(), parsed.block("b0"), parsed.block("no"), &known, None).unwrap().is_some());
}

#[test]
fn an_edge_to_a_block_that_is_no_successor_is_refused() {
    let parsed = compare("slt", 16, 4);
    assert!(on_edge(&parsed.unit(), parsed.block("yes"), parsed.block("no"), &IndexMap::default(), None).is_err());
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
    let known = IndexMap::from_iter([(parsed.value("x"), interval(0, 9, 16))]);
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
    for scoped in [dominated_edges(&unit).unwrap(), bounded(&unit).unwrap()] {
        let inside = &scoped[&cfg::id(parsed.block("b1"))];
        assert_eq!(on_edge(&unit, parsed.block("b1"), parsed.block("b4"), inside, None).unwrap(), None);
    }
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

fn computed(body: &str, name: &str, known: &[(&str, Interval)]) -> Option<Interval> {
    let parsed = Parsed::new(body);
    let known = known.iter().map(|(name, one)| (parsed.value(name), one.clone())).collect::<IndexMap<_, _>>();
    _computed(&parsed.unit(), parsed.made(name), &known, &IndexMap::default())
}

fn unary(operation: &str, width: u32) -> String {
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
        assert_eq!(computed(&unary("sext i16 %x to i32", 32), "y", &[("x", interval(low, high, 16))]), Some(interval(low, high, 32)));
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
        assert_eq!(computed(&unary(&operation, 16), "y", &[("x", interval(low, high, 16))]), expected, "{low} {high} {count}");
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
    assert_eq!(computed(body, "y", &[("x", interval(0, 10, 16)), ("z", interval(-5, 5, 16))]), Some(interval(-5, 15, 16)));
    assert_eq!(computed(body, "y", &[("x", interval(0, 10, 16))]), None);
}

#[test]
fn test_unsigned_edge_never_removes_a_possible_selector() {
    for predicate in ["ugt", "uge", "ult", "ule"] {
        for span in [(1, 3), (255, 255), (256, 260), (-3, -1), (-2, 2)] {
            for taken in [true, false] {
                for width in [16, 32] {
                    let parsed = compare(predicate, width, 255);
                    let known = IndexMap::from_iter([(parsed.value("x"), interval(span.0, span.1, width))]);
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

/// `tests/test_edge_ranges.py:guarded_loop`: `for i = 0 while i < 10: if i < 4: off = i * 2`,
/// the header also computing `header`.
fn guarded_loop(header: &str) -> Parsed {
    guarded_loop_of(16, header)
}

fn guarded_loop_of(width: u32, header: &str) -> Parsed {
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

/// `tests/test_edge_ranges.py`: i<4 bounds a word-array offset to 0..6, not the whole loop's 0..18.
#[test]
fn test_guard_refines_subscript_without_leaking_to_the_join() {
    let parsed = guarded_loop("");
    let known = bounded(&parsed.unit()).unwrap();
    let at = |name: &str| &known[&cfg::id(parsed.block(name))];
    let (counter, offset) = (parsed.value("i"), parsed.value("off"));

    assert_eq!(at("b30")[&counter], interval(0, 3, 16));
    assert_eq!(at("b30")[&offset], interval(0, 6, 16));
    assert_eq!(at("b40")[&counter], interval(0, 9, 16));
    assert_eq!(at("b50")[&counter], interval(10, 32767, 16));
}

/// matmul8: gvn forwarded a value made past the header's test to the one
/// the header tests, and inside the loop that value had no range; its
/// bounds check stayed and the loop nest stayed rolled.
#[test]
fn test_a_value_the_header_makes_is_bounded_inside_the_loop() {
    let parsed = guarded_loop("%twice = mul i16 %i, 2");
    let known = bounded(&parsed.unit()).unwrap();
    assert_eq!(known[&cfg::id(parsed.block("b20"))].get(&parsed.value("twice")), Some(&interval(0, 18, 16)));
}

/// Widths were the old x86's 16 and 32 bits: a value computed from an i8
/// counter had no interval, nor did the counter under the guard.
#[test]
fn a_value_computed_from_an_i8_counter_is_bounded() {
    let parsed = guarded_loop_of(8, "%twice = mul i8 %i, 2");
    let known = bounded(&parsed.unit()).unwrap();
    let at = |name: &str| &known[&cfg::id(parsed.block(name))];
    assert_eq!(at("b20").get(&parsed.value("twice")), Some(&interval(0, 18, 8)));
    assert_eq!(at("b30").get(&parsed.value("off")), Some(&interval(0, 6, 8)));
}

/// A loop tested after its trip had no facts in its header, which in a
/// rotated loop is the whole trip, so no access there had a bounded index.
#[test]
fn test_a_posttested_header_knows_its_counter() {
    use crate::induction::tests::{Shape, looped, shaped};
    let rotated = looped(Some(0), Some(9), llrm_mir::opcode::IntPredicate::Slt, 1, Shape { split: true, ..shaped("post", 8) });
    let known = bounded(&rotated.unit()).unwrap();
    let header = cfg::id(block(rotated.function(), "b1"));
    assert_eq!(known.get(&header).and_then(|facts| facts.get(&rotated.value("i"))), Some(&interval(0, 9, 8)));
}

/// `text` with every fact `facts` states at a block checked where the
/// block's `;check` line is, if its value is defined there: `@check`
/// calls `@f` and answers how many checks failed.
fn checked(text: &str, facts: impl Fn(&Parsed) -> super::Facts) -> (String, usize) {
    let parsed = Parsed::new(text);
    let known = facts(&parsed);
    let f = function(&parsed.module, "f");
    let graph = cfg::graph(f);
    let dominators = crate::graph::loops::dominators(&graph, None);
    let (mut out, mut label, mut serial) = (String::new(), String::new(), 0);
    for line in text.lines() {
        if let Some(name) = line.strip_suffix(':').filter(|name| !name.starts_with(' ')) {
            label = name.to_owned();
        }
        if line.trim() != ";check" {
            out += line;
            out += "\n";
            continue;
        }
        let at = cfg::id(parsed.block(&label));
        for (value, fact) in known.get(&at).into_iter().flatten() {
            let data = f.value(*value);
            let (Some(name), Some(width)) = (&data.name, parsed.unit().int_bits(llrm_mir::Operand::Value(*value))) else { continue };
            let placed = match data.def {
                llrm_mir::ValueDef::Instruction(inst) => cfg::id(f.parent(inst).unwrap()),
                llrm_mir::ValueDef::Argument(_) => cfg::id(f.entry().unwrap()),
            };
            if fact.width != width || !dominators[&at].contains(&placed) {
                continue;
            }
            serial += 1;
            let k = format!("%k{serial}");
            out += &format!(
                "  {k}.lo = icmp slt i{width} %{name}, {}
  {k}.hi = icmp sgt i{width} %{name}, {}
  {k}.or = or i1 {k}.lo, {k}.hi
  {k}.z = zext i1 {k}.or to i16
  {k}.old = load i16, ptr @bad
  {k}.new = add i16 {k}.old, {k}.z
  store i16 {k}.new, ptr @bad
",
                fact.low, fact.high
            );
        }
    }
    (out, serial)
}

/// Runs `@f(n)` of `checked`'s text: how many checks failed.
fn broken(text: &str, n: i64) -> u128 {
    let module = parsed(&format!("{DOS}@bad = global i16 0\n\n{text}\ndefine i16 @check(i16 %n) {{\nb0:\n  call void @f(i16 %n)\n  %r = load i16, ptr @bad\n  ret i16 %r\n}}\n"));
    match llrm_mir::interpret::run(&module, "check", vec![llrm_mir::interpret::Val::Int { bits: n as u128 & 0xFFFF, width: 16 }], 1_000_000) {
        Ok(llrm_mir::interpret::Val::Int { bits, .. }) => bits,
        other => panic!("{other:?}"),
    }
}

/// Every interval `bounded` and `scoped` state holds of every value there, as run.
#[test]
fn every_bounded_fact_holds_when_run() {
    let loops = [
        // A guard inside a counted loop, off a runtime bound.
        "define void @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %c = icmp slt i16 %i, 20
  ;check
  br i1 %c, label %b2, label %b5

b2:
  %g = icmp slt i16 %i, %n
  ;check
  br i1 %g, label %b3, label %b4

b3:
  %off = mul i16 %i, 3
  %less = sub i16 %off, 7
  ;check
  br label %b4

b4:
  %next = add i16 %i, 1
  ;check
  br label %b1

b5:
  ;check
  ret void
}
",
        // Nested: the inner loop counts down from the outer counter.
        "define void @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 1, %b0 ], [ %inext, %b4 ]
  %c = icmp sle i16 %i, 8
  ;check
  br i1 %c, label %b2, label %b5

b2:
  %j = phi i16 [ 30, %b1 ], [ %jnext, %b3 ]
  %d = icmp sgt i16 %j, 2
  ;check
  br i1 %d, label %b3, label %b4

b3:
  %sum = add i16 %j, %i
  %jnext = add i16 %j, -4
  ;check
  br label %b2

b4:
  %inext = add i16 %i, 1
  ;check
  br label %b1

b5:
  ret void
}
",
        // Tested after the trip, with a second counter.
        "define void @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ -5, %b0 ], [ %next, %b1 ]
  %k = phi i16 [ 100, %b0 ], [ %knext, %b1 ]
  %scaled = shl i16 %i, 2
  %next = add i16 %i, 2
  %knext = add i16 %k, -3
  %c = icmp slt i16 %next, 11
  ;check
  br i1 %c, label %b1, label %b2

b2:
  ;check
  ret void
}
",
    ];
    for text in loops {
        for facts in [bounded, super::scoped] {
            let (checking, checks) = checked(text, |parsed| facts(&parsed.unit()).unwrap());
            assert!(checks > 3, "{checks} checks of {text}");
            for n in [-3, 0, 2, 7, 100] {
                assert_eq!(broken(&checking, n), 0, "{n}: {checking}");
            }
        }
    }
}

/// An access into a global indexed by `index`, computed as `offset` from counter `%i` (0 ..= 49).
fn indexed(offset: &str, gep: &str) -> Parsed {
    Parsed::new(&format!(
        "@a = global [100 x i8] zeroinitializer

define void @f(ptr %q) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, 50
  br i1 %c, label %b2, label %b3

b2:
  %off = {offset}
  %p = {gep}
  store i8 0, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  ret void
}}
"
    ))
}

#[test]
fn an_offset_is_exact_only_when_every_partial_sum_is_a_nonnegative_index() {
    for (offset, gep, exact) in [
        ("mul i16 %i, 2", "getelementptr inbounds i8, ptr @a, i16 %off", true),
        ("add i16 %i, 3", "getelementptr inbounds i16, ptr @a, i16 %off", true),
        ("mul i16 %i, 2", "getelementptr i8, ptr @a, i16 %off", false), // no promise
        ("sub i16 %i, 5", "getelementptr inbounds i8, ptr @a, i16 %off", false), // below 0
        ("mul i16 %i, 2000", "getelementptr inbounds i8, ptr @a, i16 %off", false), // past 64K
        ("mul i16 %i, 2", "getelementptr inbounds i8, ptr %q, i16 %off", false), // no object start
    ] {
        let parsed = indexed(offset, gep);
        let found = exact_offsets(&parsed.unit()).unwrap();
        assert_eq!(found.contains(&parsed.value("off")), exact, "{offset} {gep}");
    }
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
    for scoped in [dominated_edges(&unit).unwrap(), bounded(&unit).unwrap(), super::scoped(&unit).unwrap()] {
        let at = &scoped[&cfg::id(parsed.block("b2"))];
        assert_eq!(at.get(&parsed.value("x")), Some(&interval(0, 5, 16)), "{at:?}");
        assert_eq!(at.get(&parsed.value("n")), Some(&interval(0, 99, 16)), "{at:?}");
        assert_eq!(at.get(&parsed.value("o")), Some(&interval(1, 2, 16)), "{at:?}");
        assert_eq!(at.get(&parsed.value("m")), None, "a call with no range says nothing");
    }
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
    for scoped in [dominated_edges(&unit).unwrap(), bounded(&unit).unwrap(), super::scoped(&unit).unwrap()] {
        for name in ["b1", "b2"] {
            assert_eq!(scoped[&cfg::id(parsed.block(name))].get(&parsed.value("x")), Some(&interval(0, 9, 16)), "{name}");
        }
        assert_eq!(scoped.get(&cfg::id(parsed.block("b0"))).and_then(|at| at.get(&parsed.value("x"))), None, "not in its own block");
    }
}

#[test]
fn an_assume_above_a_counted_loop_bounds_a_value_in_its_body() {
    let parsed = Parsed::new(
        "declare void @llvm.assume(i1)
declare void @use(i16, i16)

define void @f(i16 %x) {
b0:
  %low = icmp sge i16 %x, 0
  %high = icmp slt i16 %x, 10
  call void @llvm.assume(i1 %low)
  call void @llvm.assume(i1 %high)
  br label %body

body:
  %i = phi i16 [ 0, %b0 ], [ %next, %body ]
  call void @use(i16 %i, i16 %x)
  %next = add nsw i16 %i, 1
  %more = icmp slt i16 %next, 8
  br i1 %more, label %body, label %out

out:
  ret void
}
",
    );
    let unit = parsed.unit();
    let at = &bounded(&unit).unwrap()[&cfg::id(parsed.block("body"))];
    assert!(at.contains_key(&parsed.value("i")), "the loop is counted: {at:?}");
    assert_eq!(at.get(&parsed.value("x")), Some(&interval(0, 9, 16)), "{at:?}");
}
