//! What `admitted` answers for loops the interpreter could run: each of
//! GCC's refusals, and a copy that folds away being taken past them. The
//! old peelsize had no tests of its own; unroll's covered it.

use crate::graph::loops::{self, Loop};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use num_bigint::BigInt;

use super::{ENTRY, Limits, Site, admitted};
use crate::cfg;
use crate::consts;
use crate::induction;
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed};

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

    /// The outermost loop.
    fn outer(&self) -> Loop {
        let found = loops::loops(&cfg::graph(function(&self.module, "f")), None);
        found.into_iter().max_by_key(|one| one.body.len()).expect("a loop")
    }

    /// Whether the outermost loop, at the count induction proves, is admitted.
    fn admitted(&self, limits: &Limits) -> bool {
        self.entered(limits, ENTRY)
    }

    /// `admitted`, the loop entered `entries` 256ths of the times its function is.
    fn entered(&self, limits: &Limits, entries: i64) -> bool {
        let unit = self.unit();
        let facts = consts::known(&unit, None, None, None);
        let loop_ = self.outer();
        let count = induction::trip_count(&unit, &loop_, &facts).expect("a proven count");
        admitted(&unit, &loop_, &count, &facts, limits, Site { entries, ..Site::default() })
    }
}

/// `%acc` summed with `%i` for `trips` trips, and `work` more of the body.
fn summing(trips: u32, start: &str, work: &str) -> Parsed {
    Parsed::new(&format!(
        "declare void @g()
declare void @h(i16, i16, i16, i16, i16, i16)

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ {start}, %b0 ], [ %sum, %b2 ]
  %go = icmp slt i16 %i, {trips}
  br i1 %go, label %b2, label %b3

b2:
  %sum = add i16 %acc, %i
{work}  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}}
"
    ))
}

#[test]
fn a_copy_no_larger_than_the_loop_is_admitted_even_under_os() {
    let os = Limits { grows: false, ..Limits::default() };
    assert!(summing(4, "%x", "").admitted(&os));
    assert!(!summing(8, "%x", "").admitted(&os));
    assert!(summing(8, "%x", "").admitted(&Limits::default()));
}

/// Every add folds once `%acc` starts known: nothing is left to copy.
#[test]
fn a_copy_that_folds_away_is_admitted_whatever_it_would_have_grown_to() {
    let os = Limits { grows: false, ..Limits::default() };
    assert!(summing(10, "0", "").admitted(&os));
}

#[test]
fn past_max_completely_peel_times_nothing_is_copied() {
    assert!(summing(10, "0", "").admitted(&Limits::default()));
    assert!(!summing(11, "0", "").admitted(&Limits::default()));
    assert!(summing(17, "0", "").admitted(&Limits { max_unroll_iterations: 0, ..Limits::default() }));
}

/// A call is priced, one and its arguments, not refused: GCC's refusal
/// kept FPDEEP's three PRINT trips rolled, nothing downstream folding `i`.
#[test]
fn a_call_on_the_path_is_priced_not_refused() {
    let tight = Limits { max_unrolled_operations: 20, ..Limits::default() };
    assert!(summing(8, "%x", "  call void @g()\n").admitted(&Limits::default()));
    assert!(summing(8, "%x", "  call void @g()\n").admitted(&tight));
    assert!(!summing(8, "%x", "  call void @h(i16 %i, i16 %i, i16 %i, i16 %i, i16 %i, i16 %i)\n").admitted(&tight));
}

/// Eight unknown adds and the rolled work boosting the budget fourfold:
/// a budget of one refuses, the default admits.
#[test]
fn over_max_completely_peeled_insns_is_refused() {
    assert!(!summing(8, "%x", "").admitted(&Limits { max_unrolled_operations: 1, ..Limits::default() }));
    assert!(summing(8, "%x", "").admitted(&Limits { max_unrolled_operations: 2, ..Limits::default() }));
}

/// A branch on `%x` no iteration decides, once or twice a trip, twelve trips.
fn branching(twice: bool) -> Parsed {
    let second = if twice { "%c2, label %b5, label %b6" } else { "true, label %b5, label %b6" };
    Parsed::new(&format!(
        "define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b7 ]
  %acc = phi i16 [ 0, %b0 ], [ %out, %b7 ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %b2, label %b8

b2:
  %c = icmp sgt i16 %x, %i
  br i1 %c, label %b3, label %b4

b3:
  %up = add i16 %acc, 3
  br label %b4

b4:
  %mid = phi i16 [ %acc, %b2 ], [ %up, %b3 ]
  %c2 = icmp sgt i16 %x, 100
  br i1 {second}

b5:
  %more = add i16 %mid, 1
  br label %b6

b6:
  %out = phi i16 [ %mid, %b4 ], [ %more, %b5 ]
  %next = add i16 %i, 1
  br label %b7

b7:
  br label %b1

b8:
  ret i16 %acc
}}
"
    ))
}

#[test]
fn past_max_peel_branches_undecided_branches_are_refused() {
    assert!(branching(false).admitted(&Limits::default()));
    assert!(!branching(true).admitted(&Limits::default()));
}

/// An outer loop of two trips around an inner one: copied only when that shrinks it.
#[test]
fn a_loop_holding_another_is_copied_only_when_it_shrinks() {
    let nested = Parsed::new(
        "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %acc = phi i16 [ %x, %b0 ], [ %inner, %b4 ]
  %go = icmp slt i16 %i, 2
  br i1 %go, label %b2, label %b5

b2:
  %j = phi i16 [ 0, %b1 ], [ %jn, %b3 ]
  %inner = phi i16 [ %acc, %b1 ], [ %s, %b3 ]
  %jgo = icmp slt i16 %j, %n
  br i1 %jgo, label %b3, label %b4

b3:
  %s = add i16 %inner, %j
  %jn = add i16 %j, 1
  br label %b2

b4:
  %next = add i16 %i, 1
  br label %b1

b5:
  ret i16 %acc
}
",
    );
    assert!(!nested.admitted(&Limits::default()));
}

#[test]
fn the_count_is_the_one_asked_about() {
    let parsed = summing(4, "%x", "");
    let unit = parsed.unit();
    let facts = consts::known(&unit, None, None, None);
    assert!(admitted(&unit, &parsed.outer(), &BigInt::from(4), &facts, &Limits::default(), Site::default()));
    assert!(!admitted(&unit, &parsed.outer(), &BigInt::from(17), &facts, &Limits::default(), Site::default()));
}

/// QCport's savegame.c peeled loops its function enters once in 256 calls, 1 KB each, for clocks
/// nobody spends. A loop entered under one function entry in 20 is cold: a copy that grows is refused,
/// one that does not is still taken.
#[test]
fn a_cold_loop_is_not_copied_where_the_code_grows() {
    let limits = Limits::default();
    assert!(summing(8, "%x", "").entered(&limits, ENTRY / 20 + 1));
    assert!(!summing(8, "%x", "").entered(&limits, ENTRY / 256));
    assert!(summing(4, "%x", "").entered(&limits, 1));
    assert!(summing(10, "0", "").entered(&limits, 1));
}

/// QCport's 16-trip clear and fill loops (console.c, mdl.c) were copied 16 times, +100 to +400 bytes
/// each: LLVM analyses at most 10 iterations (`-unroll-max-iteration-count-to-analyze`).
#[test]
fn a_loop_of_more_than_ten_trips_is_not_copied() {
    assert!(summing(10, "%x", "").admitted(&Limits::default()));
    assert!(!summing(16, "%x", "").admitted(&Limits::default()));
}
