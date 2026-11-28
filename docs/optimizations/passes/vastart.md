> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/vastart.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/vastart.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/vastart.rs sha256:60a57700c8173850280c0f5694765db73810483fdf20b7aa771a1801dd412d8f -->

# vastart

`llvm.va_start` of a one-pointer `va_list` as a store the mid-end can see.

## How it works
Where the target's `va_list` is one pointer (x86-32 cdecl), `va_start(ap)`
only stores the address of the first variadic argument. The pin keeps the
intrinsic until selection, so `ap` is a stack slot whose address escapes
into it, and every `va_arg` loads and stores the slot (gcc keeps the
pointer in a register). Here `va_start(ap)` becomes `store
llrm.vararg.address(), ap`, `va_end(ap)` is dropped (it does nothing for
such a list) and `va_copy(d, s)` becomes a load and a store; `ap` is then
an ordinary slot that SROA promotes. Where clang leaves `va_arg` an IR
instruction (m68k: `Tti::va_arg_slot`), it is the load, the load of the
value, the step by its size and the store the selector's `expandVAArg`
makes, written here so the list slot has no user SROA cannot see through.

DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; users: x86-m32,
m68k with `va_arg_slot` = 4): the pin lowers the intrinsics in selection
only. Off unless `Tuning::promote_va_list`. Exit: the target states
`va_list` as a value the pin's mem2reg understands, or the pin does.

## Example
```text
  call void @llvm.va_start.p0(ptr %ap)      %a = call ptr @llrm.vararg.address.p0()
  ...                                  =>   store ptr %a, ptr %ap
  call void @llvm.va_end.p0(ptr %ap)
```

## Runs
First in the mid-end, before `sroa`, in a variadic function.
