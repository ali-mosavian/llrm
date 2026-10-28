> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/overalign.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/overalign.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/overalign.rs sha256:b625a397fa52fea2b72c2b141b7dfe098f0327ab8fe3ad2c1393b223ec547829 -->

# overalign

Aligns a static local that wants more than the stack gives, in MIR before
instruction selection.

## How it works
The frame keeps the stack aligned to 16 at most, so a `_Alignas(64)` local
lay wherever the stack happened to be. Until the frame realigns the stack
pointer (the pin's `and rsp, -64`), such an `alloca` becomes a byte buffer
with `align - 1` spare bytes, and its users address the first aligned
address in it:

```text
 %v = alloca [4 x i32], align 64
              |
              v
 %v = alloca i8, i32 79            ; 64 + 16 - 1
 %i = ptrtoint ptr %v to i64
 %a = add i64 %i, 63
 %m = and i64 %a, -64
 %p = inttoptr i64 %m to ptr       ; every user of %v but its lifetime
                                   ; markers reads %p
```

DEVIATE OA1 (owner w4, 2026-10-28): the pin realigns the stack instead and
addresses the local from the stack pointer; this costs the spare bytes and
four instructions per local. Exit: realign in `masm::lowered`. Dynamic
`alloca`s keep the alignment the DAG builder passes (`DynamicStackAlloc`).

## Example
`int aligned(int n) { _Alignas(64) int v[4] = {n, 1, 2, 3}; return use(v);
}` at `-m64 -O1` first had `sub rsp, 24; lea rdi, [rsp]`. Now it takes 88
bytes and `lea rax, [rsp]; add rax, 63; and rax, -64` before the first use.

From the compiler's `-S` output of `tests/frame_realign_m64.rs`' source.

## Runs
`mir over-aligned locals` in `driver::optimized`, after `ehprepare`, once
per module, whatever the -O level.
