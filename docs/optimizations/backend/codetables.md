> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/codetables.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/codetables.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/codetables.rs sha256:1284761019cf0b69cab63f0e4cc317f973605d1aca904f95012e8899dcda0abb -->

# codetables

Read-only tables kept in the code segment of the procedure that reads them.

## How it works
Where a target's data segment is scarce and its code segments are not
(real mode: DS is DGROUP), `TargetLoweringObjectFile::getSectionForConstant`
puts a constant in the text section. `placed` does that for the tables of
a module: a table moves into the code, after the one procedure that reads
it, when

- every reference to it is a load (an operand read, a table jump's cell): no
  store, no `lea`, no `offset`, nothing inline or in any data names it, so
  no pointer to it exists and a pointer into DS could never reach code;
- one procedure holds all the references, so the table is in the same
  segment as each reader whether the object has one code segment or one per
  procedure;
- its datums are bytes, labels, alignment and near pointers.

Each reference then reads through `cs:`.

```text
  mov al, byte ptr T[di]          mov al, byte ptr cs:T[di]
  (T in _DATA, 12 B)       =>     (T after the procedure's last byte)
```

## Example
A table read by one procedure, only through loads, moves into the code after
that procedure and is read through `cs:`.

```text
mov al, byte ptr T[di]          mov al, byte ptr cs:T[di]
(T in _DATA, 12 B)       =>     (T after the procedure's last byte)
```

## Used by
Named by `assemble`, `data`.
