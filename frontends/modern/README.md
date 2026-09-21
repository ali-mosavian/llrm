# Modern language frontend

`modern` is a provisional frontend identifier, not the language's final name.

This crate implements the first source-language slice:

- indentation-delimited functions and blocks;
- the complete scalar set: `bool`, `char`, signed and unsigned 8/16/32-bit
  integers, `f32`, `f64`, and `void`;
- typed parameters and return values;
- `let` and `var` bindings;
- assignment, calls, `return`, `if`/`else`, and `while`;
- `break` and `continue`; and
- strictly typed integer and floating arithmetic and comparisons.

`char` is one target-code-page byte; `\xNN` spells any code unit without
making the compiler or runtime Unicode-aware. Decimal floating literals are
stored once in read-only module data. There are no implicit numeric
conversions: literals may acquire a type from context, while nonliteral
operands must already have the same type.

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
aggregates, ownership and borrowing, strings and collections, patterns,
comprehensions, lambdas, or generators. Those should extend semantic analysis
and elaborate to the same small HIR rather than adding surface-language HIR
operations.
