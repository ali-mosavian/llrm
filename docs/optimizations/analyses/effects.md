> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/effects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/effects.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/effects.rs sha256:6b32f05a2d44af89bbbc2d4f00ffe47fcf7ca9136140eec78835801d8a125ba5 -->

# effects

What a call may do to memory its operands do not name, from LLVM's call
attributes.

## How it works
A load or store names its bytes, so it has no effect beyond them. A call
states the rest as LLVM does: `memory(...)`, `readnone`, `readonly` or
`writeonly` at the call site and on its callee, read through
`llrm_mir::memory::stated_at`. The effect is what both the site and the
callee allow (`call_effects`: reads and writes are each `site && callee`); a
callee with no body or declaration known is any effect. A volatile access
names its bytes too.

```text
 call @f(p) memory(argmem: readwrite)
   call(...)            reads, writes    argmem counts: p's bytes
   unmodeled_read/write false, false     argmem excluded: p names them
```

The "unmodeled" queries leave out the `argmem` location, because arguments
are accessed through bytes the pointer names; what is left is memory no
operand names. `exposes_memory` asks whether a raise can reach a handler in
this body, which reads memory: only an `invoke` not stated `nounwind`
(along its unwind edge).

## Example
What each instruction may do to memory no operand names:

```mir
declare void @anything()
declare void @arguments(ptr) memory(argmem: readwrite)
declare i16 @reads() memory(read)

define void @f(ptr %p) {
b:
  call void @anything()
  call void @arguments(ptr %p)
  %r = call i16 @reads()
  call void @anything() memory(none)
  store volatile i16 1, ptr %p
  store i16 1, ptr %p
  ret void
}
```

| Instruction | `unmodeled_read` | `unmodeled_write` |
| --- | --- | --- |
| `call @anything()` | true | true |
| `call @arguments(%p)` | false | false |
| `call @reads()` | true | false |
| `call @anything() memory(none)` | false | false |
| both stores, `ret` | false | false |

From unit test `a_calls_unmodeled_memory_is_what_its_attributes_leave_open`
in `effects.rs`.

## Provides
`call`, `touches_memory`, `writes_memory`, `unmodeled_read`,
`unmodeled_write`, `exposes_memory`, `states`, and the `Declarations` type
(every global as its declaration, a module analysis, since a pass holds its
own function mutably while asking about the others). Consumers include
[`crate::memory`](memory.md), [`crate::noreturn`](noreturn.md), [`crate::branchprob`](branchprob.md),
[`crate::interprocedural`](interprocedural.md) and `profit` in `llrm-transforms` (grep
`effects::`).

## Needs
`llrm_mir::memory` and `llrm_mir::facts` for what the attributes mean.

## See also
LLVM `MemoryEffects`, `FunctionAttrs`.
