> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ipacp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ipacp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ipacp.rs sha256:36bd832d1f39904da9c8115d595179c9b290a3afd1567fa581fea8e2833ea3c6 -->

# ipacp

GCC's `ipa-cp` cloning (ipa-cp.cc): a function called with a constant actual
is copied for it, the actual replaced by the constant in the copy, and the
calls redirected. `-fipa-cp-clone` (-O3) lets the copy grow the unit; a copy
that replaces the function (every call goes to it) needs no growth.

The copy is a function of its own, so the inliner takes it as any other: a
clone with its one caller moves into it, and the recursive call a clone
makes with a new constant (`place(q, row + 1, n)` in the clone for `row`) is
the next clone, to `ipa-cp-max-recursive-depth`: GCC's -O3 unrolls `queens`'
recursion this way, eight clones deep.

Differences. GCC propagates lattices over the whole call graph and evaluates
each value with its function summaries' time and size under the context
(`good_cloning_opportunity_p`); here a site's known actuals are the
constants the caller's own folding proved (`current_call_constants`), and
the benefit is the clocks `inline::folded` finds the callee no longer does,
by the site's frequency. The thresholds are GCC's.
