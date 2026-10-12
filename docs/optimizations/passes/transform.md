> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/transform.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/transform.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/transform.rs sha256:7c08f5113781d404e0e814c3b0e1e3249c1ec4234f63890e978d5e31b772133d -->

# transform

Helpers adapted from llrm-core's `optimize/transform.rs`, each copied as
a ported pass needs it; the pipeline itself is `pipeline`.
`_unreachable` is `llrm_analysis::cfg::_unreachable`.

What `gvn` reads -- `_PURE`, `_computation`, `_reaches`, `_undisturbed`
and `subexpressions` -- came with it. Left behind, each of the old
representation:
- `_widths`, `_full`, `_width` and `_copied`: a value was split into word
  halves and copied between registers; here every value is whole and nothing
  is a copy, so `stands` (what a copy numbered as) is the substitution
  itself.
- `halves`: which half of a value was read; a value here is whole. `live` is
  here for Dead.
- `reused_divides`, `divided_twice`: one divide had two answers, one dead.
  `sdiv` and `srem` are separate here, and two equal divides are one
  expression `subexpressions` finds.
- `_read` and the `flags` checks: no value is the machine's flags.
- `_reusable_float_path`, `_exact_floating`, `_exact_stored_load`,
  `_unchanged_float_environment`, `_erased_floating`: x87 exceptions,
  precision and the float environment. Floating arithmetic has no exceptions
  here (see `llrm_analysis::effects`), so it is as pure as integer
  arithmetic.
- `_phi_reading`, `_reclaimed`, `_empty_operation`: an erased operation's
  source bytes. `replace_all_uses_with` reaches phis too.

`_undisturbed` asks `memoryssa::Accesses` what a store or call writes;
the old one refused every call. `forwarded` serves only a load: no
other instruction reads memory.
- The `dgroup` parameter: a segment is not a MIR fact.

Tests of these, from `optimize/transform_tests.rs` and
`analysis/induction_tests.rs`, are here, the addresses and constants
now instructions; `test_cse_replaces_phi_uses_of_a_deleted_initializer`
keeps one case, its others varying variables, source bytes and a copied
constant. `test_a_third_equal_divide_reads_the_answer_the_first_computed`
tests `subexpressions`, which serves a divide here. Stay behind:
- test_cse_propagates_a_complete_narrow_copy_to_an_opaque_reader and
  test_cse_refuses_an_operand_that_is_only_half_its_value: halves and
  copies.
- test_deferred_runtime_float_reuse_respects_environment,
  test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not and
  test_proven_copy_unlocks_strict_floating_cse (ignored there): the x87
  environment and exactness, the last two reading BC fixtures.
- test_both_lngmix_divides_absorb: a BC corpus through `wholeseg`.
- test_value_reuse_is_one_gvn_pre_pass: the pipeline is not ported.
- test_a_served_read_names_the_value_and_not_a_register: a BC corpus, and a
  load here is replaced by its value outright.
