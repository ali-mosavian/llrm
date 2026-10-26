> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/uremloop.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/uremloop.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/uremloop.rs sha256:b7f3e57d3e8e3c5db8118db4f40d8d2b56889a01cb69f64b8a7f0e9b93a3485c -->

# uremloop

CodeGenPrepare's `foldURemOfLoopIncrement`: the remainder of a loop's
counter by a loop-invariant amount is a second counter that wraps
([CodeGenPrepare.cpp#L2227](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L2227)).

## How it works
`for (i = Start; i < End; ++i) r = (i + K) % M` with `M` and `K` invariant
and no constant `M` (a constant remainder is a multiply). The counter must
be a header phi of two inputs that steps by one with a `nuw` add. When the
first remainder, `(Start + K) % M`, simplifies away (`Start` is 0, say), the
division in the loop is replaced by a phi that counts up and drops to zero
on reaching `M`:

```text
 loop: %i = phi [0, %pre], [%n, %loop]      loop: %i = phi [0, %pre], [%n, %loop]
       %r = urem %i, %m               ==>         %r = phi [0, %pre], [%s, %loop]
       ...                                        %a = add nuw %r, 1
       %n = add nuw %i, 1                         %c = icmp eq %a, %m
                                                  %s = select %c, 0, %a
```

MIRROR: `isRemOfLoopIncrementWithLoopInvariant`, `getIVIncrement` and
`foldURemOfLoopIncrement`.

DEVIATE UREM1: `matchIncrement`'s `uadd.with.overflow` and `usub` forms are
not matched: nothing before this makes them (`combineToUAddWithOverflow` is
not translated). Exit: that translation.

## Example
`test_a_remainder_of_a_counter_by_an_invariant_is_a_wrapping_counter` in
`uremloop_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, before the store split.
