//! The x86-64 target: flat, one 64-bit pointer, System V AMD64, ELF.
//! Its descriptions are the files beside this one; this answers what they
//! cannot say.

include!(concat!(env!("OUT_DIR"), "/effects.rs"));
include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

use std::sync::LazyLock;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use llrm_target::CostModel;
use llrm_target::machine::Machine;

/// A Linux user program: flat, no ports.
pub const LINUX64: &str = include_str!("../../../../machine/platform/linux/flat64.toml");

/// The processors flat code is priced for: the columns of `timings.times`.
pub static CPUS: LazyLock<Vec<&'static str>> = LazyLock::new(|| TIMINGS.cpus());

/// The data layout and address spaces: `machines/datalayout.toml`.
pub const DATALAYOUT_TOML: &str = include_str!("machines/datalayout.toml");

/// `DATALAYOUT_TOML`, parsed once.
static LAYOUT: LazyLock<llrm_target::layout::Layout> =
    LazyLock::new(|| llrm_target::layout::Layout::parse(DATALAYOUT_TOML).expect("x86-64 datalayout.toml parses"));

/// The calling conventions: `calling.toml`.
static CALLING: LazyLock<llrm_target::calling::Calling> = LazyLock::new(|| {
    llrm_target::calling::Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses")
});

/// Their names, the first being the language's own.
static CONVENTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| CALLING.names());

/// The x86-64 target's layout.
pub fn layout() -> llrm_target::layout::Layout {
    LAYOUT.clone()
}

/// The x86-64 target as `llrm-driver` names it.
pub struct M64;

/// The registers and the address forms the descriptions state.
static REGISTERS: LazyLock<std::sync::Arc<Vec<llrm_target::registers::Register>>> =
    LazyLock::new(|| llrm_target::registers::file(include_str!("registers.regs")).expect("registers.regs parses"));

/// How flat operations are priced from the instruction forms: `opcosts.txt`.
static OPCOSTS: LazyLock<llrm_target::opcosts::Description> = LazyLock::new(|| {
    llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses")
});

/// DEVIATE THROUGHPUT (owner x86-m64, exit 2026-12-20): the x86-64 column's processor resources
/// (`machine/tune/x86/x86-64.toml` `[schedule]`, the pin's SandyBridgeModel) and latencies, each operation the
/// sum of its forms as the clock table is. A form the schedule does not list occupies no resource.
static THROUGHPUT: LazyLock<llrm_mir::target::Throughput> = LazyLock::new(|| {
    let table = TIMINGS.cpu("x86-64").expect("timings.times states the x86-64 column");
    let find = |rows: &[(String, i64)], form: &str| rows.iter().find(|(one, _)| one == form).map(|(_, value)| *value);
    let latency = |form: &str| 1000 * find(&table.latency, form).or_else(|| find(&table.clocks, form)).unwrap_or(1);
    let text = llrm_target::tune::X86.iter().find(|(name, _)| *name == "x86-64").map(|(_, text)| *text).expect("the x86-64 tune file");
    llrm_target::schedule::Schedule::parse(text).expect("x86-64.toml's [schedule] parses").throughput(
        table.issue,
        OPCOSTS.operations(&latency, 0),
        &|price| OPCOSTS.operations(price, 0),
    )
});

/// The CPUs' timings: `timings.times`.
static TIMINGS: LazyLock<llrm_target::timings::Timings> = LazyLock::new(|| {
    llrm_target::timings::Timings::parse_tuned(include_str!("timings.times"), llrm_target::tune::X86)
        .expect("timings.times parses")
});

static ADDRESS_FORMS: LazyLock<Vec<AddressForm>> = LazyLock::new(|| {
    llrm_target::addressing::forms(include_str!("machines/datalayout.toml")).expect("datalayout.toml parses")
});

impl llrm_target::Target for M64 {
    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &llrm_x86::mc::X86Objects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        llrm_x86::mc::fixup::debug_target(64)
    }

    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        llrm_x86::instructions::dialect()
    }

    /// SSE holds a float at its own size.
    fn float_width(
        &self,
        bytes: u32,
    ) -> u32 {
        bytes
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

    fn register_named(
        &self,
        name: &str,
    ) -> llrm_lir::registers::RegId {
        llrm_x86::calling::register(name)
    }

    fn convention_results(
        &self,
        convention: &llrm_target::calling::Convention,
        width: u32,
    ) -> Vec<llrm_lir::registers::RegId> {
        llrm_x86::calling::results(convention, width)
    }

    fn convention_callee_saved(
        &self,
        convention: &llrm_target::calling::Convention,
    ) -> Vec<(llrm_lir::registers::RegId, llrm_lir::registers::RegId)> {
        llrm_x86::calling::callee_saved(convention)
    }

    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn selects_by_dag(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "x86-m64"
    }

    fn machine(&self) -> Machine {
        Machine::parse(LINUX64, TIMINGS.default_cpu().expect("timings.times states a default CPU"))
            .expect("the Linux description parses")
            .with_layout(layout())
    }

    fn flat_foreign(&self) -> llrm_target::Foreign {
        static FOREIGN: LazyLock<Vec<(i64, i64)>> = LazyLock::new(|| M64.machine().flat_foreign());
        llrm_target::Foreign(|| &FOREIGN)
    }

    fn flat_ports(&self) -> llrm_target::PortTable {
        static PORTS: LazyLock<Vec<llrm_target::machine::Port>> = LazyLock::new(|| M64.machine().ports);
        llrm_target::PortTable(|| &PORTS)
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    fn conventions(&self) -> &'static [&'static str] {
        &CONVENTIONS
    }

    fn runtime(
        &self,
        language: &str,
    ) -> Option<llrm_target::runtime::Description> {
        match language {
            "c" => Some(llrm_target::runtime::Description {
                directory: concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/hosted/x86-m64"),
                text: include_str!("../../../../runtime/hosted/x86-m64/c.toml"),
                files: &[],
            }),
            _ => None,
        }
    }

    fn calling(&self) -> &'static llrm_target::calling::Calling {
        &CALLING
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

    fn aligned_calls(&self) -> bool {
        CALLING.native().aligned_calls
    }

    fn stack_alignment(&self) -> i64 {
        CALLING.native().stack_alignment.unwrap_or(CALLING.native().slot_bytes)
    }

    fn frame_register(&self) -> RegId {
        llrm_x86::calling::frame(CALLING.native())
    }

    fn callee_pops(&self) -> bool {
        CALLING.conventions.iter().any(|one| one.cleanup == llrm_target::calling::Cleanup::Callee)
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_saved_first(&self) -> bool {
        CALLING.native().frame_saved_first
    }

    fn frame_enter(&self) -> bool {
        CALLING.native().frame_enter
    }

    fn frame_slot_scratch(&self) -> Option<(RegId, RegId)> {
        let (push, pop) = CALLING.native().frame_slot_scratch.as_ref()?;
        Some((llrm_x86::calling::register(push), llrm_x86::calling::register(pop)))
    }

    /// `GR32_NOSP`/`GR64_NOSP`'s caller-saved registers, in the pin's order.
    fn frame_slot_pop_order(&self) -> Vec<RegId> {
        vec![RegId::RAX, RegId::RCX, RegId::RDX, RegId::RSI, RegId::RDI, RegId::R8, RegId::R9, RegId::R10, RegId::R11]
    }

    fn frame_wrap_split_size(&self) -> bool {
        CALLING.native().frame_wrap_split_size
    }

    fn frame_object_aligned(&self) -> bool {
        CALLING.native().frame_object_aligned
    }

    fn frame_red_zone(&self) -> i64 {
        CALLING.native().frame_red_zone
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
                .expect("x86-64 object.toml parses")
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

    fn operand_bytes(&self) -> i64 {
        8
    }

    fn huge_fill_routine(
        &self,
        _cell: u32,
    ) -> Result<llrm_target::routines::HugeString, String> {
        Err("long mode has no huge pointers".to_owned())
    }

    fn huge_copy_routine(
        &self,
        _down: bool,
    ) -> Result<llrm_target::routines::HugeString, String> {
        Err("long mode has no huge pointers".to_owned())
    }

    fn divide_routine(
        &self,
        _signed: bool,
        _wide_divisor: bool,
    ) -> Result<llrm_target::routines::Divide, String> {
        Err("long mode divides 64 bits natively".to_owned())
    }

    fn default_cpu(&self) -> &'static str {
        TIMINGS.default_cpu().expect("timings.times states a default CPU")
    }

    /// X86ISelLowering.cpp: `setPrefFunctionAlignment(Align(16))`.
    fn pref_function_alignment(&self) -> u8 {
        4
    }

    /// X86ISelLowering.cpp: `setPrefLoopAlignment(Align(16))`.
    fn pref_loop_alignment(&self) -> u8 {
        4
    }

    /// DEVIATE SD545 (owner anyisa, exit 2026-11-20): the pin pads a loop
    /// header with up to 15 bytes; gcc's `.p2align 4,,10` pads at most 10.
    /// -O2 m32 `jpeg_single` ran 15 one-byte nops 14,457 times.
    fn loop_alignment_max_skip(&self) -> u8 {
        10
    }

    /// Every x86-64 has `nopl` (`FeatureNOPL`): X86AsmBackend uses up to ten
    /// bytes.
    fn max_nop_bytes(&self) -> u8 {
        10
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
        // SSE2 is the baseline: a constant memset is stores of an xmm register.
        OperationCosts { memset_store: 16, ext_tsp_for_size: true, ..OPCOSTS.operations(price, prefix) }
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
            llrm_target::described_throughput(
                prices,
                Some(OPCOSTS.size_costs()),
                llrm_x86::tti::X86::of_cpu("x86-64")
                    .map(|one| {
                        one.with_tuning(llrm_mir::target::Tuning {
                            inline_size_guard_at_size: true,
                            // DEVIATE LFTR-SIZE (owner llrm-target-x86-m64, exit 2026-11-24; gcc-bar only): see
                            // Tuning. m64 -Os 22 files smaller (-189 B), sieve +1 cell, spill_store_order +16 B.
                            lftr_size_budget: 1,
                            // DEVIATE OS-INLINE-GROWTH (owner llrm-target-x86-m64, exit 2026-11-24; gcc-bar only): see
                            // Tuning. m64 -Os sfm16_float_* -39 B each, vla_callee_constant +8 B.
                            inline_trial_defers_to_shared_growth: true,
                            // DEVIATE IPRA-DSO-LOCAL (owner llrm-target-x86-m64, exit 2026-11-24): see Tuning.
                            ipra_binds_local: true,
                            // DEVIATE SD443 (owner w9, exit: a constant-argument pass after
                            // inlining, as gcc's IPA-CP): see the m32 entry; m64 -O2
                            // memory_expansion.c text -3281 B.
                            second_ipsccp: true,
                            // DEVIATE SD-DIVMAG (owner llrm-target-x86-m64, exit 2026-11-24): the m32
                            // entry at m64; see `Tuning::sdiv_by_magnitude`.
                            sdiv_by_magnitude: true,
                            // DEVIATE GVN-PRE-OPTSIZE, MERGEFUNC-OS, REP-MOVS-OS (owner llrm-target-x86-m64,
                            // exit 2026-11-24): m32's -Os entries, see their `Tuning` fields. 20 -Os cells
                            // of the corpus shrink, none grows.
                            load_pre_off_at_size: true,
                            merge_functions_at_size: true,
                            rep_movs_at_size: true,
                            rep_movs_at_size_max: u64::MAX,
                            // DEVIATE IU1 (owner llrm-target-x86-m64, exit 2026-11-24): see
                            // `Tti::lsr_users_in_use_list_order`.
                            lsr_users_in_use_list_order: true,
                            // DEVIATE IU1 (owner llrm-target-x86-m64, exit 2026-11-24): see
                            // `Tti::lsr_phi_users_first`.
                            lsr_phi_users_first: true,
                            lsr_size_order: true,
                            lsr_size_x86_64: true,
                            // DEVIATE MUL-OS64 (owner w9, exit 2026-11-20): the m32 MUL-OS entry
                            // at m64; gcc keeps the 3-byte `imul` at -Os where the pin makes
                            // `mov; shl; sub` (x * 31). select_of_constants -Os was +1519 B
                            // against the old selector.
                            imul_at_size: true,
                            rep_stos_at_size: true,
                            rep_stos_at_size_max: u64::MAX,
                            // DEVIATE REP-STOS-UNIT (owner llrm-target-x86-m64, exit 2026-11-30): see
                            // `Tuning::rep_stos_priced_unit`. gcc fills a frame object with `rep stosb`.
                            rep_stos_priced_unit: true,
                            // DEVIATE UNROLL-OS-TRIAL (owner llrm-target-x86-m64, exit 2026-11-24): the
                            // m32 entry, see `Tuning::unroll_trial_at_size`. gcc completely peels a
                            // small loop at -Os when the code does not grow.
                            unroll_trial_at_size: Some(30),
                            unroll_trial_bytes_strict: true,
                            // DEVIATE UNROLL-O1, UNROLL-O1-TRIAL (owner llrm-target-x86-m64, exit 2026-11-24): the
                            // m32 entries, see `Tuning::unroll_never_grows_at_o1`, `unroll_trial_at_o1`.
                            unroll_never_grows_at_o1: true,
                            unroll_trial_at_o1: Some(150),
                            // DEVIATE CARRY-SI (owner x86-m64, exit 2026-11-21): see Tuning.
                            carry_slot_indexes: false,
                            // DEVIATE RU-REC (owner x86-m64, exit 2026-11-20): see Tuning.
                            unroll_recurrence_veto: true,
                            // DEVIATE SZ2 (owner m64, exit 2026-11-19): the bound holds a
                            // callee with one site too; the pin copies it (crc, floats,
                            // tile -Os: gcc keeps the call, +40 B each).
                            inline_size_guard_single_site: true,
                            // DEVIATE UP1-REG at m64 (owner m64, exit 2026-11-20): promoted
                            // arrays that exceed the registers are not free (fpbench -O2
                            // +815 B for -75 clocks). The byte rate follows the clock pricing.
                            // DEVIATE UP1 at m64 (owner m64, exit 2026-11-20, with
                            // x86-m32's): a full unroll repays its bytes at 312
                            // milliclocks a byte, the m64 net rate (fpbench -O2:
                            // 6-trip fp nests, +815 B for -75 clocks, gcc keeps them).
                            unroll_milliclocks_per_byte: 312,
                            unroll_prices_nest: true,
                            unroll_promotion_fits_registers: true,
                            // `mov m64, imm32`: sign-extended.
                            unroll_store_immediate_bits: 32,
                            rotation_guard_must_fold_at_size: true,
                            // DEVIATE SELOPT-C (owner m64-b, exit 2026-11-24): see
                            // `selectoptimize::SelectOptimizePass`.
                            branch_constant_arm_selects: true,
                            // DEVIATE INL-O2 RATE at m64 (owner x86-m64, exit 2026-11-24), as x86-m32's:
                            // a speed caller's inline pays the callee's bytes at the m64 net rate
                            // (312 milliclocks a byte); gcc's `ipa-inline` takes a copy only where its
                            // growth is small (shellsort -O2: main took a second 255 B copy of a
                            // function that stays). -O1 and -Os unchanged.
                            speed_inline_milliclocks_per_byte: 312,
                            inline_trial_keeps_shrinking_copy: true,
                            // DEVIATE INL-O1-TRIAL-M64 (owner llrm-target-x86-m64, exit 2026-11-24): see Tuning.
                            inline_trials_at_o1: true,
                            inline_chain_trial: true,
                            // DEVIATE SD103-M64 (owner x86-m64, exit 2026-11-24), as x86-m32's SD103-M32:
                            // the pin's cost rule, with the price of a merged `a && b` it omits taken
                            // from the table: two `setcc`, a logic op and a `test`, less the branch
                            // saved. sieve `i <= 31 && i*i < n` merged is `setb; setb; and; cmp 1; jne`.
                            merged_condition_clocks: 2 * prices.operations.set + 2 * prices.operations.add
                                - prices.operations.branch,
                            // DEVIATE ROTATION-TRIAL at m64 (owner x86-m64, exit 2026-12-05, as m32's
                            // #9141): -Os rotates a counted loop whose copied guard does not fold, then
                            // compiles the module again with those loops unrotated (gcc's `-Os` shape)
                            // and keeps the smaller object.
                            rotation_trial_at_size: true,
                            // DEVIATE ROTATION-HOT at m64 (owner x86-m64, exit 2026-12-05), as m32's: an
                            // uncounted loop RS1 leaves is also tried rotated, as the pin rotates it, and
                            // kept where the finished code executes less. -Os is strict on bytes: only
                            // where the object does not grow.
                            rotation_hot_at_size: true,
                            rotation_hot_bytes_per_loop: 0,
                            // No byte/clock rate is stated for m64 (oversight
                            // 2026-11-03): a constant multiply is decomposed
                            // only where the chain is no larger than
                            // the `imul`.
                            price_pin_multiplies: true,
                            // DEVIATE LFTR1 (owner w12, review 2026-12-20): see `Tuning::lftr_any_stride`.
                            lftr_any_stride: true,
                            // DEVIATE REC-2, as m32
                            recursive_inlining: Some((8, 450)),
                            // gcc's 15/30 size units in `gcc_insns` (m64 counts the sext/gep of its IR).
                            recursive_edge_growth: Some((16, 31)),
                            // DEVIATE REC-2, as m32: -O2 takes gcc's 450 (fib 25941 -> 10793
                            // clk, frames -20%, rectwo -12%; -O3 stays at 150). Owner m64,
                            // exit 2026-11-20.
                            recursive_inlining_growth: Some((450, 150)),
                            // DEVIATE REC-7 (owner m64-clocks, exit 2026-11-30), as m32: copies
                            // are 2 deep at -O2 and -O3, not gcc's 8.
                            recursive_inlining_depth: Some((2, 2)),
                            // DEVIATE REC-2-LOSERS (owner m64, exit 2026-11-21): the copies cost
                            // net (clk + 0.312 B) against none on recchop -O3 +1020 (2194 B),
                            // bintree -O3 +134 (1160 B, 0 clk), recpow -O2 +4002 (peeled). Their
                            // bodies are load chains the call overhead hides under (call share
                            // 52%, 45%); fib/frames/rectwo are 83-93% and win 42k-250k.
                            // A call-share bar costs queens -O3 -2492 (share 24%); the fix is a
                            // trial copy priced by the sheet's latencies.
                            // DEVIATE REC5-1 (owner w2, exit 2026-11-20), as m32.
                            ipsccp_widen_keeps_sign: true,
                            ipsccp_widen_keeps_sign_at_size: true,
                            // DEVIATE UJ2 (owner m64, exit 2026-11-14): BasicTTIImpl's
                            // 28-micro-op loop buffer gave the jam of an outer loop
                            // of that size a count of 0 (particle -O3: 15850 cycles,
                            // gcc's jam 10262); the full-unroll threshold instead.
                            unroll_jam_partial_threshold: Some(300),
                            // DEVIATE UJ3 (owner m64, exit 2026-11-20): the pin never jams
                            // on X86; ours takes an exact divisor of a known outer trip
                            // count (particle -O3: count 3 of 100 left a nest copy,
                            // +433 B, +8.6% clk).
                            unroll_jam_exact_count: true,
                            // CH-STAGED (owner m64, exit 2026-11-20): the pin hoists
                            // constants after LSR on every target; m64 first.
                            constant_hoisting: true,
                            // DEVIATE ZIMM (owner m64, exit 2026-11-27): see Tuning.
                            zext_imm_is_basic: true,
                            // DEVIATE MEM-OS (owner anyisa, exit 2026-11-14): see Tuning.
                            // A piece is a load and a store at `[reg+disp8]` (`movups` and a qword 4 + 4, a
                            // dword and a byte 3 + 3, a word 4 + 4); the call is `mov edx, n` 5 and the
                            // jump 5, 14 with the setup of a frame object's pointers.
                            size_copy_price: Some(llrm_mir::target::CopyPrice { call: 14, wide_piece: 8, narrow_piece: 6 }),
                            // DEVIATE IMM-1 (owner greedy-os-causes, exit 2026-11-20): see Tuning.
                            immediate_forms_at_size: true,
                            // DEVIATE IPACP-O2 (owner w10, exit 2026-11-24): see Tuning.
                            ipa_cp_clones_at_speed: true,
                            ..llrm_mir::target::Tuning::PIN
                        })
                    })
                    .map(|one| std::rc::Rc::new(one) as std::rc::Rc<dyn llrm_mir::tti::Tti>),
                Some(std::rc::Rc::new(llrm_x86::lowering::X86 { is_64bit: true })),
                Some(THROUGHPUT.clone()),
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
    /// CARRY-SI: carrying the coalescer's slot indexes through the scheduler
    /// moved x86 cells worse at -O1 (2026-11-07), so m64 numbers the function
    /// afresh until it has priced them.
    #[test]
    fn test_m64_does_not_carry_the_slot_indexes_yet() {
        let layout = M64.layout();
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout.spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M64.flat_ports(),
        };
        let tti = (M64.cost_model())(&prices).tti().expect("m64 states its cost model");
        assert!(!tti.carries_slot_indexes());
    }

    /// REC-7 at m64: recursion copies went 8 deep (gcc's default), so rectwo -O2
    /// ran 719563 instructions in 2120 B where depth 2 runs 161513 in 392 B.
    #[test]
    fn test_m64_copies_a_recursion_two_deep_at_o2_and_o3() {
        let layout = M64.layout();
        let prices = llrm_target::CpuPrices {
            costs: Vec::new(),
            prefix: 1,
            address_stall: 0,
            registers: 6,
            call_registers: 3,
            float_registers: 0,
            address_forms: Vec::new(),
            operations: OperationCosts::default(),
            spaces: layout.spaces.roles,
            private: None,
            calling: None,
            foreign: llrm_target::Foreign::none(),
            ports: M64.flat_ports(),
        };
        let tti = (M64.cost_model())(&prices).tti().expect("m64 states its cost model");
        assert_eq!(tti.recursive_inlining_depth(), Some((2, 2)));
    }

    /// The m64 sheet had a cache line and no line fill, so no byte rate was
    /// derivable at m64 and the borrowed 312 had no number to be compared with.
    /// Zen 5 fills a 64 B line from L2 in 10 clk: 156 mclk/B.
    #[test]
    fn test_the_sheet_states_the_measured_line_fill_and_derives_its_byte_rate() {
        let cpu = super::TIMINGS.cpu("x86-64").expect("the default CPU");
        assert_eq!((cpu.cache_line, cpu.line_fill_clocks), (Some(64), Some(10)));
        assert_eq!(cpu.byte_rate(), Some(156));
    }

    /// x86-64 described no x87 registers, so an x87 value (a QB `fatan`, a
    /// `long double`) had no register class and the DAG path refused it.
    #[test]
    fn test_the_x87_stack_and_its_seven_rfp_registers_are_described() {
        use llrm_target::Target;
        let file = llrm_target::registers::parse(&M64.registers_text()).unwrap();
        let named = |class: &str| -> Vec<&str> {
            file.iter().filter(|one| one.is(class)).map(|one| one.name.as_str()).collect()
        };
        assert_eq!(named("x87"), ["st0", "st1", "st2", "st3", "st4", "st5", "st6", "st7"]);
        assert_eq!(named("rfp"), ["mm0", "mm1", "mm2", "mm3", "mm4", "mm5", "mm6"]);
        assert_eq!(M64.float_stack(), 8);
    }

    /// `object()` and the runtime tables are parsed once and kept; each must
    /// equal a fresh parse of its toml (35 parses of object.toml and 4 of the
    /// runtime table cost 3.3 M Ir of every compile).
    #[test]
    fn test_cached_tomls_equal_a_fresh_parse() {
        use llrm_target::Target;
        let fresh = llrm_target::object::ObjectFormat::parse(include_str!("machines/object.toml")).unwrap();
        assert_eq!(M64.object(), fresh);
        assert_eq!(M64.object(), fresh, "a second ask");
        for language in ["c", "nib"] {
            let Some(description) = M64.runtime(language) else { continue };
            let fresh: toml::Table = description.text.parse().unwrap();
            assert_eq!(*description.table().unwrap(), fresh, "{language}");
            assert!(std::sync::Arc::ptr_eq(&description.table().unwrap(), &description.table().unwrap()));
        }
    }

    use llrm_target::Target;

    use super::*;

    /// The roles the description names: RBP frames, RSP is the stack, and SysV
    /// keeps rbx, r12-r15 (rbp is the frame).
    #[test]
    fn the_register_file_names_sysv_amd64s_roles() {
        assert_eq!((M64.frame_register(), M64.stack_pointer()), (RegId::RBP, RegId::RSP));
        // Each is the root its family is named by and the quadword a prologue
        // pushes.
        let (roots, pushed): (Vec<RegId>, Vec<RegId>) = M64.callee_saved().into_iter().unzip();
        assert_eq!(roots, [RegId::EBX, RegId::R12D, RegId::R13D, RegId::R14D, RegId::R15D]);
        assert_eq!(pushed, [RegId::RBX, RegId::R12, RegId::R13, RegId::R14, RegId::R15]);
        assert_eq!((M64.stack_slot_bytes(), M64.stack_alignment(), M64.return_address_bytes(false)), (8, 16, 8));
        assert_eq!(M64.first_argument_offset(false), 16);
    }

    /// The pointer is a quadword and the layout string is the pin's clang's for
    /// x86_64-linux-gnu.
    #[test]
    fn the_layout_is_the_pins_clangs() {
        let layout = M64.layout();
        assert_eq!(layout.mode, 64);
        assert_eq!(
            layout.datalayout,
            "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
        );
    }

    /// A `double` argument went to the stack (floats were not placed in
    /// registers): it must take XMM0-7 beside the integer registers, and a
    /// ninth leave on the stack.
    #[test]
    fn sysv_places_floats_in_xmm_registers_and_returns_one_in_xmm0() {
        use llrm_target::calling::{Kind, Place};
        let convention = M64.calling().named("sysv64").unwrap();
        let mut arguments = vec![Kind::Word, Kind::Float(8), Kind::Float(8), Kind::Word];
        arguments.extend([Kind::Float(8); 7]);
        let placement = convention.place(&arguments);
        let register = |name: &str| Place::Registers(vec![name.to_owned()]);
        assert_eq!(placement.places[..4], [register("rdi"), register("xmm0"), register("xmm1"), register("rsi")]);
        assert_eq!(placement.places[10], Place::Stack(0));
        assert_eq!(convention.float_result_register(), Some("xmm0"));
        assert!(convention.clobbered.iter().any(|one| one == "xmm15"));
    }
}
