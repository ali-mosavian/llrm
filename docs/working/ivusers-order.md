# IVUsers user order (IU1)

`IvUsers` visits a value's users in descending program order, not use-list order.
Use-list order is edit history; the pin's order on parsed IR is the only reproducible oracle.
It decides LSR's tie between registers with equal use counts.

- Exit: use lists keep LLVM's order. Owner: w-5. Review: 2026-11-19.
- Test: `tests/ivusers_order.rs` (nbody_single force loop, `{-24,+,4}`, no `cmp ecx, 24`).

## Method caveat for the pin-comparison checklist (#3584)

`pin.py` runs on re-parsed text, so use-list-order ties differ from the pin's own pipeline.

## Follow-up rows: other passes that iterate users where order can break a tie

| Row | Site | Why order may matter |
|---|---|---|
| IU2 | `lsr_chain.rs:191`, `:481` | chain heads and other users picked in visit order |
| IU3 | `lsr_collect.rs:941` | use registration order feeds RegUses |
| IU4 | `lsr_implement.rs:704` | increment users |
| IU5 | `simplifyindvar.rs:79`, `:473` | eliminate/widen order of IV users |
| IU6 | `widen.rs:866`, `:1252`, `:1573` | narrow-use walk order |

Each: check whether the pin's order on parsed IR differs from ours on an edited function; none measured yet.
