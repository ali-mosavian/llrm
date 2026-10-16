//! Versioning on the interpreter: the answers before and after, and the loops.

use std::collections::BTreeSet;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::module::Module;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use super::{found, versioned};
use crate::testing::{f, managed, parsed, printed, results};

/// The pass without its judge: the first loop that has a stride, versioned.
struct Unjudged;

impl FunctionPass for Unjudged {
    fn name(&self) -> &'static str {
        "version-unjudged"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(mut wanted) = found(unit, analyses, &BTreeSet::new()) else { return PreservedAnalyses::all() };
        versioned(unit.context, unit.declared, unit.function, &mut wanted).expect("a loop it copies");
        PreservedAnalyses::none()
    }
}

const INPUTS: &[&[i128]] = &[&[0, 1], &[5, 1], &[5, 2], &[4, 3], &[-2, 1], &[3, 0]];

fn loop_count(module: &mut Module) -> usize {
    let function = f(module);
    loops::loops(&cfg::graph(function), function.entry().map(cfg::id)).len()
}

/// `text` through the pass: the module, and the loops it gained.
fn copied(text: &str) -> (Module, usize) {
    let mut module = parsed(text);
    let before = loop_count(&mut module);
    managed(&mut module, Unjudged);
    let gained = loop_count(&mut module) - before;
    (module, gained)
}

/// Stores `i + 1` at `a[i * s]` for `%i` below `%n`, with `%s` the stride, and
/// returns the sum of the cells it may have written.
fn striding(address: &str) -> String {
    let mut sum = String::from("  %t0 = add i16 0, 0\n");
    for cell in 0..40 {
        sum += &format!(
            "  %c{cell} = getelementptr i8, ptr %a, i16 {}\n  %v{cell} = load i16, ptr %c{cell}\n  %w{cell} = mul i16 %v{cell}, {}\n  %t{} = add i16 %t{cell}, %w{cell}\n",
            cell * 2,
            cell + 1,
            cell + 1
        );
    }
    format!(
        "define i16 @f(i16 %n, i16 %s) {{
b0:
  %a = alloca [64 x i16]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
{address}
  %x = add i16 %i, 1
  store i16 %x, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
{sum}  ret i16 %t40
}}
"
    )
}

const BY_STRIDE: &str = "  %k = mul i16 %i, %s\n  %o = mul i16 %k, 2\n  %p = getelementptr i8, ptr %a, i16 %o";

/// An address that steps by `%s` is copied for `%s` of 1, behind a test of it,
/// and every answer is the loop's, for a stride of 1, 2, 0 or 3.
#[test]
fn a_loop_that_steps_by_a_stride_is_copied_for_one() {
    let text = striding(BY_STRIDE);
    let (module, gained) = copied(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    assert!(printed(&module).contains("icmp eq i16 %s, 1"), "{}", printed(&module));
    assert_eq!(results(&module, INPUTS), results(&parsed(&text), INPUTS), "{}", printed(&module));
}

/// A stride that is a constant is no unknown.
#[test]
fn a_loop_with_a_constant_stride_is_not_copied() {
    let text = striding("  %k = mul i16 %i, 3\n  %o = mul i16 %k, 2\n  %p = getelementptr i8, ptr %a, i16 %o");
    let (module, gained) = copied(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// A product of two unknowns that no address reads is not a stride.
#[test]
fn a_product_no_address_uses_is_not_a_stride() {
    let text = striding(
        "  %k = mul i16 %i, %s\n  %o = mul i16 %i, 2\n  %p = getelementptr i8, ptr %a, i16 %o\n  %unused = add i16 %k, 1",
    );
    let (module, gained) = copied(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// The pass with its judge keeps nothing the price does not like: the copy and
/// the original are two loops, which no pipeline has yet cleaned.
#[test]
fn a_copy_that_saves_nothing_is_not_kept() {
    let text = striding(BY_STRIDE);
    let mut module = parsed(&text);
    managed(&mut module, super::Version { passes: Vec::new() });
    assert_eq!(loop_count(&mut module), 1, "{}", printed(&module));
}

/// `p[i] = q[i] + q[i] + 1` for `%n` trips, `p` one of two arrays or a third,
/// `q` one of two arrays or the third, shifted by `%k` cells: the pointers are
/// values the loop does not define and nothing says they differ. Returns every
/// cell.
fn through_pointers() -> String {
    let mut init = String::new();
    let mut sum = String::from("  %t0 = add i16 0, 0\n");
    let mut at = 0;
    for (array, base) in [("a", 100), ("b", 300), ("m", 500)] {
        for cell in 0..24 {
            init += &format!(
                "  %i{array}{cell} = getelementptr i8, ptr %{array}, i16 {}\n  store i16 {}, ptr %i{array}{cell}\n",
                cell * 2,
                base + cell * 7
            );
            sum += &format!(
                "  %r{array}{cell} = getelementptr i8, ptr %{array}, i16 {}\n  %v{array}{cell} = load i16, ptr %r{array}{cell}\n  %w{array}{cell} = mul i16 %v{array}{cell}, {}\n  %t{} = add i16 %t{at}, %w{array}{cell}\n",
                cell * 2,
                at % 11 + 1,
                at + 1
            );
            at += 1;
        }
    }
    format!(
        "define i16 @f(i16 %n, i16 %c1, i16 %c2, i16 %k) {{
b0:
  %a = alloca [64 x i16]
  %b = alloca [64 x i16]
  %m = alloca [64 x i16]
{init}  %z1 = icmp eq i16 %c1, 0
  %p = select i1 %z1, ptr %a, ptr %m
  %z2 = icmp eq i16 %c2, 0
  %qb = select i1 %z2, ptr %b, ptr %m
  %shift = mul i16 %k, 2
  %q = getelementptr i8, ptr %qb, i16 %shift
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done

body:
  %o = mul i16 %i, 2
  %pi = getelementptr i8, ptr %p, i16 %o
  %qi = getelementptr i8, ptr %q, i16 %o
  %x = load i16, ptr %qi
  %x2 = load i16, ptr %qi
  %xs = add i16 %x, %x2
  %y = add i16 %xs, 1
  store i16 %y, ptr %pi
  %i.next = add i16 %i, 1
  br label %h

done:
{sum}  ret i16 %t{at}
}}
"
    )
}

/// Two pointers that may be one array are tested for overlap, and the copy
/// entered where they do not: every answer is the loop's, for every way `p` and
/// `q` meet, shifted or not, and the copy's pointers are the restrict ones.
#[test]
fn pointers_that_may_meet_are_tested_and_the_copy_told_they_do_not() {
    let text = through_pointers();
    let (module, gained) = copied(&text);
    assert_eq!(gained, 1, "{}", printed(&module));
    assert!(printed(&module).contains("@llrm.noalias.p0"), "{}", printed(&module));
    let mut inputs = Vec::new();
    for n in [0, 1, 5, 8] {
        for (c1, c2) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            for k in [0, 1, 2, 4, 7] {
                inputs.push(vec![n, c1, c2, k]);
            }
        }
    }
    let inputs = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert_eq!(results(&module, &inputs), results(&parsed(&text), &inputs), "{}", printed(&module));
}

/// The answer is the test itself: the entry returns it where it would go to
/// the copy or the loop.
fn the_test(text: &str) -> Module {
    let mut module = parsed(text);
    managed(&mut module, Unjudged);
    let (context, function) = module.function_mut("f").expect("@f");
    let entry = function.entry().expect("an entry");
    let branch = function.terminator(entry).expect("a terminated entry");
    let condition = function.instruction(branch).operands[0];
    let (word, void) = (context.types.int(16), context.types.void());
    let wide = function.create_instruction(
        llrm_mir::opcode::Opcode::Cast(llrm_mir::opcode::CastOp::ZExt),
        word,
        vec![condition],
        llrm_mir::opcode::Flags::default(),
        None,
    );
    function.insert(wide, llrm_mir::edit::Position::Before(branch)).expect("placed");
    let result = llrm_mir::module::Operand::Value(function.instruction(wide).result.expect("a value"));
    let leave = function.create_instruction(
        llrm_mir::opcode::Opcode::Ret,
        void,
        vec![result],
        llrm_mir::opcode::Flags::default(),
        None,
    );
    function.insert(leave, llrm_mir::edit::Position::Before(branch)).expect("placed");
    function.erase(branch).expect("a branch nothing reads");
    module
}

/// The ranges of `p[0..n]` and `q[0..n]`, `q` shifted by `k` cells, meet only
/// in one array and only where the shift, either way, is less than the trips:
/// the test is true elsewhere. The arithmetic wraps, so the test is the
/// distance of the starts against each range's length, both ways.
#[test]
fn the_test_holds_exactly_where_the_ranges_do_not_meet() {
    let module = the_test(&through_pointers());
    for n in [1, 2, 5, 8] {
        for (c1, c2) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            for k in [-8i128, -5, -4, -1, 0, 1, 2, 4, 5, 7, 8] {
                let same_array = c1 == 1 && c2 == 1;
                let meet = same_array && k.abs() < n;
                let got = results(&module, &[&[n, c1, c2, k]]);
                let wanted = i128::from(!meet);
                assert!(
                    matches!(got[0], llrm_mir::interpret::Val::Int { bits, .. } if bits as i128 == wanted),
                    "n {n} c {c1} {c2} k {k}: {got:?}"
                );
            }
        }
    }
}

/// A copy is for what restrict lets the passes take out: a loop that reads each
/// cell once and writes another has nothing to take. The copy of QCport's
/// `d_faces` loops that did cost 52% of its -O3 compile (a pipeline over the
/// body, twice, for each run of it), for nothing.
#[test]
fn pointers_with_nothing_to_take_out_of_the_loop_are_not_copied() {
    let text = through_pointers().replace(
        "  %x2 = load i16, ptr %qi\n  %xs = add i16 %x, %x2\n  %y = add i16 %xs, 1\n",
        "  %y = add i16 %x, 1\n",
    );
    let (module, gained) = copied(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// A trial runs the pipeline over the whole body twice, so a big body is not
/// tried: QCport's `d_faces` at -O3 cost 52% more to compile for it, and kept
/// nothing.
#[test]
fn a_big_body_is_not_tried() {
    let padding = (0..250).map(|at| format!("  %pad{at} = add i16 %n, {at}\n")).collect::<String>();
    let text =
        through_pointers().replace("  %shift = mul i16 %k, 2\n", &format!("{padding}  %shift = mul i16 %k, 2\n"));
    let (module, gained) = copied(&text);
    assert_eq!(gained, 0, "{}", printed(&module));
}

/// The body a copy is judged against (cleaned by the pipeline, then priced) was
/// a clone of the function made for the judgement, a second copy of the body
/// for each trial: it is now priced in place and rolled back, so a trial copies
/// the body once, for the candidate.
#[test]
fn a_trial_copies_the_body_once_for_the_candidate_and_not_again_for_the_judgement() {
    let text = striding(BY_STRIDE);
    let clones = |judged: bool| {
        let mut module = parsed(&text);
        let before = llrm_mir::journal::clones();
        if judged {
            managed(&mut module, super::Version { passes: Vec::new() });
        } else {
            managed(&mut module, Unjudged);
        }
        llrm_mir::journal::clones() - before
    };
    let (unjudged, judged) = (clones(false), clones(true));
    assert_eq!(judged - unjudged, 1, "one candidate, and nothing more copied to judge it by");
}
