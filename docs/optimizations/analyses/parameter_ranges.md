> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/parameter_ranges.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/parameter_ranges.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/parameter_ranges.rs sha256:6b0a9597f892f80b2067c8bc8aa26f630b466f77448d0cc64c188099ca406af8 -->

# parameter_ranges

The range of each integer parameter of a body only the program calls, from
what its callers pass.

## How it works
`stamp` takes the bodies that are neither exported nor address-taken. For
each integer parameter that has users and no `range` attribute, it collects
what every direct call (`call` or `invoke`) passes. A call's actual is read
in its caller by [`crate::ranges`](ranges.md) under the interval scope of the call's
block, with the caller's own parameters' stamps, and for an operand with no
interval, computed from its operands two levels down. A call in a block
`ranges` finds unreachable passes nothing. The actuals are joined into
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

## Provides
`stamp`: a `range` attribute on each integer parameter whose callers all
pass a known range, and the set of bodies stamped. Consumer: the
`interprocedural` program pass in llrm-transforms, which calls it up to 4
times after its constant and inlining rounds when `Interprocedural::ranges`
is set (gcc's `-fipa-vrp`, -O2 and up), re-optimises the bodies it returns
and stops when none.

## Needs
[`crate::ranges`](ranges.md) (`bounds`, interval of an operand), [`crate::consts`](consts.md),
[`crate::effects`](../backend/effects.md) for `callee`, `llrm_mir::program::Program`.

## See also
LLVM's IPSCCP, which states an argument's range as a `range` attribute where
it has no single constant.
