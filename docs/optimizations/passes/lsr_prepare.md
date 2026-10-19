> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_prepare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_prepare.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_prepare.rs sha256:6921c1075769d2f3ec83a97725053badb8e7c712a4806ada8a4e1cd3850f0263 -->

# lsr_prepare

What LLVM's loop strength reduction does to a loop before it collects
(`OptimizeMax`, `OptimizeLoopTermCond` and `FindIVUserForCond`;
LoopStrengthReduce.cpp at `d1106deb71cc`, l.2485-2815).

## How it works
An exit test is changed to take the post-incremented value of its induction
variable, so the variable and its increment share one register. For each
exiting block that ends in a compare of an IV user: a `max` the trip count
was written with (`i != smax(n, 1)`) is undone to `i < n`; the test is moved
beside its branch (copied if it has other users); and the use is marked
post-increment, unless another use of a stride that divides this one could
share the pre-increment value. `analyze` decides from the expressions;
[`apply`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/licm/apply.rs) edits the function and says which tests it marked.

## Example
`do { p[i] = 0; } while (++i != n)`: the test `%next == %n` is on `%next`,
the incremented value, and moves to just before the latch's branch.

## Runs
Before the collection, from [`crate::lsr_pass`](lsr_pass.md).

## Limits
DEVIATE: a vector condition (`extractelement`) is not ported; every float
type counts as legal for `OptimizeShadowIV` (`isTypeLegal`: x86 has no
other).
