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
| `Fold` | partial local integer foundation; not parity | `test_sccp.py`, `test_constant_arguments.py`, `test_constant_carry.py`, `test_constant_cells.py`, `test_constant_conditions.py`, `test_constant_stores.py`, `test_folded_relocations.py`, `test_floatfold.py`, divisor-order regression in `test_transform.py` |
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

## Current corrective action

The current Rust transforms are deliberately narrow foundations. None is
recorded as a completed Python pass port. In particular, Rust CSE initially
treated reversed integer addition as distinct, contradicting
`test_cse_commutative.py`; the port must canonicalize commutative integer
expressions without reordering execution, while retaining operand order for
noncommutative and floating operations.
