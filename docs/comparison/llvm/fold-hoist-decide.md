# fold, hoist and decide on d_faces, and the front end

TL;DR: the 12x per run of `fold` on `d_faces` is one function and one feature. `d_draw_faces` holds 156 M of `fold`'s 266 M (four runs of 24 to 56 M; the other ten bodies, 0.1 to 8 M); half of a `fold` run is the float half (`FloatFacts`, 94 M of 266 M), and the integer half's changes invalidate every analysis before the float half asks for its own (the fold-only share of that recompute is about 1% of `d_faces`, see Proposed change). A ladder shape of the same mix (`mixed`) reproduces it: without floats `fold` falls 52% and LLVM's InstCombine does not move. `hoist` and `gvn` follow the loops and diamonds, `decide` is a flat 24 to 31 M whatever the body holds. The front end is 81 M against clang's 33 M, three IRs where clang builds one.

## LLVM

InstCombine is a worklist over the function's instructions; each visit folds one instruction from its operands and the constants they hold. It asks for no dataflow: no constant propagation through memory, no float analysis, no analysis of its own before the first rewrite.

[`InstructionCombining.cpp#L5780-L5800`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L5780-L5800)

```cpp
bool InstCombinerImpl::run() {
  while (!Worklist.isEmpty()) {
    // Walk deferred instructions in reverse order, and push them to the
    // worklist, which means they'll end up popped from the worklist in-order.
    while (Instruction *I = Worklist.popDeferred()) {
      // Check to see if we can DCE the instruction. We do this already here to
      // reduce the number of uses and thus allow other folds to trigger.
      // Note that eraseInstFromFunction() may push additional instructions on
      // the deferred worklist, so this will DCE whole instruction chains.
      if (isInstructionTriviallyDead(I, &TLI)) {
        eraseInstFromFunction(*I);
        ++NumDeadInst;
        continue;
      }

      Worklist.push(I);
    }

    Instruction *I = Worklist.removeOne();
    if (I == nullptr) continue;  // skip null values.

```

LICM is capped: its memory SSA queries stop at 100 and its promotion at 250 accesses.

[`LICM.cpp#L149-L160`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L149-L160)

```cpp
cl::opt<unsigned> llvm::SetLicmMssaOptCap(
    "licm-mssa-optimization-cap", cl::init(100), cl::Hidden,
    cl::desc("Enable imprecision in LICM in pathological cases, in exchange "
             "for faster compile. Caps the MemorySSA clobbering calls."));

// Experimentally, memory promotion carries less importance than sinking and
// hoisting. Limit when we do promotion when using MemorySSA, in order to save
// compile time.
cl::opt<unsigned> llvm::SetLicmMssaNoAccForPromotionCap(
    "licm-mssa-max-acc-promotion", cl::init(250), cl::Hidden,
    cl::desc("[LICM & MemorySSA] When MSSA in LICM is disabled, this has no "
             "effect. When MSSA in LICM is enabled, then this is the maximum "
```

Clang builds LLVM IR in one step from the AST: [`CodeGenFunction::GenerateCode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/clang/lib/CodeGen/CodeGenFunction.cpp#L1454) emits instructions through an IRBuilder.

## llrm

`fold` is two halves in one pass. The integer half (`_numbers`) asks for the held facts and `ThroughMemory` (every load of a cell the reaching stores fill with one number). If it changed anything, every analysis is dropped; then the float half asks for `FloatFacts`, which asks for `Annotated`, `PlainAccesses`, `Registers` and `Shape` again.

[`fold.rs#L177-L195`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-transforms/src/fold.rs#L177-L195)

```rust
fn _folded(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    calls: &Calls,
) -> bool {
    // Before the facts are asked, so that they see the constants.
    let edges = edges::conditions_decided(context, function);
    if edges {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let numbers = _numbers(context, layout, function, analyses, calls);
    if numbers {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    floatfold::_folded(context, layout, function, analyses, calls) | numbers | edges
}

```

[`fold.rs#L198-L214`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-transforms/src/fold.rs#L198-L214)

```rust
fn _numbers(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    calls: &Calls,
) -> bool {
    let (values, edge) = {
        let held = manager::Held::of(context, layout, function, analyses, true);
        let outer = std::rc::Rc::clone(analyses.outer());
        let unit = held.unit(context, layout, function, &outer);
        let exits = floatfacts::loop_exits(&unit, calls);
        // With no loop to leave the answer is the manager's, where it was of
        // these writes.
        let shared = (exits.is_empty() && *calls == manager::writes(context, layout, function, analyses))
            .then(|| analyses.get::<manager::ThroughMemory>(context, layout, function));
        let facts = match shared.as_deref() {
```

The C front end is a child process (Open Watcom's `wccq`) that records the code generator's calls; `translate` turns that stream into HIR, a second IR with its own model; `hir::mir::emit` lowers HIR to MIR, a third.

[`translate.rs#L1-L7`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/frontends/llrm-c/src/translate.rs#L1-L7)

```rust
//! One unit's trees as llrm's HIR: each C object a place or a data object,
//! each tree node its instructions. `hir::mir` makes the MIR.
//!
//! What C promises is marked in the HIR: its aliasing classes, `restrict`,
//! internal linkage for what the unit does not export, `nowrap` on signed
//! arithmetic, `inbounds` on pointer arithmetic. The last two hold for
//! objects of at most PTRDIFF_MAX bytes, as clang assumes of every object.
```

## Divergence

`fold` pays for facts InstCombine never computes, and pays for the float facts a second time after the integer half's edits dropped what they share. `hoist` is LICM plus a spill forecast and a movable-run search per candidate, `decide` is SimplifyCFG plus the held facts of every branch; both ask for their analyses first (`r01-decide` 0.47 M of its own work, 13.0 M with the analyses). The front end makes a stream, HIR and MIR where clang makes IR, and each is a heap structure of its own.

## Measured

Instructions, `d_faces.c` of qbq-llrm at `-O2 -m32 -march=i486 -fno-inline-functions`, llrm-c of main `21f5c9fc8` (callgrind inclusive, `LLRM_DEBUG=runs`), then re-run on main `1a3e557c1` for the ladder rows.

`fold` by body (M, with the analyses it asks first; four runs unless noted): `d_draw_faces` 156.0 (runs 55.7, 41.8, 34.0, 24.5), the body before it 8.5, the rest 0.1 to 1.5 each. Inside `fold` over the file (266.5 M, 48 runs): `_numbers` 137.9 (`ThroughMemory` 70.0, `Held::of` 35.9, the float solver 26.1 in two runs), the float half 94.4 over 20 runs (`Annotated` 44.1, `floatfacts::solved_over` 41.4, `PlainAccesses` 3.8). `d_draw_faces` after its first pass: 1,198 MIR instructions, 175 loads, 73 stores, 38 calls, 112 phis, 112 float operations.

Bisect on the `mixed` shape (`tools/ladder/ladder_gen.py mixed 200`: a loop over two struct pointers with field loads and stores, diamonds, double arithmetic and calls to an external function; the same C text for both compilers). llrm M, LLVM M, with one feature removed:

| variant | llrm `fold` | LLVM InstCombine | llrm `gvn` | LLVM GVN | llrm `hoist` | LLVM LICM | llrm `decide` | LLVM SimplifyCFG |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| full | 108.7 | 28.1 | 214.2 | 20.4 | 63.6 | 5.0 | 26.1 | 5.5 |
| no calls | 94.8 | 26.7 | 230.8 | 13.1 | 78.7 | 14.7 | 25.5 | 5.7 |
| no stores | 76.7 | 25.4 | 154.2 | 17.6 | 41.8 | 4.3 | 24.5 | 5.5 |
| no diamonds | 68.1 | 9.8 | 59.7 | 7.9 | 36.3 | 3.1 | 24.2 | 1.5 |
| no loop | 123.8 | | 187.0 | | 0.3 | | 30.7 | |
| no floats | 52.1 | 29.4 | 130.9 | 7.2 | 29.1 | 3.7 | 18.7 | 5.5 |

Reading it: without floats `fold` falls 52% and InstCombine is unchanged; `gvn` follows the diamonds (-72%); `hoist` needs the loop and shrinks with each feature; `decide` stays at 24 to 31 M in every variant. By size on the full shape (llrm / LLVM, M): N=50 `fold` 24.4 / 7.8, N=100 46.4 / 14.7, N=200 108.7 / 28.1, N=400 192.4 / 55.0; `gvn` 45.4 / 5.2, 95.8 / 10.4, 214.2 / 20.4, 530.7 / 39.9; `hoist` 10.0 / 9.5, 24.0 / 12.6, 63.6 / 5.0, 189.5 / 7.9; `decide` 6.6 / 2.3, 13.1 / 3.4, 26.1 / 5.5, 67.4 / 9.7. The ratios are the `d_faces` ones: `fold` 3.9x (4.4x on `d_faces`), `gvn` 10x (7.7x), `hoist` 13x to 24x (23x), `decide` 5x to 7x (16x).

On main `1a3e557c1` the same N=200 shape: `fold` 99.2, `gvn` 200.0, `hoist` 61.5, `decide` 24.6 M; the `hir verify` of 30.4 M that the older binary ran is gone (#1963).

Inside `hoist` and `decide`, from the [compile-time spec](../../architecture/compile-time-spec.md): an idle `hoist` run is 35% `Accesses::managed`, 23% `_invariant_run` (19.8 k `_movable` calls) and 17% spill forecast; an idle `decide` run is 27% `Held::of`.

The front end, `d_faces` on main: `wccq` child 18 M (perf stat with and without `--no-inherit`), `frontend translate` 42.0 M, `hir to mir` 15.3 M, `hir::unit` and the stream 5.8 M: about 81 M against clang's 33.0 M (front end of `cc1`), 2.5x. On the ladder it is linear: `arith` `translate` 58.8, 234.4 M at N=400, 1600 (16.2 k per IR instruction; clang's whole `cc1 -disable-llvm-passes` is 16.4 k per IR instruction), `hir to mir` 20.3, 79.8 M. The front end allocates 260,499 blocks (6.9% of all allocations, 31 MB).

## Proposed change

For edit-rollback (`fold`) and the owners of `hoist` and `decide`; none built here.

1. Keep what the integer half does not change: withdrawn. A `MemRef` is made of its operands, so a replaced address operand changes a cell's identity and `Annotated`, `PlainAccesses` and `ThroughMemory` cannot be vouched for across `_numbers`; only `Shape` is safe, when no CFG edit was made. edit-rollback counted 5 of 11 replacing `fold` runs with a replaced value feeding a load, store or `gep`: the ceiling is about 1% of `d_faces` -O2 with a per-run check of the replaced values' users. The exact route is `Annotated::update` from the change log, on keep-analyses' side. My 52.5 M (1.8%) assumed no replaced value feeds an address and was an upper bound that does not hold.
2. Ask for `FloatFacts` only when a float operation can fold: measured by edit-rollback and not built (it skips 6 of 20 solves, +0.04%, objects identical; compile-time spec section 8). The float half costs 94.4 M over 20 runs and 26.1 M in the float solver, 45% of `fold`, where InstCombine does not move when the floats go; the cost is the lazy query keep-analyses' design covers (is this float load fed by a known store), not the decision to ask.
3. `hoist` and `decide`: check for a candidate before asking for analyses; see [runs that change nothing](idle.md).
4. Front end: nothing proposed. It is 2.5x on 81 M, 3% of the compile; the verifier it ran (30 M) is already off.

Open: how often a float run follows a changing integer half on `d_faces`, and what `hoist`'s `Accesses::managed` builds per run ([#1994](https://github.com/ali-mosavian/llrm/issues/1994)).
