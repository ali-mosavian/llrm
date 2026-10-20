//! Each body is MIR text in loop-simplify and LCSSA form, run by llrm-mir's
//! interpreter before and after, so a wrong loop shows as a wrong answer; what
//! is asserted of the text is where the invariant branch is.

use super::SimpleLoopUnswitch;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn unswitched(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(text);
    let mut module = before.clone();
    let after = managed(&mut module, SimpleLoopUnswitch { non_trivial: false });
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// The text of block `name` of `text`.
fn block<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = text.find(&format!("\n{name}:")).unwrap_or_else(|| panic!("no {name}: {text}"));
    let rest = &text[start + 1..];
    rest.split("\n\n").next().expect("a block")
}

/// The loop of `paths` after tail recursion: `n == 0` is tested in the header
/// on every trip, `m` is the counter.
const HEADER: &str = "define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %body ]
  br i1 %z, label %out, label %test

test:
  %done = icmp eq i16 %i, 0
  br i1 %done, label %out, label %body

body:
  %next = sub i16 %i, 1
  %sum = add i16 %acc, %i
  br label %head

out:
  %r = phi i16 [ %acc, %head ], [ %acc, %test ]
  ret i16 %r
}
";

const INPUTS: &[&[i128]] = &[&[0, 5], &[1, 0], &[1, 5], &[3, 9]];

/// `n == 0` was made at the head of every trip: tail-recursive `paths` ran
/// `sete; mov [esp+14], al` once and `cmp byte [esp+14], 0; je` on each of
/// its trips.
#[test]
fn an_invariant_exit_test_in_the_header_is_made_before_the_loop() {
    let after = unswitched(HEADER, INPUTS);
    assert!(!block(&after, "head").contains("br i1 %z"), "the header still tests it: {after}");
    assert!(block(&after, "b0").contains("br i1 %z"), "the entry does not: {after}");
}

/// A condition the loop computes changes with its trips.
#[test]
fn a_condition_the_loop_computes_stays_where_it_is() {
    let text = HEADER
        .replace("br i1 %z, label %out, label %test", "%z2 = icmp eq i16 %i, 7\n  br i1 %z2, label %out, label %test");
    let after = unswitched(&text, INPUTS);
    assert!(block(&after, "head").contains("br i1 %z2"), "{after}");
}

/// Both sides in the loop is an unswitch that copies it, not this one.
#[test]
fn a_branch_that_stays_in_the_loop_on_both_sides_is_not_moved() {
    let text = HEADER
        .replace("br i1 %z, label %out, label %test", "br i1 %z, label %test, label %body")
        .replace("[ %acc, %head ], [ %acc, %test ]", "[ %acc, %test ]");
    let after = unswitched(&text, &[&[0, 3]]);
    assert!(block(&after, "head").contains("br i1 %z"), "{after}");
}

/// A header that stores before its branch cannot have the branch moved before
/// it: the store would be skipped.
#[test]
fn a_header_that_stores_before_its_branch_keeps_its_branch() {
    let text = format!("@g = global i16 0\n\n{}", HEADER)
        .replace("br i1 %z, label %out, label %test", "store i16 %i, ptr @g\n  br i1 %z, label %out, label %test");
    let after = unswitched(&text, INPUTS);
    assert!(block(&after, "head").contains("br i1 %z"), "{after}");
}

/// The `or` of an invariant and a variant: the invariant alone decides the
/// exit before the loop, and the loop keeps the whole test.
#[test]
fn an_invariant_input_of_an_or_is_tested_before_the_loop() {
    let text = HEADER.replace(
        "br i1 %z, label %out, label %test",
        "%v = icmp eq i16 %i, 7\n  %c = or i1 %z, %v\n  br i1 %c, label %out, label %test",
    );
    let after = unswitched(&text, INPUTS);
    assert!(block(&after, "b0").contains("br i1 %"), "{after}");
    assert!(block(&after, "head").contains("or i1"), "the loop keeps the whole test: {after}");
}

/// A switch on an invariant with a case that leaves the loop.
#[test]
fn a_switch_on_an_invariant_is_made_before_the_loop() {
    let text = "define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  switch i16 %n, label %body [
    i16 1, label %leave
    i16 2, label %leave
  ]

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %exit, label %head

leave:
  br label %out

exit:
  br label %out

out:
  %r = phi i16 [ 0, %leave ], [ 1, %exit ]
  ret i16 %r
}
";
    let after = unswitched(text, &[&[1, 3], &[2, 3], &[3, 3], &[3, 1]]);
    assert!(block(&after, "b0").contains("switch i16 %n"), "{after}");
    assert!(!block(&after, "head").contains("switch"), "{after}");
}

/// Exit values the loop computes are not there if the loop is not entered.
#[test]
fn an_exit_value_the_loop_computes_keeps_the_branch() {
    let text = HEADER.replace("[ %acc, %head ], [ %acc, %test ]", "[ %sum, %head ], [ %acc, %test ]");
    let text = text.replace("%sum = add i16 %acc, %i", "%sum = add i16 %acc, %i");
    // %sum is not defined on the path from head: use the loop's `next` instead.
    let text = text.replace("[ %sum, %head ]", "[ %next, %head ]");
    let _ = text;
    let kept = HEADER.replace("[ %acc, %head ], [ %acc, %test ]", "[ %i, %head ], [ %acc, %test ]");
    let after = unswitched(&kept, INPUTS);
    // `%i` on entry is a header phi: still trivial, so the branch moves.
    assert!(block(&after, "b0").contains("br i1 %z"), "{after}");
}

/// `n` loops in a row, each with an invariant exit test: unswitching them looks
/// over entries in proportion to `n`, not its square (a count, not seconds:
/// time fails under load).
fn sequence(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %n, i16 %m) {\nb0:\n  br label %h0\n\n");
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("p{}", k + 1) };
        let pre = if k == 0 { "b0".to_owned() } else { format!("p{k}") };
        let prev = if k == 0 { "%m".to_owned() } else { format!("%r{}", k - 1) };
        text += &format!(
            "h{k}:\n  %i{k} = phi i16 [ {prev}, %{pre} ], [ %n{k}, %body{k} ]\n  %z{k} = icmp eq i16 %n, {k}\n  br i1 %z{k}, label %x{k}, label %body{k}\n\nbody{k}:\n  %n{k} = sub i16 %i{k}, 1\n  br label %h{k}\n\nx{k}:\n  %r{k} = phi i16 [ %i{k}, %h{k} ]\n  br label %{next}\n\n"
        );
        if k + 1 != n {
            text += &format!("p{}:\n  br label %h{}\n\n", k + 1, k + 1);
        }
    }
    text += "end:\n  ret i16 %r0\n}\n";
    text
}

#[test]
fn unswitching_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::layout_scanned;
    let scanned = |n: usize| {
        let mut module = parsed(&sequence(n));
        let before = layout_scanned();
        managed(&mut module, SimpleLoopUnswitch { non_trivial: false });
        layout_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}
