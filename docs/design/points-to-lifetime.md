# Points-to kept across edits

gcc computes points-to twice per function (`pass_build_ealias` early, `pass_build_alias` late) and keeps it across the passes between, giving a new SSA name a conservative answer. llrm re-solves for every body state. This note is the measured bound of the gcc shape and what an edit must do to keep the answer sound.

## Measured bound

QCport host, render, model and game, 97 files. Time in `point_values` (the value solve) and `escapes` (the escape phase, one run per configuration), taken inside the solve; shares are of the compile's user CPU. "Beyond two" counts per body the runs after its first two.

| | -O1 | -O2 |
|---|---|---|
| value solves (all / beyond two) | 17,111 / 15,358 | 19,904 / 13,247 |
| escape runs (all / beyond two) | 28,954 / 27,198 | 33,315 / 25,989 |
| points-to, all | 12.4% | 11.9% |
| **beyond the first two per body** | **11.1%** | **8.8%** |
| `call-effects`, total of its span | 4.2% | 5.3% |

The bound is nearly all of the work: 878 bodies at -O1 make 46,000 runs. It is an upper bound in two ways: it assumes every later query is answered for free, and a body that is cloned (a trial inline, an unroll) takes a new lineage, so at -O2 the bodies are over-counted and "beyond two" under-counted.

`call-effects` is not on top of this: it is the escape phase and the per-call lists of the same solves, and drops with them.

## What an edit must do

The answer is positional (`escaped_before` is flow-sensitive) and keyed by value, so an edit keeps it sound by class:

- **Erase, or replace a use by a value that points to a subset.** Nothing: a stale answer is a superset.
- **A new value derived from existing ones** (GEP, cast, phi, select): the union of its operands' sets; offsets widened to the whole object when unknown. A load of a pointer: everything that has escaped or been stored to, unless a store reaches it.
- **A new alloca:** a new object, not escaped.
- **A store of a pointer, or a new call:** the stored or passed object escapes from that point on; every `escaped_before` after the edit in its reach is stale. Invalidate the body.
- **Moving an instruction across an escaping one:** invalidate the body.
- **A new function (an inline, a clone):** its own solve once, its values mapped in.
- **A changed callee declaration:** invalidate the callers (as `Depends` does today).

The log `Depends` already filters tells the classes apart; the work is the derivation rule per opcode and an `LLRM_CHECK_*` that solves again and asserts the kept answer is a superset of it.

## Cost

Objects change: a conservative answer for a new name loses what a fresh solve proves, so the first step is a count, not code: how many queries a kept answer would answer worse than a fresh one, per pass, over QCport, and the size and speed those lose. If the count is small the gain is up to the bound; if not, only the passes whose edits are all erase or derive keep their answer.
