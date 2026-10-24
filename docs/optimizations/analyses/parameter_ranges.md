> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/parameter_ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/parameter_ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/parameter_ranges.rs sha256:24e03b31d42c158e8b5bc5c7de44d1e492f585b371ef6f2c88d9b0645642406f -->

# parameter_ranges

Bounds each integer parameter of a body only the program calls by what its
callers pass, so the body's own checks on it can be decided.

## How it works
`stamp` takes the bodies that are neither exported nor address-taken. For
each integer parameter that has users and no `range` attribute, it collects
what every direct call (`call` or `invoke`) passes. A call's actual is read
in its caller by [`crate::ranges`](ranges.md) under the branch edges' intervals at the
call's block, with the caller's own parameters' stamps, and for an operand
with no interval, computed from its operands two levels down. The actuals
are joined into
`Seen`: `Nothing`, `Within(interval)` or `Anything`. A caller that has not
itself been seen yet is `waiting` and does not turn an unknown actual into
`Anything`.

Rounds repeat, each stamping what the last round saw as a `range` attribute
on the callee, so the next round's callers read it. A recursion that passes
`row + 1` under `row != 7` is found this way.

```text
 f(row) called with 0, and by f itself as f(row + 1) under row != 7
 round 0   callers outside pass 0            row in [0,0]
 round 1   f(row + 1) under row != 7         row in [0,1]
 round 2   still growing: high jumps to the next number f compares row
           with (6, 7 or 8)                  row in [0,6]
 round 3   6 + 1 is 7: high jumps to 7       row in [0,7]
 round 4   no change: fixed point, stamps stay
```

From the third round (`round >= 2`) a bound that still moves jumps to the
next number the callee compares the parameter with (one either side of each
`icmp` constant, then the type's bounds). After `ROUNDS` (8) with no fixed
point every stamp of the call is withdrawn and nothing is returned: a stamp
is a promise, and only a fixed point keeps it. A parameter seen to be
`Anything` loses an earlier stamp. A range covering the whole type states
nothing and is not stamped.

## Example
`@place` is called from `@f` with 0 and from itself with `%row + 1`:

```mir
define internal i16 @place(i16 %row, i16 %n) {
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %more
more:
  %fits = icmp ult i16 %row, 12
  br i1 %fits, label %next, label %crash
next:
  %up = add nsw i16 %row, 1
  %r = call i16 @place(i16 %up, i16 %n)
  ret i16 %r
leaf:
  ret i16 1
crash:
  ret i16 99
}

define i16 @f(i16 %a) {
b:
  %x = call i16 @place(i16 0, i16 7)
  ret i16 %x
}
```

The printed body then declares the parameter as `range(i16 0, 8) %row`.

From unit test
`test_what_the_callers_pass_bounds_a_parameter_the_body_checks` in
`crates/opt/llrm-transforms/src/interprocedural_tests.rs`.

## Provides
`stamp`: a `range` attribute on each integer parameter whose callers all
pass a known range, and the set of bodies stamped. Consumer: the
`interprocedural` program pass in llrm-transforms, which calls it up to 4
times after its constant and inlining rounds when `Interprocedural::ranges`
is set (gcc's `-fipa-vrp`, -O2 and up), re-optimises the bodies it returns
and stops when none.

## Needs
[`crate::ranges`](ranges.md) (`dominated_edges`, interval of an operand),
[`crate::consts`](consts.md),
[`crate::effects`](../backend/effects.md) for `callee`, `llrm_mir::program::Program`.

## See also
LLVM's IPSCCP, which states an argument's range as a `range` attribute where
it has no single constant.
