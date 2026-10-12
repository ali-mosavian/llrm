> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/narrowspace.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/narrowspace.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/narrowspace.rs sha256:64f28c73dec180dfdfb923ea5b005eea8e4243c482580b3dd0390e47e8a7041d -->

# narrowspace

A far pointer parameter of a function only direct calls reach, which every
call fills with an address in one near space, is a near pointer of it: the
caller passes the offset, the callee reads through that space's segment, and
the segment is neither pushed nor loaded. The space is where the object is:
DGROUP's for a module global, the stack's for a stack object of the caller
(SS, which DS need not be). A recursive call passes the parameter on, a
parameter an earlier round narrowed is its own space's. One actual that may
be elsewhere, or in the other space, keeps it far. `inferspace` then folds
the accesses in the callee and the casts at each call.
