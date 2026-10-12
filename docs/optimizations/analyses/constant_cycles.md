> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/constant_cycles.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/constant_cycles.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/constant_cycles.rs sha256:6533feff22595c9a98f898cc0e48605616b78af9385a40e7c118609e709227c1 -->

# constant_cycles

Sparse value propagation over phis, with distinct pending and overdefined
states, to find constants that only a cycle proves.

## How it works
Each value has a state: `Pending` (nothing met yet), `Known(fact)` or
`Overdefined`, with `Pending` the identity of the meet and two different
facts meeting to `Overdefined`. `propagated(unit, seeds, successors)` takes
the facts already known (`seeds`, from [`crate::consts`](consts.md)) and settles the
rest, which are the phis no seed has and the operations that depend on
them. A body with no such open phi returns its seeds at once.

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
