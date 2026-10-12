# x87 waits

For contributors to the x87 backend. Says when an explicit `wait` is emitted around a floating-to-integer store.

Ordinary x87 arithmetic checks pending exceptions itself, so llrm emits no `wait` per operation. A BASIC module is the exception: BC places a `wait` before and after `fistp`, so an error handler sees the fault in the statement that caused it.

`floatassign` ([`floatassign.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/backend/floatassign.rs#L172)) takes `basic_semantics`; it is true where the target is the BASIC module object (`Target::basic`, set by `driver/basic.rs` for QB programs), and false for C, Nib and every other path.

```
 native                       BASIC module
 fistp dword [tmp]            wait
 mov eax,[tmp]                fistp dword [tmp]
                              wait
                              mov eax,[tmp]
```

The integer is read from the owned temporary only after the second `wait`. Rounding, arithmetic order and formats are the same in both. An integer conversion nobody reads is not reloaded. See [floating-environment](floating-environment.md) for what the compiler assumes of exceptions.
