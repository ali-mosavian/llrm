# Generated lexer parity

`crates/qbfront/src/generated_parser/lexer.rs` is a table-facing scanner, not a
conversion layer over the legacy recursive parser's tokens. It uses the
highest VBDOS source-syntax superset for every selected compiler profile:
identifier underscores after the first character and logical-line
continuations are accepted uniformly. `Dialect` selects semantic/runtime/ABI
behavior after parsing; it does not remove source productions.

The differential gate is `cargo test --test generated_lexer`. It materializes
the deterministic 2,160-case `tools/qbgen` corpus, scans every source under
QB 4.5, PDS 7.1, and VBDOS, and compares acceptance, token count, every span,
literal payload, punctuation, and identifier boundary to the legacy lexer.
It intentionally permits one table-level refinement: the generated scanner
recognizes any spelling in the generated 246-token catalogue even when the
legacy typed parser currently calls that spelling an identifier. The source
slice and span must still be identical.

The recovered `qbasic-port` `prslex` behavior also establishes two forms
which the former adapter could not represent because it delegated directly to
the legacy scanner:

- `?` is `PRINT` shorthand (`tkQMark`), rather than an unknown byte.
- `&377` is default-octal, equivalent to `&O377`.

These are deliberately tested separately. They are extensions over the local
legacy lexer, not exceptions hidden by the corpus differential. Any later
legacy-lexer migration can adopt them with the same concrete witnesses.
