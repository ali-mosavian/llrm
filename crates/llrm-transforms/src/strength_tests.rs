//! llrm-core's `strength_tests.rs`, each loop MIR text the interpreter runs
//! before and after, over trip counts 0, 1 and more.
//!
//! Ported: the three `_formula_set` pricing tests, and the loop tests as
//! behaviour: the wide recurrence of a zero-extended counter, a composed
//! offset carrying an invariant pointer, a reduced counter's own phi, a
//! loop bypass left alone, the current iteration on exit, and where the new
//! instructions go. `test_strength_reduction_replaces_a_loop_multiply_with_an_add`
//! read BC's MATRIX; it is a loop of that shape here and the corpus check.
//!
//! Skipped: `test_formula_selection_uses_67h_before_spilling_or_recomputing`
//! (an x86 address form) and `test_reduction_preserves_every_live_product_result`
//! (a multiply's high half and flags; one result here).

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::induction::{Affine, AffineOperand, Derived};
use llrm_analysis::testing::{DOS, function, value};
use llrm_mir::module::{Function, InstId, Module};
use num_bigint::BigInt;

use super::{Strength, _formula_set};
use crate::profit::OperationCosts;
use crate::testing::{managed, parsed, printed, results};

/// `text` in the DOS layout, and it through `Strength`, printed.
fn reduced(text: &str) -> (Module, Module, String) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let printed = managed(&mut after, Strength::default());
    (before, after, printed)
}

/// `text` reduced, computing what it did for each of `inputs`.
fn same(text: &str, inputs: &[&[i128]]) -> String {
    let (before, after, printed) = reduced(text);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    printed
}

/// The instructions of block `name` in `printed`.
fn block<'a>(printed: &'a str, name: &str) -> Vec<&'a str> {
    let start = printed.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name}:\n{printed}")) + name.len() + 3;
    printed[start..].lines().take_while(|line| line.starts_with("  ")).map(str::trim).collect()
}

fn multiplies(printed: &str, blocks: &[&str]) -> usize {
    blocks.iter().flat_map(|name| block(printed, name)).filter(|line| line.contains(" = mul ")).count()
}

/// `for (i = 0; i < n; i++) s += i * w`, the shape of MATRIX's row
/// address, with the multiply in the latch.
const ROWS: &str = "define i16 @f(i16 %n, i16 %w) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %row = mul i16 %i, %w
  %s.next = add i16 %s, %row
  %i.next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}
";

const TRIPS: &[&[i128]] = &[&[0, 7], &[1, 7], &[2, -5], &[9, 3], &[-3, 4], &[40, 0x1234]];

/// The multiply by an invariant leaves the loop: once for the start, in
/// the preheader, and an add of the multiplier steps the new recurrence.
#[test]
fn test_strength_reduction_replaces_a_loop_multiply_with_an_add() {
    let printed = same(ROWS, TRIPS);
    assert_eq!(multiplies(&printed, &["b1", "b2"]), 0, "{printed}");
    assert_eq!(multiplies(&printed, &["b0"]), 1, "{printed}");
    assert!(block(&printed, "b1").contains(&"%lsr.iv = phi i16 [ %0, %b0 ], [ %lsr.iv.next, %b2 ]"), "{printed}");
    assert!(block(&printed, "b2").contains(&"%s.next = add i16 %s, %lsr.iv"), "{printed}");
}

/// Where the new instructions go: the start before the preheader's branch,
/// the step right before the latch's.
#[test]
fn test_inserted_counter_operations_own_their_insertion_location() {
    let printed = same(&ROWS.replace("%row = mul i16 %i, %w", "%row = mul i16 %i, 3"), TRIPS);
    assert_eq!(block(&printed, "b0"), ["%0 = mul i16 0, 3", "br label %b1"], "{printed}");
    assert_eq!(block(&printed, "b2")[1..], ["%i.next = add i16 %i, 1", "%lsr.iv.next = add i16 %lsr.iv, 3", "br label %b1"], "{printed}");
}

/// Two equal formulas share one recurrence.
#[test]
fn test_equal_formulas_share_one_recurrence() {
    let text = ROWS.replace("%s.next = add i16 %s, %row", "%again = mul i16 %i, %w\n  %both = add i16 %row, %again\n  %s.next = add i16 %s, %both");
    let printed = same(&text, TRIPS);
    assert_eq!(multiplies(&printed, &["b1", "b2"]), 0, "{printed}");
    assert_eq!(printed.matches(" = phi ").count(), 3, "{printed}");
    assert!(block(&printed, "b2").contains(&"%both = add i16 %lsr.iv, %lsr.iv"), "{printed}");
}

/// A reduced product gets a phi of its own, which its readers read and
/// its step advances; the counter stays for its compare.
#[test]
fn test_a_reduced_counter_has_its_own_loop_phi() {
    let printed = same(&ROWS.replace("%row = mul i16 %i, %w", "%row = mul i16 %i, 2"), TRIPS);
    let header = block(&printed, "b1");
    assert!(header.contains(&"%i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]"), "{printed}");
    assert!(header.contains(&"%lsr.iv = phi i16 [ %0, %b0 ], [ %lsr.iv.next, %b2 ]"), "{printed}");
    assert!(block(&printed, "b2").contains(&"%lsr.iv.next = add i16 %lsr.iv, 2"), "{printed}");
}

/// A product read after a loop that leaves from its latch: the exit reads
/// the recurrence of the trip that left, not its step.
#[test]
fn test_reduced_product_keeps_the_current_iteration_on_exit() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
  %p = mul i16 %i, 3
  %i.next = add i16 %i, 1
  %more = icmp ult i16 %i.next, %n
  br i1 %more, label %b1, label %b2

b2:
  %p.lcssa = phi i16 [ %p, %b1 ]
  ret i16 %p.lcssa
}
";
    let printed = same(text, &[&[0], &[1], &[2], &[9], &[300]]);
    assert_eq!(multiplies(&printed, &["b1"]), 0, "{printed}");
    assert!(block(&printed, "b2").contains(&"%p.lcssa = phi i16 [ %lsr.iv, %b1 ]"), "{printed}");
}

/// An entry that may skip the loop has no preheader to start a recurrence in.
#[test]
fn test_reduction_does_not_speculate_on_a_loop_bypass() {
    let text = |entry: &str| {
        format!(
            "define i16 @f(i16 %n, i1 %c) {{
b0:
  {entry}

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b1 ]
  %p = mul i16 %i, 3
  %s.next = add i16 %s, %p
  %i.next = add i16 %i, 1
  %more = icmp ult i16 %i.next, %n
  br i1 %more, label %b1, label %b2

b2:
  %r = phi i16 [ %s.next, %b1 ], [ 5, %b0 ]
  ret i16 %r
}}
"
        )
    };
    let bypassed = text("br i1 %c, label %b1, label %b2");
    let (before, _, printed) = reduced(&bypassed);
    assert_eq!(printed, llrm_mir::print::module(&before));
    let straight = text("br label %b1").replace(", [ 5, %b0 ]", "");
    let printed = same(&straight, &[&[0, 0], &[1, 0], &[6, 1]]);
    assert_eq!(multiplies(&printed, &["b1"]), 0, "{printed}");
}

/// `i` counts to 64 as a word; its product with 109 as a dword steps by
/// 109. With no trips, nothing proves the extension exact.
#[test]
fn test_zero_extended_counter_product_is_carried_as_a_wide_recurrence() {
    for bound in [0, 1, 2, 64] {
        let text = format!(
            "define i32 @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i32 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp ult i16 %i, {bound}
  br i1 %more, label %b2, label %b3

b2:
  %e = zext i16 %i to i32
  %p = mul i32 %e, 109
  %s.next = add i32 %s, %p
  %i.next = add i16 %i, 1
  br label %b1

b3:
  ret i32 %s
}}
"
        );
        let printed = same(&text, &[&[]]);
        assert_eq!(multiplies(&printed, &["b2"]), usize::from(bound == 0), "{bound}: {printed}");
        if bound != 0 {
            assert!(block(&printed, "b2").contains(&"%lsr.iv.next = add i32 %lsr.iv, 109"), "{printed}");
        }
    }
}

/// A narrow counter may wrap where nothing counts its trips: its
/// extension is no recurrence, and the product stays.
#[test]
fn test_an_extended_counter_that_may_wrap_is_not_reduced() {
    let text = "define i32 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i32 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %e = sext i16 %i to i32
  %p = mul i32 %e, 109
  %s.next = add i32 %s, %p
  %i.next = add i16 %i, 1
  br label %b1

b3:
  ret i32 %s
}
";
    let printed = same(text, &[&[0], &[1], &[5], &[-1]]);
    assert_eq!(multiplies(&printed, &["b2"]), 1, "{printed}");
}

/// A counter stepped by what the loop computes is no recurrence.
#[test]
fn test_a_variable_step_is_not_reduced() {
    let text = ROWS.replace("%i.next = add i16 %i, 1", "%k = and i16 %s.next, 3\n  %k1 = add i16 %k, 1\n  %i.next = add i16 %i, %k1");
    let printed = same(&text, TRIPS);
    assert_eq!(multiplies(&printed, &["b2"]), 1, "{printed}");
}

/// A multiplier the loop changes is no stride.
#[test]
fn test_a_variable_multiplier_is_not_reduced() {
    let printed = same(&ROWS.replace("%row = mul i16 %i, %w", "%row = mul i16 %i, %s"), TRIPS);
    assert_eq!(multiplies(&printed, &["b2"]), 1, "{printed}");
}

/// `a[2i + 6]` off a global: the address itself becomes a pointer
/// recurrence, started at `@a + 6` and stepped by 2 bytes.
#[test]
fn test_composed_offset_can_carry_an_invariant_pointer() {
    let text = "@a = global [64 x i8] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %o = mul i16 %i, 2
  %d = add i16 %o, 6
  %p = getelementptr i8, ptr @a, i16 %d
  store i16 %i, ptr %p
  %i.next = add i16 %i, 1
  br label %b1

b3:
  %q = getelementptr i8, ptr @a, i16 12
  %r = load i16, ptr %q
  ret i16 %r
}
";
    let printed = same(text, &[&[0], &[1], &[3], &[4], &[20]]);
    assert_eq!(multiplies(&printed, &["b2"]), 0, "{printed}");
    let latch = block(&printed, "b2");
    assert!(latch.contains(&"store i16 %i, ptr %lsr.iv"), "{printed}");
    assert!(latch.contains(&"%lsr.iv.next = getelementptr i8, ptr %lsr.iv, i16 2"), "{printed}");
    assert!(block(&printed, "b1").iter().any(|line| line.starts_with("%lsr.iv = phi ptr")), "{printed}");
}

/// An exact quotient of a counter stepping by its divisor is a counter.
#[test]
fn test_an_exact_quotient_becomes_a_recurrence() {
    for bound in [0, 2, 4, 40] {
        let text = format!(
            "define i16 @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp slt i16 %i, {bound}
  br i1 %more, label %b2, label %b3

b2:
  %q = sdiv i16 %i, 2
  %s.next = add i16 %s, %q
  %i.next = add i16 %i, 2
  br label %b1

b3:
  ret i16 %s
}}
"
        );
        let printed = same(&text, &[&[]]);
        let divides = block(&printed, "b2").iter().filter(|line| line.contains("sdiv")).count();
        assert_eq!(divides, usize::from(bound == 0), "{bound}: {printed}");
    }
}

/// `(i + x) * 2` beside `i * 2`, where `i` is only the loop's control:
/// the bare root is credited, and carries its stride class as the
/// invariant `x * 2` plus its recurrence.
#[test]
fn test_a_credited_root_carries_its_stride_class() {
    let text = "define i16 @f(i16 %n, i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b2 ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %b2, label %b3

b2:
  %r = mul i16 %i, 2
  %t = add i16 %i, %x
  %m = mul i16 %t, 2
  %both = xor i16 %r, %m
  %s.next = add i16 %s, %both
  %i.next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}
";
    let printed = same(text, &[&[0, 5], &[1, 5], &[3, -2], &[17, 0x7fff]]);
    assert_eq!(multiplies(&printed, &["b2"]), 0, "{printed}");
    assert_eq!(printed.matches(" = phi ").count(), 3, "{printed}");
}

/// `j + o` read by `j + o + 1`, a formula, and by a compare: it stays
/// live beside its reader's recurrence, so it is carried in the same run.
/// It was a leaf only once its reader was carried, and a second run
/// reduced it (nib-matmul, demo-deedlines).
#[test]
fn test_a_formula_read_beyond_the_formulas_is_carried_in_one_run() {
    let text = "define i16 @f(i16 %n, i16 %o) {
b0:
  br label %b1

b1:
  %j = phi i16 [ 0, %b0 ], [ %j.next, %b4 ]
  %s = phi i16 [ 0, %b0 ], [ %s.next, %b4 ]
  %more = icmp slt i16 %j, %n
  br i1 %more, label %b2, label %b3

b2:
  %t = add i16 %o, %j
  %low = icmp ult i16 %t, 64
  br i1 %low, label %b4, label %b3

b4:
  %u = add i16 %t, 1
  %s.next = add i16 %s, %u
  %j.next = add i16 %j, 1
  br label %b1

b3:
  %r = phi i16 [ %s, %b1 ], [ -1, %b2 ]
  ret i16 %r
}
";
    let (_, mut module, once) = reduced(text);
    assert!(!block(&once, "b2").iter().any(|line| line.contains("= add i16 %o, %j")), "{once}");
    assert_eq!(managed(&mut module, Strength::default()), once);
    same(text, &[&[0, 3], &[1, 70], &[5, 60], &[20, -4]]);
}

/// A function holding `lines`, and the instruction defining each of `names`.
fn holding(lines: &str, names: &[&str]) -> (Module, Vec<InstId>) {
    let module = parsed(&format!("define void @f(i16 %c, i16 %d, i16 %v, i32 %w, i32 %seed) {{\nb0:\n{lines}  ret void\n}}\n"));
    let f = function(&module, "f");
    let made = |name: &str| {
        let wanted = value(f, name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    };
    let found = names.iter().map(|name| made(name)).collect();
    (module, found)
}

fn counter(f: &Function, name: &str, width: u32) -> Affine {
    Affine { value: value(f, name), start: AffineOperand::constant(0, width), step: AffineOperand::constant(1, width), header: 1 }
}

fn formula(op: InstId, of: &Affine, by: AffineOperand, offsets: Vec<(AffineOperand, BigInt)>) -> Derived {
    Derived { op, of: of.clone(), by, offsets, pointer: None }
}

#[test]
fn test_formula_selection_prices_complete_sibling_groups() {
    let (module, at) = holding(
        "  %p = mul i16 %c, 8
  %a0 = add i16 %p, 0
  %a1 = add i16 %p, 16
  %q = mul i16 %d, 8
  %b0 = add i16 %q, 0
  %b1 = add i16 %q, 16
  %b2 = add i16 %q, 32
",
        &["p", "a0", "a1", "q", "b0", "b1", "b2"],
    );
    let f = function(&module, "f");
    let group = |counter: &Affine, at: &[InstId]| {
        let mut out = vec![formula(at[0], counter, AffineOperand::constant(8, 16), vec![])];
        for (index, &one) in at[1..].iter().enumerate() {
            out.push(formula(one, counter, AffineOperand::constant(8, 16), vec![(AffineOperand::constant(16 * index as i64, 16), BigInt::from(1))]));
        }
        out
    };
    let small = group(&counter(f, "c", 16), &at[..3]);
    let large = group(&counter(f, "d", 16), &at[3..]);
    let candidates = [small.clone(), large.clone()].concat();
    let costly_addresses = OperationCosts { add: 1, address: 100, load: 1, memory_update: 1, ..OperationCosts::default() };

    let selected = _formula_set(f, &candidates, Some(4), &BTreeSet::new(), &costly_addresses, None);

    assert_eq!(selected, [vec![small[0].clone()], large[1..].to_vec()].concat());
}

#[test]
fn test_formula_selection_recomputes_a_cheap_scaled_index_under_pressure() {
    let costs = OperationCosts { add: 2, multiply: 22, shift: 3, load: 4, memory_update: 8, ..OperationCosts::default() };
    for (line, kept) in [("%x = mul i16 %c, 2", false), ("%x = mul i16 %c, %v", true)] {
        let (module, at) = holding(&format!("  {line}\n"), &["x"]);
        let f = function(&module, "f");
        let by = if kept { AffineOperand::Value(value(f, "v"), 16) } else { AffineOperand::constant(2, 16) };
        let one = formula(at[0], &counter(f, "c", 16), by, vec![]);
        let references = BTreeMap::from([(value(f, "x"), 1)]);
        let selected = _formula_set(f, &[one], Some(0), &BTreeSet::new(), &costs, Some(&references));
        assert_eq!(!selected.is_empty(), kept, "{line}");
    }
}

#[test]
fn test_formula_selection_prices_a_complete_affine_formula_under_pressure() {
    let (module, at) = holding("  %x = add i32 %w, %seed\n", &["x"]);
    let f = function(&module, "f");
    let complete = formula(
        at[0],
        &counter(f, "w", 32),
        AffineOperand::constant(24, 32),
        vec![(AffineOperand::constant(-128, 32), BigInt::from(1)), (AffineOperand::Value(value(f, "seed"), 32), BigInt::from(1))],
    );
    let costs = OperationCosts { add: 2, multiply: 22, shift: 3, address: 2, load: 4, memory_update: 8, ..OperationCosts::default() };
    let references = BTreeMap::from([(value(f, "x"), 1)]);
    assert_eq!(_formula_set(f, std::slice::from_ref(&complete), Some(0), &BTreeSet::new(), &costs, Some(&references)), vec![complete]);
}

/// Over the rich-MIR corpus: every reduced module verifies, some loop is
/// reduced, and a third run finds nothing more. The second may: a new
/// integer recurrence is a counter, and an address off it a formula.
#[test]
fn every_corpus_reduction_verifies_and_settles() {
    let mut fired = 0;
    for (name, mut module) in llrm_analysis::testing::corpus() {
        let mut runs = Vec::new();
        for _ in 0..3 {
            let mut manager = llrm_mir::passes::PassManager::default();
            manager.verify_each = true;
            manager.add(Strength::default());
            manager.run(&mut module).unwrap_or_else(|error| panic!("{name}: {error}"));
            runs.push(printed(&module));
        }
        fired += runs[0].matches("lsr.iv.next").count();
        assert!(runs[2] == runs[1], "{name}: a third run changed it");
    }
    assert!(fired > 0, "the corpus multiplies a counter somewhere");
}
