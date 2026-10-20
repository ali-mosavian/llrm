> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/attrsummary.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/attrsummary.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/attrsummary.rs sha256:414c5e8d02880cf1c5a4caa6a16062360bb9de6ac93015c033fff29c1d4b1918 -->

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

DEVIATE A1: no slices of tracked globals. `GlobalsModRef` answers those per
global; here they are the unknown access. Checked by
`the_unknown_access_stands_for_globals`.
