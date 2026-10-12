> Generated from the `//!` headers of `crates/opt/llrm-analysis/src`; edit the headers, not this file.

# Analyses

From the `//!` headers of `crates/opt/llrm-analysis/src`: 36 documented, 0 undocumented.

| Name | Summary | Provides | Page |
|---|---|---|---|
| alias | Strong, flow-sensitive alias analysis: what each pointer may point to, what escapes and when, and what each call reads and writes. | `pointers`, `point_values`, `points_to` and `PointsTo` (`values`, `escaped`, | [alias](alias.md) |
| assumptions | What each block assumes, as LLVM's AssumptionCache: the conditions of the `llvm.assume` calls in it. | The assumed conditions per block. Consumers: [`crate::guards`](guards.md) turns them | [assumptions](assumptions.md) |
| avail | Available loads and dead stores, as LLVM's GVN load forwarding and DSE find them through MemoryDependence. | `forwardable_walk`, `dead_stores`, `dead_stores_escaping`, and the | [avail](avail.md) |
| branchprob | Static branch probabilities and block frequencies, as LLVM's `BranchProbabilityInfo` and `BlockFrequencyInfo` estimate them when no profile says otherwise. | `Odds { taken, frequency, by }` from `estimated`, `propagated` and | [branchprob](branchprob.md) |
| cellmap | A map from memory cell to fact, indexed by the object each cell lies in, so a write is tested only against the cells it can reach. | The container. Consumers: [`crate::alias`](alias.md), [`crate::consts`](consts.md), | [cellmap](cellmap.md) |
| cfg | A function's blocks as the graph walks see them: block ids, successors, dominance, natural loops and reachability, built once per body. | `Shape` (a `Analysis` named `shape`), the | [cfg](cfg.md) |
| constant_cycles | Sparse value propagation over phis, with distinct pending and overdefined states, to find constants that only a cycle proves. | `State`, `Successors` and `propagated`. Consumers: [`crate::consts`](consts.md) | [constant_cycles](constant_cycles.md) |
| consts | Which values are known, and what they are: integer constant propagation and folding, with memory read through MemorySSA. | `Known`, `known` (registers, cached as [`crate::manager::Registers`](manager.md)), | [consts](consts.md) |
| demanded | Which bits of each integer value anything reads: LLVM's `DemandedBits`. | `demanded(context, function)`: a `ValueId -> u128` map. Cached as | [demanded](demanded.md) |
| dependence | Data dependence distances over a two-deep loop nest, and the outer unroll factor that unroll-and-jam may use for them. | `distances`, `factor`. Consumer: `unroll_jam` | [dependence](dependence.md) |
| difference | What a set of compares proves of the unknowns they test, by difference bounds: LLVM's ConstraintElimination. | `proves(width, facts, ranges, predicate, left, right)`. Its one consumer is | [difference](difference.md) |
| effects | What a call may do to memory its operands do not name, from LLVM's call attributes. | `call`, `touches_memory`, `writes_memory`, `unmodeled_read`, | [effects](effects.md) |
| floatbounds | Floating operations that are exact over bounded, not necessarily known, integers. | `exact`, `evaluated`, `_memory`. No pass consumes them: only this module's | [floatbounds](floatbounds.md) |
| floatfacts | Exact finite values of floating-point operations, and the memory they pass through. | `Solved { integers, facts }`, `solved_with`, `solved_over`, `known`, | [floatfacts](floatfacts.md) |
| frameescape | Which frame objects' addresses reach where, and which are exposed. | `exposed_allocas`, `exposes`, `framed`, `analysed`. Consumers: | [frameescape](frameescape.md) |
| generated | Random loops for checking the recurrence form (`induction::Scev`) against the interpreter. | `case`, `seeds`, `observed`, `Inputs`, `Rng`, `Tracked`, built only for | [generated](generated.md) |
| globalsaa | Which globals no code outside the program reaches but by name: LLVM's GlobalsAA, GCC's ipa-reference. | `Globals` (`tracked`, `unsummarized`, `entries`, `covers`), `analysis`, | [globalsaa](globalsaa.md) |
| graph | Block-graph algorithms (loops, depth, reducibility, dominators) over any IR whose blocks implement `loops::Node`. | The algorithms of `loops`. Consumers: [`crate::cfg`](cfg.md) (`Loop`, `Node`, | [graph](graph.md) |
| guards | What the branches that dominate a block prove on entry to it. | Guard facts and the implication queries above. Consumers: `decide`, | [guards](guards.md) |
| induction | Which values are affine functions of a loop's counter, and how many trips a loop makes. | `AffineOperand`, `Affine`, `CountedLoop`, `Counted`, `ExitCount`, `Scev`, | [induction](induction.md) |
| interprocedural | Facts that cross direct procedure boundaries: constant arguments, constant returns, purity and termination of calls. | The functions above, and the re-export of `noreturn::terminal_sites`. | [interprocedural](interprocedural.md) |
| liveness | Which SSA values are live at each block's entry and exit, and how many are live at once. | `Liveness`, `live`, `live_where`, `pressure`, `pressure_of`, `entry_values`, | [liveness](liveness.md) |
| manager | The memory and range analyses as entries of llrm-mir's analysis manager: asked for by name, cached, kept or brought up to date after a pass. | The entries: `ExposedFrames`, `PointerValues`, `Pointers`, `Annotated`, | [manager](manager.md) |
| memory | The memory model every alias query reads: objects, byte slices, provenance, accesses (`MemRef`) and the `Unit` that carries a function's facts. | The types above; `exposed_frames`, `object_of`, `global_object`, `typed`, | [memory](memory.md) |
| memoryssa | Memory SSA: a single memory state versioned by stores and writing calls, with a walker that finds the writes a load may see. | `Accesses` (analysis `accesses`), `MemorySSA` via `built`, `Kind`, `Access`, | [memoryssa](memoryssa.md) |
| nearest | The items placed in blocks that dominate a given block, nearest first. | The `Nearest` index and its `Walk` iterator. Consumers: [`crate::avail`](avail.md) and | [nearest](nearest.md) |
| noreturn | Which bodies and blocks cannot reach a normal return, and the edit that cuts a block after a call that cannot return. | `TerminalSites` (analysis of the function with no body proven noreturn), | [noreturn](noreturn.md) |
| observers | Which memory cells nothing outside a function can read. | `private` and `private_of` (the same test with `Published`, the manager's | [observers](observers.md) |
| occurrence | The instructions of a function as occurrences: operations and phis, apart. | The three walks. Consumers: [`crate::induction`](induction.md), [`crate::ssa`](ssa.md), and the | [occurrence](occurrence.md) |
| parameter_ranges | The range of each integer parameter of a body only the program calls, from what its callers pass. | `stamp`: a `range` attribute on each integer parameter whose callers all | [parameter_ranges](parameter_ranges.md) |
| peelsize | Whether a loop is worth copying out completely, decided before anything is cloned. | `admitted`, `Limits`, `Site`. Consumers: the `peel` and `unroll` passes in | [peelsize](peelsize.md) |
| pointerfacts | Constant byte offsets between pointer values, and disjointness of two accesses that share a base. | `Offsets` (made by `offsets`), `Location`. Consumers: [`crate::memoryssa`](memoryssa.md), | [pointerfacts](pointerfacts.md) |
| ranges | Non-wrapping integer intervals per block, from the branch edges that dominate a block and the counted loops that hold it. | The interval facts above. In the manager they are `DominatedEdges` and | [ranges](ranges.md) |
| regions | Whether two memory references can meet: `may_alias`, `overlapping`, and the indexes that find the candidates without comparing every pair. | `may_alias`, `overlapping`, `typed_apart`, `foreign_provenance`, | [regions](regions.md) |
| sliceset | An ordered set of memory slices that holds most of itself as a run shared with other sets. | `SliceSet`, `Shared` and `whole`. Consumers: [`crate::alias`](alias.md) (call | [sliceset](sliceset.md) |
| ssa | SSA helpers: use lists, dead-phi pruning, value substitution and an SSA updater. | The helpers above. Consumers in llrm-transforms: `dead`, `promote` | [ssa](ssa.md) |
