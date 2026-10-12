> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/argpromotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/argpromotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/argpromotion.rs sha256:14f4cda3c67340c3f1ea6cfbf93e6f9cf1168fc435ce01cc5393e46009755899 -->

# argpromotion

Argument promotion (LLVM's ArgumentPromotion): a pointer parameter a
function only reads at fixed offsets is replaced by the fields read.

## How it works
Only functions `direct_only` accepts are candidates: internal, never named
but as a callee, and every call a plain call with as many arguments as
parameters. Functions are visited callees first, and each pointer parameter
from the last to the first.

A parameter is promoted when every use of it is one of

- a non-volatile load at a fixed non-negative offset (through GEPs with
  constant offsets, which are erased);
- the same parameter passed on, in that position, to the function's own
  recursive call.

The loads must be of at most four distinct fields (`MOST_FIELDS`), each read
as one type, and not overlap. A caller makes the loads before the call, so
they must be safe there and read what the function would have read:

- a field the function loads in its entry block before any call or write is
  the same load, or
- the parameter is `dereferenceable` for the field's bytes and nothing
  writes them while the function runs (the pointer is `memory::invariant`,
  or the function writes no memory).

```text
 caller:  call f(ptr %d, i16 %k)           %n = load i16, ptr %d+4
 f:       %n = load i16, ptr %d+4  ==>     call f(i16 %n, i16 %k)
          ... uses %n                f:    ... uses the parameter %n
```

The function's loads of a field are replaced by the new parameter, the
parameter list is rewritten, and each call is rewritten to match; a call
from the function to itself passing the parameter on passes the fields it
holds instead, with no load. The first load's metadata is kept on the
caller's load.

Profitability is priced by the target for the level being built
(`OperationCosts`): the words each call pushes more (`argument`) and the
loads each caller gains (`load`), against the loads the function loses. At
-Os the price is in bytes and the loads count at every call; otherwise in
clocks, where a call inside a loop whose pointer the loop does not change
counts no loads (they will be hoisted). A recursive function is promoted
when the extra words cost no more than the loads it saves.

A slice descriptor read for its length (a Nib `&mut [T]`, a struct by
pointer in C, a BASIC array descriptor) is the case it was written for: each
call pushed a far pointer that the callee loaded one word through.

## Runs
In the interprocedural step, once, after its inlining loop and before
constant propagation of the callers' arguments, so that a length that now
crosses the call as a value is propagated into the body. Every body it
changes is run through the function pipeline again. Not at -O0.

## See also
[`crate::deadargs`](deadargs.md), [`crate::narrowspace`](narrowspace.md); LLVM's ArgumentPromotion.
