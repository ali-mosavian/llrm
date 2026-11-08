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
    /// CPU's table gives, with no byte term (gcc's `synth_mult` at
    /// -O1..-O3); false: the chain where `speed_milliclocks_per_byte` pays
    /// for its bytes.
    pub multiply_by_clocks: bool,
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
    /// DEVIATE INL-O1 (owner x86-m32, exit 2026-11-24): where no byte rate is
    /// stated (-O1), an inline that stays is held to the -Os bound (the callee
    /// copied only if it does not grow the code); false: the pin's threshold.
    pub inline_never_grows_without_rate: bool,
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
    /// At -Os a multiply by a constant stays an `imul` (the pin's `hasMinSize`
    /// rule, which the pin applies at -Oz only): the chain is no smaller.
    pub imul_at_size: bool,
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
    /// `-unswitch-threshold` of the non-trivial loop unswitch (-O3): the cost
    /// (loop size times copies) a candidate must stay under; none: 50.
    pub unswitch_threshold: Option<i64>,
    /// `InlineParams::DefaultThreshold`, the inliner's bar for a caller not
    /// optimized for size; none: 225, or 250 at -O3.
    pub inline_threshold: Option<i64>,
    /// Two compares joined by `and`/`or` stay two branches at every level, as
    /// the pin does at optsize; false: the pin's cost rule decides.
    pub split_merged_conditions: bool,
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
    /// DEVIATE LSR-SIZE (owner x86-m32, exit 2026-11-24): see
    /// `Tti::lsr_size_order`.
    pub lsr_size_order: bool,
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
    /// DEVIATE CS2: where there is no conditional move, a select's arm only
    /// it reads is made on its own path (`Tti::sinks_select_arms`). false: the
    /// pin's, both operands before the branch.
    pub sink_select_arms: bool,
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
        divide_by_bytes: false,
        narrow_divide_live_limit: 0,
        sink_spares_ranges: false,
        promote_va_list: false,
        branch_compare_trees_at_size: false,
        unroll_never_grows_at_o1: false,
        inline_never_grows_without_rate: false,
        inline_o1_milliclocks_per_byte: 0,
        inline_o1_unit_bytes_milli: 1000,
        speed_inline_milliclocks_per_byte: 0,
        call_sequence_fixed_clocks: 0,
        call_sequence_argument_clocks: 0,
        call_sequence_pop_clocks: 0,
        inline_o1_weighs_entry: false,
        unroll_o1_analyzed_trips: None,
        second_ipsccp: false,
        price_pin_multiplies: false,
        imul_at_size: false,
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
        unswitch_threshold: None,
        inline_threshold: None,
        split_merged_conditions: false,
        inline_size_guard_at_size: false,
        inline_size_guard_single_site: false,
        inline_chain_trial: false,
        unroll_promotion_fits_registers: false,
        lsr_size_order: false,
        unroll_promotion_fits_x87_stack: false,
        rotation_guard_must_fold_at_size: false,
        rotation_header_priced_at_size: false,
        size_rotation_header_limit: None,
        keep_constant_displacements: false,
        keep_constant_arith_operands: false,
        recursive_inlining: None,
        recursive_edge_growth: None,
        recursive_inlining_growth: None,
        recursive_inlining_depth: None,
        recursive_tre_first: (false, false),
        peel_folded_entry: false,
        unroll_milliclocks_per_byte: 0,
        sink_select_arms: false,
        inline_variable_memcpy: false,
        size_copy_price: None,
        ipsccp_widen_keeps_sign: false,
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
    /// Bytes of one line of the CPU's data cache (its first level); 0: the
    /// target states none (no cache, or not known).
    pub cache_line: i64,
    /// Clocks to fill one such line from memory; 0: not stated.
    pub line_fill_clocks: i64,
}

impl OperationCosts {
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
            cache_line: 0,
            line_fill_clocks: 0,
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
        assert_eq!(pin.inline_threshold, None);
    }
}
