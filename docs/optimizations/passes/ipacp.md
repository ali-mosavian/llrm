> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ipacp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ipacp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ipacp.rs sha256:9586675d26239761749daba533becac4b2ccd676741ba91c94b0f18701727e1a -->

# ipacp

Copies a function for the constants its callers pass and redirects those
calls to the copy, so the copy's loops and branches fold on values the
original cannot assume: GCC's `ipa-cp` cloning.

## How it works
Each call whose callee is defined and copyable, and has at least one
actual that the caller's own folding proved constant
(`interprocedural::current_call_constants`), is grouped with the others
that pass the same constants. A group is a clone candidate when

- some site is hot: a site in a function other than `main`, or one whose
  block runs 1.5 times per call or more;
- the constants save work: `time_saved` runs an optimistic constant
  propagation over the callee with its parameters bound, and prices the
  instructions that fold or are only reached by a decided branch, at their
  block frequency (held to ten trips);
- the benefit pays for the size: `benefit * 1000 >= 500 * size`
  (`ipa-cp-eval-threshold`), where `benefit` is `saved / 5` plus the loop
  bonus (`ipa-cp-loop-hint-bonus`, 64, earned where a loop's exit tests a
  known value against a varying one), scaled by the sites' summed frequency,
  and `size` is the callee's operations; and the unit's growth stays within
  `ipa-cp-unit-growth` (10% of the unit, or of 16000 operations if larger).
  A function in a cycle with others, not one calling itself, loses
  `ipa-cp-recursion-penalty` (40%) of the benefit.

A callee that is private, not addressed, and called only from the group
needs no growth test: every call goes to the copy and the original dies.

A caller whose calls go to a copy that says what the callee said (the same
attributes) is vouched for (`Changed::unsound` names the others): its passes
need not look at the new name ([`crate::settled`](settled.md)).

The copy is an internal function `<origin>.constprop.N` with each known
parameter replaced by its constant. A later call with the same constants
(`Cloning::contexts`) goes to the copy already made. A function gets at
most eight clones (`ipa-cp-max-recursive-depth`), which also bounds the
recursion: a clone that calls itself with a new constant (`place(q, row + 1,
n)` in the clone for `row`) is a candidate for the next clone, so GCC's -O3
unrolls `queens` into eight, one a row. The copy is an ordinary function:
the inliner takes a clone with one caller into it.

GCC propagates lattices over the whole call graph and prices a value with
each function's time and size summaries under the context
(`good_cloning_opportunity_p`). Here a site's constants are those the
caller's own folding proved, and `saved` is the clocks `time_saved` finds
the callee no longer spends. The thresholds are GCC's.

## Example
With `Threshold::cp_clone` on. Before:

```mir
define i16 @g(i16 %k, i16 %x) {
; ... a loop that runs %k trips
}

define i16 @f(i16 %x) {
b0:
  %a = call i16 @g(i16 4, i16 %x)
  %b = call i16 @g(i16 5, i16 %x)
  %s = add i16 %a, %b
  ret i16 %s
}
```

After:

```mir
define i16 @f(i16 %x) memory(none) willreturn nounwind norecurse {
b0:
  %a = call i16 @g.constprop.1(i16 %x)
  %b = call i16 @g.constprop.2(i16 %x)
  %s = add i16 %a, %b
  ret i16 %s
}

define internal i16 @g.constprop.1(i16 %x) memory(none) willreturn nounwind norecurse {
; ...
head:
  ; ...
  %go = icmp slt i16 %i, 4
; ...
}
```

`@g` stays for other callers; `@g.constprop.2` is the same with 5.

From unit test
`test_a_function_called_with_two_constants_is_cloned_for_each_at_o3` in
`interprocedural_tests.rs`.

## Runs
In [`crate::interprocedural`](../analyses/interprocedural.md)'s constants loop, after constant arguments are
specialised, repeated until nothing changes; every changed body goes back
through the pipeline as `ipa-cp.`. Only where `Threshold::cp_clone` is set
(`-fipa-cp-clone`): -O3 and -Omax; without it cloning does nothing and -O2
gets only the constants every call agrees on. See
`docs/reference/levels.md`.

## Needs
[`crate::inline`](inline.md) (copyability, call counts, operation counts),
[`crate::profit`](profit.md) (prices, block frequencies), [`llrm_analysis::consts`](../analyses/consts.md).

## See also
GCC's
[`ipa-cp.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ipa-cp.cc).
