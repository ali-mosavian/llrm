> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopexit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopexit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopexit.rs sha256:482c57810c44c27ff99fbb18a340f4993922362220bd6c3d3bac0f8ef7be8974 -->

# loopexit

Affine exit values evaluated, and finite counted loops with no effect
deleted: LLVM's IndVarSimplify exit-value rewriting and LoopDeletion.
Adapted from llrm-core's `optimize/loopexit.rs`, the port of
`qbopt/optimize/loopexit.py`.

The integer equivalent of evaluating an AddRec at its backedge count: a
fixed increment sums to N * step; an affine increment also contributes
N(N-1)/2 times its stride. Results are modulo their own width. Only the
controlling recurrence must be proven not to wrap.

What changed with the IR:
- A deleted loop's header computes its phis' exit values, placed by
  `counting::Seeds`, and branches to the exit; the latch goes as
  unreachable. The old header kept its cleared operations as byte owners.
- A constant exit value is an operand where the old one was a copy placed in
  the exit block, and an exit block always has a terminator.
- The preheader and exit are the counted proof's.
- `_disposable`'s kinds are `add`, `sub`, `icmp` and `br`: `Copy` and
  `Nothing` have no instruction, `Increment` and `Decrement` are `add`s.
- `_widened_counters` reads a `sext`; the old `SignExtend` it was.

llrm-mir's `loopdeletion` deletes a loop of any shape whose exit values
come from outside it; it evaluates none, so a loop computing what is
read after it stays there.
