> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitsink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitsink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitsink.rs sha256:98f53b8dd0832728b359c8d564b589ed60558a1cd6bf720a54747f62ab4edfc1 -->

# exitsink

An add, subtract or extension in a loop that only exit phis read, moved to
those exits: LLVM's LICM sinking an instruction only used outside the loop.

## How it works
A candidate is an `add`, `sub`, `sext`, `zext` or `trunc` in a loop whose
result is read only by phis outside the loop, one of which has this value as
its only input (an LCSSA exit phi). The operation is computed in the exit,
in place of the phi, and the loop's copy goes once nothing reads it. Each
other exit that reads it takes its own copy in its turn.

An operand the loop defines is read through an exit phi, as LCSSA has it
(reusing one that exists); an invariant operand is read directly. The
rewrite repeats to a fixed point, so a chain of updates moves together.

```text
 loop:  %p1 = add %p, 2          loop:  (the add is gone)
        ...                             ...
 exit:  %r = phi [%p1, latch]    exit:  %q = phi [%p, latch]
                            ==>        %r = add %q, 2
```

The last trip's update of a pointer strength reduction carried is computed
once, on the way out, and so is the `sext` of a narrow counter whose value
`work += i` reads after the loop (mandel: `inc cx; movsx eax,cx; cmp cx,20h`
every trip).

## Example
Before (`looped("add i16 %i, 40", "%i.next")`):

```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
  %i.next = add i16 %i, 1
  %last = add i16 %i, 40
  %more = icmp ult i16 %i.next, %n
  br i1 %more, label %b1, label %b2

b2:
  %e = phi i16 [ %last, %b1 ]
  ret i16 %e
}
```

After, `b2` as the test asserts it (`%last` leaves `b1` once nothing reads
it):

```mir
b2:
  %i.lcssa = phi i16 [ %i, %b1 ]
  %0 = add i16 %i.lcssa, 40
  ret i16 %0
}
```

From unit test `test_final_update_moves_to_the_exit` in `exitsink_tests.rs`.

## Runs
Step 4 of [`crate::indvars`](indvars.md): on whenever `indvars` is, in the fixed point
and after the loop-header copy. Not at -O0; off with `-fno-strength-reduce`.

## Keeps
Blocks and edges are unchanged; dominators and loops are preserved.

## See also
[`crate::lcssa`](lcssa.md); LLVM LICM `sinkRegion`.
