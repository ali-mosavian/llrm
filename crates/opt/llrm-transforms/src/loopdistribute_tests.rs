//! Splitting on the interpreter: the answers before and after, and the loops.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use super::{distributed, found};
use crate::dead::Dead;
use crate::testing::{f, managed, parsed, printed, results};

/// The pass without its judge: the first loop that splits, split and cleaned.
struct Unjudged;

impl FunctionPass for Unjudged {
    fn name(&self) -> &'static str {
        "distribute-unjudged"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(split) = found(unit, analyses, &BTreeSet::new()) else { return PreservedAnalyses::all() };
        distributed(unit.function, &split).expect("a loop it copies");
        PreservedAnalyses::none()
    }
}

const INPUTS: &[&[i128]] = &[&[0], &[1], &[4], &[5], &[9], &[-2]];

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text` through the split and the removal of what it leaves unused: the
/// module and how many loops it gained.
fn split(text: &str) -> (Module, usize) {
    let mut module = parsed(text);
    let before = loop_count(&mut module);
    managed(&mut module, Unjudged);
    managed(&mut module, Dead);
    let gained = loop_count(&mut module) - before;
    (module, gained)
}

fn same_answers(
    module: &Module,
    text: &str,
) {
    assert_eq!(results(module, INPUTS), results(&parsed(text), INPUTS), "{}", printed(module));
}

/// Cells of each array that are initialised and summed: every one the loops
/// may touch, for `%n` up to nine and the offsets of the tests.
const CELLS: usize = 12;

/// The straight-line stores that give every cell of `a`, `b` and `c` a value of
/// its own, and the sum that reads them all back with a weight of its own.
fn initialised() -> (String, String) {
    let mut init = String::new();
    let mut sum = String::from("  %total0 = add i16 0, 0\n");
    for (array, base) in [("a", 100), ("b", 200), ("c", 300)] {
        for cell in 0..CELLS {
            init += &format!(
                "  %i{array}{cell} = getelementptr i8, ptr %{array}, i16 {}\n  store i16 {}, ptr %i{array}{cell}\n",
                cell * 2,
                base + cell * 7
            );
        }
    }
    let mut previous = "total0".to_string();
    for (array, weight) in [("a", 1), ("b", 3), ("c", 5)] {
        for cell in 0..CELLS {
            sum += &format!(
                "  %r{array}{cell} = getelementptr i8, ptr %{array}, i16 {}\n  %v{array}{cell} = load i16, ptr %r{array}{cell}\n  %w{array}{cell} = mul i16 %v{array}{cell}, {weight}\n  %s{array}{cell} = add i16 %{previous}, %w{array}{cell}\n",
                cell * 2
            );
            previous = format!("s{array}{cell}");
        }
    }
    sum += &format!("  ret i16 %{previous}\n");
    (init, sum)
}

/// A loop of `%n` trips with `body`, over `a`, `b` and `c` of twelve cells,
/// tested before each trip; the answer sums every cell. `%o` is the byte
/// offset of the trip and `%i` its number.
fn looping(body: &str) -> String {
    let (init, sum) = initialised();
    format!(
        "define i16 @f(i16 %n) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  %c = alloca [64 x i16]
{init}  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done

body:
  %o = mul i16 %i, 2
  %pa = getelementptr i8, ptr %a, i16 %o
  %pb = getelementptr i8, ptr %b, i16 %o
  %pc = getelementptr i8, ptr %c, i16 %o
{body}
  %i.next = add i16 %i, 1
  br label %h

done:
{sum}}}
"
    )
}

/// Two stores that read nothing of each other's cells: two loops, which run
/// the same trips and leave the same arrays.
#[test]
fn two_independent_stores_become_two_loops() {
    let text =
        looping("  store i16 7, ptr %pa\n  %x = load i16, ptr %pc\n  %y = mul i16 %x, 3\n  store i16 %y, ptr %pb");
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// What the second store reads the first wrote the same trip: the first
/// loop's stores all come before the second's reads, as they did.
#[test]
fn a_store_read_on_the_same_trip_splits_in_order() {
    let text = looping(
        "  %x = load i16, ptr %pc\n  store i16 %x, ptr %pa\n  %w = load i16, ptr %pa\n  %v = add i16 %w, 1\n  store i16 %v, ptr %pb",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// What the first store would read of the second's cell a trip later is written
/// before it, and the other way round: the two are one loop.
#[test]
fn stores_that_depend_on_each_other_both_ways_stay_one_loop() {
    // a[i + 1] = b[i]; b[i + 1] = a[i]: each reads what the other wrote the
    // trip before.
    let text = looping(
        "  %pa1 = getelementptr i8, ptr %pa, i16 2\n  %pb1 = getelementptr i8, ptr %pb, i16 2\n  %x = load i16, ptr %pb\n  store i16 %x, ptr %pa1\n  %y = load i16, ptr %pa\n  store i16 %y, ptr %pb1",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A load two stores store from keeps them together: values pass between the
/// loops only through memory.
#[test]
fn a_load_both_stores_use_keeps_them_in_one_loop() {
    let text =
        looping("  %x = load i16, ptr %pc\n  store i16 %x, ptr %pa\n  %y = add i16 %x, 1\n  store i16 %y, ptr %pb");
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A later trip's read of a cell the other store writes first forces the
/// order: the store of `b[i]` of what `a[i + 1]` held, and `a[i]` written:
/// the read must come before the write, so the loop of `b` runs first.
#[test]
fn the_order_follows_the_dependences_not_the_body() {
    let text = looping(
        "  store i16 7, ptr %pa\n  %pa1 = getelementptr i8, ptr %pa, i16 2\n  %x = load i16, ptr %pa1\n  store i16 %x, ptr %pb",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A call may touch anything.
#[test]
fn a_loop_with_a_call_is_not_split() {
    let text =
        format!("declare void @g()\n{}", looping("  call void @g()\n  store i16 7, ptr %pa\n  store i16 8, ptr %pb"));
    let (_, gained) = split(&text);
    assert_eq!(gained, 0);
}

/// A value of the loop read after it would have to come from a loop that is
/// not the last.
#[test]
fn a_loop_whose_value_is_read_after_it_is_not_split() {
    let text = looping("  store i16 7, ptr %pa\n  store i16 8, ptr %pb")
        .replace("  ret i16 ", "  %leak = add i16 %i, 1\n  ret i16 ");
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// Two parameter pointers may be one object: a store through one and a read
/// through the other keep the loop whole, and `noalias` lets them part.
#[test]
fn pointers_that_may_alias_are_not_told_apart() {
    for (kind, wanted) in [("", 0), ("noalias", 1)] {
        let text = format!(
            "define void @f(ptr {kind} %a, ptr {kind} %b, ptr {kind} %c, i16 %n) {{
b0:
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done

body:
  %o = mul i16 %i, 2
  %pa = getelementptr i8, ptr %a, i16 %o
  %pb = getelementptr i8, ptr %b, i16 %o
  %pc = getelementptr i8, ptr %c, i16 %o
  %x = load i16, ptr %pc
  store i16 %x, ptr %pa
  store i16 7, ptr %pb
  %i.next = add i16 %i, 1
  br label %h

done:
  ret void
}}
"
        );
        let (module, gained) = split(&text);
        assert_eq!(gained, wanted, "{kind}: {}", printed(&module));
    }
}

/// A division may trap, and which loop it is in decides what the other has
/// done by then.
#[test]
fn a_loop_that_may_trap_is_not_split() {
    let text =
        looping("  %x = load i16, ptr %pc\n  %q = sdiv i16 %x, %n\n  store i16 %q, ptr %pa\n  store i16 7, ptr %pb");
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// The split is kept only for what it saves: two loops of plain stores cost
/// more than the one, so the pass, with its judge, leaves the loop whole.
#[test]
fn a_split_that_saves_nothing_is_not_kept() {
    let text =
        looping("  store i16 7, ptr %pa\n  %x = load i16, ptr %pc\n  %y = mul i16 %x, 3\n  store i16 %y, ptr %pb");
    let mut module = parsed(&text);
    managed(&mut module, super::Distribute { passes: Vec::new() });
    assert_eq!(loop_count(&mut module), 1, "{}", printed(&module));
}

/// Splitting pays only for a part that is an idiom: stores of loaded values
/// scaled are neither a fill nor a copy, so a loop of two stays whole however
/// independent they are.
#[test]
fn a_loop_none_of_whose_stores_is_an_idiom_is_not_split() {
    let text = looping(
        "  %x = load i16, ptr %pc\n  %y = mul i16 %x, 3\n  store i16 %y, ptr %pa\n  %w = load i16, ptr %pb\n  %z = mul i16 %w, 5\n  store i16 %z, ptr %pb",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// A header tested before each trip runs once more than the body, and what it
/// stores comes in the loop that has it: a store in it is not split off, for
/// the extra run would meet the other loop's cells at trip 0.
#[test]
fn a_store_in_a_header_tested_before_each_trip_is_not_split() {
    let text = looping("  store i16 7, ptr %pa\n  store i16 8, ptr %pb").replace(
        "  %more = icmp slt i16 %i, %n\n",
        "  %ho = mul i16 %i, 2\n  %hp = getelementptr i8, ptr %a, i16 %ho\n  %hq = getelementptr i8, ptr %hp, i16 2\n  store i16 9, ptr %hq\n  %more = icmp slt i16 %i, %n\n",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
    same_answers(&module, &text);
}

/// The same loop tested after each trip, behind the test that it runs: split,
/// and the same arrays come out.
#[test]
fn a_loop_tested_after_each_trip_splits_the_same() {
    let text = looping("  store i16 7, ptr %pa\n  %x = load i16, ptr %pc\n  %y = mul i16 %x, 3\n  store i16 %y, ptr %pb")
        .replace("  br label %h\n\nh:\n  %i = phi i16 [ 0, %b0 ], [ %i.next, %body ]\n  %more = icmp slt i16 %i, %n\n  br i1 %more, label %body, label %done\n\nbody:\n",
                 "  %go = icmp sgt i16 %n, 0\n  br i1 %go, label %pre, label %done\n\npre:\n  br label %body\n\nbody:\n  %i = phi i16 [ 0, %pre ], [ %i.next, %body ]\n")
        .replace("  %i.next = add i16 %i, 1\n  br label %h\n", "  %i.next = add i16 %i, 1\n  %more = icmp slt i16 %i.next, %n\n  br i1 %more, label %body, label %exit\n\nexit:\n  br label %done\n");
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// A counter that falls is the same question with the cells met the other
/// way: `a[n - i]` written and `a[n - i - 1]` read are one loop.
#[test]
fn a_counter_that_falls_is_ordered_by_its_own_direction() {
    let text = looping(
        "  %fo = sub i16 18, %o\n  %fa = getelementptr i8, ptr %a, i16 %fo\n  store i16 %i, ptr %fa\n  %fb = getelementptr i8, ptr %fa, i16 -2\n  %x = load i16, ptr %fb\n  store i16 %x, ptr %pb",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}

/// Splitting gives the idiom a loop of its own, and no more: two stores that
/// are not one are the loop they were in, not two.
#[test]
fn what_is_not_an_idiom_stays_one_loop() {
    let text = looping(
        "  store i16 7, ptr %pa\n  %x = load i16, ptr %pc\n  %y = mul i16 %x, 3\n  store i16 %y, ptr %pb\n  %pb2 = getelementptr i8, ptr %pb, i16 24\n  %w = load i16, ptr %pc\n  %z = mul i16 %w, 5\n  store i16 %z, ptr %pb2",
    );
    let (module, gained) = split(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    same_answers(&module, &text);
}
