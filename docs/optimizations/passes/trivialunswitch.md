> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/trivialunswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/trivialunswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/trivialunswitch.rs sha256:15bcd5fe0ad20206a9a40ddfe6bc11990d77ba1386bb01098bc8bfb735af6fb0 -->

# trivialunswitch

LLVM's SimpleLoopUnswitch, the trivial form (`unswitchTrivialBranch`): a
branch on a loop-invariant condition that leaves the loop on one side is
decided before the loop, not on every trip.

The branch, in the loop's header, goes to its preheader: whichever way it
goes the first time it goes every time, so the loop is entered only where it
stays, and where it leaves the header's tests are never made. Nothing is
copied. Tail recursion makes such loops (`paths(n, m)` tests `n == 0` on
every trip of the loop that tests `m == 0`); unswitch.rs copies the loop for
a condition inside its body and leaves the header's alone.

Where it differs from LLVM: only the loop's header holds the branch (LLVM
follows a chain of blocks that each continue unconditionally), and the
exit's phis take their value on the new edge only from a value defined
outside the loop or a header phi's value on entry; any other value keeps the
branch.
