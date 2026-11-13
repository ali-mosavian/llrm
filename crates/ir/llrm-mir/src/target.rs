//! What analyses may ask of the target, which MIR does not state: the
//! driver names one for its program (`program::Program::target`), as
//! LLVM's `TargetMachine` gives its analyses `TargetTransformInfo`.

use std::collections::BTreeSet;

use crate::spaces::Spaces;

/// The tuning limits of a target: one default (the pin's value, which every
/// target without an entry takes) and the overrides a target description
/// states. The names are the pin's `TargetLoweringBase` fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    /// DEVIATE SD-REPFILL: at size, a constant fill whose stores take more
    /// bytes than a `rep stos` fill is the fill. None: the pin's stores.
    pub size_fill: Option<SizeFill>,
    /// `MaxStoresPerMemset`, and at size `MaxStoresPerMemsetOptSize`.
    pub max_stores_per_memset: usize,
    pub max_stores_per_memset_opt_size: usize,
    /// `MaxStoresPerMemcpy` and `...OptSize`: the load and store pairs of a
    /// constant copy.
    pub max_stores_per_memcpy: usize,
    pub max_stores_per_memcpy_opt_size: usize,
    /// `MaxStoresPerMemmove` and `...OptSize`.
    pub max_stores_per_memmove: usize,
    pub max_stores_per_memmove_opt_size: usize,
    /// Clocks a byte of code is worth, in thousandths, where a decision is
    /// made for size; 0: bytes alone decide.
    pub size_milliclocks_per_byte: i64,
    /// What a byte is worth in thousandths of a clock at speed; 0: bytes and
    /// clocks are not traded.
    pub speed_milliclocks_per_byte: i64,
    /// A fill is weighed against its loop on the target's prices at every
    /// level (`loopidioms::fills_pay`): strictly where the rate is 0 (neither
    /// clocks nor bytes may grow), by the rate where it is not. false: the
    /// pin's, which forms the memset whenever the stores allow.
    pub price_fills: bool,
    /// A multiply by a constant at speed is the chain of fewest clocks the
    /// CPU's table gives, with no byte term (the search at
    /// -O1..-O3); false: the chain where `speed_milliclocks_per_byte` pays
    /// for its bytes.
    pub multiply_by_clocks: bool,
    /// DEVIATE PO1 (owner x86-m32-b, exit 2026-11-24 for the targets that keep it):
    /// at -Os the arguments of a call are popped into a dead register in place
    /// of `add sp,N` (one byte shorter, up to 7 clocks slower a call). false:
    /// the pin's, which pops only at `minsize` (X86FrameLowering.cpp:4116).
    pub pops_arguments_at_size: bool,
    /// The target has a libc `memcpy` to call, so LoopIdiomRecognize forms a
    /// copy of run-time length whatever its size, as the pin does. false:
    /// DEVIATE (loopidioms.rs `variable_copy_pays`): the copy stays a loop
    /// where the inline `rep movs` that replaces it is more bytes.
    pub libc_memcpy: bool,
    /// DEVIATE SD446: a divide by a constant is chosen against the multiply-high
    /// sequence by clocks plus bytes at `speed_milliclocks_per_byte`
    /// (`hardware_divide_wins`); false: the clocks and margin alone.
    pub divide_by_bytes: bool,
    /// DEVIATE DIV-LIVE (owner x86-m32, exit 2026-11-24): at -Os a divide by a
    /// constant of a type narrower than a word keeps its `div` (DIV-OPTSIZE-1)
    /// only while at most this many values are live across it; 0: always. The
    /// `div` pins eax, edx and the divisor's register, the narrow multiply
    /// sequence none.
    pub narrow_divide_live_limit: u32,
    /// The widest divide (in bits) `narrow_divide_live_limit` governs: 31 is
    /// "narrower than a word" on a 32-bit target; a 16-bit target whose `div
    /// edx:eax` pins the same registers at 32 bits sets 32.
    pub narrow_divide_live_max_bits: u32,
    /// Whether `narrow_divide_live_limit` also holds at -Os (false: -O1 only).
    pub narrow_divide_live_at_size: bool,
    /// DEVIATE SINK-EXT (owner x86-m32, exit 2026-11-24): at size, MachineSink
    /// leaves an instruction where it is when moving it would replace its one
    /// result by more operands that die at it, across a later instruction that
    /// clobbers registers (more values spilled). false: the pin's sink.
    pub sink_spares_ranges: bool,
    /// DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27): the target's
    /// `va_list` is one pointer, so `llvm.va_start` of it is a store of the
    /// first variadic argument's address (`llrm.vararg.address`) the mid-end
    /// can keep in a register. false: the pin's `va_start`, which leaves the
    /// list in memory.
    pub promote_va_list: bool,
    /// DEVIATE SETCC-TREE-BRANCH (owner x86-m32, exit 2026-12-05): at -Os a
    /// sum `x + zext(c)` whose `c` is an `and`/`or` of two or more word
    /// compares is a branch around `x + 1` (the byte price of the `setcc`
    /// chain decides), where the pin keeps the `setcc`/`or`/`movzx` chain.
    /// false: the pin's lowering.
    pub branch_compare_trees_at_size: bool,
    /// At -O1 (`OptLevel` 1) a full unroll is made only where its cost after
    /// the folding the analysis simulates does not exceed the rolled loop's
    /// size (gcc's size-neutral complete unroll); false: the pin's thresholds.
    pub unroll_never_grows_at_o1: bool,
    /// DEVIATE UNROLL-O1-TRIAL (owner x86-m32-c, exit 2026-12-05): the unrolled size, in
    /// the unroll units, up to which a loop the -O1 rule above refused is unrolled on
    /// trial; the driver compiles the module again and keeps the unroll only if neither
    /// its bytes nor its executed work grow (`Tti::unroll_trial_threshold`). `None`: no
    /// trial.
    pub unroll_trial_at_o1: Option<u32>,
    /// Whether that trial takes only a step of a nest of known trips (a lone loop the -O1 rule refused
    /// did not pay on x86-m32: 1 of 46 programs). False: any refused loop under the threshold, which
    /// is what the 6502 and m68k trials were fit on (float_in_place_update: 9441 B at -O1 against 19 B).
    pub unroll_trial_nests_only: bool,
    /// DEVIATE UNROLL-OS-TRIAL (owner x86-m32, exit 2026-11-24): the unrolled size, in the unroll
    /// units, up to which a loop the -Os threshold refused is unrolled on trial; kept where the
    /// finished object is smaller, or grows by a few bytes a function and runs less
    /// (`Tti::unroll_trial_at_size`). `None`: no trial.
    pub unroll_trial_at_size: Option<u32>,
    /// DEVIATE IPRA-DSO-LOCAL (owner llrm-target-x86-m64, exit 2026-11-24): a function that binds
    /// locally (external linkage, hidden or protected, or default visibility in code that is not
    /// shared-library code) takes part in the callee-written-registers facts, as GCC's `-fipa-ra`
    /// uses every definition no other definition can replace; the pin's `RegUsageInfoCollector`
    /// takes local linkage only (`isSafeForNoCSROpt`) and is off by default.
    pub ipra_binds_local: bool,
    /// DEVIATE UNROLL-OS-TRIAL-STRICT (owner llrm-target-x86-m64, exit 2026-11-24): the -Os unroll trial is kept
    /// only where the object is smaller, never for a few bytes a function that the code runs less
    /// (`trial::keeps_unroll_at_size`): bytes are the one strict axis at -Os. False: m32's rule.
    pub unroll_trial_bytes_strict: bool,
    /// Whether the -Os inliner counts the saving of a callee that goes with its last call in
    /// instruction units, as the pin does. False (D13): the saving is not taken, because a copy
    /// carries more bytes than its units where an argument is several words (x86-m16:
    /// QCport `pl_box_axis`, 21 units, ~70 B a copy, three copies +181 B).
    pub inline_dying_credit_at_os: bool,
    /// `Tti::inline_trial_keeps_shrinking_copy`: a priced inline trial keeps a copy that
    /// leaves the module no larger and saves clocks, though no callee goes with it.
    pub inline_trial_keeps_shrinking_copy: bool,
    /// DEVIATE INL-O1 SIZE (owner m32-o1-small, exit 2026-11-14): the inliner
    /// counts a callee in bytes of encoded code by instruction kind
    /// (`Tti::inline_size_view`), not in the pin's units of one per
    /// instruction. false: the pin's units.
    pub inline_size_bytes: bool,
    /// DEVIATE INL-O1 (owner x86-m32, exit 2026-11-24): where no byte rate is
    /// stated (-O1), an inline that stays is held to the -Os bound (the callee
    /// copied only if it does not grow the code); false: the pin's threshold.
    pub inline_never_grows_without_rate: bool,
    /// DEVIATE OB1: see `Tti::unroll_object_bound`.
    pub unroll_object_bound: Option<u32>,
    /// With `inline_never_grows_without_rate`: what a byte of growth costs an
    /// -O1 inline, in thousandths of a clock, against the call's latency at
    /// the site's frequency. m32: half the -O2 rate (-O1 weighs code size).
    pub inline_o1_milliclocks_per_byte: i64,
    /// Thousandths of a byte one unit of the inliner's code size is: the
    /// units count instructions (one each, a call 3 to 5), the bytes a callee
    /// takes are 3.5 to 5 times that on x86 (22 callees of the m32 O1 corpus:
    /// median 3.99 bytes per unit). Scales the growth the -O1 bytes-vs-clocks
    /// check compares with its per-byte rate; 1000: one byte a unit.
    /// DEVIATE INL-O1 unit scale (owner x86-m32, exit 2026-12-05). Stated cost,
    /// exit 2026-11-15 (strict -O1 clocks, not waived): x_sf_cmp64 81196 ->
    /// 93965 clk (best gcc/clang 90307), x_sf_div 146540 -> 152986 (149453),
    /// x_sf_mul 82587 -> 88263 (88107). Recovery: weigh callee size against
    /// the site's frequency and loop depth.
    pub inline_o1_unit_bytes_milli: i64,
    /// DEVIATE INL-O2 RATE (owner x86-m32, exit 2026-11-22): what a byte of
    /// growth is worth, in thousandths of a clock, to a speed caller's inline
    /// (`Tti::speed_inline_milliclocks_per_byte`; the pin has no such rule).
    /// 0: the pin's threshold alone.
    pub speed_inline_milliclocks_per_byte: i64,
    /// `Tti::speed_inline_rate_at_o1`: whether that rate binds at -O1 too
    /// (false by default; m32's -O1 has its own rule, INL-O1).
    pub speed_inline_rate_at_o1: bool,
    /// `Tti::callee_saved_clocks`: (scratch registers, preserved registers,
    /// clocks to save and restore one). None: not priced.
    pub callee_saved: Option<(i64, i64, i64)>,
    /// `Tti::inline_hot_trial_frequency`.
    pub inline_hot_trial_frequency: Option<i64>,
    /// `Tti::inline_hot_trial_at_o3`.
    pub inline_hot_trial_at_o3: bool,
    /// DEVIATE SD-DIVMAG (owner x86-m32, exit 2026-11-22): a signed divide by a
    /// negative constant is the negation of the divide by its magnitude, so it
    /// shares the multiply-high of the remainder (`x % -c` is `x % c`); the
    /// pin multiplies twice (div_constants_long -O2 +4.1% clk over gcc).
    /// false: the pin's `BuildSDIV`.
    pub sdiv_by_magnitude: bool,
    /// What one call costs in clocks when the inliner prices its latency
    /// (`Tti::call_sequence_cost`): the call and return, each argument word
    /// pushed, and the caller's pop of them. The pin prices the call as one
    /// instruction (4 on x86), which at 486 prices (call 3, ret 5, a push and
    /// a pop each) is a third of the sequence. 0 fixed: the pin's flat price.
    pub call_sequence_fixed_clocks: i64,
    pub call_sequence_argument_clocks: i64,
    pub call_sequence_pop_clocks: i64,
    /// DEVIATE INL-O1 ENTRY (owner x86-m32, exit 2026-11-15): the -O1
    /// bytes-vs-clocks check weighs a site's frequency by how often its
    /// function runs (the calls to it up the call graph), not by the
    /// function's entry alone: the bottom-up walk prices a helper in the
    /// function that calls it, which a hot loop up the graph runs 30 times
    /// more. false: the site's own frequency. Costs at m32 -O1 (A/B
    /// e09ff5f2a4^..e09ff5f2a4): x_jpeg_fixed +2.2% clk for -33% B (8415 ->
    /// 5647), x_sf_mul +22 clk (0.03%) over the best external. Closes: jpeg
    /// re-run against the best external (not yet measured); mul by the
    /// frequency of the call's own loop depth, or the row stays to the exit.
    pub inline_o1_weighs_entry: bool,
    /// Trips the -O1 no-growth rule above analyses for folds: the pin's
    /// `-unroll-max-iteration-count-to-analyze` (10) when `None`.
    pub unroll_o1_analyzed_trips: Option<u32>,
    /// IPSCCP runs a second time after ArgumentPromotion changed a body (the
    /// pin runs it once; DEVIATE SD443, interprocedural.rs).
    pub second_ipsccp: bool,
    /// A multiply by a constant is decomposed only where the chain's saved
    /// clocks pay for its extra bytes at `speed_milliclocks_per_byte`, the
    /// pin's own decompositions included (a tie keeps the `imul`); false:
    /// the pin's `combineMul` decomposes unconditionally.
    pub price_pin_multiplies: bool,
    /// DEVIATE LFTR1 (owner w12, review 2026-12-20): `FindLoopCounter` takes a
    /// counter with any constant step that cannot wrap, and a limit that costs
    /// one operation at most for it, not the pin's step of one
    /// (`isLoopCounter`, `m_scev_One`). x86 only: a step of two or `--i` kept
    /// its `ult`/signed test for the pin's LSR, and removing the rewrite is
    /// worse on every x86 width (m64 jpeg_single +16 B, m16 jpeg +4 B x8,
    /// m32 heapsort +1.8% clk, ld_horner +2.7%; 0 cells better at m64/m16,
    /// 2026-11-09). false: the pin's.
    pub lftr_any_stride: bool,
    /// At -Os a multiply by a constant stays an `imul` (the pin's `hasMinSize`
    /// rule, which the pin applies at -Oz only): the chain is no smaller.
    /// A constant `memset` past the store limit is `rep stos` at `-Os`
    /// (alignment or not), not the library call: the setup is 9-11 B against
    /// the call's 12-16. DEVIATE REP-OS (owner w9, exit 2026-11-20): the pin
    /// calls `memset` unless the destination is dword aligned and the size
    /// within 128 bytes.
    pub rep_stos_at_size: bool,
    /// A constant `memcpy` is `rep movsb` at `-Os` (any alignment, any size),
    /// not the library call: the setup is a few bytes against the call's push
    /// and pop. DEVIATE REP-MOVS-OS (owner x86-m32, exit 2026-11-30): the pin
    /// does this at `-Oz` only (`EmitTargetCodeForMemcpy`, `hasMinSize`).
    pub rep_movs_at_size: bool,
    /// A signed divide by a power of two stays an `idiv` at `-Os` (and `-Oz`, or a `minsize` function, always).
    /// The pin expands at `-Os` (`isIntDivCheap`, [X86ISelLowering.cpp#L65389]: x86 answers yes at `minsize` only; clang
    /// `-Os` does not set `minsize`), so no target sets this. m32 did from #8672 to 2026-11-11: stft_fixed `-Os` 96.8M ->
    /// 188.9M clk (+95%) and jpeg_fixed +64% against main, for -127 B and -790 B; `-Os` is strict per axis.
    ///
    /// [X86ISelLowering.cpp#L65389]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L65389
    pub sdiv_pow2_stays_at_size: bool,
    /// Functions with the same body become one at `-Os` (`MergeFunctions`, `llrm_transforms::mergefunc`): a local copy is
    /// dropped, an exported one is a `jmp` to the other. DEVIATE MERGEFUNC-OS (owner x86-m32, exit 2026-11-30): the pin's
    /// default pipeline does not run the pass (`-enable-merge-functions` is off), gcc folds them at `-Os` (`-fipa-icf`).
    pub merge_functions_at_size: bool,
    pub imul_at_size: bool,
    /// DEVIATE MUL-HOT (owner x86-m32, exit 2026-11-24): at `-Os` a multiply by a constant in a block that runs
    /// more than once per run of the program (its frequency over its function's entry times how often the
    /// function's direct callers run it) is decomposed as the pin does at `-Os` (`Tti::mul_chains_hot_at_size`); the
    /// `imul` stays in the blocks that run once.
    pub mul_chains_hot_at_size: bool,
    /// `UnrollingPreferences::Threshold` and `MaxPercentThresholdBoost` of the
    /// full-unroll pass; none: the pass's own, by optimization level.
    pub unroll_threshold: Option<u32>,
    pub unroll_max_percent_threshold_boost: Option<u32>,
    /// `UnrollingPreferences::PartialThreshold`: what a partly unrolled loop
    /// (the runtime unroll, the unroll-and-jam outer count) may total; none:
    /// the pass's own, 150.
    pub unroll_partial_threshold: Option<u32>,
    /// The `PartialThreshold` the unroll-and-jam (D1) counts the outer loop
    /// against; none: `unroll_partial_threshold`'s. The outer loop holds the
    /// inner one, which stays rolled, so a loop-buffer sized threshold
    /// refuses every nest.
    pub unroll_jam_partial_threshold: Option<u32>,
    /// The unroll-and-jam (D1) takes, of a known outer trip count, the
    /// largest count that divides it (2 or more) rather than one that leaves
    /// a remainder: the remainder is a copy of the whole nest.
    pub unroll_jam_exact_count: bool,
    /// `ConstantHoisting` runs after LSR, as the pin's `addIRPasses` does for
    /// every target (CH-STAGED: enabled target by target, each with its A/B).
    pub constant_hoisting: bool,
    /// DEVIATE ZIMM (owner m64, exit 2026-11-27): a 64-bit constant of 32
    /// leading zeros costs one instruction to materialize (`mov r32, imm32`
    /// zero-extends), not the pin's two (`getIntImmCost(int64_t)` takes
    /// `isInt<32>` only). false: the pin's.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6882>
    pub zext_imm_is_basic: bool,
    /// How many values the x87 register stack holds (8) where floats live
    /// there (a target without SSE2 floating point); none: floats are not
    /// held on a stack. The loop unroll caps its copies by it.
    pub x87_stack_slots: Option<u32>,
    /// A float product of a value by a constant, where the same value times
    /// the negated constant is made too, is made once: `a + v*(-C)` is
    /// `a - v*C`, exact in IEEE (DEVIATE X87-SHARE, x87 targets only).
    pub x87_share_negated_products: bool,
    /// At `-Os` a zero fill of eight bytes or more is `fldz` and one `fst
    /// qword` per double, not two `mov dword` zero stores: 4 bytes a double
    /// at a short displacement against 8 (DEVIATE X87-ZERO-FILL, x87 targets
    /// only; the pin's `getOptimalMemOpType` is `i32` without SSE).
    pub x87_zero_fill_at_size: bool,
    /// A stack slot a loop holds in a register is shared only by what is as
    /// wide (`slots::width_rule_allows`); false: the pin's `ColorSlot`,
    /// which shares any two slots whose ranges do not overlap.
    pub slots_loop_width_rule: bool,
    /// DEVIATE CARRY-SI (x86 m16, m32, m64: owners the three x86 sessions, exit
    /// 2026-11-21): the coalescer's slot indexes reach the allocator through the
    /// pre-RA scheduler with the entries its erased and moved instructions left
    /// empty (the pin's one `SlotIndexes`), so range sizes, and the dequeue and
    /// spill weights that divide by them, are the pin's. false: the function
    /// is numbered afresh after the scheduler, as the x86 numbers were tuned
    /// (A/B 2026-11-07: x86_32 O1 14 cells +704 B worse).
    pub carry_slot_indexes: bool,
    /// DEVIATE RA-CLK (x86 m16; owner x86-m16, exit 2026-11-29): the spill
    /// weight of a value that is an address component (the base, index or
    /// selector of a memory operand) is this many thousandths of a reference
    /// alike; 1000: the pin's `getSpillWeight`, which weighs every reference
    /// alike. Spilled, such a value is reloaded into one of the few registers
    /// 16-bit addressing allows (bx, bp, si, di) before the access, where any
    /// other value is folded or moves through any register. Applies only where
    /// fewer registers address than hold a value. A/B 2026-11-09 vs 1000, 141
    /// bench cells at -O1/-O2/-Os, outputs identical: -Os +235 B, -O2 +80 B
    /// (12 worse, 5 better), -O1 as below (strict on both axes). Weights 3000,
    /// 4000 and 6000 give the same code on the 14-program sample.
    /// -O1 cells (bytes, clocks; 3000 -> 1000): w1000 better 8: sfm16_float
    /// 2248/176044 -> 2236/174044, sfm16_float_{add,div,f2i,i2f,mul,sub}
    /// 1893/16016 -> 1888/16016, 1905/44772 -> 1900/44772, 1920/17338 ->
    /// 1915/17338, 1868/11229 -> 1863/11229, 1898/19595 -> 1893/19595,
    /// 1898/17195 -> 1893/17195, stft_fixed 1074/48760 -> 1064/48760; worse 7:
    /// jpeg_fixed 2967/74028620 -> 2973/74062036, jpeg_single 2817/86293387
    /// -> 2823/86326803, matmul 293/23467 -> 299/23531, recchop 177/11429 ->
    /// 183/11556, sfm16_float_cmp 1993/44039 -> 1994/45240, shellsort
    /// 172/11672 -> 176/11993, stft_single 842/49421 -> 844/49421;
    /// trades 2: grep 497/49952 -> 523/49865, quicksort 252/319777 ->
    /// 261/303228. Closed Os row M16-OS-MATMUL-DIVISOR: matmul -Os 289 B/23807
    /// clk against 284/23679 at 1000 is this weight's price, not a defect.
    /// The heavier address component evicts v91 from ECX, adding 3 stack
    /// moves (+5 B at -Os, -6 B at -O1); dists cc 381b0a8398f2, llc
    /// 16601539e87f.
    pub address_reference_milliweight: u32,
    /// DEVIATE RU-REC (owner x86-m64, exit 2026-11-20): a loop whose loop-carried
    /// latency exceeds its issue-bound time is not unrolled at run time
    /// (`loopunrollpass::recurrence`); the pin's `UP.Runtime` asks nothing of
    /// the loop. false: the pin's.
    pub unroll_recurrence_veto: bool,
    /// `-unswitch-threshold` of the non-trivial loop unswitch (-O3): the cost
    /// (loop size times copies) a candidate must stay under; none: 50.
    pub unswitch_threshold: Option<i64>,
    /// `InlineParams::DefaultThreshold`, the inliner's bar for a caller not
    /// optimized for size; none: 225, or 250 at -O3.
    pub inline_threshold: Option<i64>,
    /// DEVIATE BFS-1 (owner x86-m16-b, exit 2026-11-25): the inliner's
    /// cold-call-site test reads the site's frequency from a flow in which
    /// edges into `unreachable`-ended failure blocks carry no mass, where the
    /// block has another successor. The pin reads the full flow, in which
    /// checks (`!prof` 2^-20 failure edges) in deep loop nests leak most of
    /// the mass and make the code after them cold (threshold 45, no bonuses).
    /// false: the pin's.
    pub cold_site_ignores_sinks: bool,
    /// `getMinimumJumpTableDensity`, in percent, at every level; none: the
    /// pin's 10 (40 when tuned for size).
    pub min_jump_table_density: Option<u64>,
    /// Two compares joined by `and`/`or` stay two branches at every level, as
    /// the pin does at optsize; false: the pin's cost rule decides.
    pub split_merged_conditions: bool,
    /// DEVIATE SD103-M32 (owner x87, exit 2026-11-24): clocks a merged `a && b`
    /// costs over two branches where the pin's latency rule does not see them: a
    /// `setcc` per compare, the logic op and a `test`, less the branch the
    /// merge saves. `getJumpConditionMergingParams`' base cost is lowered by
    /// it, except where the DAG folds the pair into one compare
    /// (`foldLogicOfSetCCs`). 0: the pin's budget.
    pub merged_condition_clocks: i64,
    /// DEVIATE LSR-OUTER (owner x87, exit 2026-11-27): clocks of a multiply
    /// the CPU executes (`imul`); with `loop_carried_clocks` it prices reducing
    /// an outer loop's multiply to a counter. 0: the pin (no outer-loop
    /// reduction).
    pub multiply_clocks: i64,
    /// Clocks a value held across an inner loop costs each outer trip: a store
    /// and a load.
    pub loop_carried_clocks: i64,
    /// DEVIATE IMM-1 (owner greedy-os-causes, exit 2026-11-20): at size an
    /// `add`/`sub`/`and`/`or`/`xor` whose immediate has several users keeps
    /// the table's immediate form, where the pin's `X86DAGToDAGISel::Select`
    /// loads the constant once and takes the register form. The pin's form is
    /// larger where the immediate encodes short (m64 `-Os`: `and r64, imm`
    /// of a 32-bit mask, `movzx` masks). false: the pin's.
    pub immediate_forms_at_size: bool,
    /// The inliner's D11 bound holds a caller optimized for size on this target
    /// (the real-mode description's `inline_size_guard` is the same bound
    /// at m16): a callee that stays is copied only if it does not grow the
    /// code, summed over its sites. false: the pin's `OptSizeThreshold` 50
    /// alone decides, which allows the growth.
    pub inline_size_guard_at_size: bool,
    /// With `inline_size_guard_at_size`, the bound holds a callee with one
    /// site too (otherwise only two or more: the pin's single-site inline
    /// stands). A callee that stays and is copied once only grows the code.
    pub inline_size_guard_single_site: bool,
    /// An -Os caller's SZ2 trial also inlines the sites its splice brings, priced by the
    /// ordinary rules, before it measures: `foo` into `main` and then `bar` into that copy
    /// meet the constants together, as the pin's callee-first order gives. One round.
    pub inline_chain_trial: bool,
    /// A full unroll's discount for a local array it makes constant-indexed
    /// (promoted to registers) holds only while the promoted elements fit the
    /// register file (UP1). false: every such access is free.
    pub unroll_promotion_fits_registers: bool,
    /// The signed width of the immediate a store of an integer takes (x86-64: 32, sign-extended to the
    /// store's width); 0: the target has no store-immediate form. A full unroll's constants that fit it
    /// are instruction bytes, not live values (`Tti::unroll_store_immediate_bits`).
    pub unroll_store_immediate_bits: u32,
    /// DEVIATE LSR-SIZE (owner x86-m32, exit 2026-11-24): see
    /// `Tti::lsr_size_model` (`LsrSizeModel::X86`).
    pub lsr_size_order: bool,
    /// With `lsr_size_order`: price the code in x86-64's encodings (REX.W,
    /// `llrm_x86::encoding`) and the exit test's compare.
    pub lsr_size_x86_64: bool,
    /// DEVIATE IU1 (owner llrm-target-x86-m64, exit 2026-11-24): see
    /// `Tti::lsr_users_in_use_list_order`.
    pub lsr_users_in_use_list_order: bool,
    /// DEVIATE LSR-PHI (owner x87, exit 2026-11-24): see
    /// `Tti::lsr_phi_operand_uses`.
    pub lsr_phi_operand_uses: bool,
    /// UP1 for the floats of an x87 target: the register file a promoted
    /// float array's elements must fit is the x87 stack (`x87_stack_slots`,
    /// 8), which holds nothing across the nest's inner loops. Ints keep the
    /// rule above. false: floats are free (DEVIATE UP1-X87, owner x87).
    pub unroll_promotion_fits_x87_stack: bool,
    /// A size build rotates a loop only when the header's exit test folds in
    /// the preheader (RS1): an unfolded copy is a compare and a branch, more
    /// than the loop-closing jump it removes. false: the pass's header limit
    /// alone.
    pub rotation_guard_must_fold_at_size: bool,
    /// DEVIATE ROTATION-TRIAL: see `Tti::rotation_trial_at_size`.
    pub rotation_trial_at_size: bool,
    /// DEVIATE ROTATION-HOT: see `Tti::rotation_hot_at_size`.
    pub rotation_hot_at_size: bool,
    /// `Tti::rotation_hot_bytes_per_loop`.
    pub rotation_hot_bytes_per_loop: i64,
    /// DEVIATE GVN-PRE-OPTSIZE: see `Tti::load_pre_off_at_size`.
    pub load_pre_off_at_size: bool,
    /// A size build rotates a loop only when the copied header is no bigger
    /// than the branches the rotation removes (the loop-closing jump and
    /// the exit test), in `CodeSize` (arm64's w15 rule). false: the header
    /// limit alone.
    pub rotation_header_priced_at_size: bool,
    /// Header instructions a size build's loop rotation may copy
    /// (`RotationMaxHeaderSize`), where the machine states none itself
    /// ([`Machine::size_rotation_header_limit`] wins). None: the pin's 16.
    pub size_rotation_header_limit: Option<u32>,
    /// A constant offset of an address is a displacement the access folds;
    /// SimplifyCFG's sinking of two such accesses behind a phi of the offsets
    /// is refused (it costs the displacement). false: the pin's.
    pub keep_constant_displacements: bool,
    /// DEVIATE LU30: a static alloca is the frame register and a
    /// displacement, not a register: LSR leaves it out of a formula's
    /// registers where an address takes `[frame + index + displacement]`
    /// (x86-m16: `[bp+si+disp]`). false: the pin's.
    pub frame_slot_is_free_base: bool,
    /// DEVIATE LU31, see `Tti::lsr_address_fewest_registers`.
    pub lsr_address_fewest_registers: bool,
    /// DEVIATE SINK-CONST-PHI: a constant operand of an arithmetic instruction
    /// is not made a phi by sinking (the pin: cheap), except at size.
    pub keep_constant_arith_operands: bool,
    /// DEVIATE REC-2: (depth, callee `gcc_insns`) of the recursion copied at
    /// speed, gcc's `max-inline-recursive-depth` and
    /// `max-inline-insns-recursive` (`Tti::recursive_inlining`); none: the
    /// pin's inliner.
    pub recursive_inlining: Option<(u32, i64)>,
    /// gcc's `max-inline-insns-auto` at -O2 and -O3 (15, 30 of its size
    /// units) as the most `gcc_insns` a recursive callee may have for
    /// `recursive_inlining` to run on it, calibrated per target; none: no
    /// such bar.
    pub recursive_edge_growth: Option<(i64, i64)>,
    /// DEVIATE REC-2: what the copies of a recursive function in itself may add
    /// at (-O2, -O3), in `gcc_insns` (gcc's `max-inline-insns-recursive`, 450);
    /// none: `inline::MAX_INLINE_INSNS_RECURSIVE`.
    pub recursive_inlining_growth: Option<(i64, i64)>,
    /// DEVIATE REC-7: the copies' depth at (-O2, -O3), where the target wants
    /// a depth apart from `recursive_inlining`'s; none: that one's.
    pub recursive_inlining_depth: Option<(u32, u32)>,
    /// DEVIATE REC-2: at (-O2, -O3) a tree recursion's tail calls are
    /// eliminated before it is copied into itself (gcc:
    /// `pass_tail_recursion` runs early, `passes.def` before
    /// `pass_ipa_inline`), not after (D10).
    pub recursive_tre_first: (bool, bool),
    /// DEVIATE IPACP-O3 (owner x86-m32-c, exit 2026-12-20): ipa-cp clones at
    /// -O3, as gcc's `-fipa-cp-clone` is on there; the pin makes none
    /// (`Tti::ipa_cp_clones_at_speed`). DEVIATE IPACP-O2 (owner w10, exit
    /// 2026-11-24): and at -O2, where gcc has none: queens m32 151807 ->
    /// 139804 clk for +293 B, m64 -9.4% instructions for +830 B.
    pub ipa_cp_clones_at_speed: bool,
    /// DEVIATE PL6: the first iteration of an innermost loop is peeled where
    /// its entry constants fold enough of the body to pay the copy at the
    /// speed rate (`Tti::peels_folded_entry`). false: the pin's
    /// `computePeelCount`.
    pub peel_folded_entry: bool,
    /// DEVIATE UP1: a full unroll the pin's threshold allows is made only
    /// where the clocks it saves repay the bytes it adds at this rate in
    /// thousandths of a clock a byte (`Tti::unroll_milliclocks_per_byte`) at
    /// -O2/-O3. 0: the pin's threshold alone.
    pub unroll_milliclocks_per_byte: i64,
    /// DEVIATE UP1-NEST (`Tti::unroll_prices_nest_loops_without_outer_counter`).
    pub unroll_prices_nest: bool,
    /// DEVIATE PHICONST-2 (`Tti::folds_phi_constant_chains`).
    pub phi_constant_chains: bool,
    /// DEVIATE CS2: where there is no conditional move, a select's arm only
    /// it reads is made on its own path (`Tti::sinks_select_arms`). false: the
    /// pin's, both operands before the branch.
    pub sink_select_arms: bool,
    /// DEVIATE SELOPT-C (`selectoptimize::SelectOptimizePass`): SelectOptimize runs
    /// at -O2/-O3 with constant-arm selects included. false: the pin's, which
    /// runs it nowhere.
    pub branch_constant_arm_selects: bool,
    /// DEVIATE IND-PREFIX (`Tti::narrow_counter_is_taxed`): 16-bit counters
    /// are widened like illegal ones, for the operand-size prefix.
    pub prefixed_16_bit_counters: bool,
    /// DEVIATE PIL-OS (owner alim, exit 2026-11-27): PartiallyInlineLibCalls
    /// leaves a call in a function tuned for size (`optsize`, `minsize`)
    /// alone. false: the pin's, which splits it there too and grows the code
    /// by a compare, a branch and a copy of the call.
    pub partial_libcalls_skip_at_size: bool,
    /// DEVIATE MEM-1: an `llvm.memcpy` of a length known only at run time, in
    /// a function not tuned for size, is `rep movsd` and `rep movsb` inline,
    /// not a call of `memcpy` (gcc's `rep_prefix_4_byte` stringop strategy for
    /// the 486 and P5). false: the pin's `EmitTargetCodeForMemcpy`, a libcall.
    /// Linux x86-64 stays false: glibc's `memcpy` of 0-32 bytes is 6.4 TSC
    /// ticks a call, `rep movsb` 8.2 (Ryzen 9 9950X3D, fsrm; 2026-11-05).
    pub inline_variable_memcpy: bool,
    /// DEVIATE MEM-OS (owner anyisa, exit 2026-11-14), the neighbour of SD541: tuned for size, a constant
    /// `memcpy`/`memmove` whose load and store pieces take more bytes than the call of the library
    /// routine (`mov edx, n` and the call) is the call, at the target's `CopyPrice`. The pin counts pieces
    /// (`MaxStoresPerMemcpyOptSize`). gcc -Os calls from 32 B (mv64 10 B against the pin's 31).
    /// None: the pin's pieces.
    pub size_copy_price: Option<CopyPrice>,
    /// DEVIATE REC5-1 (owner w2, exit 2026-11-20): IPSCCP widens a range that
    /// keeps growing to its half-line, keeping the sign, not to overdefined
    /// (the pin). recpow -O2: `i % 97` of a loop counter was a signed divide
    /// all the way into the callee.
    pub ipsccp_widen_keeps_sign: bool,
    /// REC5-1 also at -Os. m32 leaves it off (bytes-strict): the unsigned
    /// remainder is `xor edx, edx; div`, a byte over `cdq; idiv` (recgcd,
    /// frames, recmany, switch: 2..5 B).
    pub ipsccp_widen_keeps_sign_at_size: bool,
}

/// Encoded bytes of a constant fill, stores against `rep stos`. Each pair or
/// triple is [frame object, global]: a short displacement against an absolute
/// address.
/// The bytes a constant copy tuned for size is priced in (`Tuning::size_copy_price`): the library call
/// (`mov edx, n` and the call) against the load and store pieces, a wide one (16 or 8 bytes) or a narrower one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyPrice {
    pub call: u64,
    pub wide_piece: u64,
    pub narrow_piece: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeFill {
    /// A dword, word and byte store of an immediate.
    pub store: [[u64; 3]; 2],
    /// The value in `eax` for zero, all ones and any other byte.
    pub value: [u64; 3],
    /// `lea`/`mov edi`, `mov ecx` and the `rep stos`.
    pub setup: [u64; 2],
    /// One `fst qword` of the x87 zero fill.
    pub fst: [u64; 2],
}

impl Tuning {
    /// X86ISelLowering.cpp: 16/8, 8/4, 8/4 stores; no clock price for a byte.
    pub const PIN: Tuning = Tuning {
        size_fill: None,
        max_stores_per_memset: 16,
        max_stores_per_memset_opt_size: 8,
        max_stores_per_memcpy: 8,
        max_stores_per_memcpy_opt_size: 4,
        max_stores_per_memmove: 8,
        max_stores_per_memmove_opt_size: 4,
        size_milliclocks_per_byte: 0,
        speed_milliclocks_per_byte: 0,
        price_fills: false,
        multiply_by_clocks: false,
        libc_memcpy: false,
        pops_arguments_at_size: true,
        divide_by_bytes: false,
        narrow_divide_live_limit: 0,
        narrow_divide_live_max_bits: 31,
        narrow_divide_live_at_size: true,
        sink_spares_ranges: false,
        promote_va_list: false,
        branch_compare_trees_at_size: false,
        unroll_never_grows_at_o1: false,
        unroll_trial_at_o1: None,
        unroll_trial_nests_only: false,
        unroll_trial_at_size: None,
        ipra_binds_local: false,
        unroll_trial_bytes_strict: false,
        inline_dying_credit_at_os: true,
        inline_trial_keeps_shrinking_copy: false,
        inline_never_grows_without_rate: false,
        unroll_object_bound: None,
        inline_size_bytes: false,
        inline_o1_milliclocks_per_byte: 0,
        inline_o1_unit_bytes_milli: 1000,
        speed_inline_milliclocks_per_byte: 0,
        sdiv_by_magnitude: false,
        speed_inline_rate_at_o1: false,
        inline_hot_trial_frequency: None,
        inline_hot_trial_at_o3: false,
        callee_saved: None,
        call_sequence_fixed_clocks: 0,
        call_sequence_argument_clocks: 0,
        call_sequence_pop_clocks: 0,
        inline_o1_weighs_entry: false,
        unroll_o1_analyzed_trips: None,
        second_ipsccp: false,
        price_pin_multiplies: false,
        lftr_any_stride: false,
        imul_at_size: false,
        mul_chains_hot_at_size: false,
        rep_stos_at_size: false,
        rep_movs_at_size: false,
        sdiv_pow2_stays_at_size: false,
        merge_functions_at_size: false,
        unroll_threshold: None,
        unroll_max_percent_threshold_boost: None,
        unroll_partial_threshold: None,
        unroll_jam_partial_threshold: None,
        unroll_jam_exact_count: false,
        constant_hoisting: false,
        zext_imm_is_basic: false,
        x87_stack_slots: None,
        x87_share_negated_products: false,
        x87_zero_fill_at_size: false,
        slots_loop_width_rule: false,
        carry_slot_indexes: true,
        address_reference_milliweight: 1000,
        unroll_recurrence_veto: false,
        unswitch_threshold: None,
        inline_threshold: None,
        cold_site_ignores_sinks: false,
        min_jump_table_density: None,
        split_merged_conditions: false,
        merged_condition_clocks: 0,
        multiply_clocks: 0,
        loop_carried_clocks: 0,
        immediate_forms_at_size: false,
        inline_size_guard_at_size: false,
        inline_size_guard_single_site: false,
        inline_chain_trial: false,
        unroll_promotion_fits_registers: false,
        unroll_store_immediate_bits: 0,
        lsr_size_order: false,
        lsr_size_x86_64: false,
        lsr_users_in_use_list_order: false,
        lsr_phi_operand_uses: false,
        unroll_promotion_fits_x87_stack: false,
        rotation_guard_must_fold_at_size: false,
        rotation_trial_at_size: false,
        rotation_hot_at_size: false,
        rotation_hot_bytes_per_loop: 4,
        load_pre_off_at_size: false,
        rotation_header_priced_at_size: false,
        size_rotation_header_limit: None,
        keep_constant_displacements: false,
        frame_slot_is_free_base: false,
        lsr_address_fewest_registers: false,
        keep_constant_arith_operands: false,
        recursive_inlining: None,
        recursive_edge_growth: None,
        recursive_inlining_growth: None,
        recursive_inlining_depth: None,
        recursive_tre_first: (false, false),
        ipa_cp_clones_at_speed: false,
        peel_folded_entry: false,
        unroll_milliclocks_per_byte: 0,
        unroll_prices_nest: false,
        phi_constant_chains: false,
        sink_select_arms: false,
        branch_constant_arm_selects: false,
        prefixed_16_bit_counters: false,
        partial_libcalls_skip_at_size: false,
        inline_variable_memcpy: false,
        size_copy_price: None,
        ipsccp_widen_keeps_sign: false,
        ipsccp_widen_keeps_sign_at_size: false,
    };
}

/// What a target says of `GlobalMerge`
/// ([GlobalMerge.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/GlobalMerge.cpp));
/// a target that merges nothing answers none.
#[derive(Clone, Copy, Debug)]
pub struct GlobalMerge {
    /// The reach of a base register's immediate: `MaxOffset`.
    pub max_offset: u64,
    /// `OnlyOptimizeForSize`: only uses in `minsize` functions count.
    pub size_only: bool,
    /// DEVIATE ARM64-GM-OPTSIZE (owner w15, exit 2026-11-30): with `size_only`,
    /// uses in `optsize` functions count too, not only `minsize` ones (the
    /// pin's `-Os` merges nothing; gcc's section anchors do: stft_fixed -Os
    /// 15 `adrp` against gcc's 5). False: the pin.
    pub optsize_counts: bool,
    /// The object format is Mach-O: the merged struct is `internal`, not
    /// `private`.
    pub mach_o: bool,
}

/// Where the target keeps no program data, as linear addresses: old
/// `abi::machine::Machine::foreign_span`. A real-mode target has some (its
/// video memory and ROM); any other none.
pub trait Machine {
    /// The linear bytes that `width`-byte accesses at `selectors` and
    /// `offsets` (unsigned words) reach, where foreign memory holds them all.
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)>;

    /// The address spaces by role: the numbers the target's description gives
    /// them.
    fn spaces(&self) -> Spaces;

    /// Whether the target's pipeline takes vector values. None does yet
    /// (vectors are deferred, not designed out): a function holding one is
    /// refused before the mid-end, which assumes scalars.
    fn has_vector_mir(&self) -> bool {
        false
    }

    /// The alignment this target's frames really keep; none: whatever the IR
    /// asks. An `alloca` of more (up to [`Machine::max_alloca_align`]) is
    /// clamped to it at pipeline entry, so the optimizer never assumes an
    /// alignment the frame does not give.
    fn kept_stack_align(&self) -> Option<u64> {
        None
    }

    /// An `alloca` aligned beyond this is kept at its alignment, the frame
    /// realigning the stack pointer for it, instead of clamped to
    /// [`Machine::kept_stack_align`]; none: every excess is clamped.
    fn realigned_above(&self) -> Option<u64> {
        None
    }

    /// The largest `alloca` alignment this target accepts; none: it realigns
    /// its stack. A larger one is a named refusal at pipeline entry.
    fn max_alloca_align(&self) -> Option<u64> {
        None
    }

    /// The target's throughput model, where it states one: `None` prices a loop
    /// by the sum of `costs()` (every target but x86-64).
    fn throughput(&self) -> Option<&Throughput> {
        None
    }

    /// Whether a dynamic `alloca` (a variable size, or outside the entry block)
    /// keeps the alignment it asks for: `DYNAMIC_STACKALLOC` masks the
    /// address, so it needs neither the clamp to [`Machine::kept_stack_align`]
    /// nor the refusal at [`Machine::max_alloca_align`], which are for the
    /// frame's own objects. False: clamped and refused like every other
    /// `alloca`.
    fn honors_dynamic_alloca_align(&self) -> bool {
        false
    }

    /// What each operation costs on this target, for profitability.
    fn costs(&self) -> OperationCosts;

    /// LLVM's per-instruction cost model of this target, for the passes that
    /// translate LLVM's (InlineCost, the SCEV expander, loop strength
    /// reduction); none: they price by the generic rules.
    fn tti(&self) -> Option<std::rc::Rc<dyn crate::tti::Tti>> {
        None
    }

    /// The library functions the target's runtime has and the widths of `int`
    /// and `size_t` (`TargetLibraryInfo`'s `initialize` of the triple); none:
    /// the pin's defaults for an unknown OS, `int` as wide as a pointer.
    fn library(&self) -> Option<std::rc::Rc<dyn crate::libfunc::Library>> {
        None
    }

    /// The target's `TargetLowering` facts, for the passes that translate
    /// code reading them; none: the base class's answers.
    fn lowering(&self) -> Option<std::rc::Rc<dyn crate::lowering::Lowering>> {
        None
    }

    /// Whether a loop that stores one word or dword to every cell of an object
    /// is a `memset.pattern` fill at -Os, which isel makes `rep stosw` or
    /// `rep stosd` (DEVIATE: the pin leaves it a loop).
    fn fills_by_pattern(&self) -> bool {
        false
    }

    /// `isOperationLegal(ISD::SHL, pointer)`: whether a shift of a pointer-wide value by a variable
    /// count is an instruction, not a call or a loop. False: a switch builds no bit tests
    /// (`1 << x` and a mask).
    fn variable_shift_legal(&self) -> bool {
        true
    }

    /// The target's tuning limits: the pin's values, with this target's
    /// overrides. A target with no entry here takes `Tuning::PIN`.
    fn tuning(&self) -> Tuning {
        Tuning::PIN
    }

    /// Clocks a byte of code is worth, in thousandths, where a decision is
    /// made for size: the smaller form is kept while the clocks it costs are
    /// under this per byte saved. None: bytes alone decide.
    fn size_milliclocks_per_byte(&self) -> i64 {
        self.tuning().size_milliclocks_per_byte
    }

    /// What each operation costs in code bytes, for a decision made for
    /// size; by default the same prices.
    fn size_costs(&self) -> OperationCosts {
        self.costs()
    }

    /// What each operation costs in real code bytes, for the decisions that
    /// weigh bytes against clocks at a rate (`Tti::*_milliclocks_per_byte`):
    /// unit prices there are bytes of a fixed-width target only if this says
    /// so. Not the pin's thresholds, which read instruction units. By default
    /// `size_costs`.
    fn byte_costs(&self) -> OperationCosts {
        self.size_costs()
    }

    /// The prices of the Os inline trial's byte estimate (`function_bytes`),
    /// which may be calibrated against the bytes written where `size_costs`
    /// is not. By default `size_costs`.
    fn trial_costs(&self) -> OperationCosts {
        self.size_costs()
    }

    /// Whether a division by a constant stays a divide instruction on
    /// `bits`-bit values, `size` saying the build is for size
    /// (`isIntDivCheap`, read the other way): the remainder is then the
    /// divide's own, not `x - (x / c) * c`. By default no: a division is a
    /// multiply.
    fn keeps_division(
        &self,
        bits: u32,
        size: bool,
    ) -> bool {
        let _ = (bits, size);
        false
    }

    /// How many integer values fit in registers at once.
    fn registers(&self) -> i64;

    /// Of `registers`, how many survive a call.
    fn call_registers(&self) -> i64;

    /// How many floating values fit in registers at once: the depth of a
    /// register stack, or a file of their own. None: floating values share
    /// `registers`, or the target does not say.
    fn float_registers(&self) -> i64 {
        0
    }

    /// Registers an access through a pointer wider than its offset takes
    /// for its selector, besides those its address names.
    fn far_access_registers(&self) -> i64 {
        0
    }

    /// Header instructions loop rotation may copy in a size build
    /// (`RotationMaxHeaderSize`): none stated, the pin's 16.
    fn size_rotation_header_limit(&self) -> Option<u32> {
        None
    }

    /// Segment registers a far pointer's selector can be held in, besides
    /// `registers`: none where the target has none.
    fn segment_registers(&self) -> i64 {
        0
    }

    /// Whether an arithmetic result is made in the register of its first
    /// operand (x86: `sub dst, src`), so a first operand that stays live is
    /// copied before the result is made.
    fn two_address(&self) -> bool {
        false
    }

    /// Whether a call followed by a return can be emitted as a tail call
    /// (LLVM's `TargetLowering::mayBeEmittedAsTailCall`), which is what makes
    /// `CodeGenPrepare` copy the return into the call's block. Not wasm.
    fn may_be_emitted_as_tail_call(&self) -> bool {
        true
    }

    /// Whether the target has several registers a comparison's result can
    /// be kept in (LLVM's `hasMultipleConditionRegisters`), so that one made
    /// in a block and read by a branch in another is held there. Not where
    /// the result is the flags, one register that anything arithmetic
    /// overwrites: x86.
    fn multiple_condition_registers(&self) -> bool {
        false
    }

    /// Of `registers`, how many an address's pointer and index may be held
    /// in, where only some can: none where any can.
    fn address_registers(&self) -> i64 {
        0
    }

    /// Of `registers`, how many survive a call to `callee`, named where the
    /// call is direct: its own contract may keep more than any call does.
    fn kept_across(
        &self,
        _callee: Option<&str>,
    ) -> i64 {
        self.call_registers()
    }

    /// The convention a function nothing outside the program reaches takes,
    /// where the target states one.
    fn private_convention(&self) -> Option<PrivateConvention> {
        None
    }

    /// The convention a function in `convention` takes when its callers'
    /// cleanup of the stack arguments moves to the function: C's own,
    /// `fastcc`, by default; a target whose description states more says which.
    fn callee_pop(
        &self,
        convention: u32,
    ) -> Option<u32> {
        (convention == 0).then_some(crate::opcode::FAST)
    }

    /// The bytes of stack the `arguments` take under `convention`, where the
    /// target knows; else every argument is on it.
    fn stack_argument_bytes(
        &self,
        _convention: u32,
        _arguments: &[Argument],
    ) -> Option<i64> {
        None
    }

    /// What multiplying by the constant `factor`, above one, costs: a
    /// multiply, or the shifts and adds the target makes it of.
    fn multiply_by(
        &self,
        _factor: i64,
    ) -> i64 {
        self.costs().multiply
    }

    /// The indexed addresses a memory access may use, native form first.
    fn address_forms(&self) -> Vec<AddressForm>;

    /// How `GlobalMerge` runs at `-O3` (`aggressive`) or below; none where
    /// the target does not run it.
    fn global_merge(
        &self,
        _aggressive: bool,
    ) -> Option<GlobalMerge> {
        None
    }

    /// Whether a `width`-byte load at an address a multiple of `align` may
    /// trap wherever it points. By default any may: only one known
    /// dereferenceable runs where the program would not have run it.
    fn load_may_trap(
        &self,
        _width: u64,
        _align: u64,
    ) -> bool {
        true
    }

    /// Whether the module is single-threaded, so every object is thread-local
    /// ([`Triple::getDefaultThreadModel`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/TargetParser/Triple.cpp#L2546)
    /// is `Single` for wasm, `POSIX` for every other triple here). LICM's
    /// `isThreadLocalObject` reads it.
    fn single_threaded(&self) -> bool {
        false
    }

    /// Where `Intrinsic::Window` makes a huge pointer far: the far pointers'
    /// space, and the bytes from the one it makes that no displacement
    /// carries out of. None where the target has no such window.
    fn huge_window(&self) -> Option<(u32, i64)> {
        None
    }

    /// The width in bits of the index an indexed addressing mode adds to a base
    /// register, where the target has one narrower than its pointers (the
    /// 6502's `lda (base),y`: 8); `None` for a target that adds a full offset.
    /// Loop passes that split a pointer into a base and an index read it.
    fn index_register_bits(&self) -> Option<u32> {
        None
    }

    /// Whether an I/O access to a port in the inclusive range `ports` may
    /// read or write memory. By default any may.
    fn port_touches_memory(
        &self,
        _ports: (i64, i64),
    ) -> bool {
        true
    }
}

/// What advancing a pair pointer in a carrying space costs, given the price
/// of its instructions: the offset widened, the displacement added, the
/// carry copied and shifted down and up by the stride, added to the
/// selector, and the offset and selector copied out.
pub const fn carry_cost(
    extend: i64,
    add: i64,
    shift: i64,
    r#move: i64,
) -> i64 {
    extend + 2 * add + 2 * shift + 3 * r#move
}

/// What advancing a pair pointer in a carrying space by a constant costs:
/// the offset's sum, its carry spread to a mask, the mask cut to the
/// selector's stride, and added to the selector.
pub const fn step_cost(add: i64) -> i64 {
    4 * add
}

/// How a core that overlaps instructions runs a loop of operations: the resources they occupy, and the latency
/// of each. `MachineTraceMetrics::Trace::getResourceLength` reads a trace's resource bound as the largest, over
/// the model's processor resources, of the cycles the instructions put on one over its units, the issue width
/// being one more resource (micro-ops over `IssueWidth`); `SelectOptimize::computeLoopCosts` chains the
/// latencies through a loop's dependences. The integer clock API of [`OperationCosts`] is unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Throughput {
    /// Latency of each operation, thousandths of a clock.
    pub latency: OperationCosts,
    pub resources: Vec<Resource>,
}

/// One processor resource: `costs` holds, for each operation, the thousandths of a cycle it occupies it, and
/// `units` how many of it the core has (the issue width, for the resource of micro-ops, whose `costs` count them).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub units: i64,
    pub costs: OperationCosts,
}

/// Machine-neutral costs a MIR profitability decision may compare.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationCosts {
    /// The target's native integer width in bits where an operation on a wider
    /// integer is several of the native one (an 8-bit machine adds an `i32` in
    /// four byte adds): the prices of `add`, `load`, `store`, `move` and
    /// `shift` are per native word and `profit::operation` multiplies them by
    /// [`OperationCosts::words`]. 0: a price is for the operation as the type
    /// has it, as on every target but the 6502.
    pub native_bits: i64,
    pub add: i64,
    pub multiply: i64,
    pub divide: i64,
    /// The same on a dword where the target's own word is smaller
    /// (`imul`/`idiv` r32 in real mode, before the operand-size prefix);
    /// equal to `multiply`/`divide` elsewhere.
    pub multiply_long: i64,
    pub divide_long: i64,
    /// An integer product wider than the target's word (an `i64` on a 32-bit CPU), where the CPU's own multiply is
    /// narrower: the CPU's whole sequence. 0: priced as `multiply_long`.
    pub multiply_wide: i64,
    /// The product of two values extended from the word (`i32` by `i32` into `i64`), the form a fixed-point multiply
    /// expands to. 0: priced as `multiply_wide`.
    pub multiply_widening: i64,
    /// Percent by which a constant divide's multiply-high sequence must cost
    /// more than the divide for the divide to stay (`isIntDivCheap`,
    /// priced); 0 where the target states none.
    pub divide_margin: i64,
    /// The multiply-high of a constant divide's sequence: the product's upper
    /// half, where the CPU's multiply ends early on a short multiplier (the
    /// 486's 13..42) and the multiplier is the dividend, whose value the
    /// selector does not know: priced at the minimum, as gcc's
    /// `mul_highpart_cost` prices a non-constant multiplier (`mult_init`).
    pub multiply_high: i64,
    pub shift: i64,
    pub address: i64,
    /// What a `lea` of dwords costs where a prefix widens it (real mode: the
    /// operand-size and address-size prefixes, `66 67 8D ModRM` and a SIB byte
    /// or a displacement); 0 where the target states none, and the price is
    /// then `address` plus the prefixes' prices.
    pub address_dword: i64,
    /// What a third part of that address (a base, an index and a
    /// displacement: the SIB byte and the displacement both) adds to
    /// `address_dword`; 0 where the target states none.
    pub address_third: i64,
    /// An address advanced in a space whose displacement carries into its
    /// selector (a huge pointer's): the offset's sum, its carry, and the
    /// selector stepped by it.
    pub carry: i64,
    /// The same advance by a constant (`step_cost`).
    pub carry_step: i64,
    pub load: i64,
    pub store: i64,
    /// A constant stored to memory: the register store, plus the immediate
    /// in code bytes.
    pub store_immediate: i64,
    pub memory_update: i64,
    pub branch: i64,
    pub prefix: i64,
    pub r#move: i64,
    pub call: i64,
    pub return_: i64,
    /// What one argument word costs around a call: pushed by the caller, read
    /// by the callee.
    pub argument: i64,
    /// What a caller pays to take `n` words of arguments off the stack: the
    /// cheaper of `n` pops (`pop` each) and one `adjust` of the stack
    /// pointer.
    pub pop: i64,
    pub adjust: i64,
    /// What a return popping its callee's arguments costs more than a plain
    /// one.
    pub return_pops: i64,
    pub float_add: i64,
    pub float_multiply: i64,
    pub float_divide: i64,
    pub float_load: i64,
    /// What a floating load costs more than the operation that reads it where
    /// the target folds it into that operation's memory form (x87 `fmul
    /// m32`): the load moved out of a loop saves this much each trip, not
    /// `float_load`.
    pub float_load_folded: i64,
    /// Whether a price counts a load its one reader takes from memory at
    /// `float_load_folded`, as the pass that moves loads asks (`hoist`); the
    /// description never states it, and the passes that price a body
    /// by its loads count each as an instruction.
    pub folds_loads: bool,
    pub float_store: i64,
    /// What dropping a floating value from the register stack costs once its
    /// last reader is past (x87 `fstp st(0)`): one held across a loop is
    /// released after it.
    pub float_release: i64,
    /// A float converted to an integer (`fistp` under the control word that
    /// truncates, and the word back); 0 where the target converts in one cheap
    /// instruction.
    pub float_to_int: i64,
    /// A float compare whose answer is in a register (x87 `fcom`, `fnstsw`,
    /// `sahf`); 0 where the target states none.
    pub float_compare: i64,
    pub extend: i64,
    /// A comparison held as a value (`setcc`), before it is extended.
    pub set: i64,
    pub fill: i64,
    pub fill_cell: i64,
    /// What of `fill` is the save and restore of a segment register around the
    /// string instruction; 0 where the fill needs none.
    pub fill_segment: i64,
    /// `Tuning::max_stores_per_memset` for these prices: a constant memset of
    /// at most that many stores is stores, not a fill; 0: the table does not
    /// say.
    pub memset_stores: i64,
    /// The code bytes of the routine a fill through a huge pointer is laid
    /// down as, in a table priced by bytes; 0 where there is none.
    pub huge_fill: i64,
    /// The same for a copy or move through huge pointers: the smaller of the
    /// upward and downward routines.
    pub huge_copy: i64,
    /// `rep movs` as a copy sets it up (ES, the two addresses, the count) and
    /// what each cell costs it; `direction` is `std` and `cld` around a
    /// backward one.
    pub copy: i64,
    pub copy_cell: i64,
    /// What a copy of a length known only at run time adds to `copy`: the
    /// count in dwords and the odd bytes after it, a second `rep movs`.
    pub copy_variable: i64,
    pub direction: i64,
    /// How many operations a completely unrolled loop may total before the work
    /// it saves is boosted (`peelsize::Limits`); 0: the target states none
    /// and the pass's own is used.
    pub unroll_budget: i64,
    /// gcc's by-pieces ratios for a constant memset (`SET_RATIO` for a
    /// nonzero byte, `CLEAR_RATIO` for zero; the `_size` ones when tuned for
    /// size): stores while their number is under it, else a fill. 0: the
    /// target states none for the CPU.
    pub pieces_set: i64,
    pub pieces_clear: i64,
    pub pieces_set_size: i64,
    pub pieces_clear_size: i64,
    /// Bytes of the widest store a constant memset expands to; 0: the
    /// target's pointer width, a dword at least.
    pub memset_store: i64,
    /// The fewest case values for which a switch is a jump table (gcc's
    /// `case_values_threshold`); 0: the target has no table jump.
    pub jump_table: i64,
    /// The bytes every function has beyond its operations (a callee-saved
    /// register saved and restored): added once to a function's size
    /// estimate; 0 where the table states none.
    pub function: i64,
    /// Percent a function's estimate (operations plus `function`) is scaled
    /// by for what the lowering adds that no operation names (spills, branch
    /// relaxation), measured against objects; 100: none.
    pub function_scale: i64,
    /// Whether `function_bytes` scales the prices by [`OperationCosts::words`] (an `i32` add on an 8-bit machine is
    /// four byte adds). False: one price per operation, the table the other byte decisions read.
    pub width_scaled: bool,
    /// Whether a GEP with only constant indices costs nothing: the displacement is part of the load or store that
    /// reads the address (`ldy #offset`, an absolute address plus a constant). False: priced as `address`.
    pub constant_offsets_fold: bool,
    /// Whether the branch is priced with its compare (the `branch` snippet is a compare and a jump, which `ICmp`
    /// already prices): a `br` costs nothing, as an unconditional jump is mostly a fall-through. False: `branch`
    /// each.
    pub branch_in_compare: bool,
    /// What a load from a global or a stack slot costs per native word (an absolute address, no pointer to
    /// form); 0: what any load does.
    pub direct_load: i64,
    /// The same for a store.
    pub direct_store: i64,
    /// Whether a call to an intrinsic the cost model calls free (`llvm.lifetime.*`, `llvm.assume`) is 0 bytes in a
    /// function's estimate. False: priced as a call, which is what the inlining bars of the targets fit before this
    /// field were calibrated on (x86 m16: a stack-record call stopped being inlined when it was 0).
    pub free_intrinsics: bool,
    /// Whether a fixed-point or saturating intrinsic (`llvm.{s,u}mul.fix[.sat]`, `llvm.{s,u}shl.sat`) is priced as the
    /// operations it expands to, on this target's table, in a function's estimate. False: priced as a call.
    pub expanded_intrinsics: bool,
    /// Bytes of one line of the CPU's data cache (its first level); 0: the
    /// target states none (no cache, or not known).
    pub cache_line: i64,
    /// Clocks to fill one such line from memory; 0: not stated.
    pub line_fill_clocks: i64,
    /// Block placement at -Os/-Oz is `MachineBlockPlacement`'s
    /// `-apply-ext-tsp-for-size` (Ext-TSP over unit counts), which the pin
    /// ships off.
    pub ext_tsp_for_size: bool,
}

impl OperationCosts {
    /// An integer product wider than the word: `multiply_wide`, else `multiply_long`.
    pub fn wide_product(&self) -> i64 {
        if self.multiply_wide > 0 { self.multiply_wide } else { self.multiply_long }
    }

    /// The product of two word-wide values in a double-wide result: `multiply_widening`, else [`Self::wide_product`].
    pub fn widening_product(&self) -> i64 {
        if self.multiply_widening > 0 { self.multiply_widening } else { self.wide_product() }
    }

    /// The ratio under which a constant memset of `zero` bytes, or of any other
    /// byte, is stores; 0 where none is stated. `size`: tuned for size.
    pub fn pieces(
        &self,
        size: bool,
        zero: bool,
    ) -> i64 {
        match (size, zero) {
            (true, true) => self.pieces_clear_size,
            (true, false) => self.pieces_set_size,
            (false, true) => self.pieces_clear,
            (false, false) => self.pieces_set,
        }
    }

    /// How many native words an integer of `bits` bits is: one where the
    /// target states no native width.
    pub fn words(
        &self,
        bits: u32,
    ) -> i64 {
        if self.native_bits > 0 { (i64::from(bits) + self.native_bits - 1) / self.native_bits } else { 1 }.max(1)
    }

    /// What a caller pays to take `words` words of arguments off the stack.
    pub fn cleanup(
        &self,
        words: i64,
    ) -> i64 {
        (words * self.pop).min(self.adjust)
    }

    /// `price` for an operation on `width`-byte values where the code's own
    /// operand size is `operand` bytes: a word or dword that is not that
    /// size runs under the operand-size prefix (a dword in real mode, a word in
    /// flat code).
    pub fn sized(
        &self,
        price: i64,
        width: i64,
        operand: i64,
    ) -> i64 {
        price + if width != operand && width > 1 { self.prefix } else { 0 }
    }
}

/// What one `lea` of `width`-byte values costs, where the target has an
/// address form for it. A `word` one is a plain address (a base and an
/// index, unscaled, no prefix); any other takes a form that scales, which
/// costs its address-size prefix, and runs under the operand-size prefix a
/// dword does.
pub fn three_operand_of(
    costs: &OperationCosts,
    forms: &[AddressForm],
    width: i64,
    operand: i64,
    scale: i64,
    word: bool,
    parts: usize,
) -> Option<i64> {
    if word {
        return forms.iter().any(|form| !form.secondary && form.index_width == 2).then_some(costs.address);
    }
    let form = forms.iter().find(|form| form.index_width == 4 && form.scales.contains(&scale))?;
    if costs.address_dword > 0 && width != operand {
        return Some(
            costs.address_dword + costs.address_third * parts.saturating_sub(2) as i64 + form.use_cost - costs.prefix,
        );
    }
    Some(costs.sized(costs.address, width, operand) + form.use_cost)
}

/// [`three_operand_of`] for an address of two parts (a base and an index, or a
/// base and a displacement).
pub fn three_operand(
    costs: &OperationCosts,
    forms: &[AddressForm],
    width: i64,
    operand: i64,
    scale: i64,
    word: bool,
) -> Option<i64> {
    three_operand_of(costs, forms, width, operand, scale, word, 2)
}

impl Default for OperationCosts {
    fn default() -> Self {
        Self {
            native_bits: 0,
            add: 1,
            multiply: 1,
            divide: 1,
            multiply_long: 1,
            divide_long: 1,
            multiply_wide: 0,
            multiply_widening: 0,
            divide_margin: 0,
            multiply_high: 1,
            shift: 1,
            address: 1,
            address_dword: 0,
            address_third: 0,
            carry: 1,
            carry_step: 1,
            load: 1,
            store: 1,
            store_immediate: 1,
            memory_update: 1,
            branch: 1,
            prefix: 0,
            r#move: 1,
            call: 1,
            return_: 1,
            argument: 2,
            pop: 1,
            adjust: 1,
            return_pops: 0,
            float_add: 1,
            float_multiply: 1,
            float_divide: 1,
            float_load: 1,
            float_load_folded: 1,
            folds_loads: false,
            float_store: 1,
            float_release: 0,
            float_to_int: 0,
            float_compare: 0,
            extend: 1,
            set: 1,
            fill: 1,
            fill_cell: 1,
            fill_segment: 0,
            memset_stores: 0,
            huge_fill: 0,
            huge_copy: 0,
            copy: 1,
            copy_cell: 1,
            copy_variable: 0,
            direction: 1,
            unroll_budget: 0,
            pieces_set: 0,
            pieces_clear: 0,
            pieces_set_size: 0,
            pieces_clear_size: 0,
            memset_store: 0,
            jump_table: 0,
            function: 0,
            function_scale: 100,
            width_scaled: false,
            constant_offsets_fold: false,
            branch_in_compare: false,
            direct_load: 0,
            direct_store: 0,
            free_intrinsics: false,
            expanded_intrinsics: false,
            cache_line: 0,
            line_fill_clocks: 0,
            ext_tsp_for_size: false,
        }
    }
}

/// One legal indexed-address family and its costs above the native form.
///
/// Machine-neutral: MIR may know an address can use a four-byte index with
/// scales 1/2/4/8, never how x86 spells it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AddressForm {
    pub index_width: i64,
    pub scales: BTreeSet<i64>,
    pub extra_bytes: i64,
    pub use_cost: i64,
    pub extension_cost: i64,
    pub secondary: bool,
    // How many distinct bases one index can pair with at once; None is any.
    pub partners: Option<i64>,
    // The registers an address takes as its base and as its index, where an
    // address is one of each: a register is of one class or the other. None is
    // any.
    pub bases: Option<i64>,
    pub indices: Option<i64>,
    // Compatibility name for `secondary`; both views stay identical.
    pub fallback: Option<bool>,
}

impl AddressForm {
    /// The dataclass constructor and `__post_init__`.
    pub fn new(
        index_width: i64,
        scales: BTreeSet<i64>,
        extra_bytes: i64,
        use_cost: i64,
        extension_cost: i64,
        secondary: bool,
        fallback: Option<bool>,
    ) -> Result<Self, String> {
        if fallback.is_some() && secondary && fallback != Some(secondary) {
            return Err("an address form cannot disagree about whether it is secondary".to_owned());
        }
        let selected = fallback.unwrap_or(secondary);
        Ok(Self {
            index_width,
            scales,
            extra_bytes,
            use_cost,
            extension_cost,
            secondary: selected,
            partners: None,
            bases: None,
            indices: None,
            fallback: Some(selected),
        })
    }

    /// How many registers can each hold an address alone: the one that
    /// pairs and its partners. None is any.
    pub fn address_registers(&self) -> Option<i64> {
        self.partners.map(|partners| partners + 1)
    }

    /// The classes of an address's two registers, where the form has them.
    pub fn register_classes(&self) -> Option<(i64, i64)> {
        self.bases.zip(self.indices)
    }

    /// Whether this form is cheap enough to try before a frame spill.
    pub fn before_spill(
        &self,
        costs: &OperationCosts,
    ) -> bool {
        let direct = self.extension_cost + self.use_cost <= costs.load;
        let amortized = self.extension_cost <= costs.r#move
            && self.extension_cost + self.use_cost <= costs.shift + costs.address + costs.r#move + costs.store;
        !self.secondary || direct || amortized
    }
}

/// What an argument is, for where a convention puts it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argument {
    pub bytes: i64,
    pub integer: bool,
    pub pointer: bool,
    pub floating: bool,
    /// The front end passes it in memory whatever it is (a struct by value).
    pub memory: bool,
}

/// The convention the target gives a private function, and those a function
/// must have for it to take it: a marked function (Pascal's, an interrupt's)
/// keeps the protocol its marker names.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PrivateConvention {
    pub to: u32,
    pub from: Vec<u32>,
}

/// A target that states nothing: no foreign memory, unit prices, and no
/// registers, which leaves pressure unpriced.
pub struct Neutral;

impl Machine for Neutral {
    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn foreign_span(
        &self,
        _: (i64, i64),
        _: (i64, i64),
        _: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    fn registers(&self) -> i64 {
        0
    }

    fn call_registers(&self) -> i64 {
        0
    }

    /// An address adds one index, unscaled and free.
    fn address_forms(&self) -> Vec<AddressForm> {
        vec![AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("no fallback to disagree")]
    }
}

#[cfg(test)]
mod operand_size_tests {
    use super::*;

    fn form(
        index_width: i64,
        secondary: bool,
        use_cost: i64,
    ) -> AddressForm {
        AddressForm::new(index_width, BTreeSet::from([1, 2, 4, 8]), 0, use_cost, 0, secondary, None).expect("a form")
    }

    /// The operand-size prefix is for the size that is not the code's own: a
    /// dword in real mode, a word when flat.
    #[test]
    fn the_prefix_is_for_the_size_that_is_not_the_codes_own() {
        let costs = OperationCosts { prefix: 1, ..OperationCosts::default() };
        assert_eq!(
            (
                costs.sized(5, 4, 2),
                costs.sized(5, 2, 2),
                costs.sized(5, 4, 4),
                costs.sized(5, 2, 4),
                costs.sized(5, 1, 4)
            ),
            (6, 5, 5, 6, 5)
        );
    }

    /// A flat target's dword address has no prefix and its price is the
    /// `lea`'s; it had none, because only the prefixed (secondary) form was
    /// looked for. A word address needs a form of index width 2.
    #[test]
    fn a_native_dword_address_is_priced_and_a_word_one_needs_a_word_form() {
        let costs = OperationCosts { address: 2, prefix: 1, ..OperationCosts::default() };
        let flat = [form(4, false, 0)];
        assert_eq!(three_operand(&costs, &flat, 4, 4, 2, false), Some(2));
        assert_eq!(three_operand(&costs, &flat, 2, 4, 1, true), None);
        let real = [form(2, false, 0), form(4, true, 1)];
        assert_eq!(three_operand(&costs, &real, 4, 2, 2, false), Some(2 + 1 + 1));
        assert_eq!(three_operand(&costs, &real, 2, 2, 1, true), Some(2));
    }

    /// Real mode's `lea`: 3 bytes in words (`8D 40 07`), 5 as dwords behind the
    /// two prefixes (`67 66 8D 40 07`), 6 with a base, an index and a
    /// displacement (`67 66 8D 44 01 07`); all three read as one price
    /// (`address` plus the prefixes) before `address_dword`, so a fold that
    /// replaced `inc` and `add` (5 bytes) by the six-byte form looked free.
    #[test]
    fn a_dword_lea_is_priced_by_its_parts_and_a_word_one_is_not() {
        let costs =
            OperationCosts { address: 6, prefix: 1, address_dword: 10, address_third: 2, ..OperationCosts::default() };
        let real = [form(2, false, 0), form(4, true, 1)];
        assert_eq!(three_operand_of(&costs, &real, 2, 2, 1, true, 3), Some(6));
        assert_eq!(three_operand(&costs, &real, 4, 2, 1, false), Some(10));
        assert_eq!(three_operand_of(&costs, &real, 4, 2, 1, false, 3), Some(12));
        let unstated = OperationCosts { address: 6, prefix: 1, ..OperationCosts::default() };
        assert_eq!(three_operand_of(&unstated, &real, 4, 2, 1, false, 3), Some(8));
    }
}

#[cfg(test)]
mod tests {
    use super::Tuning;

    /// X86ISelLowering.cpp:2936-2941 and the clock-free default: a target with
    /// no entry in its description takes exactly these.
    #[test]
    fn the_default_tuning_is_the_pins() {
        let pin = Tuning::PIN;
        assert_eq!((pin.max_stores_per_memset, pin.max_stores_per_memset_opt_size), (16, 8));
        assert_eq!((pin.max_stores_per_memcpy, pin.max_stores_per_memcpy_opt_size), (8, 4));
        assert_eq!((pin.max_stores_per_memmove, pin.max_stores_per_memmove_opt_size), (8, 4));
        assert_eq!(pin.size_milliclocks_per_byte, 0);
        assert_eq!(
            (pin.unroll_threshold, pin.unroll_max_percent_threshold_boost, pin.unroll_partial_threshold),
            (None, None, None)
        );
        assert_eq!(pin.x87_stack_slots, None);
        assert!(!pin.x87_share_negated_products);
        assert!(!pin.unroll_promotion_fits_x87_stack);
        assert!(!pin.x87_zero_fill_at_size);
        assert!(!pin.slots_loop_width_rule);
        assert!(pin.carry_slot_indexes);
        assert_eq!(pin.address_reference_milliweight, 1000);
        assert_eq!(pin.inline_threshold, None);
    }
}
