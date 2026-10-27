> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/attrsummary.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/attrsummary.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/attrsummary.rs sha256:a2767a0c730e211eee8a4ea55f0efe9152589f4e22eefaf224114d7a0d124050 -->

# attrsummary

`Summary` for each defined function, derived from its attributes alone.

The drop-in for `alias::summaries` that the call-effect readers move onto:
what `FunctionAttrs` states (`memory(...)`, `readonly`, `writeonly`,
`nocapture`) in the shape `Summary` has.

- `argmem` access is a whole-object slice of each pointer parameter that
  `readnone`, `readonly` and `writeonly` do not rule out.
- Any other location (`other`, `inaccessiblemem`, `errnomem`) is an unknown
  read or write.
- A pointer parameter without `nocapture` is captured.

## Example
A function that only reads through its pointer parameter:

```mir
define void @f(ptr readonly %p) memory(argmem: readwrite) {
b0:
  ret void
}
```

`summaries(module)["f"]` reads parameter 0 and writes nothing.

From unit test `a_readonly_parameter_is_read_not_written` in
`attrsummary_tests.rs`.

A tracked global (`GlobalsModRef::is_non_address_taken`) of a function that
has a `FunctionInfo` is a whole-object slice in `reads` or `writes` as
`mod_ref_for_global` says, and `globals_listed` keeps the unknown access
from reaching it. Without a `FunctionInfo` the unknown access reaches it.

## Used by
Named by `manager`.
