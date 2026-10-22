# One ScalarEvolution across the loop passes — w-3, plan note 2026-10-28

## Premise, corrected

First stated as 3.8x node creations (1774 for 468 distinct) and `get_scev` 36.7 M against the pin's 4.0 M, an upper bound of ~25 M. That compared different inputs and counted per-loop recreation with first builds. On the same input (docs/architecture/persistent-scev.md, w-5): repeated work 2.7 M; IndVars 1.28x and LSR 1.3x the pin's per visit.

Measured with the sharing of #3659 (indvars, loopdeletion, unroll), d_faces -O2: 1549.71 M → 1541.52 M (-8.2 M, -0.53%); `get_scev` 36.7 M → 33.2 M. The gain is trip-count work that no longer starts from an empty SE. LSR moved -0.2 M. A slice 2 (`Scevs` incremental, LSR adopting it) has a ceiling of a few M: closed.

## What exists

`Scevs` (llrm-analysis/src/scev/analysis.rs) is a manager analysis, used by `decide` and `algebraic`; no pass names it in `PreservedAnalyses`, so it is dropped after every editing pass. `ScevSession` is a pass-local copy with `forget_since(mark)`, which drops the expressions the change log made stale (`ScalarEvolution::forget_touched`: `forgetValue`, `forgetLoop`-users, trip counts of the touched loops) and everything when the CFG changed.

## Design, in the pin's terms

- The pin's loop passes preserve SE and call `forgetValue`/`forgetLoop`/`forgetBlockAndLoopDispositions` at each edit. Ours takes the change log: `Scevs` becomes INCREMENTAL, `update` = `forget_touched` for the edits since the entry's mark when `touched.cfg`, `blocks_created` and `blocks_erased` are empty; otherwise `None` (derived afresh). A pass that is SE-unaware therefore costs the expressions it touched, not the whole SE.
- A pass that owns an SE for its run (indvars, loopdeletion, unroll, lsr) takes the held one, keeps it current as it does today, and stores it back (`Analyses::replace`) before returning `preserve_updated::<Scevs>()`. Its own loops/tree/assumptions stay its own; they are not shared.
- Preserved sets: indvars, loopdeletion, lsr, loop-idiom: edit without CFG change → preserve updated. unroll, rotate, unswitch, interchange, unroll-and-jam: CFG changes → not preserved (SE dropped). licm: moves instructions, CFG unchanged → preserve updated once its edits are logged.
- Staleness is the risk (#3631 was wrong output waiting). `==` on `ScevCache` is always true, so `LLRM_CHECK_PRESERVED` cannot see a stale SE today. First PR: an `Analysis::consistent` hook ANDed into `Entry::still_true`; `Scevs` implements it with `ScalarEvolution::verify`. Negative control, red first: a test pass that edits an IV's operands and claims `Scevs`.

## `ScevSession::of`/`within` callers (non-test)

loopinterchange.rs ×2, looploadelim.rs ×5, indvars.rs, simplifyindvar.rs ×2, congruentiv.rs, exitpredicate.rs, exitopt.rs, exitvalues.rs, canonexit.rs, lsr_pass.rs, rotate.rs ×3, loopunrollandjam.rs ×2, profit.rs, backend valuefacts.rs. Own `ScalarEvolution::new`: loopdeletion.rs ×2, loopunrollpass.rs ×2, loopunrollandjam.rs ×3, simpleloopunswitch.rs, loopidiom/pass.rs.

## Steps

1. Oracle and negative control: #3657 (merged).
2. Sharing in indvars, loopdeletion and unroll: #3659. Stops there.
3. Each slice: d_faces -O2 Ir both arms; objsweep and QCport 65×4 objects identical; `LLRM_CHECK_PRESERVED`/`STALE` over the corpus with 0 stale; tests/run and qbrt.

Owners: w-3; SCEV semantics w-5 (told); manager funcattrs-port (told, no collision).
