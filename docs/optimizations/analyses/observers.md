> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/observers.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/observers.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/observers.rs sha256:1f03f810af5849d7ffd750cc1e3cc8e897dc9aec0ac201b5187dd6d84e176feb -->

# observers

Which memory cells nothing outside a function can read.

## How it works
A store is dead when everything after it overwrites the cell before reading
it, or when nothing after it can read the cell at all. `avail::dead_stores`
answers the first; this answers the second: at a call or an exit, which
cells are still observable.

`private` returns a test on a `MemRef`. A reference is private when its
provenance is non-empty and every slice lies inside a `Frame` object, within
that object's extent, whose address did not escape (`PointsTo::escaped`: a
call, a return, a `ptrtoint` or a store outside the frame) and that is not
published. Globals are never private: other functions and, unless
`internal`, other modules reach them.

An object is published when something reads it without naming it:

1. A load or store whose `MemRef` is not `named` (a variable index into a
   local is not a name) publishes every known object in its provenance.
2. A call publishes the objects of its read arguments. `nocapture` keeps the
   address from escaping, not the callee from reading it during the call.
3. A pointer stored into a published object publishes its pointee, repeated
   to a fixed point: a callee given a struct loads the pointer in it.

```text
 %s = alloca {ptr}      store %buf -> %s      call @f(ptr %s)
                                                  |
   %s  published (read argument) ----------------+
   %buf published (stored into %s, step 3)
```

## Provides
`private` and `private_of` (the same test with `Published`, the manager's
cached set, passed in), and the `Published` analysis. Consumer: the `dse`
pass in llrm-transforms.

## Needs
[`crate::manager`](manager.md)'s `Pointers` (points-to) and `ExposedFrames`,
[`crate::alias`](alias.md), [`crate::memory`](memory.md).
