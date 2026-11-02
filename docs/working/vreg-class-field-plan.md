# A register class on each virtual register

Plan only. Scope as approved: the class field, set by isel as the pin's `createVirtualRegister` sets it, read by the coalescer env (retires DEVIATE C10), plus `recompute_class`. The wider LIR data model is not touched.

## Today

A LIR value is a `u32`. `LirBody` has `origin` and `pins` per value and no class. `backend/regclass.rs` derives one from the uses (`classes`, `classes_given`) each time a phase asks; the base/index two-colouring of `[base+index]` pairs is applied only there (`allocate.rs`, `greedy_rewritten`). Values made after isel take their parent's order unrecomputed (`greedy/inlinespiller.rs` about L362 and L514-L523; `greedy/shadow.rs` `note_split_from`, `Env::reclass` skips pair members because `confinement_of` returns `Unknown` for them). `ls.c` `_ls_animate@3`: two reload values confined to {EBX}, no ladder places them.

## The pin

| where | what it does |
|---|---|
| [PHIElimination.cpp#L415](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L415) | the phi temporary is made in the result's class |
| [TwoAddressInstructionPass.cpp#L1445](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1445) | the copy's register is made in the tied register's class |
| [LiveRangeEdit.cpp#L55](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeEdit.cpp#L55), [InlineSpiller.cpp#L791](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InlineSpiller.cpp#L791), [#L1383](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InlineSpiller.cpp#L1383) | `createFrom(old)`: the old register's class |
| [LiveRangeEdit.cpp#L426](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeEdit.cpp#L426), [SplitKit.cpp#L628](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SplitKit.cpp#L628) | `calculateRegClassAndHint` calls `recomputeRegClass`: widen to what the new value's own instructions accept |
| [RegisterCoalescer.cpp#L4392](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L4392) | `recomputeRegClass` on a joined register |

## Touched types and callers (grep counts, not a call graph)

- `model/lir.rs` `LirBody`: one new map, value to class, beside `pins`. The class is today's palette (`BTreeSet<RegId>`) from `RegisterClasses`; no new vocabulary. Struct literals of `LirBody`: 15 outside tests (`driver/basic.rs` 4, `allocate.rs` 4, `peephole.rs` 2, `masm.rs` 2, `jumps.rs` 2, `liveintervals.rs` 1) and 9 in tests.
- Isel makes values at 119 sites: `isel.rs` 98, `isel/matcher.rs` 12, `isel/wide.rs` 5, `isel/huge.rs` 4 (`fresh()`, `fresh_held`). Each sets a class.
- Passes after isel number new values at 27 call sites in 10 files (`_next_value`, `_next_value_following`): `splitkit.rs` 9, `allocate.rs` 8, `floatassign.rs` 2, `constrain.rs` 2, `phielimination.rs`, `twoaddr.rs`, `ssarepair.rs`, `spiller.rs`, `phiconst.rs`, `floatalloc.rs` 1 each. Plus the greedy creators (`inlinespiller.rs`, `spliteditor.rs`, `shadow.rs`), which splitkit moves onto `recompute_class`.
- Readers: `coalesce.rs` `llvm_env` (its palettes and the `touched`/fixpoint stand-in go), `coalescer/env.rs` `TargetEnv::narrow` (becomes the recompute after a join), `regclass.rs` (`classes` stays the oracle that `recompute_class` is built from).

## Numbered DEVIATEs

- CF-1 (a target fact, from `RegisterClasses`, not a deviation): the m16 `[base+index]` two-colouring (base {BX}, index {SI,DI}) has no pin function; `recompute_class` respects it for a pair that names the value. Exit: none while the target needs it.
- CF-2: during the change a value with no entry reads its class from `regclass::classes` as today, so passes move over one at a time. Exit: every site sets a class; the fallback is deleted with the last site.

## Order

1. Field, isel sets it, `phielimination` and `twoaddr` copy the pin's classes. Fail-first: `test_a_far_pointers_phi_is_not_copied_through_ax` (seen to fail on parity 3b70c16822 when written; re-run before step 1).
2. `recompute_class` over `regclass::collected`, with the `%51`/%53 shape: a word stored through an address, confined to {EBX} by inheritance, widens to the general class; a pair member keeps its side.
3. Coalescer reads the field; DEVIATE C10 and the `touched` stand-in are deleted.
4. Splitkit calls `recompute_class` at the spiller and split sites (their PR).

Each step priced on all rows and QCport, no tuning; steps 1-3 may be one PR if the isel part is small after the first site is done, else two.

## Not done

No code; the counts above are greps. Whether a one-map field costs compile time (the map is built per body) is not measured. The wider LIR data model waits for the green gate.
