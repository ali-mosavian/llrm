> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fold.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fold.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fold.rs sha256:20ca651d06c6d45d27ec23e37719095486ea2e9397e8cccf425ab58f648b6dfa -->

# fold

Values known to be numbers, replaced by those numbers: LLVM's SCCP and
InstSimplify. Adapted from llrm-core's `optimize/transform.rs` `Fold`
(`folded`, `_folded_op`, `_folded_division`, `_folded_phi_edges`,
`_constant_operands`). What is known is consts' answer.

What changed with the IR:
- An operation whose answer was known became a move of that number, and each
  reader's register operand became the number. Here every use of the value
  is the constant, and Dead takes the definition. So a store's value, a
  call's argument (`_constant_argument`), a fill's byte and count
  (`_constant_fill`) and an address's index (`_constant_based`) need no case
  of their own, and a load from a known cell is a value consts knows.
- Operand order is no machine's: an ordered operand is replaced like any
  other. A commutative operation still takes its constant on the right.
- An edge fold put a copy of each number in its parent; a phi here takes the
  constant, so the parent's terminator no longer matters.
- consts reads the module's globals through the outer proxy, and what each
  call writes from the manager's `Writes`.
- The old Fold left floatfold out of a body with loops, whose x87
  observation points belonged to floatloop; the rich MIR observes no FP
  exception, so it folds every body.

Dropped, no rich MIR analogue: `_constant_update` (a read-modify-write of
a cell); `_symbol_copies` and the symbol half of `_literal_of` (a
global's address is already a constant operand); the checks for a
second result something reads (`wanted`: one result per instruction).

Tests, in `fold_tests.rs`: `divisor_constants_propagate_without_reordering`,
and consts' tests that waited for this port:
`test_signed_widening_produces_a_whole_long_constant`'s folding half,
`test_pointer_displacement_constants_preserve_order_and_width`,
`test_constant_subtraction_preserves_operand_order` and
`test_a_known_factor_becomes_a_multiply_operand`. Skipped:
`test_constant_operand_keeps_its_memory_address_dependency` (every use is
replaced at once, so no reader keeps an orphaned value), and the width
halves (a fact here is as wide as its value).
