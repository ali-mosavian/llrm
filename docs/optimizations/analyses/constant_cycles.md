> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/constant_cycles.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/constant_cycles.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/constant_cycles.rs sha256:d0d1ee5bb318b977b982dfa7ccd4958a6340a57251827f51fb472f247e0dcb7b -->

# constant_cycles

Sparse value propagation over phis, with distinct pending and overdefined
states, to find constants that only a cycle proves.

## How it works
Each value has a state: `Pending` (nothing met yet), `Known(fact)` or
`Overdefined`, with `Pending` the identity of the meet and two different
facts meeting to `Overdefined`. `propagated(unit, seeds, successors, only)`
takes the facts already known (`seeds`, from [`crate::consts`](consts.md)) and settles
the rest, which are the phis no seed has and the operations that depend on
them: all of them, or the values `only` names (an incremental `Registers`
brings up to date the values a change reached and no others). Values that
cannot come out known (one made from an argument, a load or a call) are left
out of the propagation. A body with no such open phi returns its seeds at
once.

A worklist re-evaluates a value when one of its inputs changes. A phi meets
the facts of its incoming values (constants evaluate through
`consts::_operand`); a supported operation (a foldable binary operation,
`trunc`, `sext`) folds through `consts::_result` when every input is known,
stays pending while some input is pending and none is overdefined, and
otherwise is overdefined. When nothing is left on the worklist and pending
values remain, they are made overdefined and propagation continues.
Pending is never read as LLVM `undef`: an unresolved value is overdefined
before the result is returned.

```text
 b1:  %i = phi [0, b0], [%j, b1]    %i: meet(Known 0, pending) = Known 0
      %j = add %i, 0                %j: Known 0
                                    %i again: meet(Known 0, Known 0), stable
```

An optional successor evaluator (`Successors { choose, reads }`) finds the
executable edges. Only the entry is live at first; a block's `choose`
answers which successors run from the facts so far, and is asked again only
when a value in `reads` for that block changes. A phi then meets only the
incoming values of executable edges, and the entry's phis also meet
`Overdefined`, since the entry runs before any back edge. A block whose
branch `choose` defers (`None`) keeps every successor once nothing else is
left to resolve.

## Example
A loop whose carried value adds 0 to itself, so only the cycle proves it:

```mir
define void @f(i32 %incoming, i1 %more) {
b0:
  br label %b10

b10:
  %joined = phi i32 [ 7, %b0 ], [ %carried, %b10 ]
  %carried = add i32 %joined, 0
  br i1 %more, label %b10, label %b20

b20:
  ret void
}
```

| Value | Fact |
| --- | --- |
| `%joined` | `Known { n: 7, width: 32 }` |
| `%carried` | `Known { n: 7, width: 32 }` |

With step 1, or with `%incoming` as the source of the add, neither is known.

From unit test `test_unchanged_loop_value_is_constant_through_its_backedge`
in `constant_cycles_tests.rs`.

## Provides
`State`, `Successors` and `propagated`. Consumers: [`crate::consts`](consts.md)
(`known`, with no evaluator) and `decide` in `llrm-transforms`, which
passes an evaluator to find which branches fold.

## Needs
[`crate::consts`](consts.md) for the per-operation folding, and [`crate::cfg`](cfg.md).

## See also
LLVM SCCP (`SCCPSolver`) and its `unknown`, `constant`, `overdefined`
lattice. `LLRM_CHECK_CYCLES` re-derives the result without the early
return and asserts it matches.
