# Takeover checkpoint — 2026-09-09

Goal: correct modern-compiler-quality output, machine-independent MIR, every documented target within 1.5x. Not complete. Work is uncommitted on `restore-through-lir`, based on `db9f25c`; inherited changes and stashes are preserved.

## Implemented in the working tree

- Write-through memory promotion with SSA construction limited to newly introduced variables.
- Fixed allocation priority, implicit address/register requirements, runtime-frame spill placement, returned high halves and synthetic-edge layout accounting.
- Resolved branch edges, trivial-phi removal and dead-block byte ownership.
- Corrected carry-operation recognition, transitive value substitution and leading-instruction deletion.
- Constant operand propagation; two-address conversion handles immediate first operands and ends the copied source's use.
- MIR fixed-point iteration. Hoisting allocates fresh variable IDs from the value graph, including promoted cells.
- No re-raising of fallback machine output. A failed backend leaves the original input unmarked.
- Stage dumps observe the actual production MIR and machine passes. The scoreboard rejects missing LIR completion and unmappable output.

## Measurements

Current PDS `/G2` modeled costs, not hardware timings:

| Program | Cost | Target | Ratio |
| --- | ---: | ---: | ---: |
| bools | 176 | 126 | 1.40x |
| press | 420 | 308 | 1.36x |
| hotlop | 646 | 312 | 2.07x |
| lngmix | 921 | 210 | 4.39x |
| harr | 11694 | 1834 | 6.38x |
| matrix | 14708 | 6210 | 2.37x |
| segld | 26602 | 6704 | 3.97x |

The bounded emission scan completed 487 fixtures in 20 seconds: 413 LIR, 74 fallback. Most fallbacks concern unknown event-call interfaces. The subsequent chained-load fix restored QuickBASIC `procs` to LIR; the whole scan has not been repeated after that fix.

Focused runtime checks passed the changed arithmetic kernels across PDS, QuickBASIC and VBDOS. The fixed-point variable collision was caught as `lngmix` printing 4081664/55 instead of 142900; after its fix both `lngmix` and `lngmxx` pass all three. QuickBASIC `procs`, `arridx` and `flags` also pass after transitive substitution was repaired. Full commit gates have not run on the final tree.

## Next work

1. Independent review remains outstanding. The isolated, read-only Opus request failed because Claude Code OAuth expired; Fable is unavailable. The user authorized proceeding without it. No Desktop session was resumed.
2. Review SSA rebuilding in hoisting and remaining alias/width assumptions before expanding optimization. Preserve explicit inputs, flags and return values through every substitution.
3. Resolve unsupported event interfaces and VBDOS procedure contracts from evidence; never count fallback as success. `pds-g2.obj` and `qb45.obj` also have conflicting pin/requirement refusals.
4. Complete intrinsic value recognition at raise, then reduce loop stores, address recomputation and register pressure. Promoting long high-half carry reads was tried and removed because it increased spills and cost.
5. Repair remaining failures and run the required batch/commit gates. The checkpoint gate failed lint checks and reported 787 type diagnostics; its full pytest run was interrupted once the gate was already blocked. The user explicitly authorized an unsigned checkpoint with hooks bypassed and incomplete validation recorded. This exception does not waive future gates. The removed legacy rewrite arm left an unused `orphaned_externals_renamed` function referring to undefined `RENAMABLE_IF_ORPHANED`; FIXMUL integration is still unfinished.

The old architecture/status prose contains stale descriptions. In particular, spilling exists, promotion is enabled in production, and the optimizer now iterates on MIR rather than through emission. Runtime correctness and the 1.5x target remain separate gates.
