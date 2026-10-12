> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/tailrec.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/tailrec.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/tailrec.rs sha256:b91114ac2e8772aa8f04ae1c1b027d8e23eac030a1102eab29a536958a602d55 -->

# tailrec

A call a function makes to itself as the last thing it does is a branch to
its own entry with the arguments rewritten: LLVM's TailRecursionElimination,
and gcc's tree-tailcall.

Where the call's result only feeds an associative, commutative `add`, `mul`,
`and`, `or` or `xor` on its way out (`return f(n - 1) + x`), the call is
still one: an accumulator, `acc` in the loop's header, starts at the
operation's identity and takes `x` each trip, and every other way out of the
function returns `acc op value`. gcc's `fib` is `fib(n - 1)` called, `fib(n
- 2)` looped; `hanoi`'s second call is the same.

What changed with the IR:
- The call is found in MIR, where the frontend's returns through one join
  block are a block of phis and a `ret`; a block that branches to it takes
  the value of its arm, as LLVM's `foldReturnAndProcessPred` does by copying
  the `ret` there.
- The function's own id comes from the pass manager (`Unit::id`): LLVM
  compares the callee with `F`.
- A frame object whose address escapes keeps the call: the callee may read
  it. The one escape analysis (`frameescape`) answers; LLVM's
  `AllocaDerivedValueTracker` is its own.

The loop is the ordinary loop passes' to improve: the pass names nothing
about the machine.
