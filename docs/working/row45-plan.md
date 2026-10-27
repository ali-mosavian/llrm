# Row 45: PeepholeOptimizer translation plan

Owner: w-2. Pin d1106deb71cc81016406a1917d0508d2df296266, `llvm/lib/CodeGen/PeepholeOptimizer.cpp`
([base](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PeepholeOptimizer.cpp)).

The pin's pass runs before register allocation, on virtual registers. Ours (`peephole.rs`, `peep/`)
runs after it, on physical registers, with its own rule groups (DEVIATE, not a translation). The
translation is a new pre-RA pass over LIR values; the post-RA rules stay where they are until a
premise per group says the pin's pass makes them dead.

Status: todo, audit (X86 hook exists in some form, compare), blocked (blocker, date).

| # | Pin function | Line | Status |
|---|---|---|---|
| 1 | `run` (the block walk, `LocalMIs`, `ImmDefMIs`, `FoldAsLoadDefCandidates`) | #L1776 | todo: pass skeleton before phielim's successor |
| 2 | `optimizeRecurrence`, `findTargetRecurrence` | #L1719, #L1648 | done: `recurrences_commuted` in backend/peepholeopt.rs |
| 3 | `isNAPhysCopy`, `foldRedundantNAPhysCopy` | #L1591, #L1595 | todo |
| 4 | `optimizeCmpInstr` (`TII->optimizeCompareInstr`) | #L948 | audit: ours is `peep` rules on flags; X86InstrInfo::optimizeCompareInstr is the pin's |
| 5 | `optimizeUncoalescableCopy`, `optimizeSelect`, `optimizeCondBranch` | #L1388, #L987, #L999 | todo; select/branch hooks are no-ops on X86 |
| 6 | `optimizeCoalescableCopy`, `optimizeCoalescableCopyImpl`, `rewriteSource`, `findNextSource` + `ValueTracker` | #L1298, #L1225, #L1330, #L1016, #L1978-#L2258 | todo: copy-source chasing; blocked on sub-register copies being values in LIR (check) |
| 7 | `isMoveImmediate`, `foldImmediate` (`TII->foldImmediate`) | #L1481, #L1503 | audit: X86InstrInfo::foldImmediate |
| 8 | `optimizeExtInstr` | #L784 | todo: ours is `extensions` (own); compare |
| 9 | `isLoadFoldable`, `foldLoadInto` (`TII->optimizeLoadInstr`, `foldMemoryOperand`) | #L1436, #L1457 | audit: ours folds loads after RA (`operand folding`) |
| 10 | `foldRedundantCopy` | #L1554 | audit: ours is `machinecp` post-RA |

## Order of work

1. Plan PR (this), then pass skeleton (1) with the instruction walk and the empty hooks.
2. 4, 7 and 9 first: they are what the premise says the pin's pass gains (`-disable-peephole`: 72 of 77 IRs differ).
3. 6 with the value tracker, then 8, 10, 3, 5, 2.
4. Each piece: source test, premise assertion, the post-RA group it makes dead (or not), and its DEVIATE row.

Dates: skeleton and 4/7/9 by 11-04 (queue), the rest 11-13.
