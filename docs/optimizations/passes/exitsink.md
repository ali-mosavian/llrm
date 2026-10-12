> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/exitsink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/exitsink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/exitsink.rs sha256:9d52456193ebc0db3f5e96a1c5ab12a9e38ff2e3da680dd243fd39de52bd4e49 -->

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

## Runs
Step 4 of [`crate::indvars`](indvars.md): on whenever `indvars` is, in the fixed point
and after the loop-header copy. Not at -O0; off with `-fno-strength-reduce`.

## Keeps
Blocks and edges are unchanged; dominators and loops are preserved.

## See also
[`crate::exitfold`](exitfold.md), [`crate::lcssa`](lcssa.md); LLVM LICM `sinkRegion`.
