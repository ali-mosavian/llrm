> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/tailrec.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/tailrec.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/tailrec.rs sha256:c4cc81a6df49d9b8afdc3266e5d78e9d013588181d5284da5a4f30b0ffa9a213 -->

# tailrec

A call a function makes to itself as the last thing it does becomes a
branch to its own entry with the arguments rewritten.

## How it works
A site is a block that ends in `ret`, or in a `br` to a join block holding
only phis and a `ret` (the frontend's one return), where the last work
before the end is a self call: same callee, type and calling convention,
not marked `notail`. Only work with no effect may follow the call, and it
moves above it.

The call's result may also feed one `add`, `mul`, `and`, `or` or `xor`
that is returned (`return f(n - 1) + x`). Then the loop carries an
accumulator: a phi in the loop header that starts at the operation's
identity (0, 1 for `mul`, all ones for `and`), takes the other operand of
the operation on each trip, and is combined with the value of every other
`ret`. Of two calls summed, the later is the tail and the earlier stays.
All sites of one function must use the same operation; sites of another
keep their calls.

The entry block keeps the `alloca`s and branches to a new header
`tailrecurse`, which takes the rest of the entry. Each parameter that some
site changes gets a phi there (the entry's value, then each site's
argument); a parameter every site passes on unchanged gets none, and keeps
`noalias`, which the changed ones lose.

```text
  before                        after
  b0: ...                       b0:  (allocas) br tailrecurse
      %v = call f(%n-1)         tailrecurse:
      %s = add %v, %x             %n'  = phi [%n, b0], [%n-1, tailrecurse]
      ret %s                      %acc = phi [0, b0], [%acc+%x, ...]
                                  ...
                                  br tailrecurse     ; was the call
                                other exits: ret (%acc + value)
```

The pass refuses a function that has a frame object whose address escapes
(`ExposedFrames`: a callee could read what the next trip overwrites), an
`alloca` outside the entry or of a dynamic size, a `byval`, `byref`,
`inalloca` or `sret` parameter or argument, or a call to a `returns_twice`
routine.

Profitability: the carried values (every parameter the body reads, plus
the accumulator) must stay in registers. Where a call remains in the loop
they must fit in the target's registers kept across a call with one to
spare; otherwise in any register (`profit::registers`, [`crate::spill`](spill.md)).
Spilled, the loop costs the stores and reloads the pushed arguments were.
A target that prices no registers always passes. `LLRM_DEBUG=tailrec`
prints the carried count and the room.

## Example
Before:
```mir
define ... @pipeline.body(i16 %0, i16 %1) ... {
b1:
  %2 = icmp eq i16 %0, 0
  br i1 %2, label %b3, label %b2

b2:
  %3 = add nsw i16 %1, %0
  %4 = sub nsw i16 %0, 1
  %5 = call regparm3cc addrspace(1) i16 @"_sumto@3"(i16 %4, i16 %3)
  br label %b4

b3:
  br label %b4

b4:
  %6 = phi i16 [ %5, %b2 ], [ %1, %b3 ]
  ret i16 %6
}
```
After:
```mir
define ... @pipeline.body(i16 %0, i16 %1) ... {
b1:
  br label %tailrecurse

tailrecurse:
  %2 = phi i16 [ %0, %b1 ], [ %6, %b2 ]
  %3 = phi i16 [ %1, %b1 ], [ %5, %b2 ]
  %4 = icmp eq i16 %2, 0
  br i1 %4, label %b3, label %b2

b2:
  %5 = add nsw i16 %3, %2
  %6 = sub nsw i16 %2, 1
  br label %tailrecurse

b3:
  br label %b4

b4:
  %7 = phi i16 [ %3, %b3 ]
  ret i16 %7
}
```
The self call is a branch to the new header `tailrecurse`; `%2` and `%3`
carry `n` and the accumulated `x` round the loop.

From `--dump` of `sumto` (`if (n == 0) return x; return sumto(n - 1, x +
n);`) at -O2 -m16, stages `dead` and `tailrec`.

## Runs
In the function pipeline after `gvn::EqualCalls`, so two equal calls are
one first, and before `loopsimplify`, so the loop it makes goes to the
loop passes. Once per round of the fixed point; at -O2 and above, off at
-O0 and -O1 and by `-fno-optimize-sibling-calls`.

## Needs
`ExposedFrames` for the escape check; `profit::registers` for the room.

## Keeps
Nothing: a change invalidates every analysis.

## See also
LLVM's TailRecursionElimination and gcc's `tree-tailcall.cc`, which also
accumulate through associative operations.
