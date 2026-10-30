> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpmemalign.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpmemalign.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpmemalign.rs sha256:9cb3cc547add4b39782a3c06eb780f6b2034f611eb9225a8ffb27ead0d497cf1 -->

# cgpmemalign

The alignment of a memory intrinsic's pointers, raised to what is known of
them: `CodeGenPrepare::optimizeCallInst`, which sets the destination (and
the source of a transfer) `align` to `getKnownAlignment` where that is
larger than the call states.

## How it works
Each `llvm.memcpy`, `llvm.memmove` and `llvm.memset` call has its pointer
arguments' alignment (`Value::getPointerAlignment`: a global's or alloca's
stated alignment, less what a constant GEP may add) compared with the
`align` attribute it carries (one when it has none). The larger is written
back. The pin runs the pass at -O1 and up only, so `-O0` keeps the call;
here the pipeline does not run at -O0 at all.

## Example
`call void @llvm.memcpy.p0.p0.i32(ptr @b, ptr @a, i32 100, i1 false)` with
`@a` and `@b` declared `align 4` becomes the same call with `align 4` on
both pointers, so x86's `EmitTargetCodeForMemcpy` copies with `rep movsl`
and not a call of `memcpy`.

## Runs
In `cgpaddr`, with the pass's other CodeGenPrepare rewrites.

## See also
`backend::memcpycall` reads the alignment this states.
