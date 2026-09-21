# Modern language frontend

`modern` is a provisional frontend identifier, not the language's final name.

This crate implements the first source-language slice:

- indentation-delimited functions and blocks;
- `i16`, `i32`, `bool`, and `void`;
- typed parameters and return values;
- `let` and `var` bindings;
- assignment, calls, `return`, `if`/`else`, and `while`;
- `break` and `continue`; and
- integer arithmetic and comparisons.

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

Not implemented in this first slice are unsigned and byte types, imports,
aggregates, ownership and borrowing, strings and collections, patterns,
comprehensions, lambdas, or generators. Those should extend semantic analysis
and elaborate to the same small HIR rather than adding surface-language HIR
operations.
