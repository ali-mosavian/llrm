# IVUsers user order (IU1, M16-LSR-IVORDER)

Since #7651 `IvUsers` visits a value's users in use-list order, as the pin does
([IVUsers.cpp#L174](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/IVUsers.cpp#L174)).
Before it, IU1 visited them in descending program order, which equals the pin's order on freshly parsed text.
The two differ where an earlier pass relinked uses differently from LLVM.

## M16-LSR-IVORDER

DEVIATE M16-LSR-IVORDER (owner w12, exit 2026-11-12): QB PLASMA's b32 loop gets its induction variables in another order than the pin's
loop-reduce run on the same parsed text (the 0-start counter third, the pin's last); `test_hir_part3::test_a_register_live_through_a_loop_is_parked_for_its_counters` is red (69 -> 72 `[bp]` references).
First bad commit #7651. Not a mistranslation of `IVUsers.cpp#L174` (every consumer reads `function.users()`, the arena model of LLVM's list):
the use-list state reaching LSR differs. Trace: pin asserts `opt` against ours on parsed `in.ll` (ours [2,14,20,746,10], pin [14,10,746,2,20]).
- Fix: the order mirrors the pin's; fail-first: the PLASMA test on parsed text and a pinned-IR order test.

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
