//! The flat 32-bit x86 target: CS = DS = SS, base 0, 4 GB, one 32-bit pointer.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

include!(concat!(env!("OUT_DIR"), "/effects.rs"));
include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

use std::sync::LazyLock;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::CostModel;
use llrm_target::machine::Machine;

/// Flat DOS under an extender, and the PC ports it shares.
pub const DOS32: &str = concat!(
    include_str!("../../../../machine/platform/dos/flat32.toml"),
    include_str!("../../../../machine/platform/dos/ports.toml")
);

/// The processors flat code is priced for: the columns of `timings.times`.
pub static CPUS: LazyLock<Vec<&'static str>> = LazyLock::new(|| TIMINGS.cpus());

/// The data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: LazyLock<llrm_target::layout::Layout> =
    LazyLock::new(|| llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("flat datalayout.toml parses"));

/// The calling conventions: `calling.toml`.
static CALLING: LazyLock<llrm_target::calling::Calling> = LazyLock::new(|| {
    llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses")
});

/// Their names, the first being the language's own.
static CONVENTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| CALLING.names());

/// The flat target's layout.
pub fn layout() -> llrm_target::layout::Layout {
    LAYOUT.clone()
}

/// The flat 32-bit x86 target as `llrm-driver` names it.
pub struct M32;

/// The registers and the address forms the descriptions state.
static REGISTERS: LazyLock<std::sync::Arc<Vec<llrm_target::registers::Register>>> =
    LazyLock::new(|| llrm_target::registers::file(include_str!("registers.regs")).expect("registers.regs parses"));

/// How flat operations are priced from the instruction forms: `opcosts.txt`.
static OPCOSTS: LazyLock<llrm_target::opcosts::Description> = LazyLock::new(|| {
    llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses")
});

/// The CPUs' timings: `timings.times`.
static TIMINGS: LazyLock<llrm_target::timings::Timings> = LazyLock::new(|| {
    llrm_target::timings::Timings::parse_tuned(include_str!("timings.times"), llrm_target::tune::X86)
        .expect("timings.times parses")
});

/// The pin's register pressure sets for the registers described above:
/// `pressure.sets`.
static PRESSURE: LazyLock<llrm_target::pressure::PressureSets> =
    LazyLock::new(|| llrm_target::pressure::parse(include_str!("pressure.sets")).expect("pressure.sets parses"));

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| {
    llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses")
});

/// The target's tuning. DEVIATE: a multiply by a constant the pin leaves an
/// `imul` (13-42 clocks on a 486, Intel 240440-002 Table 10.1) is, at -O1..-O3,
/// the shift/`lea` chain of fewest clocks on the CPU's table with no byte term,
/// as a shift-and-add search does (`arithmetic::chain_by_clocks`, SD446); -Os
/// keeps bytes. x_iir was 2.17x gcc's clocks. Owner llrm-target-x86-m32.
///
/// DEVIATE M32-MUL-O1-CHAIN (owner llrm-target-x86-m32, exit 2026-11-24,
/// re-dated 2026-11-11 from 2026-11-10; the O1 rule is strict per axis, so the
/// exit is a priced choice that keeps the chain only where its clocks and bytes
/// are both no worse; oversight 2026-11-08: a Pareto chain search, equal clocks
/// and fewer bytes, replaces the fewest-clocks chain; per-site pricing stays
/// unshipped, it moves no row). Open vs the byte bar: bintree +32, recpow +29,
/// x_spill_store_order +54 B (also tail_calls_edges +32, stft_fixed +32,
/// copy_runs +40 against the `imul` build). At -O1
/// the chain is the trade against the pin's `imul` (`x * 1664525`: 29 B and ~9
/// clocks against 6 B and 13-42), not a Pareto win. harness code bytes /
/// simulated clocks at -O1, chain | `imul` | best gcc/clang (2026-11-08):
/// x_iir 141/49.5k | 111/130.5k | 122/60.0k; mul_constants 2285/132.6k |
/// 1782/287.0k | 1777/142.2k; urem_eq 467/792k | 340/1444k | 374/1468k;
/// bintree 308/74.0k | 282/79.4k | 276/84.1k; recpow 267/442k | 220/532k |
/// 238/575k; x_spill_store_order 687/82.2k | 611/99.5k | 633/103.3k;
/// copy_runs 818/6.3k | 778/8.3k | 964/8.1k. The chain wins clocks 7-62% for
/// 26-503 B; it is above the best byte bar on the first six rows, the `imul`
/// above the clock bar on x_iir and mul_constants. Exit (if ever): a general
/// mechanism with the chain's clocks and `imul`-like bytes (a shorter chain
/// search, or the block's frequency pricing the choice).
///
/// A code byte costs the CPU's instruction-cache fill, once per run: a line's
/// fill clocks over its bytes (`line_fill_clocks` / `cache_line` of the CPU's
/// sheet; 486: 5 / 16 = 312 milliclocks), so `speed_milliclocks_per_byte` is
/// derived, never set. Owner llrm-target-x86-m32.
fn tuning(costs: &llrm_mir::target::OperationCosts) -> llrm_mir::target::Tuning {
    let per_byte = llrm_target::timings::derived_byte_rate(costs.cache_line, costs.line_fill_clocks);
    llrm_mir::target::Tuning {
        multiply_by_clocks: true,
        libc_memcpy: true,
        pops_arguments_at_size: false,
        // DEVIATE X87-FCMP-AH at -O1 (owner x87-c, exit 2026-11-24): `sahf`,
        // not `test ah`, where bytes are held: x_ld_cmp3 O1 184 -> 180 B. m16 O1
        // keeps the test until its counted cells are judged.
        keeps_sahf_where_bytes_are_held: true,
        // DEVIATE SZ4 (owner x86-m32, exit 2026-11-27): -Os copies a loop
        // header of at most 4 instructions, not the pin's 16 (the copy is a
        // compare and a branch, more than the jump rotation removes). m32 -Os
        // kernels (73): x_hash -29 B, x_rle -8 B, none larger; limits 1..3 grow
        // x_insertion/x_jpeg_single, 1 grows 60 cells.
        size_rotation_header_limit: Some(4),
        // DEVIATE RS1 at m32 (owners x86-m32-bytes, exit 2026-12-20, as m64 and
        // arm64): -Os rotates a loop only if its copied exit test folds in the
        // preheader or counts its trips, as gcc's
        // `ch_base::copy_headers` does at -Os (the copy is a compare and a
        // branch, more than the jump it removes). 296 kernels, -Os: 17 shrink
        // (-296 B: x_jpeg_fixed 2995 -> 2898, x_jpeg_single 3041 -> 2960,
        // x_primes 100 -> 85), gap to best(gcc, clang) 4361 -> 4102 B. Os is
        // strict bytes, clocks not bound:
        // shellsort/x_isqrt/x_popcount/x_collatz +9..11%, 8 others +-1%. ROW
        // M32-RS1-COLLATZ (owner x86-m32-bytes, exit 2026-11-27): x_collatz
        // 84 -> 85 B (best 74), an unrotated inner loop changes the outer
        // loop's registers (cause card to the greedy owners).
        rotation_guard_must_fold_at_size: true,
        // DEVIATE ROTATION-TRIAL (owner greedy-os-causes, exit 2026-12-05, with
        // RS1): a counted loop RS1 rotates is also tried unrotated in a second
        // compile, kept only if the object is smaller. Os, 98 cells, unrotated
        // everywhere: 21 smaller (x_jpeg_fixed -213 B, x_radix -20,
        // x_idioms -16, x_prefix -12), 64 larger (x_transpose +34,
        // x_matmul_f +29, x_life +25).
        rotation_trial_at_size: true,
        // DEVIATE ROTATION-HOT (owner x86-m32, exit 2026-11-24):
        // see `Tti::rotation_hot_at_size`.
        rotation_hot_at_size: true,
        // DEVIATE GVN-PRE-OPTSIZE (owner x86-m32, exit 2026-12-01):
        // see `Tti::load_pre_off_at_size`.
        load_pre_off_at_size: true,
        // DEVIATE UNROLL-O1 (owner x86-m32, exit 2026-11-27,
        // with `unroll_at_o1`): at -O1 a full unroll is made only where its
        // simulated cost does not exceed the rolled loop's size; O1 is strict
        // (no cell larger), gcc's complete unroll is size-neutral.
        unroll_never_grows_at_o1: true,
        // DEVIATE UNROLL-O1-TRIAL (owner x86-m32-c, exit 2026-12-05, with
        // UNROLL-O1): a loop that rule refuses is unrolled by the pin's
        // threshold (150 units,
        // `UnrollThresholdDefault`) in a second compile, kept only if neither
        // the bytes nor the executed work grow. x_life O1: the 3x3 nest
        // unrolled is -15 B and -62% clk (the rolled nest spills); matmul O1
        // is +1601 B and stays rolled.
        unroll_trial_at_o1: Some(150),
        unroll_trial_nests_only: true,
        // DEVIATE UNROLL-OS-TRIAL (owner x86-m32, exit 2026-11-24): see `Tuning::unroll_trial_at_size`. gcc
        // completely peels a small loop at -Os when the code does not grow (x_life: the 3x3 window, 360 -> 347 B).
        unroll_trial_at_size: Some(30),
        // DEVIATE UNROLL-O1A (owner w9, exit 2026-11-27, with UNROLL-O1):
        // the -O1 rule analyses the folds of up to 16 trips,
        // gcc's `max-completely-peel-times`, not the pin's 10 (a 16-trip loop
        // over a constant table folds to nothing and stayed rolled: 64 -> 158
        // B).
        unroll_o1_analyzed_trips: Some(16),
        // DEVIATE SD443 (owner w9, exit: a constant-argument pass after
        // inlining, as gcc's IPA-CP): a function called directly only after
        // inlining keeps its address-taken parameters through the pin's one
        // IPSCCP; a second one after promotion folds them. m32
        // memory_expansion -1332 B (-O2/-O3), second_ipsccp.c -368 B.
        second_ipsccp: true,
        // DEVIATE MUL-OS (owner x86-m32, exit 2026-11-20): at -Os the pin
        // decomposes a constant multiply (`lea; lea`, 8 B for x * 11) where
        // gcc keeps the 3-byte `imul`; the pin keeps it at -Oz only.
        imul_at_size: true,
        // DEVIATE MUL-HOT (owner x86-m32, exit 2026-11-24): see `Tuning::mul_chains_hot_at_size`.
        mul_chains_hot_at_size: true,
        // DEVIATE LFTR1 (owner w12, review 2026-12-20):
        // see `Tuning::lftr_any_stride`.
        lftr_any_stride: true,
        // DEVIATE SD103-M32 (owner x87, exit 2026-11-24): the pin's cost rule,
        // with the price of a merged `a && b` it omits taken from the table:
        // two `setcc`, a logic op and a
        // `test`, less the branch saved. sieve -O1: `i <= 31 && i*i < n` merged
        // is `setb; setb; and; cmp 1; jne`, 20% more clocks; a pair the DAG
        // folds to one compare stays merged.
        merged_condition_clocks: 2 * costs.set + 2 * costs.add - costs.branch,
        // DEVIATE LSR-OUTER (owner x87, exit 2026-11-27):
        // see `Tuning::multiply_clocks`.
        multiply_clocks: costs.multiply,
        loop_carried_clocks: costs.store + costs.load,
        rep_stos_at_size: true,
        rep_movs_at_size: true,
        sdiv_pow2_stays_at_size: false,
        // DEVIATE MERGEFUNC-OS (owner x86-m32, exit 2026-11-30):
        // see `Tuning::merge_functions_at_size`.
        merge_functions_at_size: true,
        x87_stack_slots: Some(8),
        x87_share_negated_products: true,
        unroll_promotion_fits_x87_stack: true,
        lsr_size_order: true,
        // DEVIATE IU1-M32 (owner w12, exit 2026-11-24): the m64 entry at m32; see
        // `Tti::lsr_users_in_use_list_order`. x_transpose -O1 139,335 -> 122,886 clk, 237 -> 217 B.
        lsr_users_in_use_list_order: true,
        // DEVIATE LSR-PHI (owner x87, exit 2026-11-24): see `Tuning::lsr_phi_operand_uses`.
        lsr_phi_operand_uses: true,
        // DEVIATE LU14 (owner x86-m32, exit 2026-11-30): see `Tuning::lsr_both_use_orders`.
        lsr_both_use_orders: true,
        // DEVIATE FMULADD-SPLIT (owner x87-c, exit 2026-11-24): see
        // `Tti::splits_invariant_fmuladd`.
        splits_invariant_fmuladd: true,
        narrow_divide_live_limit: 4,
        sink_spares_ranges: true,
        promote_va_list: true,
        branch_compare_trees_at_size: true,
        inline_never_grows_without_rate: true,
        // DEVIATE OB1 (owner w12, exit 2026-11-27, `unrollbound.rs`): gcc
        // completely peels a loop whose trips the object it indexes bounds
        // (max-completely-peel-times 16); the pin does not. x_matmul_f /
        // x_ld_matmul -O3: 392 B and 3 fmul against gcc's 1183 B and 14. Tried
        // at m64 on 2026-11-08, hook on: only x_matmul_f / x_ld_matmul O2/O3
        // change (-848 B on x_matmul_f), but executed instructions rise 53%
        // (7416 -> 11364: guarded peeled copies against a 4x runtime unroll);
        // left None. The m64 x_matmul_f size gap (ours 1751 B, pin 2296 B, 50
        // packed ops) is vector-shaped: vector-deferred list.
        unroll_object_bound: Some(16),
        inline_o1_milliclocks_per_byte: per_byte,
        // DEVIATE INL-O1 SIZE (owner m32-o1-small, exit 2026-11-14): the
        // inliner counts a callee in bytes of encoded code by instruction kind
        // (`sizes`), not one unit an instruction scaled by a fitted 3.5.
        // x_jpeg_single -O1 inlined `next` (about 190 B, 8 sites) at an
        // estimated 82 B:
        // +1533 B and +0.5 M clk.
        inline_size_bytes: true,
        inline_o1_unit_bytes_milli: 1000,
        speed_inline_milliclocks_per_byte: per_byte,
        sdiv_by_magnitude: true,
        call_sequence_fixed_clocks: costs.call + costs.return_,
        call_sequence_argument_clocks: costs.argument,
        call_sequence_pop_clocks: costs.adjust,
        inline_o1_weighs_entry: true,
        x87_zero_fill_at_size: true,
        speed_milliclocks_per_byte: per_byte,
        // DEVIATE REPFILL (owner x86-m32, exit 2026-11-27): at -Os a constant
        // fill is `rep stos` where its stores take more bytes (gcc -Os; the pin
        // keeps up to MaxStoresPerMemsetOptSize stores). `mov dword/word/byte
        // [esp+d8], imm` 8/7/5 B, at an absolute address 10/9/7; `xor eax, eax`
        // 2,
        // `or eax, -1` 3, `mov eax, imm32` 5; `lea edi, [esp+d8]` (4) or `mov
        // edi, imm32` (5), `mov ecx, imm32` 5, `rep stos` 2; `fst qword` 4 / 6.
        // DEVIATE SINK-CONST-PHI (owner x86-m32, exit 2026-11-27): the pin
        // sinks
        // `z -= a` / `z += b` of a diamond into the join as `z += phi(-a, b)`;
        // on a 486 each arm then pays a `mov imm` and one a taken `jmp`
        // (x_cordic -O3
        // +20% against gcc, which keeps an immediate op per arm). Not at -Os.
        keep_constant_arith_operands: true,
        // DEVIATE PHICONST-2 (owner x87, exit 2026-11-24):
        // see `Tti::folds_phi_constant_chains`.
        phi_constant_chains: true,
        size_fill: Some(llrm_mir::target::SizeFill {
            store: [[8, 7, 5], [10, 9, 7]],
            value: [2, 3, 5],
            setup: [11, 12],
            fst: [4, 6],
        }),
        // DEVIATE SZ1 (owner x86-m32, exit 2026-11-20): D11 holds at -Os, so a
        // callee whose copies grow the code stays a call (called_once_os:
        // gcc -Os does; the pin inlines it).
        inline_size_guard_at_size: true,
        // DEVIATE SZ2 (owner x86-m32, exit 2026-11-20): -Os keeps a once-called
        // callee whose copy grows the code (gcc does; the pin copies it).
        inline_size_guard_single_site: true,
        // DEVIATE SZ3 (owner x86-m32, exit 2026-11-20): an -Os SZ2 trial also
        // inlines the sites its splice brings. The pin inlines callee-first
        // (`bar` into `foo`, cost 15 < 75, then `foo` into
        // `main`); the size guard prices each edge alone and keeps both calls.
        // byvalue_struct_address
        // -Os 147 -> 39 B (clang 23), 176 m32 programs, no other cell moves.
        inline_chain_trial: true,
        // DEVIATE REC-2: gcc 15.2's `max-inline-recursive-depth` 8 and
        // `max-inline-insns-recursive` 450, at -O2/-O3 (never -Os). Owner w10.
        recursive_inlining: Some((8, 450)),
        // DEVIATE REC-2/REC-10 bar: 22 `gcc_insns` at -O2 and -O3, fitted to
        // the corpus (gcc's own is 15 at -O2, 30 at -O3, in its size units).
        // At 12 at -O2 recchop (14) was refused where gcc copies it; at 22
        // five -O2 programs copy and all get faster (recchop 7183 -> 5397 clk,
        // x_hanoi2 219202 -> 160540); the other 116 and every -O3 cell are
        // unchanged. recchop and bintree's `walk` are both 14 and gcc takes
        // one: no bar matches it. Owner x86-m32; exit 2026-12-10: the
        // estimate becomes gcc's own size count.
        recursive_edge_growth: Some((22, 22)),
        // DEVIATE REC-2: the copies may add gcc's 450 at -O2 (fib 240256 ->
        // 99687 clk, x_fib, frames), 150 at -O3 (rectwo, bintree, recchop are
        // worse with 450; their O3 card is open). Owner x86-m32, exit
        // 2026-11-20.
        recursive_inlining_growth: Some((450, 150)),
        // DEVIATE REC-7 (owner m32, exit 2026-11-27): depth 2 at -O2 and -O3.
        // rectwo -O2 1449397 -> 347314 clk (1965 -> 277 B), fib 99618 -> 19993
        // (962 -> 215 B), frames 5035113 -> 4585856. -O3 had gcc's 8: over 12
        // recursion programs it cost +5556 B and
        // +1.12 M instructions over -O2 (REC-8).
        recursive_inlining_depth: Some((2, 2)),
        // DEVIATE INL-HOT (owner m32, exit 2026-11-30): a call the pin's
        // threshold alone refuses, at a site 16x the caller's entry or hotter,
        // is tried inlined at -O2 and kept where the rate pays, the trial
        // counting the callee's preserved registers (x_sf_* -O2
        // clk -10..-23%, +516..676 B; jpeg -0.4% clk,
        // +442..586 B). Trips are the pin's block frequency (31 in x_sf_add's
        // loop, 400 run), not a profile. -O3 too since 2026-11-12: jpeg no
        // longer meets a trial there.
        inline_hot_trial_frequency: Some(16),
        // x_sf_* -O3 clk -10..-16 k, +528..657 B; the other 116 programs are
        // unchanged at -O2 and -O3.
        inline_hot_trial_at_o3: true,
        // A callee that keeps more than eax/ecx/edx live pushes and pops ebx,
        // esi, edi, ebp once per call: the trial's clocks saved count it.
        callee_saved: Some((3, 4, costs.argument + costs.pop)),
        // DEVIATE REC-2/REC-8: gcc eliminates tail calls before it copies a
        // recursion (passes.def:97, then pass_ipa_inline :164). Here the tree's
        // copies come first at
        // -O2 and -O3: GVN merges the pure same-argument calls across them (fib
        // 99618 clk against 228901 with the loop first), and -O3 with the loop
        // first and depth 8 lost to -O2 on fib, frames and rectwo. Owner
        // x86-m32, exit 2026-11-20.
        recursive_tre_first: (false, false),
        // DEVIATE IPACP-O3 (owner x86-m32-c, exit 2026-12-20): queens -O3
        // 160324 -> 79692 clk (gcc 94851, which has `-fipa-cp-clone` on
        // at -O3).
        ipa_cp_clones_at_speed: true,
        peel_folded_entry: true,
        // DEVIATE UP1 (owner x86-m32; exit 2026-11-20: the pin's cost model or
        // gcc's unroll estimate priced in clocks and bytes replaces it): a
        // full unroll must repay its bytes at the speed rate.
        unroll_milliclocks_per_byte: per_byte,
        sink_select_arms: true,
        // DEVIATE IND-PREFIX (owner x87, exit 2026-11-30): `short i` is a
        // 32-bit counter; the 486 and P5 pay a prefix on every 16-bit
        // operation (ring -O2: 67177 clk, gcc 61179).
        prefixed_16_bit_counters: true,
        // DEVIATE MEM-1 (the pin's `EmitTargetCodeForMemcpy`,
        // X86SelectionDAGInfo.cpp L328, leaves a variable length to a libcall):
        // at -O2/-O3 on the 486 and P5, the only CPUs this target accepts, it
        // is `rep movsd` inline, as gcc's `rep_prefix_4_byte`. x_mergesort -O3:
        // its short copy loops were `call _memcpy`; inline, 72146 -> 66173 clk
        // (gcc 60026: row MEM-1-R1). -O1 and size keep the call
        // (`bytes_strict`: no byte rate pays for the ~17 B; x_jpeg_single -O1
        // inlined it twice). Owner w2. Exit 2026-11-20.
        inline_variable_memcpy: true,
        // DEVIATE REC5-1 (owner w2, exit 2026-11-20), not at -Os:
        // see `Tuning::ipsccp_widen_keeps_sign`.
        ipsccp_widen_keeps_sign: true,
        ..llrm_mir::target::Tuning::PIN
    }
}

impl M32 {
    /// The pressure sets and classes of the pin's x86.
    pub fn pressure_sets() -> &'static llrm_target::pressure::PressureSets {
        &PRESSURE
    }

    /// The limit of pressure set `set` here (`computePSetLimit`): a register
    /// may be allocated if its root is a `gpr`.
    pub fn pressure_limit(set: usize) -> u32 {
        PRESSURE.limit(set, |name| {
            REGISTERS
                .iter()
                .find(|one| one.name == name)
                .and_then(|one| REGISTERS.iter().find(|root| root.name == one.root))
                .is_some_and(|root| root.is("gpr"))
        })
    }
}

impl llrm_target::Target for M32 {
    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &llrm_x86::mc::X86Objects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        llrm_x86::mc::fixup::debug_target(32)
    }

    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn selects_by_dag(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "x86-m32"
    }

    // DEVIATE LEAF-FOLD-OS (owner m32, exit 2026-11-20): the pin keeps each
    // `mov imm; jmp` leaf of a sparse switch (PHIElimination.cpp L841, L858,
    // L879 split an edge only for a value live out past the phis, so the
    // leaf stays and costs a `jmp`); at -Os the constant is made in the
    // branching block and the branch goes to the join.
    fn leaf_fold_at_size(&self) -> bool {
        true
    }

    // DEVIATE UNCHEAP-IMM-M16 (owner x86-m32, exit 2026-11-24): m16 is also
    // `Not64BitMode` and the pin makes +1/-1 there the same way; it keeps the
    // default until the m16 owner prices it.
    fn uncheap_move_immediate_at_size(&self) -> fn(u8, i64) -> bool {
        llrm_target::x86_uncheap_move_immediate_at_size
    }

    // DEVIATE CSE-CMP (owner x86-m32-bytes, exit 2026-12-20): the pin's
    // MachineCSE shares a branch-opening compare with the same subtract on
    // one side, moving the subtract in front of the branch and copying the
    // operand the other side reads. x_isqrt -O2: -7.7% clocks, -10 B (-O1
    // -5.9%, -Os -4.7%); the other 286 non-m16 kernels at -O1/-O2/-O3/-Os
    // are byte-identical.
    fn cse_keeps_compare_with_live_minuend(&self) -> bool {
        true
    }

    fn pressure(&self) -> Option<(&'static llrm_target::pressure::PressureSets, Vec<u32>)> {
        Some((
            M32::pressure_sets(),
            (0..PRESSURE.sets.len())
                // A set whose largest class the table leaves out holds no value
                // here (a class is described when a register of the target is
                // in it): its raw limit stands.
                .map(|set| {
                    if PRESSURE.class(&PRESSURE.sets[set].largest).is_some() {
                        M32::pressure_limit(set)
                    } else {
                        PRESSURE.sets[set].limit
                    }
                })
                .collect(),
        ))
    }

    fn machine(&self) -> Machine {
        Machine::parse(DOS32, TIMINGS.default_cpu().expect("timings.times states a default CPU"))
            .expect("the flat DOS description parses")
            .with_layout(layout())
    }

    fn flat_foreign(&self) -> llrm_target::Foreign {
        static FOREIGN: LazyLock<Vec<(i64, i64)>> = LazyLock::new(|| M32.machine().flat_foreign());
        llrm_target::Foreign(|| &FOREIGN)
    }

    fn flat_ports(&self) -> llrm_target::PortTable {
        static PORTS: LazyLock<Vec<llrm_target::machine::Port>> = LazyLock::new(|| M32.machine().ports);
        llrm_target::PortTable(|| &PORTS)
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    fn conventions(&self) -> &'static [&'static str] {
        &CONVENTIONS
    }

    fn calling(&self) -> &'static llrm_target::calling::Calling {
        &CALLING
    }

    fn physical_addresses(&self) -> Vec<(String, u64)> {
        llrm_x86::physical_addresses()
    }

    fn runtime(
        &self,
        language: &str,
    ) -> Option<llrm_target::runtime::Description> {
        match language {
            "nib" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/nib/x86-m32"),
                text: include_str!("../../../../runtime/nib/x86-m32/nib.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/nib/x86-m32/stack.toml"))],
            }),
            "c" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/c/x86-m32"),
                text: include_str!("../../../../runtime/c/x86-m32/c.toml"),
                files: &[("stack.toml", include_str!("../../../../runtime/c/x86-m32/stack.toml"))],
            }),
            _ => None,
        }
    }

    fn os_layer(&self) -> Option<llrm_target::os::Layer> {
        Some(llrm_target::os::Layer {
            directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/shared/dos/m32"),
            text: include_str!("../../../../runtime/shared/dos/m32/os.toml"),
            facts: llrm_x86::DOS_FACTS,
        })
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        LAYOUT.clone()
    }

    fn address_roles(&self) -> llrm_mir::spaces::Spaces {
        LAYOUT.spaces.roles
    }

    /// An argument takes a dword at least: `calling.toml`'s.
    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn stack_alignment(&self) -> i64 {
        CALLING.native().stack_alignment.unwrap_or(CALLING.native().slot_bytes)
    }

    fn frame_register(&self) -> RegId {
        llrm_x86::calling::frame(CALLING.native())
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_enter(&self) -> bool {
        CALLING.native().frame_enter
    }

    fn frame_slot_scratch_of(
        &self,
        convention: &llrm_target::calling::Convention,
    ) -> Option<(RegId, RegId)> {
        let (push, pop) = convention.frame_slot_scratch.as_ref()?;
        Some((llrm_x86::calling::register(push), llrm_x86::calling::register(pop)))
    }

    /// `GR32_NOSP`/`GR64_NOSP`'s caller-saved registers, in the pin's order.
    fn frame_slot_pop_order(&self) -> Vec<RegId> {
        vec![RegId::EAX, RegId::ECX, RegId::EDX]
    }

    fn frame_wrap_split_size(&self) -> bool {
        CALLING.native().frame_wrap_split_size
    }

    /// Past EBP and the return address; there is no far call.
    fn first_argument_offset(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::first_argument_offset(CALLING.native(), far)
    }

    /// Flat: no segments, one model.
    fn object(&self) -> llrm_target::object::ObjectFormat {
        static OBJECT: std::sync::LazyLock<llrm_target::object::ObjectFormat> = std::sync::LazyLock::new(|| {
            llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml"))
                .expect("flat object.toml parses")
        });
        OBJECT.clone()
    }

    fn return_address_bytes(
        &self,
        far: bool,
    ) -> i64 {
        llrm_x86::calling::return_address_bytes(CALLING.native(), far)
    }

    fn stack_pointer(&self) -> RegId {
        llrm_x86::calling::stack(CALLING.native())
    }

    fn string_roots(&self) -> Option<llrm_target::StringRoots> {
        Some(llrm_x86::registers::STRING_ROOTS)
    }

    fn x87_stack_top(&self) -> Option<RegId> {
        Some(llrm_x86::registers::X87_TOP)
    }

    /// What cdecl32 keeps, but the frame register.
    fn callee_saved(&self) -> Vec<(RegId, RegId)> {
        llrm_x86::calling::callee_saved(CALLING.native())
    }

    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        llrm_x86::calling::register(name)
    }

    fn convention_results(
        &self,
        convention: &llrm_target::calling::Convention,
        width: u32,
    ) -> Vec<RegId> {
        llrm_x86::calling::results(convention, width)
    }

    fn convention_callee_saved(
        &self,
        convention: &llrm_target::calling::Convention,
    ) -> Vec<(RegId, RegId)> {
        llrm_x86::calling::callee_saved(convention)
    }

    fn march(
        &self,
        name: &str,
    ) -> Option<&'static str> {
        TIMINGS.march(name)
    }

    fn marches(&self) -> Vec<&'static str> {
        TIMINGS.marches()
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    fn forms_text(&self) -> String {
        llrm_x86::instructions::joined(include_str!("instructions/x86.instr"))
    }

    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        llrm_x86::instructions::dialect()
    }

    fn operand_bytes(&self) -> i64 {
        4
    }

    fn huge_fill_routine(
        &self,
        cell: u32,
    ) -> Result<llrm_target::routines::HugeString, String> {
        llrm_x86::helpers::huge_fill(cell).map_err(|refused| refused.message)
    }

    fn huge_copy_routine(
        &self,
        down: bool,
    ) -> Result<llrm_target::routines::HugeString, String> {
        llrm_x86::helpers::huge_copy(down).map_err(|refused| refused.message)
    }

    fn divide_routine(
        &self,
        signed: bool,
        wide_divisor: bool,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide(signed, wide_divisor, 32).map_err(|refused| refused.message)
    }

    fn divide_routine_used(
        &self,
        signed: bool,
        wide_divisor: bool,
        used: llrm_target::routines::Used,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide_used(signed, wide_divisor, 32, used).map_err(|refused| refused.message)
    }

    fn fast_divide_in_loops_at_size(&self) -> bool {
        true
    }

    fn divide_routine_small(
        &self,
        signed: bool,
        wide_divisor: bool,
        used: llrm_target::routines::Used,
    ) -> Result<llrm_target::routines::Divide, String> {
        llrm_x86::helpers::divide_small(signed, wide_divisor, 32, used).map_err(|refused| refused.message)
    }

    fn float_width(
        &self,
        _: u32,
    ) -> u32 {
        llrm_x86::X87_WIDTH
    }

    fn move_bytes(&self) -> Option<llrm_target::MoveBytes> {
        Some(llrm_target::MoveBytes {
            plain: 4,
            copy: llrm_x86::encoding::copy_move_bytes,
            literal: llrm_x86::encoding::literal_move_bytes,
        })
    }

    fn register_op_bytes(
        &self,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::register_bytes(width, 4)
    }

    fn shift_op_bytes(
        &self,
        count: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::shift_bytes(count, width, 4)
    }

    fn multiply_immediate_bytes(
        &self,
        number: i64,
        width: i64,
    ) -> i64 {
        llrm_x86::encoding::imul_immediate_bytes(number, width, 4)
    }

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
    }

    /// X86ISelLowering.cpp: `setPrefFunctionAlignment(Align(16))`.
    fn pref_function_alignment(&self) -> u8 {
        4
    }

    /// DEVIATE SD543: no `rep` on `bsf`; the 386, 486 and Pentium have no
    /// `tzcnt`, so the pin's prefix is a dead byte. Owner m16. Exit 2026-11-20.
    fn bsf_rep_prefix(&self) -> bool {
        false
    }

    /// X86ISelLowering.cpp: `setPrefLoopAlignment(Align(16))`.
    fn pref_loop_alignment(&self) -> u8 {
        4
    }

    /// DEVIATE ALIGN-O1 (owner m32, exit 2026-11-27): no loop alignment at
    /// -O1, as gcc. `mandel` -O1 ran 5,376 one-byte nops against gcc's 0.
    fn align_loops_at_o1(&self) -> bool {
        false
    }

    /// DEVIATE SD545 (owner anyisa, exit 2026-11-20): the pin pads a loop
    /// header with up to 15 bytes; gcc's `.p2align 4,,10` pads at most 10.
    /// -O2 m32 `jpeg_single` ran 15 one-byte nops 14,457 times.
    fn loop_alignment_max_skip(&self) -> u8 {
        10
    }

    /// The i486 and Pentium have no `nopl` (`FeatureNOPL`): one byte.
    fn max_nop_bytes(&self) -> u8 {
        1
    }

    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<llrm_target::timings::CpuTable> {
        TIMINGS.cpu(name).cloned()
    }

    fn operation_costs(
        &self,
        price: &dyn Fn(&str) -> i64,
        prefix: i64,
    ) -> OperationCosts {
        OPCOSTS.operations(price, prefix)
    }

    fn register_capacity(&self) -> i64 {
        llrm_target::registers::allocatable(&REGISTERS) as i64
    }

    fn address_forms(
        &self,
        _: &OperationCosts,
        _: i64,
    ) -> Vec<AddressForm> {
        ADDRESS_FORMS.clone()
    }

    fn cost_model(&self) -> CostModel {
        |prices| {
            llrm_target::described_with(
                prices,
                Some(OPCOSTS.size_costs()),
                // Every CPU the target accepts (i486, pentium) prices alike:
                // the x87 tables.
                layout()
                    .tti
                    .zip(layout().llvm_cpu)
                    .and_then(|(tti, cpu)| llrm_x86::tti::X86::described(&tti, &cpu))
                    .map(|one| {
                        std::rc::Rc::new(
                            one.with_registers(prices.registers as u32).with_tuning(tuning(&prices.operations)),
                        ) as std::rc::Rc<dyn llrm_mir::tti::Tti>
                    }),
                Some(std::rc::Rc::new(llrm_x86::lowering::X86 { is_64bit: false })),
            )
        }
    }

    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId> {
        llrm_x86::calling::results(CALLING.native(), width)
    }
}

#[cfg(test)]
mod tests {
    /// #9101 narrowed every target's -O1 unroll trial to nests; only m32 was
    /// fit on that (a lone loop paid in 1 of 46 programs), so m32 says so
    /// itself.
    #[test]
    fn test_the_o1_unroll_trial_takes_nests_only() {
        let tuning = super::tuning(&llrm_mir::target::OperationCosts::default());
        assert!(tuning.unroll_trial_at_o1.is_some() && tuning.unroll_trial_nests_only);
    }

    /// -O3 copied a recursion to depth 8 with the loop first, gcc's setting,
    /// while -O2 copies to depth 2 with the tree first. Over 12 recursion
    /// programs -O3 was bigger by 5556 B and ran 1.12 M more instructions than
    /// -O2 (fib 131 K against 14 K, frames 2.69 M against 1.76 M, rectwo 232 K
    /// against 157 K). -O3 takes -O2's recursion policy.
    #[test]
    fn test_o3_copies_a_recursion_as_o2_does() {
        let tuning = super::tuning(&llrm_mir::target::OperationCosts::default());
        let depth = tuning.recursive_inlining_depth.expect("m32 copies recursions");
        assert_eq!(depth.1, depth.0, "recursive_inlining_depth (-O2, -O3)");
        let first = tuning.recursive_tre_first;
        assert_eq!(first.1, first.0, "recursive_tre_first (-O2, -O3)");
    }

    /// The GPRs were ordered eax ecx edx ebx esi edi, so the callee-saved ebx
    /// was the first of them taken; the pin's GR32 order
    /// (X86RegisterInfo.td) is eax ecx edx esi edi ebx.
    #[test]
    fn test_general_registers_are_allocated_in_the_pins_order() {
        let order: Vec<&str> = super::REGISTERS
            .iter()
            .map(|r| r.name.as_str())
            .filter(|n| ["eax", "ecx", "edx", "esi", "edi", "ebx"].contains(n))
            .collect();
        assert_eq!(order, ["eax", "ecx", "edx", "esi", "edi", "ebx"]);
    }

    /// The limits the pin's `computePSetLimit` gives the x86-32 sets with the
    /// six general registers allocatable, worked by hand from the table:
    /// set 35 (GR16) is 66 - 2 * (34 - 6) of its largest class,
    /// LOW32_ADDR_ACCESS_RBP; GR32_BPSP (ebp, esp) has none free, so its
    /// raw 6; GR32_BSI has both; GR8_NOREX has its eight; GR8 is 38 - (36 -
    /// 8). Without the reserved-register step every set read its raw limit
    /// and the scheduler saw 66 words of room.
    #[test]
    fn pressure_set_limits_come_off_the_reserved_registers() {
        let limits: Vec<(usize, u32)> =
            [35, 1, 3, 13, 26].iter().map(|&set| (set, super::M32::pressure_limit(set))).collect();
        assert_eq!(limits, [(35, 10), (1, 6), (3, 6), (13, 10), (26, 10)]);
    }

    /// `Target::pressure()` panicked ("the largest class of a set is
    /// described") on the sets whose largest class the table leaves out, so
    /// every m32 compile with a tracked region failed. It gives one limit
    /// per set.
    #[test]
    fn pressure_gives_a_limit_to_every_set() {
        use llrm_target::Target;
        let (sets, limits) = super::M32.pressure().expect("m32 has pressure sets");
        assert_eq!(limits.len(), sets.sets.len());
        assert_eq!(limits[35], 10);
    }

    /// `object()` and the runtime tables are parsed once and kept; each must
    /// equal a fresh parse of its toml (35 parses of object.toml and 4 of the
    /// runtime table cost 3.3 M Ir of every compile).
    #[test]
    fn test_cached_tomls_equal_a_fresh_parse() {
        use llrm_target::Target;
        let fresh = llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).unwrap();
        assert_eq!(M32.object(), fresh);
        assert_eq!(M32.object(), fresh, "a second ask");
        for language in ["c", "nib"] {
            let Some(description) = M32.runtime(language) else { continue };
            let fresh: toml::Table = description.text.parse().unwrap();
            assert_eq!(*description.table().unwrap(), fresh, "{language}");
            assert!(std::sync::Arc::ptr_eq(&description.table().unwrap(), &description.table().unwrap()));
        }
    }

    /// gcc's by-pieces ratios of a constant memset, read from the description:
    /// `i486_cost` and `pentium_cost` (x86-tune-costs.h, gcc 13.4.0), and the
    /// size table every CPU takes.
    /// Greedy allocates the x87 values into the RFP registers FP0..FP6, LLVM's
    /// seven (FP7 is the stackifier's scratch); they are no stack position,
    /// so the float stack stays eight deep.
    #[test]
    fn test_seven_rfp_registers_and_an_eight_deep_float_stack() {
        use llrm_target::Target;
        let file = llrm_target::registers::parse(&M32.registers_text()).unwrap();
        let rfp: Vec<&str> = file.iter().filter(|one| one.is("rfp")).map(|one| one.name.as_str()).collect();
        assert_eq!(rfp, ["mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6"]);
        assert!(file.iter().filter(|one| one.is("rfp")).all(|one| !one.is("x87") && !one.is("gpr")));
        assert_eq!(M32.float_stack(), 8);
    }

    #[test]
    fn test_the_memset_ratios_are_gccs_per_cpu() {
        let (i486, p5) = (TIMINGS.cpu("486").expect("a 486"), TIMINGS.cpu("P5").expect("a P5"));
        assert_eq!(
            (i486.set_ratio, i486.clear_ratio, p5.set_ratio, p5.clear_ratio),
            (Some(3), Some(3), Some(6), Some(6))
        );
        assert_eq!((i486.set_ratio_size, i486.clear_ratio_size), (Some(3), Some(2)));
        assert_eq!((p5.set_ratio_size, p5.clear_ratio_size), (Some(3), Some(2)));
    }

    /// Loop interchange prices a walk by the cache line: 16 bytes on the 486's
    /// 8 KB cache, 32 on the Pentium's (the 64 it would take for a CPU that
    /// states none made a 486 walk of 32 bytes cost as two lines).
    #[test]
    fn test_the_cache_line_is_the_cpus() {
        let (i486, p5) = (TIMINGS.cpu("486").expect("a 486"), TIMINGS.cpu("P5").expect("a P5"));
        assert_eq!((i486.cache_line, p5.cache_line), (Some(16), Some(32)));
    }

    const EAX: RegId = RegId::EAX;
    const EBP: RegId = RegId::EBP;
    const EBX: RegId = RegId::EBX;
    const ECX: RegId = RegId::ECX;
    const EDI: RegId = RegId::EDI;
    const EDX: RegId = RegId::EDX;
    const ESI: RegId = RegId::ESI;
    const ESP: RegId = RegId::ESP;
    use llrm_target::Target;

    use super::*;

    /// The flat target's machine answered every port with "may touch any
    /// memory" (the trait's default) though its platform lists the PC's
    /// devices, so a load of a cell before `in al, dx` was reloaded after it:
    /// the table is the platform's, whatever the CPU mode.
    #[test]
    fn test_m32_answers_whether_a_port_reaches_memory_from_its_platforms_table() {
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let model = (M32.cost_model())(&prices);
        assert!(!model.port_touches_memory((0x3DA, 0x3DA)), "the VGA status register");
        assert!(model.port_touches_memory((0x0B, 0x0B)), "the 8237's mode register may start a transfer");
        assert!(model.port_touches_memory((0x300, 0x300)), "a port the platform does not list");
    }

    /// CARRY-SI closed: carrying the coalescer's slot indexes through the
    /// scheduler (the pin's numbering) moved x86_32 -O1 14 cells +704 B, each
    /// still below the pin's own bytes (pindiff m32), so m32 takes it.
    #[test]
    fn test_m32_carries_the_slot_indexes() {
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let tti = (M32.cost_model())(&prices).tti().expect("m32 states its cost model");
        assert!(tti.carries_slot_indexes());
    }

    /// The pin's full-unroll thresholds made matmul 2188 B against gcc's 382 at
    /// m32 -O2 (tile 489 vs 161): m32 takes the size threshold, so a full
    /// unroll never grows code.
    #[test]
    fn test_m32_full_unroll_may_not_grow_the_code() {
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let tti = (M32.cost_model())(&prices).tti().expect("m32 states its cost model");
        assert_eq!(tti.unroll_threshold_overrides(), (None, None));
        // The x87 stack the unroll counts float values against.
        assert_eq!(tti.x87_stack_slots(), Some(8));
    }

    /// A code byte is priced by the CPU's own instruction-cache fill, never a
    /// literal: the 486 fills a 16 byte line in 5 clocks (312 milliclocks per
    /// byte), the Pentium a 32 byte line in 5 (156), and the price follows the
    /// sheet. Without the derivation m32 had 0 (no byte price) and its chains
    /// and pool constants could not be weighed against their bytes.
    #[test]
    fn test_a_code_byte_costs_the_cpus_line_fill_per_line_byte() {
        let rate = |cache_line, line_fill_clocks| {
            let prices = llrm_target::CpuPrices {
                costs: Vec::new(),
                prefix: 1,
                address_stall: 0,
                registers: 6,
                call_registers: 3,
                float_registers: 0,
                address_forms: Vec::new(),
                operations: OperationCosts { cache_line, line_fill_clocks, ..OperationCosts::default() },
                spaces: layout().spaces.roles,
                private: None,
                calling: None,
                foreign: llrm_target::Foreign::none(),
                ports: M32.flat_ports(),
            };
            (M32.cost_model())(&prices).tti().expect("m32 states its cost model").speed_milliclocks_per_byte()
        };
        let (i486, p5) = (TIMINGS.cpu("486").expect("a 486"), TIMINGS.cpu("P5").expect("a P5"));
        let sheet = |cpu: &llrm_target::timings::CpuTable| {
            rate(cpu.cache_line.expect("a line"), cpu.line_fill_clocks.expect("a fill"))
        };
        assert_eq!((sheet(i486), sheet(p5)), (312, 156));
        assert_eq!(rate(16, 10), 625, "a slower fill costs more per byte");
        assert_eq!(rate(0, 5), 0, "no cache line, no byte price");
    }

    /// The flat 32-bit target hands a pass that translates LLVM's the x87 X86
    /// cost model (`Machine::tti`): i486 and pentium price alike there.
    #[test]
    fn test_m32_states_llvms_cost_model_for_the_passes_that_translate_it() {
        use llrm_mir::tti::{ArithOp, Cost, CostKind, OperandInfo, Ty};
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M32.flat_ports(),
        };
        let tti = (M32.cost_model())(&prices).tti().expect("m32 states its cost model");
        let divide = tti.arithmetic_cost(
            ArithOp::SDiv,
            Ty::Int(32),
            CostKind::RecipThroughput,
            OperandInfo::ANY,
            OperandInfo::ANY,
        );
        assert_eq!(divide, Cost::of(25));
    }

    /// -Os on m32 priced in clocks: the description had no byte table, so
    /// `size_costs` was the clock costs and a pass asked what a hoisted
    /// float costs in bytes was told 3, a load's clocks, not the 2 bytes of its
    /// release.
    #[test]
    fn test_m32_prices_code_size_in_bytes() {
        let prices = llrm_target::CpuPrices {
            costs: vec![("mov_rm".into(), 1)],
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts { load: 1, ..Default::default() },
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: llrm_target::PortTable::none(),
        };
        let model = (M32.cost_model())(&prices);
        let sizes = model.size_costs();
        assert_eq!((sizes.load, sizes.float_release, sizes.call, sizes.r#move), (6, 2, 5, 4));
        assert_eq!(model.costs().load, 1, "the clock prices are the CPU's");
    }

    /// The x87 byte rows were a flat 6: a register `faddp` (2 B encoded) priced
    /// at 3x, and the memory operand of `fadd m` at 0 (`x87_add_m` =
    /// `x87_add`), so folding a load looked free at -Os. A register form is
    /// the integer register row.
    #[test]
    fn m32_x87_byte_rows_split_register_and_memory_forms() {
        let sizes = OPCOSTS.size_costs();
        // Mean encoded bytes of 71 float programs at -O2/-Os, per form, with
        // the band of estimate/measured the integer rows hold: 1.4-1.5
        // for memory, 2 for register.
        for (form, estimate, measured, band) in [
            ("register add", sizes.float_add, 2.00, 1.9..=2.1),
            ("fld m", sizes.float_load, 5.56, 1.3..=1.6),
            ("fstp m", sizes.float_store, 4.12, 1.3..=1.6),
            ("add m", sizes.float_add + sizes.float_load_folded, 5.87, 1.3..=1.6),
            ("fstp st", sizes.float_release, 2.00, 0.9..=1.1),
        ] {
            let ratio = estimate as f64 / measured;
            assert!(band.contains(&ratio), "{form}: {estimate} B estimated, {measured} B encoded");
        }
        assert_eq!(sizes.float_add, sizes.add, "a register x87 add is the integer register row");
        assert!(sizes.float_load_folded > 0, "a memory operand costs bytes over the register form");
    }

    /// A condition held as a value had no price: `phiopt` priced it as the
    /// `movzx` after it (3 clocks) and made x_switch 59% slower (45,159 clocks
    /// to 71,757) where `setcc` is 4 clocks on a 486 and 3 bytes.
    #[test]
    fn test_m32_prices_a_condition_held_as_a_value() {
        let table = M32.cpu_table("486").expect("the 486");
        let clocks = table.clocks.iter().find(|(form, _)| form == "setcc").map(|&(_, clocks)| clocks);
        assert_eq!(clocks, Some(4));
        let priced = OPCOSTS.operations(&|form| table.clocks.iter().find(|(one, _)| one == form).map_or(1, |p| p.1), 0);
        assert_eq!((priced.set, OPCOSTS.size_costs().set), (4, 3));
    }

    /// Flat code indexes by dwords natively: one form, no prefix, any scale.
    #[test]
    fn test_m32_has_one_native_dword_address_form() {
        let forms = M32.address_forms(&OperationCosts::default(), 0);
        assert_eq!(forms.len(), 1);
        assert!(
            !forms[0].secondary
                && forms[0].index_width == 4
                && forms[0].scales == std::collections::BTreeSet::from([1, 2, 4, 8])
        );
        let model = (M32.cost_model())(&llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: M32.register_capacity(),
            call_registers: M32.callee_saved().len() as i64,
            float_registers: 0,
            address_forms: forms.clone(),
            operations: OperationCosts::default(),
            spaces: layout().spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: llrm_target::PortTable::none(),
        });
        assert_eq!((model.registers(), model.call_registers()), (6, 5));
        assert_eq!(model.address_forms(), forms);
    }

    #[test]
    fn test_dos32_is_a_flat_machine_with_the_pc_ports() {
        let machine = M32.machine();
        assert_eq!(machine.addressing, llrm_target::machine::Addressing::Flat);
        assert!(machine.segments.is_none());
        assert!(!machine.ports.is_empty());
    }

    /// One address space of 32-bit pointers: `near` and `far` are space 0, an
    /// unmarked dword pointer is near, and the pair kinds are none.
    #[test]
    fn test_m32_layout_is_one_32_bit_space() {
        let layout = M32.layout();
        assert!(layout.datalayout.starts_with("e-p:32:32"));
        let spaces = layout.spaces;
        assert_eq!(
            (spaces.near, spaces.far, spaces.segment, spaces.huge, spaces.fixed, spaces.unmarked(4)),
            (0, 0, None, None, None, Ok(0))
        );
    }

    /// The register default comes first: the language's own functions use it,
    /// and `cdecl32` is named after it.
    #[test]
    fn test_calling_toml_states_watcall32_at_the_top_of_its_table() {
        let one = CALLING.native();
        assert_eq!(
            (one.name.as_str(), CONVENTIONS.as_slice()),
            ("watcall32", &["watcall32", "cdecl32", "fastcc32", "sysv32"][..])
        );
        assert_eq!(one.aggregate.as_ref().map(|one| one.style.as_str()), Some("hidden-pointer"));
        assert_eq!(
            (one.promotion.as_str(), one.wide_slots, one.results.keys().cloned().collect::<Vec<_>>()),
            ("slot", 2, vec!["1", "2", "4", "8", "float", "pointer"].into_iter().map(String::from).collect::<Vec<_>>())
        );
        assert_eq!(CALLING.by_cc("cdecl").map(|one| one.name.as_str()), Some("cdecl32"));
    }

    /// The aggregate return was marked provisional while the C ABI was open: it
    /// is Open Watcom's flat ABI with this one stated difference, and the
    /// description says so.
    #[test]
    fn test_the_c_abi_is_open_watcoms_with_one_stated_difference() {
        let text = include_str!("machines/calling.toml");
        assert!(!text.to_lowercase().contains("provisional"), "the aggregate rule is decided");
        assert!(
            text.contains("Open Watcom's 386 flat ABI")
                && text.contains("THE DIFFERENCE FROM OPEN WATCOM")
                && text.contains("aggregate = \"hidden-pointer\"")
        );
    }

    /// watcall32 (calling.toml): EBP and ESP frame, every register but EAX kept
    /// by a callee that is not given one in an argument, first stack
    /// argument at [ebp+8]; cdecl32 keeps ESI/EDI/EBX (the pin's `CSR_32`).
    #[test]
    fn test_m32_answers_watcall32() {
        let frame = M32.frame_registers();
        assert_eq!(
            (M32.object().bitness, M32.object().header),
            (32, vec![".386".to_owned(), ".model flat".to_owned()])
        );
        assert_eq!((frame.pointer, frame.stack), (EBP, ESP));
        assert_eq!(frame.saved, [(EBX, EBX), (ECX, ECX), (EDX, EDX), (ESI, ESI), (EDI, EDI)]);
        assert_eq!((M32.stack_slot_bytes(), M32.first_argument_offset(false)), (4, 8));
        // No far call: the return address is a dword either way.
        assert_eq!((M32.return_address_bytes(false), M32.return_address_bytes(true)), (4, 4));
        assert_eq!([4, 8].map(|width| M32.results(width)), [vec![EAX], vec![EAX, EDX]]);
        assert_eq!(
            llrm_x86::calling::callee_saved(CALLING.named("cdecl32").unwrap()),
            [(ESI, ESI), (EDI, EDI), (EBX, EBX)]
        );
    }

    /// Every convention that decorates a symbol for OMF says how for COFF: Open
    /// Watcom's own COFF objects (wcc386 -eoc) name `_c` for cdecl and `w_`
    /// for its register convention, as its OMF ones do. A convention with
    /// no `coff` entry leaves its COFF symbols undecorated.
    #[test]
    fn a_convention_that_decorates_for_omf_decorates_the_same_for_coff() {
        let names = CALLING.names();
        assert!(names.len() >= 2);
        for name in names {
            let symbol = &CALLING.named(name).unwrap().symbol;
            assert_eq!(symbol.get("coff"), symbol.get("omf"), "{name}");
        }
    }

    /// The debug numbers of the file: the i386 psABI's DWARF registers (eax 0
    /// .. edi 7, st0 at 11) and CodeView's (ebp 22, ax 9), checked against
    /// llvm's CodeViewRegisters.def. A view at lane 0 is read from its
    /// root's low bytes, so it has the root's DWARF number; one at lane 8 has
    /// none.
    #[test]
    fn the_register_file_numbers_each_debug_format() {
        let file = &*REGISTERS;
        let find = |name: &str| file.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("no {name}"));
        let numbers = |name: &str| (find(name).dwarf, find(name).codeview);
        assert_eq!(numbers("eax"), (Some(0), Some(17)));
        assert_eq!(numbers("ebp"), (Some(5), Some(22)));
        assert_eq!(numbers("esp"), (Some(4), Some(21)));
        assert_eq!(numbers("ax"), (Some(0), Some(9)));
        assert_eq!(numbers("ah"), (None, Some(5)));
        assert_eq!(numbers("st3"), (Some(14), Some(131)));
        assert!(
            file.iter().filter(|one| one.lane != 0 && one.name != one.root).all(|one| one.dwarf.is_none()),
            "a view past lane 0 has no DWARF number"
        );
    }
}
