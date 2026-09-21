# Modern language frontend

`modern` is a provisional frontend identifier, not the language's final name.

This crate implements the first source-language slice:

- indentation-delimited functions and blocks;
- the complete scalar set: `bool`, `char`, signed and unsigned 8/16/32-bit
  integers, `f32`, `f64`, and `void`;
- named signed fixed-point types backed by `i16` or `i32` storage;
- typed parameters and return values;
- `let` and `var` bindings;
- assignment, calls, `return`, `if`/`else`, `while`, and typed half-open
  integer ranges;
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

Fixed-point types make their representation explicit:

```text
type fixed8 = fixed i16, fraction=8
type fixed16 = fixed i32, fraction=16
```

The fractional count is part of the named type and must be between one and
one less than the storage width. Distinct declarations are distinct types,
even when their storage and fraction match. Integer literals are scaled
exactly at compile time. Decimal literals are rounded once to the nearest
representable quantum, with ties away from zero. There are no implicit
conversions between fixed types, integers, or floats.

Addition, subtraction, negation, and comparison use the signed stored
representation directly. Multiplication and division widen first (`i16` to
`i32`, `i32` to an internal `i64`) and then rescale, so storage-width overflow
cannot destroy the intermediate product. Multiplication discards low
fractional bits with an arithmetic shift; division shifts the widened
numerator before signed division. Narrowing back to storage follows the
language's current wrapping integer policy. No descriptor, heap object, or
fixed-point arithmetic runtime routine is generated. Printing uses the
existing output boundary with the raw value and a compile-time fractional-bit
argument. Both direct `print(value)` and fixed values inside f-strings render
canonical signed base-10 `integer.fraction`: at least one digit appears on
each side of the decimal point, and redundant trailing fractional zeroes are
removed. Thus Q8 raw values `384`, `-64`, and `512` print as `1.5`, `-0.25`,
and `2.0`, respectively; the scaled storage integer is never printed as the
source value.

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
`for index in start..end` evaluates both bounds once and visits `start` through
`end - 1`. Its immutable induction variable has the bounds' integer type, so
`for step_no in 0..step_count` runs exactly `step_count` iterations when
`step_count` is nonnegative.
Literal indices are checked by the frontend. The
`fixtures/nbody.mod` fixed-point integrator—using a named Q23.9 `scalar` and
an array of six `body` structs—is
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
