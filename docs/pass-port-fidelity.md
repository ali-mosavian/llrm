# Pass port fidelity

The Python MIR optimizer and its regression history are the behavioral
specification for the Rust port. A Rust transform with a familiar name is not
a port merely because it implements a textbook optimization. It becomes a
ported pass only when its Python behavior, refusals, pipeline position, and
regressions have moved with it.

## Acceptance gate

Every pass commit must name:

1. the Python implementation functions it replaces;
2. its exact position in the production pipeline;
3. the Python unit and regression tests ported in the same commit;
4. the observable symptom each regression protects;
5. the alias, effect, CFG, floating-point, ownership, and profitability facts
   on which the Python pass relies;
6. every behavior still deferred, with an explicit refusal in Rust.

The commit must also carry a test-by-test provenance table. Every directly
relevant Python test case is marked either `ported` or `deferred`; a deferred
case names the Rust refusal that prevents the unsupported behavior from
running. A ported regression keeps the original observable symptom in its
test name or documentation and asserts that symptom rather than the shape of
the new implementation. Tests may be translated to the typed Rust IR, but
their inputs, semantic boundary, and expected result may not be weakened.

Before acceptance, the regression is observed failing against the uncorrected
Rust implementation (or by a focused mutation that removes the ported rule)
and passing after the correction. This fail-first evidence is recorded without
running a broad suite; the focused execution counts toward the global ten
percent verification budget.

The primary agent reviews the Python and Rust implementations side by side.
Passing new Rust tests is necessary but does not establish parity by itself.
Fixture-named branches, machine details in IR transforms, silent fallbacks, and
weakened refusal conditions are rejection grounds.

The production pipeline is taken directly from
`qbopt/optimize/transform.py::pipeline`:

```text
provenance
split_pointers
sroa
fold
decide
loopsimplify
lcssa
floatloop
hoist
drop_stores
gvn
promote
strength
algebraic
dead
place
unroll
peel
fill
```

Structural pointer/SROA passes run at structural boundaries. The scalar
pipeline runs to a size-bounded fixed point with repeated-state detection.
Unroll and peel candidates are independently normalized, converged, costed,
and either committed or rejected. Optional unswitching follows convergence;
loop entry rotation remains a later production-flow step. Rust must preserve
that behavior rather than treating a one-shot `llrm-opt` pass list as the
production pipeline. `strength` and `unswitch` remain off by default.

## Migration map

| Python behavior | Rust status | Regression specification to port |
| --- | --- | --- |
| `provenance`, `split_pointers` | absent | `test_mir_alias.py`, `test_pointer_memory.py`, `test_pointer_offset.py`, pointer cases in `test_promote.py` |
| `promote.Sroa` | absent | all SROA cases in `test_promote.py`, `test_unary_promotion.py`, and `test_transform.py` |
| `Fold` | partial local integer foundation; not parity | `test_consts.py`, `test_constant_arguments.py`, `test_constant_call_memory.py`, `test_constant_carry.py`, `test_constant_cells.py`, `test_constant_conditions.py`, `test_constant_cycles.py`, `test_constant_division.py`, `test_constant_index.py`, `test_constant_stores.py`, `test_folded_relocations.py`, `test_high_product.py`, `test_pointer_constants.py`, `test_sccp.py`, adjacent `test_floatfold.py`/`test_float_recurrences.py`/`test_scalar_division.py`, and divisor-order/pipeline regressions in `test_transform.py` |
| `Decide` | partial literal branch foundation; not parity | SCCP edge cases plus branch, ownership, phi-copy, and unreachable regressions in `test_transform.py` |
| `LoopSimplify`, `LoopClosedSSA` | absent | `test_loopsimplify.py`, `test_lcssa.py`, `test_lcssa_merges.py`, adjacent `test_indvars.py` and `test_layout.py` cases |
| `FloatLoop` | absent | `test_float_loop_exit.py`, `test_floatfold.py`, `test_float_values.py`, `test_native_float_licm.py`, `test_native_checkpoints.py` |
| `Hoist` and sunk stores | absent | hoist regressions in `test_transform.py`, all `test_loopmotion.py`, and adjacent induction/last-store/float-loop tests |
| `DropStores` | partial exact same-block global foundation; not parity | dead-store/load regression in `test_transform.py`, `test_dead_ownership.py`, `test_memoryssa_forward.py`, partial-write cases in `test_promote.py`, `test_floating_environment.py` |
| `Gvn` | partial same-block pure-expression foundation; not parity | `test_cse_commutative.py`, `test_cse_dominance.py`, `test_gvn_join.py`, `test_memory_cse.py`, `test_memoryssa_forward.py`, `test_memory_joins.py`, GVN/PRE/forwarding/divmod regressions in `test_transform.py` |
| `Promote` | absent | promotion cases in `test_promote.py`, `test_flow.py`, `test_mir_alias.py`, `test_loopmotion.py`, `test_unary_promotion.py` |
| `Strength` | absent and remains default-off | `test_strength_offsets.py`, `test_induction_identity.py`, `test_ivshare.py`, `test_cpu_profile.py`, `test_ranges.py`, `test_flow.py` |
| `Algebraic` | partial local integer foundation; not parity | all `test_algebraic.py` and `test_invariant_values.py` |
| `Dead` | partial pure-result foundation; not parity | `test_dead_ownership.py`, `test_dead_call_deliveries.py`, `test_dead_address_arithmetic.py`, dead-code regressions in `test_transform.py` |
| `Place` | absent; live pipeline/prose discrepancy must be characterized | the two place regressions in `test_transform.py` |
| `Unroll`, `Peel`, `Fill` | absent | `test_unroll.py`, `test_unroll_budget.py`, `test_loopclone.py`, `test_loopexit.py`, `test_float_loop_exit.py`, candidate/fixed-point cases in `test_transform.py` |
| optional `Unswitch` | absent and remains default-off | all `test_unswitch.py` |
| post-transform rotation | absent | `test_rotate.py`, `test_countdown.py`, `test_induction_identity.py`, `test_rewind.py` |
| interprocedural inline/induction/exit/IV sharing | absent | `test_inline.py`, `test_indvars.py`, `test_loopexit.py`, `test_ivshare.py` |

`widen` and `absorb` are recognition work at the raise boundary, not Rust IR
passes. `forward`, `segments`, `drop_loads`, `reuse`, and the old `cse` selector
are behaviors consolidated into Python `gvn`; they must not reappear as
separate machine-aware Rust transforms.

## Corrected mismatch

The current Rust transforms are deliberately narrow foundations. None is
recorded as a completed Python pass port. In particular, Rust CSE initially
treated reversed integer addition as distinct, contradicting
`test_cse_commutative.py`. It now canonicalizes the Python pass's commutative
integer expression set only in its comparison key, without reordering the
retained instruction, and preserves operand order for noncommutative and
floating operations. Dominance, alias-aware memory GVN/PRE, load forwarding,
and divmod reuse remain explicitly outside this narrow foundation.

## Fold audit

The source audit for `Fold` identifies the actual port boundary as constant
propagation and materialization, not only literal instruction evaluation. The
Python behavior combines a width-aware known-value lattice, byte-granular
memory facts, alias and call invalidation, phi and cyclic-phi convergence,
carry and paired divmod facts, and ownership-preserving rewrites. Its scalar
pipeline runs `fold` immediately before `decide` to a size-bounded,
repeated-state-detecting fixed point.

The present Rust `ConstantFold` deliberately covers only literal, single-result
integer expressions. It is not parity because it has no partial-width facts,
memory lattice, call contracts, anchored phi cycles, per-edge folding, carry,
multi-result divmod, signed high product, or relocation/source-ownership
contract. Two apparent similarities are specifically non-equivalent: Python
masks shift counts to the target width while Rust currently refuses an
out-of-range count, and Python folds the raised paired `DIVMOD` form while
Rust folds generic scalar division and remainder.

Implementation therefore proceeds in representation-safe slices: establish
the complete literal evaluator/refusal contract; add a deterministic portable
known-value analysis with agreement joins and anchored cycles; compose fold,
branch decision, unreachable cleanup, and dead-code removal at a fixed point;
then add memory propagation only together with exact address, overlap, alias,
no-wrap, and call-effect proofs. Carry, divmod, extraction/concatenation, and
high-product behavior wait for explicit multi-result representation. Each
slice ports its corresponding Python cases from the table above in the same
commit.
