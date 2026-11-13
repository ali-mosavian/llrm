//! What the shared mid-end asks of m68k: [`Machine`] and the
//! [`Tti`] its loop strength reduction prices addresses with.
//!
//! The pin's M68k overrides no `TargetTransformInfo` hook and no
//! `isLegalAddressingMode`, so it takes `BasicTTIImpl`'s defaults over
//! `TargetLoweringBase::isLegalAddressingMode` (a 16-bit offset, no global
//! base, `r+r` or `r+i` but not `r+r+i`, `2*r` only alone); this file states
//! those answers.
//!
//! # See also
//! `llvm/lib/CodeGen/TargetLoweringBase.cpp` (`isLegalAddressingMode`),
//! pin d1106deb71cc.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};
use llrm_mir::tti::{
    AddressingMode, ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty,
};

/// The m68k cost model the passes of the mid-end read.
#[derive(Clone, Copy, Debug, Default)]
pub struct M68kMachine {
    /// The processor the code is for: a fact of the target description, read
    /// by the cost model through [`M68k::cpu`].
    pub cpu: crate::dagisel::hooks::Cpu,
}

impl Machine for M68kMachine {
    fn foreign_span(
        &self,
        _selectors: (i64, i64),
        _offsets: (i64, i64),
        _width: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn tti(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(M68k::for_cpu(self.cpu)))
    }

    /// Frames keep what the shipped targets ask for (m68k Linux, AmigaOS and
    /// TOS: 4) and nothing realigns them (`frame.rs`): an object asking more
    /// is clamped to it, so the optimizer never folds `ptr & 7` to 0 on a
    /// frame that gives an address ending in 4.
    fn kept_stack_align(&self) -> Option<u64> {
        Some(crate::frame::SHIPPED_STACK_ALIGN as u64)
    }

    /// The datalayout gives `double` an 8-byte alignment and `i64` an 8-byte
    /// preferred one, so locals of those are accepted (and clamped to what the
    /// frame keeps). An object aligned further is kept: the frame realigns
    /// the stack pointer for it (`frame::Layout`).
    fn max_alloca_align(&self) -> Option<u64> {
        None
    }

    fn realigned_above(&self) -> Option<u64> {
        Some(crate::frame::REALIGN_ABOVE as u64)
    }

    /// DEVIATE M68K-ALLOCA-ALIGN4: a dynamic `alloca` of any alignment is
    /// masked into place by `DYNAMIC_STACKALLOC`, which is stated against the
    /// 4 the frames keep (the pin's `getStackAlignment` is 8, with frames that
    /// keep 8): an align-8 request is masked here where the pin's is not.
    /// Clamping it instead would hand back 4-aligned memory for
    /// `__builtin_alloca_with_align(n, 64)`. Owner m68k. Exit: when the
    /// frames keep 8, or 2026-11-30.
    fn honors_dynamic_alloca_align(&self) -> bool {
        true
    }

    fn lowering(&self) -> Option<Rc<dyn llrm_mir::lowering::Lowering>> {
        Some(Rc::new(crate::lowering::M68kLowering))
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    /// Code bytes per operation at this CPU ([`crate::opcosts`]); clocks are
    /// still one per operation.
    fn size_costs(&self) -> OperationCosts {
        crate::opcosts::size_costs(self.cpu).unwrap_or_else(|why| panic!("m68k operation sizes: {why}"))
    }

    /// `d0`..`d7`, `a0`..`a5`.
    fn registers(&self) -> i64 {
        14
    }

    /// `d2`..`d7`, `a2`..`a5`.
    fn call_registers(&self) -> i64 {
        10
    }

    /// Every arithmetic instruction names its destination as a source.
    fn two_address(&self) -> bool {
        true
    }

    /// `(d8, An, Xn)`: a base and an index of the same size, unscaled (the
    /// 68020's scale is a later subtarget).
    fn address_forms(&self) -> Vec<AddressForm> {
        let scales: BTreeSet<i64> = [1].into_iter().collect();
        vec![AddressForm::new(4, scales, 0, 0, 0, false, None).expect("a well-formed address form")]
    }
}

/// Whether `(d8,An,Xn)` is dearer than `(An)` on `cpu`, read from the timing
/// tables: clocks first, then the extension word's bytes at the CPU's price
/// (`timing::order`). On the 68000 it is 14 clk against 8 for a long.
pub fn index_is_dearer_than_plain(cpu: crate::dagisel::hooks::Cpu) -> bool {
    use crate::dagisel::hooks::Cpu;
    static DEARER: std::sync::LazyLock<[bool; 6]> = std::sync::LazyLock::new(|| {
        [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060].map(|cpu| {
            let price = |text: &str, bytes: u32| {
                let clocks = crate::timing::cost(cpu, text, bytes, false).expect("a priced move").clocks;
                crate::timing::order(cpu, false, clocks, bytes)
            };
            price("move.l (0,%a1,%d1), %d0", 4) > price("move.l (%a1), %d0", 2)
        })
    });
    DEARER[cpu as usize]
}

/// The m68k `TargetTransformInfo`: the base class's answers.
#[derive(Clone, Copy, Debug, Default)]
pub struct M68k {
    /// The processor the code is for; the 68000 where none is given.
    pub cpu: crate::dagisel::hooks::Cpu,
    /// The 68020's scaled index, `(d8, An, Xn*2/4/8)`; the 68000 has none.
    ///
    /// DEVIATE M68K-TTI-SCALED (owner m68k, review 2026-12-20): the pin's
    /// `isLegalAddressingMode` is the base class's, which allows `2*r` alone
    /// and no scale of 4 or 8
    /// ([TargetLoweringBase.cpp#L2139-L2171](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L2139-L2171));
    /// the 68020 addresses `(d8, An, Xn*2/4/8)` in one instruction. A hardware
    /// fact, so the exit is the pin giving M68k its own hook.
    pub scaled_index: bool,
    /// `CodeSize` in bytes of code, which the inliner counts a callee with
    /// ([`Tti::inline_size_view`]); the other passes' thresholds are in the
    /// pin's units, where it is false.
    pub code_size_bytes: bool,
    /// With `code_size_bytes`: the bytes are a speed caller's, where a
    /// quotient by a constant is the magic multiply, not the divide
    /// (`sizes::constant_division`).
    pub speed_lowering: bool,
}

impl M68k {
    /// The cost model of `cpu`.
    pub fn for_cpu(cpu: crate::dagisel::hooks::Cpu) -> M68k {
        M68k { cpu, ..M68k::default() }
    }

    /// The clocks of one instruction of `bytes` bytes.
    fn clocks(
        &self,
        text: &str,
        bytes: u32,
    ) -> Option<i32> {
        crate::timing::cost(self.cpu, text, bytes, false).ok().map(|one| i32::try_from(one.clocks).unwrap_or(0))
    }

    /// `(clocks, bytes)` of the cheapest way to write `op` with `constant` on a
    /// data register at `bits`, by clocks and then bytes.
    fn op_price(
        &self,
        op: llrm_mir::opcode::BinaryOp,
        bits: u32,
        constant: Option<u64>,
        source_dies: bool,
    ) -> Option<(i32, i32)> {
        use llrm_mir::opcode::BinaryOp;
        let size = size_letter(bits)?;
        let mnemonic = match op {
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Xor => "eor",
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Shl => "lsl",
            _ => return None,
        };
        let long = bits == 32;
        let mut forms: Vec<(i32, i32)> = Vec::new();
        let mut form = |text: String, bytes: i32| {
            if let Some(clocks) = self.clocks(&text, bytes as u32) {
                forms.push((clocks, bytes));
            }
        };
        let register =
            |m: &str| if m == "eor" { format!("eor.{size} %d1, %d0") } else { format!("{m}.{size} %d1, %d0") };
        match (op, constant) {
            (BinaryOp::Shl, Some(amount)) if (1..=8).contains(&amount) => form(format!("lsl.{size} #{amount}, %d0"), 2),
            (BinaryOp::Shl, _) => return None,
            (_, None) => form(register(mnemonic), 2),
            (_, Some(value)) => {
                let width = if long { u64::MAX >> 32 } else { (1u64 << bits) - 1 };
                let value = value & width;
                let small = value <= 8 || width - value < 8;
                // The immediate forms overwrite their operand: one that lives
                // on is copied first.
                let (copy_clocks, copy_bytes) =
                    if source_dies { (0, 0) } else { (self.clocks("move.l %d1, %d0", 2)?, 2) };
                if matches!(op, BinaryOp::Add | BinaryOp::Sub) && small {
                    if let Some(clocks) = self.clocks(&format!("addq.{size} #4, %d0"), 2) {
                        forms.push((clocks + copy_clocks, 2 + copy_bytes));
                    }
                }
                if let Some(clocks) = self.clocks(&format!("{mnemonic}.{size} #{value}, %d0"), if long { 6 } else { 4 })
                {
                    forms.push((clocks + copy_clocks, if long { 6 } else { 4 } + copy_bytes));
                }
                let signed = if long { value as u32 as i32 } else { 0 };
                if long && op != BinaryOp::Sub && (-128..=127).contains(&signed) {
                    // moveq + register form: the operation commutes
                    if let (Some(load), Some(clocks)) =
                        (self.clocks("moveq #1, %d1", 2), self.clocks(&register(mnemonic), 2))
                    {
                        forms.push((load + clocks, 4));
                    }
                }
            }
        }
        forms.into_iter().min()
    }
}

/// The size letter of a native integer width.
fn size_letter(bits: u32) -> Option<char> {
    match bits {
        8 => Some('b'),
        16 => Some('w'),
        32 => Some('l'),
        _ => None,
    }
}

impl Tti for M68k {
    /// The pin's M68k has no FP register class: `FADD` is not legal on `float`
    /// or `double`, so `getFPOpCost` is `TCC_Expensive` (every float
    /// operation is a soft-float call).
    fn fp_op_expensive(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Float | Ty::Double)
    }

    /// A pair of `i64` divide and remainder is one call (`__divmoddi4`) where
    /// the multiply back is not the 68020's `muls.l`: the pin's
    /// `DivRemPairs` would divide and then multiply the quotient by the divisor
    /// with two `__mulsi3` calls (DEVIATE M68K-DIVMOD64: the pin's M68k has no
    /// such routine; exit: none).
    fn has_div_rem_op(
        &self,
        bits: u32,
        _signed: bool,
    ) -> bool {
        bits == 64 && !self.cpu.has_wide_multiply()
    }

    /// DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; m68k joins it):
    /// `va_list` is `char *`, so `va_start` is a store of the first
    /// variadic argument's address and the list lives in a register, as gcc has
    /// it.
    fn promote_va_list(&self) -> bool {
        true
    }

    fn va_arg_slot(&self) -> Option<u32> {
        Some(crate::dagisel::lowering::VA_SLOT)
    }

    /// DEVIATE LFTR1 (owner w12, review 2026-12-20): any constant
    /// non-wrapping stride counts; the pin's step-one rule costs bytes here.
    fn lftr_any_stride(&self) -> bool {
        true
    }

    /// `BasicTTIImplBase::getTypeBasedIntrinsicInstrCost`, which the pin's M68k
    /// runs: legal 1, custom 2, expanded `SingleCallCost`, by the table the
    /// lowering itself
    /// uses ([`llrm_codegen::backend::selectiondag::intrinsic_cost`]). An
    /// intrinsic that translation does not reach keeps the trait's price.
    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        _: &[Ty],
        kind: CostKind,
    ) -> Cost {
        if let Some(Ty::Int(bits)) = ret {
            let lowering = lowering_of(self.cpu);
            // DEVIATE M68K-INTRINSIC-EXPANSION (owner m68k-68020, review
            // 2026-12-20): the pin prices an expansion at 1 under
            // `TCK_CodeSize` (one call); the loop unroller asks
            // that kind, so a `smul.fix.sat` that expands to dozens of
            // instructions read as one and the 3x3 loops of
            // `fixed_point` unrolled (-O2 1644 -> 3604 B at the 68020).
            // Here an expansion costs the same under every kind.
            let _ = kind;
            let code_size = false;
            if let Some(cost) =
                llrm_codegen::backend::selectiondag::intrinsic_cost::cost(&lowering, name, bits, code_size)
            {
                return Cost::of(cost);
            }
        }
        if llrm_mir::tti::is_free_intrinsic(name) {
            Cost::FREE
        } else {
            llrm_mir::tti::base_intrinsic_cost(name).unwrap_or(Cost::INVALID)
        }
    }

    fn opt_size_inline_byte_gate(&self) -> bool {
        true
    }

    /// DEVIATE NARROW-CYCLE (owner m68k, exit 2026-11-20): crc16's `crc` lives
    /// in a word and is carried in a long; a word `add` or `eor` is 4 clk
    /// against 8 on the 68000.
    fn narrows_demanded_cycles(&self) -> bool {
        true
    }

    /// `op` at each size as the target would write it: the register form, the
    /// immediate form, the `moveq` and register form of a long constant that
    /// fits it, `addq` and `lsl #n`; clocks from the timing tables.
    fn narrowing_gain(
        &self,
        op: llrm_mir::opcode::BinaryOp,
        wide_bits: u32,
        narrow_bits: u32,
        constant: Option<u64>,
        source_dies: bool,
    ) -> llrm_mir::tti::Saving {
        let gone = |bits| if bits == 0 { Some((0, 0)) } else { self.op_price(op, bits, constant, source_dies) };
        let (Some(wide), Some(narrow)) = (gone(wide_bits), gone(narrow_bits)) else {
            return llrm_mir::tti::Saving::default();
        };
        llrm_mir::tti::Saving { clocks: wide.0 - narrow.0, bytes: wide.1 - narrow.1 }
    }

    /// The `move.l` copy and `tst` of the old value that `cmpcarry` drops
    /// when a word or byte `add` doubles it and the carry is its top bit.
    fn shift_out_test_gain(
        &self,
        narrow_bits: u32,
    ) -> llrm_mir::tti::Saving {
        let Some(size) = size_letter(narrow_bits) else { return llrm_mir::tti::Saving::default() };
        let (Some(copy), Some(test)) = (self.clocks("move.l %d0, %d1", 2), self.clocks(&format!("tst.{size} %d1"), 2))
        else {
            return llrm_mir::tti::Saving::default();
        };
        llrm_mir::tti::Saving { clocks: copy + test, bytes: 4 }
    }

    /// `moveq #0, %d0; move.<s> %d1, %d0`.
    fn extension_cost(
        &self,
        narrow_bits: u32,
        wide_bits: u32,
    ) -> llrm_mir::tti::Saving {
        let (Some(size), 32) = (size_letter(narrow_bits), wide_bits) else { return llrm_mir::tti::Saving::default() };
        let (Some(zero), Some(copy)) =
            (self.clocks("moveq #0, %d0", 2), self.clocks(&format!("move.{size} %d1, %d0"), 2))
        else {
            return llrm_mir::tti::Saving::default();
        };
        llrm_mir::tti::Saving { clocks: zero + copy, bytes: 4 }
    }

    /// DEVIATE UNROLL-O1 (owner m68k, exit 2026-12-04, with `unroll_at_o1` in
    /// `targets.toml`): clang marks every loop `unroll.disable` at -O1, so a
    /// two-trip float loop stayed rolled at 170 B where gcc's complete unroll
    /// folds it to 16 (float_in_place_update). At -O1 a loop is unrolled where
    /// the copies cost no more than the rolled loop after the folding the
    /// analysis simulates (gcc's size-neutral complete unroll); a call the
    /// copies keep is priced as one instruction, where the pin's analysis gives
    /// up on it.
    fn unroll_never_grows_at_o1(&self) -> bool {
        true
    }

    fn unroll_o1_prices_calls(&self) -> bool {
        true
    }

    /// `UnrollingPreferences::Threshold` of the full unroll, 32 where the pin's
    /// is 300: an iteration's loop control is about 24 clk on the 68000 (an
    /// `addq`, a `cmp` and a taken `bcc`), 6 B at 4 clk/B, so a copy of the
    /// body pays for itself only when the body is a couple of instructions.
    /// The 400% boost stays, which keeps a hot small loop unrolled (crc16
    /// -O2: 3894 against 5264 clk). 300 seeds -O2 68020: 669352 -> 516950 B,
    /// the 24 programs unchanged. DEVIATE: the pin's target sets none. Exit:
    /// review 2026-11-20.
    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (Some(32), Some(400))
    }

    /// DEVIATE M68K-INLINE-RATE (owner m68k, exit 2026-11-27): an inline into a
    /// speed caller (-O2, -O3) that grows the code is refused when its bytes at
    /// the CPU's `timings.times` rate cost more clocks than the call saves at
    /// the site's frequency; the rate is the one the O2/O3 verdicts are judged
    /// at (0.5 B per clk on the 68000, 1.33 on the 68020, 3.2 from the 68030).
    /// The pin inlines to its threshold (225) alone; the flat 50 this replaces
    /// kept a loop's call to a 100-instruction callee (a call per trip).
    /// No exemption for a callee loop with a constant bound: it admitted
    /// growth with no gain (`switch`, `sieve` 68020).
    fn speed_inline_milliclocks_per_byte(&self) -> i64 {
        crate::timing::byte_milliclocks(self.cpu)
    }

    /// A `select` is a branch: `M68kISD::CMOV` is expanded to a diamond
    /// (`EmitLoweredSelect`); the 68000 family has no conditional move. The
    /// speculation of a branch into selects then takes the limit of targets
    /// without one (condtocond D3).
    fn has_cmov(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "M68k"
    }

    fn call_sequence_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        Some(crate::sizes::call_sequence(self.cpu, shape, kind))
    }

    fn callee_frame_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        crate::sizes::callee_frame(self.cpu, shape, kind)
    }

    /// A size build refuses an inline of a callee that stays when it grows
    /// the code (SZ1): the callee's bytes against the call sequence's.
    fn opt_size_inline_by_code_size(&self) -> bool {
        true
    }

    /// DEVIATE INL-O1-TRIAL-M68K (owner m68k, exit 2026-11-24): at -O1 a callee
    /// the growth bound refused is tried in the caller and kept where the
    /// caller comes out smaller (uint64_to_float: `classify` x6 folds `main`
    /// to constants, 368 B and 1514 B of routines against clang's 92 B).
    fn inline_trials_at_o1(&self) -> bool {
        true
    }

    /// -O1 walks the call graph a second time once the dead globals are gone,
    /// as m32 does: `bar` stored a sum nothing reads, ends as a `ret`, and
    /// `foo` drops its call (byvalue_struct_address -O1, 24 B above clang).
    fn inline_second_round_at_o1(&self) -> bool {
        true
    }

    /// A switch lowers to a compare and a branch a cluster (`cmpi.l #imm` 6 B,
    /// `bcc.w` 4 B; the peeled `pick` of switch_peeled_table: 7 of them), or a
    /// range check, the dispatch and a word a case.
    fn switch_code_size(
        &self,
        clusters: u64,
        table: u64,
    ) -> i64 {
        if !self.code_size_bytes {
            return 0;
        }
        if table > 0 { 18 + 2 * table as i64 } else { 10 * clusters as i64 }
    }

    /// DEVIATE SZ2-TRIPS (owner m68k, exit 2026-11-24): the callee's loop runs
    /// its trips in the clocks the priced trial weighs against the copy's
    /// bytes (recsum -O2: the constant trip count makes the `%` unsigned,
    /// 24,009 clk the once-through count never saw).
    fn inline_trial_by_trips(&self) -> bool {
        true
    }

    /// DEVIATE NRW-1 (`narrowrange`, owner m68k, exit 2026-12-05): where the
    /// CPU has no 32-bit divide instruction (68000, 68010) the divide is a
    /// libcall, and `divu.w` (16 bits) is one instruction.
    fn range_narrowing_width(&self) -> Option<u32> {
        let has_long_divide = self.cpu.at_least(crate::dagisel::hooks::Cpu::M68020);
        (!has_long_divide).then_some(16)
    }

    /// DEVIATE UNROLL-OS-TRIAL (owner m68k, exit 2026-11-24): the unrolled size
    /// up to which a loop the -Os threshold refused is unrolled on trial; the
    /// driver keeps it where the finished functions are smaller (as x86-m32's
    /// 30). The pin never unrolls at -Os past its threshold.
    fn unroll_trial_at_size(&self) -> Option<u32> {
        Some(100)
    }

    /// DEVIATE UNROLL-O1-TRIAL (owner m68k, exit 2026-11-24): the same bar at
    /// -O1, where the pin does not unroll past its threshold and gcc does:
    /// the driver keeps the unrolled module where its finished functions are
    /// no larger and no slower (float_in_memory_loop: 16 `fadd`s on a constant
    /// fold to one, 100 B and 1264 B of soft-float routines against gcc's 70
    /// B).
    fn unroll_trial_threshold(&self) -> Option<u32> {
        Some(100)
    }

    /// DEVIATE UNROLL-NEST-TRIAL (owner m68k, exit 2026-11-27): the -O2 and -O3
    /// trial that unrolls, by the pin's threshold, the loops with a subloop
    /// the threshold override (32) refused: an outer loop over a
    /// constant table folds its loads (shift_int64_variable -O2: 328 B and
    /// 248818 clk to 56 B and 10016), a bigger one costs bytes the clocks
    /// do not repay (fcmp_all +5.5 KB), so the finished code decides.
    fn unroll_trial_nests_at_speed(&self) -> bool {
        true
    }

    /// DEVIATE PEEL-OS-TRIAL: the same trial peels (`loopunrollpass`).
    fn peels_on_trial_at_size(&self) -> bool {
        true
    }

    /// The inliner counts a callee in bytes of code (`sizes`).
    fn inline_size_view(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(M68k { code_size_bytes: true, ..*self }))
    }

    fn inline_size_view_at(
        &self,
        speed: bool,
    ) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(M68k { code_size_bytes: true, speed_lowering: speed, ..*self }))
    }

    /// `TargetLoweringBase::isLegalAddressingMode`.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        if base_offset <= -(1 << 16) || base_offset >= (1 << 16) - 1 || base_gv {
            return false;
        }
        match scale {
            0 => true,
            1 => !(has_base_reg && base_offset != 0),
            2 if !self.scaled_index => !(has_base_reg || base_offset != 0),
            2 | 4 | 8 => self.scaled_index,
            _ => false,
        }
    }

    /// `getScalingFactorCost`: a second register in the address is one where
    /// the tables say it is dearer than a plain address
    /// ([`index_is_dearer_than_plain`]), else nothing; an illegal mode is
    /// invalid. The pin's M68k has no override (the base class prices every
    /// legal mode 0); x86 and arm64 state the rule
    /// ([X86TargetTransformInfo.cpp#L8045](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L8045)).
    /// DEVIATE M68K-LSR-INDEX (owner m68k-lsr-addressing, exit none: the
    /// table is the cause): LSR keeps a pointer where an index needs as many
    /// registers. Closes M68K-LSR-IDX-CLOCKS.
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) {
            Cost::of(i64::from(scale != 0 && index_is_dearer_than_plain(self.cpu)))
        } else {
            Cost::INVALID
        }
    }

    /// The per-target value of `SCEVCheapExpansionBudget`: one operation. A
    /// limit that costs a single operation (`n - 1`) is repaid by the counter
    /// that goes (`subq`+`bne` for `addq`+`cmp`+`blt`); one that costs more
    /// (`smax(n, 1)`) is not. Measured at -Os on the matrix at 0, 1 and the
    /// pin's 4: 1 shrinks partition and memset (4 and 2 B) and changes no
    /// other cell; 4 grows arrmax, arrsum, copy, countgt, fib and sum by 6 to
    /// 14 B. The same value serves every level, so no `optsize` split.
    /// DEVIATE (value; owner m68k, exit none: a per-target table value; the
    /// pin's budget is 4).
    fn lftr_expansion_budget(
        &self,
        _optsize: bool,
    ) -> u32 {
        1
    }

    /// DEVIATE M68K-LSR-FRAME (owner m68k-lsr-addressing, exit none while the
    /// backend folds `(d8,%sp,Xn)`, `frameindexed.rs`): a stack slot is `%sp`
    /// and a displacement, so as the base of an address it takes no register.
    /// The pin counts every `alloca` as one (`llc -debug-only=loop-reduce`,
    /// d1106deb: 7 regs for 4 stack arrays and 3 IVs). Its M68k has no TTI, so
    /// the default registers-first order keeps one pointer IV per array (6
    /// regs); its x86 shares one index (`reg(%a) + 1*reg({0,+,4})`) because
    /// `X86TTIImpl::isLSRCostLess` ranks instructions first (4 against 7,
    /// [X86TargetTransformInfo.cpp#L7202](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7202)),
    /// not because the slot is free.
    ///
    /// Rows, owner m68k-lsr-addressing, exit 2026-11-30, fix a general
    /// index-versus-pointer cost via Tti (no per-program patch):
    /// - M68K-LSR-IDX-CLOCKS: `(d8,An,Xn)` is 14 clk against 8 for `(An)`. O1
    ///   cells leaving the bar (clocks, executed): bintree 68000 +1784
    ///   (+0.06%), 68020 +894 (+0.18%), each for -4 B; frameless O1 68000 +68
    ///   (+0.12%), 68020 +10 (+0.05%), each for -2 B. O2/Os frameless cost the
    ///   same clocks.
    /// - M68K-LSR-D2U64-BYTES (owner m68k-lsr-addressing and w12):
    ///   pin-faithful. The pin's LSR picks the single IV with a compare for
    ///   double_to_uint64's loops (`llc -print-after=loop-reduce`); our earlier
    ///   two-IV end-biased form (LG3) was smaller: +10 B at 68000 O1/O2/Os,
    ///   68020 O1 +8, O2 +26, Os +10, executed clocks equal. 68000 -Os: 304 B
    ///   against gcc 330 and clang 386, so below the bar. Closes through the
    ///   LSR size model (the exit compare's `moveq`+`cmp` is in `insns`, which
    ///   the registers-first order ignores).
    fn frame_address_is_free(&self) -> bool {
        true
    }

    /// DEVIATE: the pin's M68k states no preference; `(An)+` steps a pointer
    /// by the access size for free (descriptors in `indexed.rs`), so LSR keeps
    /// a pointer walk rather than an index beside a counter. Exit: 2026-11-20,
    /// when LSR rates the indexed formula beside the walk (M68K-COPY-OS).
    fn preferred_addressing_mode(&self) -> AddressingMode {
        AddressingMode::PostIndexed
    }

    fn is_post_inc_load_legal(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(8 | 16 | 32) | Ty::Pointer)
    }

    fn is_post_inc_store_legal(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(8 | 16 | 32) | Ty::Pointer)
    }

    /// `(An)+` steps by the access size: a walk by 1, 2 or 4 bytes, upward.
    fn is_post_inc_step_legal(
        &self,
        access_bytes: i64,
        step: i64,
    ) -> bool {
        step == access_bytes
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::cmp_sel(op, ty);
        }
        // `BasicTTIImplBase::getCmpSelInstrCost`: a legal operation costs 1
        // times the type-legalization overhead, `LT.first`: the registers the
        // type splits into (an `i64` or a `double` two, an `i128` four).
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1454
        let registers = match ty {
            Ty::Int(bits) => bits.div_ceil(32).max(1),
            Ty::Double => 2,
            Ty::Float | Ty::X87 | Ty::Pointer => 1,
        };
        Basic.cmp_sel_cost(op, ty, kind) * i64::from(registers)
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        indexed: bool,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::memory(op, ty, indexed);
        }
        Basic.memory_op_cost(op, ty, kind, indexed)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::control_flow(phi);
        }
        Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: ImmUser,
        operand: u32,
        value: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost_inst(user, operand, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        Basic.is_truncate_free(from, to)
    }

    fn cast_cost(
        &self,
        op: CastKind,
        to: Ty,
        from: Ty,
        kind: CostKind,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::cast(op, to, from);
        }
        Basic.cast_cost(op, to, from, kind)
    }

    /// DEVIATE 20: each select a two-entry phi would become costs the branch
    /// and move it lowers to, in bytes as in clocks (a fold saves one branch
    /// and adds a move per select). gcd -O2 44 -> 26 B, 236 -> 138 clk; -Os
    /// 44 -> 26 B. Exit 2026-11-12.
    fn select_branch_extra(
        &self,
        _optsize: bool,
    ) -> i64 {
        1
    }

    fn sizes_exit_values(&self) -> bool {
        true
    }

    /// DEVIATE 19: `addi`/`subi`/`addq` and `cmpi` take any 32-bit immediate;
    /// the pin's M68k lowering keeps the base class's `false`. Exit
    /// 2026-11-12.
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// DEVIATE 17: a size build keeps a store or copy loop (a `memset` call
    /// is three pushes, `jsr` and `lea`: 16 B, the loop 10 to 14). Exit
    /// 2026-11-12.
    fn memory_idioms_at_optsize(&self) -> bool {
        false
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        left: OperandInfo,
        right: OperandInfo,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::arithmetic(
                op,
                ty,
                right,
                self.cpu.at_least(crate::dagisel::hooks::Cpu::M68020),
                self.speed_lowering,
            );
        }
        // A quotient by a constant is the magic multiply the DAG makes of it:
        // its clocks, not one operation's (SZ2-TRIPS: the unsigned and the
        // signed differ by the sign's three instructions a trip).
        if kind == CostKind::Latency
            && let Some(clocks) = crate::sizes::constant_division_clocks(
                self.cpu,
                op,
                ty,
                right,
                self.cpu.at_least(crate::dagisel::hooks::Cpu::M68020),
            )
        {
            return Cost::of(clocks);
        }
        // DEVIATE: the pin's base class prices a multiply or divide it expands
        // at one operation (`BasicTTIImpl::getArithmeticInstrCost`'s last
        // line). Past 32 bits the m68k makes a library call of it, and the
        // closed form of a sum (`(n-1)(n-2)/2` in `i33`) is then 250 B of
        // `__mulsi3` calls for a loop of 16. Only the call is priced so: a
        // multiply `ExpandIntRes_MUL` makes of 32-bit `UMUL_LOHI`s
        // (`mulu.l`, 68020), or `visitMUL` makes of shifts and an add, is the
        // pin's one operation. Exit: none, a target's prices.
        if matches!(
            op,
            ArithOp::Mul | ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
        )
            && matches!(ty, Ty::Int(bits) if bits > 32)
            && !(op == ArithOp::Mul && self.multiply_makes_no_call(left, right))
        {
            return Cost::EXPENSIVE;
        }
        Basic.arithmetic_cost(op, ty, kind, left, right)
    }
}

impl M68k {
    /// Whether the lowering writes a wide multiply of `left` by `right` with no
    /// library call: from 32-bit `UMUL_LOHI`s when the target has them
    /// (`ExpandIntRes_MUL`), else only a power of two, its negation, or a
    /// constant whose word multiplies are all chains
    /// ([`Self::constant_product_is_chains`]).
    fn multiply_makes_no_call(
        &self,
        left: OperandInfo,
        right: OperandInfo,
    ) -> bool {
        use llrm_codegen::backend::selectiondag::{
            Action,
            isd::ISD,
            mvt::{EVT, MVT},
        };
        let wide = lowering_of(self.cpu).operation_action(ISD::UMulLoHi, EVT::Simple(MVT::I32));
        if matches!(wide, Action::Legal | Action::Custom | Action::Promote) {
            return true;
        }
        [left, right]
            .into_iter()
            .any(
                |operand| operand.power_of_two
                    || operand.negated_power_of_two
                    || operand
                        .value
                        .and_then(|value| i64::try_from(value).ok())
                        .is_some_and(|value| self.constant_product_is_chains(value)),
            )
    }
}

impl M68k {
    /// Whether `x * value` in 64 bits is shifts and adds only. The expansion
    /// (`forceExpandMultiply`) multiplies each 32-bit word of `x` by the
    /// constant's low word, its two 16-bit halves and its high word; a
    /// multiplier with no priced chain (DEVIATE G2, `dagisel::synth`) is a
    /// `__mulsi3` call. Priced at speed: a chain's clocks count in the loops
    /// the unroller prices, so the call is taken only where none exists.
    fn constant_product_is_chains(
        &self,
        value: i64,
    ) -> bool {
        let (low, high) = (value as u32, (value >> 32) as u32);
        [low, low & 0xFFFF, low >> 16, high].into_iter().all(|word| {
            word < 2
                || word.is_power_of_two()
                || word == u32::MAX
                || crate::dagisel::synth::plan(self.cpu, crate::timing::Price::Rate, 32, u128::from(word)).is_some()
        })
    }
}

/// The CPU's SelectionDAG lowering, built once per thread.
fn lowering_of(cpu: crate::dagisel::hooks::Cpu) -> Rc<llrm_codegen::backend::selectiondag::TargetLowering> {
    thread_local! {
        static LOWERING: std::cell::RefCell<Vec<(crate::dagisel::hooks::Cpu, Rc<llrm_codegen::backend::selectiondag::TargetLowering>)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    LOWERING.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, one)) = cache.iter().find(|(known, _)| *known == cpu) {
            return one.clone();
        }
        let made = Rc::new(crate::dagisel::lowering::target_lowering_with(cpu, true));
        cache.push((cpu, made.clone()));
        made
    })
}

#[cfg(test)]
mod lftr_tests {
    use super::*;
    /// DEVIATE LFTR1: following the pin's step-one rule left byte-loops of
    /// stride 2..8 unreduced and cost bytes; the knob must stay on here.
    #[test]
    fn lftr_counts_any_constant_stride() {
        assert!(M68k::default().lftr_any_stride());
    }
}
