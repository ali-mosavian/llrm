> Generated from the `//!` headers of `crates/opt/llrm-transforms/src`; edit the headers, not this file.

# Passes

From the `//!` headers of `crates/opt/llrm-transforms/src`: 59 documented, 0 undocumented.

The order the passes run in: [pipeline](pipeline.md), The MIR fixed point every driver runs: llrm-core's `optimize/transform.rs` `pipeline`, `applied`, `recorded`, `Applied` and `_Transaction` (with `_applied` and `_transacted`), on llrm-mir's pass manager.

| Name | Summary | Runs | Page |
|---|---|---|---|
| addresssink | CodeGenPrepare's `sinkCmpExpression`: a comparison a branch in another block reads is made again beside that branch, so isel keeps it in the flags (a loop's test, GVN'd with the copy of it made ahead of the loop, was an `i1` held across the loop's body: `sete`, a store of the byte, `cmp` and `jne`, hanoi). |  | [addresssink](addresssink.md) |
| algebraic | Algebraic identities over integer values, as LLVM's InstCombine and, for a loop-carried operand, Reassociate. |  | [algebraic](algebraic.md) |
| argpromotion | LLVM's ArgumentPromotion: a function only direct calls reach takes, in place of a pointer it only reads, the fields it reads through it. |  | [argpromotion](argpromotion.md) |
| availableexternally | LLVM's EliminateAvailableExternally: a body the program holds only to inline from is a declaration again once inlining is done, so no copy of it is emitted and what calls it still reaches the runtime's own routine. |  | [availableexternally](availableexternally.md) |
| calleepop | LLVM's GlobalOpt gives an internal function whose every caller it sees a calling convention of its own (`fastcc`). |  | [calleepop](calleepop.md) |
| canonical | Canonical forms, as LLVM's InstCombine puts them: compares with the constant on the right, no neutral terms, and a zero test at the width of what it tests. |  | [canonical](canonical.md) |
| cfg | Adapted from llrm-core's `optimize/cfg.rs`, the port of `qbopt/optimize/cfg.py`: exact MIR control-flow cleanup. |  | [cfg](cfg.md) |
| counting | What a counted-loop rewrite places before the loop: its seeds, skip test and exit values. |  | [counting](counting.md) |
| dead | Instructions nothing needs, removed: LLVM's ADCE and DCE. |  | [dead](dead.md) |
| deadargs | LLVM's DeadArgumentElimination for a function only direct calls reach: a parameter nothing reads (but its own recursive calls, which pass it on to itself) goes, and with it the argument of every call. |  | [deadargs](deadargs.md) |
| decide | Branches whose way is known, decided: LLVM's JumpThreading and SimplifyCFG on known conditions. |  | [decide](decide.md) |
| dse | LLVM's DSE: llrm-core's `DropStores` and `without_dead_stores`, from `optimize/transform.rs`. |  | [dse](dse.md) |
| edges | Adapted from llrm-core's `optimize/edges.rs`, the port of `qbopt/optimize/edges.py`: place semantic operations on conditional edges, not on either arm. |  | [edges](edges.md) |
| exitfold | Exits their counts decide, folded: LLVM IndVarSimplify's `optimizeLoopExits`. |  | [exitfold](exitfold.md) |
| exitsink | An add, subtract or extension in a loop that only exit phis read, moved to those exits: LLVM's LICM sinking an instruction only used outside the loop, a copy to each exit that reads it. |  | [exitsink](exitsink.md) |
| expand | Values placed from their sums: LLVM's `SCEVExpander`. |  | [expand](expand.md) |
| fill | A counted loop that stores one byte value into consecutive bytes is one `llvm.memset`: LLVM's LoopIdiomRecognize. |  | [fill](fill.md) |
| fixednarrow | A fixed-point product whose operands' product fits the width: `llvm.smul.fix(a, b, s)` is `(a * b) >> s` at the width, as `mul` and `ashr` are, where `ranges` proves the 64-bit product the intrinsic stands for never exceeds 32 bits. |  | [fixednarrow](fixednarrow.md) |
| floatfold | Float values floatfacts proves exact, replaced by their constants: LLVM's InstSimplify and InstCombine for floats. |  | [floatfold](floatfold.md) |
| floatloop | A counted loop of exact float work in memory, run once: its last trip, from the memory the trips before leave, which floatfacts proves. |  | [floatloop](floatloop.md) |
| fold | Values known to be numbers, replaced by those numbers: LLVM's SCCP and InstSimplify. |  | [fold](fold.md) |
| gepoffset | LLVM's SeparateConstOffsetFromGEP: the constant an index adds, under a scale or through an extension its flags allow, moves into a `gep` of its own, where every address mode takes it as a displacement. |  | [gepoffset](gepoffset.md) |
| globaldce | LLVM's GlobalDCE: a module's own body or variable that nothing live names -- no code outside the program, no live body, no live initializer -- is deleted. |  | [globaldce](globaldce.md) |
| globalopt | An internal global the module only ever loads from made `constant`, as LLVM's GlobalOpt marks one it finds never stored: a load of it is then its initializer's bytes (`memory::constant_bits`). |  | [globalopt](globalopt.md) |
| gvn | Adapted from llrm-core's `optimize/gvn.rs`, the port of `qbopt/optimize/gvn.py`: global value numbering and partial redundancy elimination. |  | [gvn](gvn.md) |
| hoist | Loop-invariant work done once, before the loop: LLVM's LICM hoisting. |  | [hoist](hoist.md) |
| homes | A phi whose value is a cell's: what the optimizer proved of a loop-carried value that the program also stores, kept for selection as `!llrm.home` on the phi, naming one store of the cell. |  | [homes](homes.md) |
| indvars | IndVarSimplify's exit rewriting reused across an outer loop: llrm-core's `optimize/indvars.rs`, the port of `qbopt/optimize/indvars.py`, adapted to the rich MIR. |  | [indvars](indvars.md) |
| inferspace | LLVM's InferAddressSpaces: a far pointer made from a near one by `addrspacecast` addresses what the near one does, so what only reads or writes through it, or steps it, can use the near one and drop the selector: no `les`, no segment register held across a loop. |  | [inferspace](inferspace.md) |
| inline | Adapted from llrm-core's `optimize/inline.rs`, the port of `qbopt/optimize/inline.py`: selective whole-module MIR inlining. |  | [inline](inline.md) |
| interprocedural | Adapted from llrm-core's `optimize/interprocedural.rs`: the whole-program step a frontend runs once every body has reached its own fixed point: inline, carry constants across direct calls, drop dead pure calls and the tails of terminal ones, and send each changed body back through its pipeline until no body changes. |  | [interprocedural](interprocedural.md) |
| ipacp | GCC's `ipa-cp` cloning (ipa-cp.cc): a function called with a constant actual is copied for it, the actual replaced by the constant in the copy, and the calls redirected. |  | [ipacp](ipacp.md) |
| jumpthread | A switch in a loop that the paths into it decide, entered from the end of each path at the case it decides: LLVM's `DFAJumpThreading` and GCC's finite-state-machine threader (`tree-ssa-threadbackward.cc`). |  | [jumpthread](jumpthread.md) |
| lcssa | Adapted from llrm-core's `optimize/lcssa.rs`, the port of `qbopt/optimize/lcssa.py`: every supported natural loop in closed SSA form. |  | [lcssa](lcssa.md) |
| lcssamerges | Adapted from llrm-core's `optimize/lcssamerges.rs`, the port of `qbopt/optimize/lcssamerges.py`: a loop with several exits is closed by a phi at each exit, merged by phis where the exits' paths join. |  | [lcssamerges](lcssamerges.md) |
| loadjoins | LLVM's GVN load PRE at joins: llrm-core's `optimize/loadjoins.rs`, the port of `qbopt/optimize/loadjoins.py`, adapted to the rich MIR. |  | [loadjoins](loadjoins.md) |
| loopclone | Adapted from llrm-core's `optimize/loopclone.rs`, the port of `qbopt/optimize/loopclone.py`: clone loop iterations as CFGs, retaining a residual loop and every exit. |  | [loopclone](loopclone.md) |
| loopexit | Affine exit values evaluated, and finite counted loops with no effect deleted: LLVM's IndVarSimplify exit-value rewriting and LoopDeletion. |  | [loopexit](loopexit.md) |
| loopmotion | Loop stores nothing inside the loop observes, sunk to its single exit: LICM's store sinking, as `promoteLoopAccessesToScalars` leaves one store after the loop. |  | [loopmotion](loopmotion.md) |
| loopsimplify | Adapted from llrm-core's `optimize/loopsimplify.rs`, the port of `qbopt/optimize/loopsimplify.py`: every reducible loop gets one preheader, one latch and dedicated exits. |  | [loopsimplify](loopsimplify.md) |
| lsr | A loop's induction variables chosen once, by cost: LLVM's LoopStrengthReduce and GCC's ivopts. |  | [lsr](lsr.md) |
| mirror | A pointer that follows a counter which only some trips step: the address `base + k * i` of a target whose addresses take no scale `k`. |  | [mirror](mirror.md) |
| narrowspace | A far pointer parameter of a function only direct calls reach, which every call fills with an address in one near space, is a near pointer of it: the caller passes the offset, the callee reads through that space's segment, and the segment is neither pushed nor loaded. |  | [narrowspace](narrowspace.md) |
| peel | Exact CFG loop peeling, priced before anything is cloned: llrm-core's `optimize/peel.rs`, the port of `qbopt/optimize/peel.py`, adapted to the rich MIR. |  | [peel](peel.md) |
| phiopt | A phi of two constants that a branch chooses between becomes arithmetic of the branch's condition: gcc's `pass_phiopt` (`conditional_replacement`, `two_value_replacement`, and match.pd's `cond ? c : 0` as `-cond & c` through `match_simplify_replacement`, tree-ssa-phiopt.cc) and LLVM's `FoldTwoEntryPHINode` (SimplifyCFG). |  | [phiopt](phiopt.md) |
| ports | Each I/O port call narrowed to the memory its device reaches, as the target describes the port. |  | [ports](ports.md) |
| profit | Adapted from llrm-core's `optimize/profit.rs`, the port of `qbopt/optimize/profit.py`: machine-neutral profitability shared by MIR transforms. |  | [profit](profit.md) |
| promote | Write-through promotion: a load of a cell a store left its value in on every path becomes that value, a phi where paths join; every store stays. |  | [promote](promote.md) |
| rotate | Enter a loop proven to run at least once at its body, not at its test; and a loop that is not proven, behind a copy of its test (gcc's `-ftree-ch`, `tree-ssa-loop-ch.cc`): llrm-core's `optimize/rotate.rs`, the port of `qbopt/optimize/rotate.py`, adapted to the rich MIR. |  | [rotate](rotate.md) |
| spares | What the optimizer proved of a fixed-cell load, kept for selection: each write that leaves its bytes as they were, as `!llrm.spares` on the load. |  | [spares](spares.md) |
| spill | What values past the target's registers cost: the one spill model every pricing of register pressure shares. |  | [spill](spill.md) |
| splitcopy | A `memcpy` into a local nothing else can reach is the loads and stores of the bytes the local is read as. |  | [splitcopy](splitcopy.md) |
| tailrec | A call a function makes to itself as the last thing it does is a branch to its own entry with the arguments rewritten: LLVM's TailRecursionElimination, and gcc's tree-tailcall. |  | [tailrec](tailrec.md) |
| transform | Helpers adapted from llrm-core's `optimize/transform.rs`, each copied as a ported pass needs it; the pipeline itself is `pipeline`. |  | [transform](transform.md) |
| trivialunswitch | LLVM's SimpleLoopUnswitch, the trivial form (`unswitchTrivialBranch`): a branch on a loop-invariant condition that leaves the loop on one side is decided before the loop, not on every trip. |  | [trivialunswitch](trivialunswitch.md) |
| unroll | Bounded full unrolling of small exact-trip loops: llrm-core's `optimize/unroll.rs`, the port of `qbopt/optimize/unroll.py`, adapted to the rich MIR. |  | [unroll](unroll.md) |
| unroll_jam | Unroll-and-jam of a loop nest at -O3: gcc's `pass_loop_jam` ([`gimple-loop-jam.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc)), LLVM's `LoopUnrollAndJamPass`. |  | [unroll_jam](unroll_jam.md) |
| unswitch | Adapted from llrm-core's `optimize/unswitch.rs`, the port of `qbopt/optimize/unswitch.py`: specialize a loop around a pure invariant condition, entirely in MIR. |  | [unswitch](unswitch.md) |
| window | A counted loop's huge pointers split into windows: LLVM's IRCE, with the carry into the selector as the check that cannot fire inside one. |  | [window](window.md) |
