> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cxxeh.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cxxeh.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cxxeh.rs sha256:24025e5024d9e159b6eb23b3426d3e18b264a7601be8008b56abe63a80dd39a8 -->

# cxxeh

Make a C++ landing pad one the unwinder can enter, before instruction selection.

## How it works
Clang's IR is the Itanium model: a pad is entered by the unwinder with the exception pointer and a selector, and
`resume` hands both back. A pad here is entered as QB's is (see [`super::ehprepare`](ehprepare.md)): the unwinder jumps to it with
no register intact, so what it carries is kept in memory, in `__eh_exception` and `__eh_selector`, and the pad reads
them. `resume` becomes a call of `__unwind_resume(exception, selector)`, which does not return.

`llvm.eh.typeid.for(T)` is the selector the unwinder reports for a catch of `T`: the place of `T` among the catch
clauses of the function, counted from 1 in the order they first appear. The tables of the next stage number them the
same way.

Values live into a pad are kept in stack slots, as for QB's pad ([`super::ehprepare`](ehprepare.md)'s `demote_live`).

## Runs
In `driver::optimized`, with `ehprepare`; a module with no function using `__gxx_personality_v0` is untouched.

## See also
The plan: `docs/working/cxx-eh.md`.
