> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/homes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/homes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/homes.rs sha256:b96ef4bc0cead5a2e89a9eaf0bcb2acd00632c7ee493bcad32ea7e3f5075c7e1 -->

# homes

Marks each phi whose value also sits in a memory cell with `!llrm.home`,
naming a store of that cell, so a spill of the phi reads the cell instead of
paying for a slot. GCC's `REG_EQUIV` by store; LLVM has no counterpart.

## How it works
A phi `p` is in cell `c` when each input `v` was stored to `c` and nothing
has written `c` since, at the end of the block `v` comes from, and nothing
writes `c` while `p` is live. `p` is then what `c` holds at every point it
is read. A store of `p` itself writes what is there. Only stores of an exact
cell (an object's own bytes at a constant offset, non-volatile) are
candidates; liveness and the paths between a store and a block are computed
once per function.

The mark is a hint. The backend re-checks it against the body as selection
leaves it (`storedhomes`: nothing writes the cell while the value lives, and
every definition is a plain copy). Where such a value is spilled it has no
store of its own and its restore is the load promotion removed.

## Example
Before:

```mir
define double @f(i32 %n, double %d) {
pre:
  %cell = alloca double
  %other = alloca double
  store double 0.0, ptr %cell
  br label %loop

loop:
  %i = phi i32 [ 0, %pre ], [ %j, %loop ]
  %s = phi double [ 0.0, %pre ], [ %t, %loop ]
  %t = fadd double %s, %d
  store double %t, ptr %cell
  store double %d, ptr %other
  %j = add i32 %i, 1
  %c = icmp slt i32 %j, %n
  br i1 %c, label %loop, label %done

done:
  ret double %t
}
```

After:

```mir
loop:
  %i = phi i32 [ 0, %pre ], [ %j, %loop ]
  %s = phi double [ 0.000000e+00, %pre ], [ %t, %loop ], !llrm.home !0
  %t = fadd double %s, %d
  store double %t, ptr %cell
; ...

!0 = !{i32 7}
```

`%s` is what `%cell` holds whenever it is read: `%t` is stored to the cell
on every trip and nothing else writes it. `!0` is the id of that store. The
counter `%i` is stored nowhere and is not marked.

From unit test `test_a_stored_loop_carried_value_is_its_cells` in
`homes_tests.rs`.

## Runs
Last in the module pipeline, after `spares`, so the instruction ids it names
are the ones selection sees. On whenever optimizing.

## Needs
`Summaries`, `memoryssa::Accesses`, liveness.

## Keeps
Adds metadata only; no instruction changes.

## See also
Consumers: the spillers, `floatassign` and `isel` in `llrm-codegen`. Tests:
`homes_tests.rs`.
