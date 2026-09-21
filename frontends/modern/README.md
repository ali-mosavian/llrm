# Modern language frontend

`modern` is a provisional frontend identifier, not the language's final name.

This crate implements the first source-language slice:

- indentation-delimited functions and blocks;
- the complete scalar set: `bool`, `char`, signed and unsigned 8/16/32-bit
  integers, `f32`, `f64`, and `void`;
- typed parameters and return values;
- `let` and `var` bindings;
- assignment, calls, `return`, `if`/`else`, and `while`;
- `break` and `continue`;
- strictly typed integer and floating arithmetic and comparisons;
- fixed one-dimensional arrays, written `[T; length]`, with literals and
  indexed loads and stores;
- source-ordered, nested `struct` layouts and literals in fixed arrays, plus
  allocation-free array iteration through explicit references; and
- byte strings, allocation-free f-strings, and `print`.

`char` is one target-code-page byte; `\xNN` spells any code unit without
making the compiler or runtime Unicode-aware. Decimal floating literals are
stored once in read-only module data. There are no implicit numeric
conversions: literals may acquire a type from context, while nonliteral
operands must already have the same type.

Strings deliberately match the real-mode systems boundary. A string value is
a 16-bit near pointer to NUL-terminated payload bytes. Two little-endian
16-bit words immediately precede the payload: `length`, then `capacity`.
String literals are read-only module objects. In this slice an f-string is
valid only as a direct `print` argument; it streams literal and interpolated
pieces to typed `__print_*` calls, so formatting introduces no allocation or
hidden general-purpose runtime.

Fixed arrays are contiguous local objects with a zero lower bound. Struct
fields stay in source order, with at most two-byte alignment for the 16-bit
target. Indexing and field selection are structural HIR and lower to ordinary
address arithmetic; no array, field-access, or iterator helper is emitted.
`for item in &array` gives `item` an immutable scoped view of the element;
`for item in &mut array` requests a mutable view and is rejected for an
immutable array. By-value array iteration is reserved until aggregate move
semantics are implemented. A mutable view's field stores update the original
element.
`is` and `is not` compare the identity of scoped views, while `==` and `!=`
remain value comparisons (struct value equality is not in this slice).
Literal indices are checked by the frontend. The
`fixtures/nbody.mod` fixed-point integrator—an array of six `body` structs—is
the current end-to-end feature gate for structs, arrays, nested loops,
strings, f-strings, and printing.

In the implemented slice, primitive expressions are copied values; `let`
creates an immutable place and `var` a mutable place. Whole-aggregate
assignment is deliberately absent until move semantics are specified. Array
iteration creates explicit, non-owning views confined to the loop body.
General first-class references, owning moves, and explicit cloning remain
future work.

Canonical language code uses lowercase `snake_case` for functions, variables,
parameters, fields, and user-defined types. This is the language and standard
library convention, not a lexical restriction; interop and generated code may
retain another system's spelling when needed.

The frontend performs name resolution and strict type checking, constructs an
explicit control-flow graph, and writes qbopt's versioned common-HIR JSON. It
does not emit bytecode, p-code, machine instructions, or target registers.

Run it with:

```text
cargo run --manifest-path frontends/modern/Cargo.toml -- program.mod
```

`--tokens` and `--syntax` expose the two earlier stages without performing
semantic analysis.

The current stopping point is semantic MIR. The frontend document is accepted
by `qbopt.hir.decode`, `qbopt.hir.verify`, and `qbopt.hir.lower`; a
freestanding ABI adapter and final object writer have not been added yet.

Not implemented in this slice are explicit numeric conversions, imports,
resizable collections, general ownership and borrowing, patterns,
comprehensions, lambdas, or generators. General string construction is also
absent: f-strings are currently a print facility, not heap values. Those
features should extend semantic analysis and elaborate to the same small HIR
rather than adding surface-language HIR operations.
