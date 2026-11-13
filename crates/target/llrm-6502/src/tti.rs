//! The 6502's cost model: what an instruction costs in byte-wide operations.
//!
//! The unit is one byte-wide ALU operation (`clc; lda; adc; sta`: 7 bytes, 11
//! clocks). A type is `ceil(bits / native)` of them, the native width read from
//! the data layout (`n8`) as `BasicTTIImplBase::getTypeLegalizationCost` splits
//! a type until it is legal.
//!
//! DEVIATE: `BasicTTIImplBase` scales only `RecipThroughput` and prices an
//! expanded scalar at 1; here every kind scales (as `X86TTIImpl` and
//! `AArch64TTIImpl` do) and a multiply, divide or variable shift is the call it
//! emits. Reason: `SimplifyCFG`'s speculation budget asks `SizeAndLatency`, and
//! a 16-bit `add` or `icmp` is two byte operations here, not one. Exit: none;
//! the numbers below are the target's.
//!
//! Derived by hand from the emitted code (`llrm-cc --target=6502-sim65 -O2
//! -S`), unit = i8 add:
//!
//! | operation | bytes, clocks | units |
//! |---|---|---|
//! | add/sub/and/or/xor, i8 / i16 | 7 / 13, 11 / 22 | 1 / 2 |
//! | icmp, i8 / i16 (`lda; cmp` / `lda; cmp; lda; sbc`) | 4 / 8 | 1 / 2 |
//! | select, i8 / i16 (`lda #1; bcc; lda #0` per byte) | 7 / 13 | parts + 1 |
//! | load or store through a pointer, i8 / i16 (`ldy #n; lda (rc),y` per byte, one copy) | 4 / 13, 7 / 22 | 1 / 2 (measured 1 / 3) |
//! | shift by a constant (`asl; rol` pairs, per bit) | 4 per bit at i16 | parts |
//! | multiply by a constant (shifts and adds), i16 | 28, 70 | 3 * parts |
//! | `__mulqi3`, `__mulhi3` (8 / 16 iterations of about 25 / 45 clocks) | 200, 700 | 10 * parts^2 + 10 |
//! | `__udivhi3` (16 iterations of about 60 clocks) | 960 | 22 * parts^2 + 3 |
//! | `__ashlhi3`, `__lshrhi3`, `__ashrhi3` (a loop of about 16 clocks per bit, 8 on average) | 130 | 6 * parts |
//! | a call as code (`jsr`, two operands in `rc2..`) | 3 + 4 * parts | 2 + parts |
//!
//! A memory operation is `parts`, as `BasicTTIImplBase::getMemoryOpCost` prices
//! it, not the measured 2 * parts - 1: with 3 for a word load, `SimplifyCFG`'s
//! two-entry-phi budget (4) refused to flatten `c >= '0' && c <= '9'`
//! (compare 2 + load 3), and `InstCombine` never saw its range check (isdigit
//! 28 -> 50 bytes, isalpha 30 -> 74). The speed-priority form of such a limit
//! belongs in the per-target override table.
//!
//! A trunc is free (a register is a byte); a zero extension is the new high
//! bytes cleared, a sign extension twice that.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::tti::*;
use llrm_target::Target as _;

use crate::target::Mos;

/// The most the unrolled copy of a loop may cost, in the units above. A trip
/// of a loop costs its counter add, its compare and its branch beyond the body
/// (one unit each); unrolling pays for itself while the whole copy is no more
/// than a few trips of that control.
const FULL_UNROLL_UNITS: u32 = 24;

/// The cost model of the 6502 and 65C02.
#[derive(Clone, Copy, Debug)]
pub struct Mos6502 {
    /// The native integer, in bits (`n8`).
    byte_bits: u32,
    /// The pointer, in bits (`p:16`).
    pointer_bits: u32,
    /// `CostKind::CodeSize` is bytes of code, not byte-wide operations: the
    /// view the inliner counts a callee in ([`Tti::inline_size_view`]). The
    /// other passes' limits are in the units above.
    code_size_bytes: bool,
}

impl Mos6502 {
    pub fn new() -> Mos6502 {
        let layout = DataLayout::parse(&Mos.layout().datalayout).expect("the 6502's datalayout parses");
        Mos6502 {
            byte_bits: layout.largest_legal_integer(),
            pointer_bits: layout.pointer(0).bits,
            code_size_bytes: false,
        }
    }

    /// The byte-wide operations a value of `ty` takes.
    fn parts(
        &self,
        ty: Ty,
    ) -> i64 {
        let bits = match ty {
            Ty::Int(bits) => bits,
            Ty::Pointer => self.pointer_bits,
            Ty::Float => 32,
            Ty::Double => 64,
            Ty::X87 => 80,
        };
        i64::from(bits.div_ceil(self.byte_bits).max(1))
    }

    /// Bytes of code of `units` byte-wide operations of one kind, where the
    /// view counts bytes: `price(parts)` for the operation, `units` otherwise.
    fn code(
        &self,
        kind: CostKind,
        units: i64,
        price: impl FnOnce() -> i64,
    ) -> Cost {
        if self.code_size_bytes && kind == CostKind::CodeSize { Cost::of(price()) } else { Cost::of(units) }
    }

    /// A call to a runtime routine: `jsr` and the operands in `rc2..` (4 bytes
    /// a byte-wide part), or past a word the long form through `__sa` and
    /// `__sb` (measured, `x * C` of 32 bits: 59 bytes).
    fn routine_bytes(parts: i64) -> i64 {
        if parts <= 2 { 3 + 4 * parts } else { 14 * parts + 3 }
    }

    /// A call to a runtime routine whose loop costs `clocks` units.
    fn call(
        &self,
        parts: i64,
        kind: CostKind,
        clocks: i64,
    ) -> Cost {
        match kind {
            CostKind::CodeSize if self.code_size_bytes => Cost::of(Self::routine_bytes(parts)),
            CostKind::CodeSize | CostKind::SizeAndLatency => Cost::of(2 + parts),
            CostKind::RecipThroughput | CostKind::Latency => Cost::of(clocks),
        }
    }
}

impl Default for Mos6502 {
    fn default() -> Mos6502 {
        Mos6502::new()
    }
}

impl Tti for Mos6502 {
    fn name(&self) -> &'static str {
        "mos6502"
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let parts = self.parts(ty);
        match op {
            // `lda; cmp` per byte (4 B; 11 B a byte past a word, which is a
            // chain of byte compares and the booleans that join them)
            CmpSel::ICmp => self.code(kind, parts, || if parts <= 2 { 4 * parts } else { 11 * parts }),
            // `lda #1; bcc; lda #0` per byte
            CmpSel::Select => self.code(kind, parts + 1, || 6 * parts + 1),
            // soft float: a call
            CmpSel::FCmp => self.call(parts, kind, 10 * parts * parts + 10),
        }
    }

    // The 6502 has no conditional move: `selects.rs` expands every select to a
    // branch.
    /// `getUnrollingPreferences`: full unrolling only. The pin's default
    /// threshold (300) unrolls an 8-trip bit loop to 222 bytes where the
    /// rolled loop is 88; partial and runtime unrolling stay off
    /// (`loop_micro_op_buffer_size` is 0). At `-Os` the pin's zero
    /// `OptSizeThreshold` lets a loop go only where the unrolled body costs
    /// nothing more than the rolled one.
    fn get_unrolling_preferences(
        &self,
        _has_calls: bool,
        up: &mut UnrollingPreferences,
    ) {
        up.threshold = FULL_UNROLL_UNITS;
    }

    /// DEVIATE 6502-UNROLL-O1 (owner llrm-target-6502, exit 2026-11-27, with
    /// the rows' `unroll_at_o1` and x86-m32's UNROLL-O1): at -O1, which is
    /// strict on bytes, a full unroll is made only where the copies cost no
    /// more than the rolled loop, by size or by the cost the analysis finds
    /// once the trip's constants fold. clang marks every loop
    /// `unroll.disable` at -O1, so 6502 -O1 kept float_in_place_update's
    /// two-trip loop (9441 B against 19 B at -Os); the bare
    /// `-funroll-loops` grew byval_struct +178 B, frameless +27 B,
    /// loop_fusion +10 B and shrank double_to_int64 -1466 B.
    fn unroll_never_grows_at_o1(&self) -> bool {
        true
    }

    /// 6502-UNROLL-O1: no peeling at -O1. `-funroll-loops` turned it on there
    /// and known_loads_walks grew +4 B (6502) and +47 B (65C02) for -94k
    /// clocks; -O1 is strict on bytes.
    fn peels_at_o1(&self) -> bool {
        false
    }

    /// SZ1: at -Os an inline that stays must not grow the code. The pin's
    /// `OptSizeThreshold` 50 inlines a 27-instruction `chrget` into nine
    /// callers, 84 bytes each against a 3-byte call.
    fn opt_size_inline_by_code_size(&self) -> bool {
        true
    }

    /// `function_bytes` is calibrated against the objects
    /// (tests/size_calibration.rs).
    fn opt_size_inline_trial_by_bytes(&self) -> bool {
        true
    }

    fn opt_size_inline_guard_yields_to_negative_cost(&self) -> bool {
        true
    }

    // Rule 2 (user 11-04, a byte of growth must buy 3 clocks at -O2/-O3) is
    // off: `speed_inline_milliclocks_per_byte` stays 0. DEVIATE 6502-RATE:
    // the frequency the rule weighs with caps a loop at about 32 trips,
    // where div_constants_long runs 1500, so it priced 6.8 KB for 4.05M
    // clocks (600 clk/B) and tail_calls +12.6k clocks for 944 B. Owner:
    // llrm-target-6502. Exit: a frequency that knows the trip count,
    // 2026-11-20.

    /// A full unroll at -O2/-O3 must save 3 clocks for each code byte it adds
    /// (the 6502 rate, `DEVIATE 6502-RATE`), on the real clock and byte tables
    /// (`crate::opcosts`, `DEVIATE 6502-OPCOSTS`): the unit prices it used
    /// before read a 32-bit store fill as a few instructions (220 B for 46
    /// clocks, byval_struct). Owner: llrm-target-6502. Exit: 2026-11-20.
    fn unroll_milliclocks_per_byte(&self) -> i64 {
        3000
    }

    /// DEVIATE UNROLL-TRIAL (owner llrm-target-6502, exit 2026-12-05): the
    /// pin's threshold of 300 counts IR instructions, about 4 a trip of a
    /// small loop; this target counts 15 units a trip (`LLRM_DEBUG=unroll`:
    /// `trips 64, size 15`), so the pin's reach is 300 * 15 / 4 = about 1100
    /// here. A loop under it that the threshold refused is unrolled on trial
    /// and kept if the simplified function pays (loop_fusion's `clear`, 64
    /// trips of 15). Evidence: raising the threshold alone to 1100 changed 37
    /// programs, 5 for the worse on both axes (div_constants_ulong +15,986 B).
    fn unroll_trial_threshold(&self) -> Option<u32> {
        Some(1100)
    }

    /// DEVIATE OS-INLINE-TRIAL (owner 6502-c, exit 2026-12-05): the byte model
    /// is within 25% of the written bytes for two functions in three; a
    /// module over 512 instructions takes seconds to compile once
    /// (div_constants_long -Os: 3.0 s, 13.0 s with the trial).
    fn opt_size_inline_trial_limits(&self) -> Option<(i64, usize)> {
        Some((25, 512))
    }

    /// A 32-bit counter or product is four byte adds; the datalayout's `n8`
    /// made LSR leave all of them (shrink_wrapped_frame: `n * i` a 32-bit
    /// multiply per trip, 56k of its 64k clocks). Not 16: with it the
    /// switch_* and byval_inline cells grew on both axes (DEVIATE LSR-N,
    /// exit 2026-11-20).
    ///
    /// 16 since 2026-11-10: `n8` made LSR leave every `p16` pointer and index,
    /// so `g[i]` over 54-byte rows called `__mulhi3` each trip (rep_count at
    /// -O1: 147,011 clocks, 133,095 now). The wider LSR needs the cost model
    /// below (`is_lsr_cost_less`, the immediates, the drop hooks); without it
    /// switch_*, function_table and byval_inline grew. Cells still above the
    /// tip: row 6502-LSR-WORSE (owner 6502-d, exit 2026-11-24).
    fn lsr_native_integer_widths(&self) -> &'static [u32] {
        &[16, 32]
    }

    /// A divide is a call here: `v / 10 >= 10` stays a compare of the quotient
    /// the loop carries, not `v >= 100` with `v` kept across the call
    /// (DEVIATE ICMPDIV-1, `tti.rs` of llrm-mir).
    fn keeps_icmp_of_shared_division(&self) -> bool {
        true
    }

    /// `isLSRCostLess` with the target's own instruction counts, the shape of
    /// the pin's x86 override ("instruction number 1st priority",
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7202)
    /// but with the terms weighed. LSR counts a 16-bit counter step and a
    /// 16-bit base add as one instruction each; here a step is `inc; bne;
    /// inc` (3), an add of two cells seven, a multiply a call (40), an
    /// immediate added four. Zero-page cells are cheap: registers come after.
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            // u64: a rejected formula carries `u32::MAX` terms.
            let work = 3 * u64::from(c.add_rec_cost)
                + 7 * u64::from(c.num_base_adds)
                + 40 * u64::from(c.num_iv_muls)
                + 4 * u64::from(c.imm_cost);
            (work, c.num_regs, c.scale_cost, c.setup_cost)
        };
        key(a) < key(b)
    }

    fn address_index_max(&self) -> Option<u64> {
        Some(u64::from(u8::MAX))
    }

    /// `narrowiv`: a counter of a width the datalayout does not hold (`n8`
    /// only) whose range fits a byte is a byte counter.
    fn narrows_counters(&self) -> bool {
        true
    }

    /// `shouldDropLSRSolutionIfLessProfitable`: a loop keeps its code where
    /// LSR's own model rates the rewrite worse, as on the z80 (DEVIATE LU15):
    /// with 16-bit widths on, switch_empty_cases, switch_jump_table and
    /// function_table grew by an extra counter (owner 6502-d, exit 2026-11-24).
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        true
    }

    /// `i * 54` is a `__mulhi3` call where the step is not `(2^k +- 1) * 2^z`
    /// (`mul_by_constant_stays_in_line`): LSR's baseline charges the call that
    /// the reduction removes (DEVIATE LU17).
    fn lsr_written_step_multiplies(
        &self,
        step: i64,
        bits: u32,
    ) -> u32 {
        u32::from(!crate::dagisel::combine::mul_by_constant_stays_in_line(bits, i128::from(step), false))
    }

    /// DEVIATE LSR-OUTER (hook of x87's #9182): `i * 54` feeding an inner loop
    /// is a `__mulhi3` call on every trip of the outer loop; a counter held
    /// across the inner loop is a pair of zero-page cells. Reduced where the
    /// multiply is no shift-and-add chain (`mul_by_constant_stays_in_line`),
    /// or not a constant.
    fn reduces_outer_multiply(
        &self,
        multiplier: Option<u128>,
    ) -> bool {
        multiplier.is_none_or(|m| !crate::dagisel::combine::mul_by_constant_stays_in_line(16, m as i128, false))
    }

    /// DEVIATE LU24: a pointer that steps is weighed against `p + n`.
    fn lsr_address_left_to_cost_model(&self) -> bool {
        true
    }

    /// DEVIATE LU23: an address `p + n` is an add of two cells each trip.
    fn lsr_written_pointer_add_is_base_add(&self) -> bool {
        true
    }

    /// DEVIATE LU22: the inner loop's byte counter, extended, is the counter.
    fn lsr_extension_of_phi_exists(&self) -> bool {
        true
    }

    /// DEVIATE LU20: the bound of an exit test is a cell next to the counter.
    fn lsr_written_exit_bound_is_live(&self) -> bool {
        true
    }

    /// A constant offset in an address with one pointer cell is the `y` of
    /// `(zp),y`, loaded after LSR (`gepoffset`): LSR's unfolded add of it is
    /// no instruction. 54 stores of `g[i][k]` otherwise cost 53 base adds and
    /// the loop kept its `__mulhi3` call (DEVIATE LU16).
    fn unfolded_address_offsets_fold(&self) -> bool {
        true
    }

    /// `adc #k` and `cmp #k` take any constant a byte at a time. The pin's
    /// base answer is "none", which made LSR bake every offset into a counter
    /// of its own (switch_jump_table: a second counter for the exit test).
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// See [`Self::is_legal_add_immediate`]: `cmp #k` per byte.
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// `jsr` and `rts`, and each argument byte through a cell (`lda`, `sta`).
    fn call_sequence_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        let bytes: i64 = shape
            .arguments
            .iter()
            .map(|argument| match argument.byval_bytes {
                Some(bytes) => bytes as i64,
                None => argument.ty.map_or(1, |ty| self.parts(ty)),
            })
            .sum();
        Some(match kind {
            CostKind::CodeSize if self.code_size_bytes => Cost::of(3 + 4 * bytes),
            CostKind::CodeSize | CostKind::SizeAndLatency => Cost::of(2 + bytes),
            CostKind::RecipThroughput | CostKind::Latency => Cost::of(12 + 6 * bytes),
        })
    }

    /// The soft-stack frame a callee with static allocas builds and tears
    /// down: `rs0` adjusted both ways (30 clocks, `staticlocals`).
    fn callee_frame_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        (shape.callee_frame_bytes > 0).then(|| match kind {
            CostKind::CodeSize if self.code_size_bytes => Cost::of(24),
            CostKind::CodeSize | CostKind::SizeAndLatency => Cost::of(2),
            CostKind::RecipThroughput | CostKind::Latency => Cost::of(30),
        })
    }

    // The same model counting encoded bytes where `CostKind::CodeSize` asks.
    fn inline_size_view(&self) -> Option<std::rc::Rc<dyn Tti>> {
        Some(std::rc::Rc::new(Mos6502 { code_size_bytes: true, ..*self }))
    }

    fn has_cmov(&self) -> bool {
        false
    }

    /// A select is a branch around a copy: an arm only it reads (`x - 32` of
    /// `c - 97 < 26 ? c - 32 : c`) is made on its own path (`cgpselectarms`).
    fn sinks_select_arms(&self) -> bool {
        true
    }

    fn sinks_select_arms_for_size(&self) -> bool {
        true
    }

    fn memory_op_cost(
        &self,
        _: MemoryOp,
        ty: Ty,
        kind: CostKind,
        _: bool,
    ) -> Cost {
        // `ldy #n; lda (rc),y` is 4 bytes a byte
        self.code(kind, self.parts(ty), || 4 * self.parts(ty))
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        // a phi is a copy the allocator coalesces; a branch is `bne l` (2) and
        // the jump past the other arm (3)
        self.code(kind, Basic.cf_instr_cost(phi, kind).value, || if phi { 0 } else { 3 })
    }

    fn int_imm_cost(
        &self,
        _: i128,
        bits: u32,
    ) -> Cost {
        Cost::of(i64::from(bits.div_ceil(self.byte_bits).max(1)))
    }

    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost_inst(op, index, imm, bits)
    }

    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        matches!((from, to), (Ty::Int(from), Ty::Int(to)) if to < from)
    }

    fn is_legal_addressing_mode(
        &self,
        gv: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        // `(zp),y` with a constant `y` (a pointer cell and 0..=255), or an
        // absolute `lda g+k`; no scaled index (a 16-bit add is not an address).
        match (gv, base) {
            _ if scale != 0 => false,
            (false, true) => (0..=255).contains(&offset),
            (true, true) => false,
            _ => (0..=0xFFFF).contains(&offset),
        }
    }

    // The runtime's unsigned divide leaves the remainder behind (`rc6:rc7`), so
    // a pair is one call.
    fn has_div_rem_op(
        &self,
        bits: u32,
        signed: bool,
    ) -> bool {
        !signed && bits <= 2 * self.byte_bits
    }

    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        let (to, from) = (self.parts(dst), self.parts(src));
        match op {
            CastKind::Trunc | CastKind::BitCast => Cost::FREE,
            // `lda #0; sta` a new byte; the sign byte is a branch and a fill
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr => {
                self.code(kind, (to - from).max(0), || 4 * (to - from).max(0))
            }
            CastKind::SExt => self.code(kind, 2 * (to - from).max(0), || 8 * (to - from).max(0)),
            // soft float: a call
            _ => self.call(to.max(from), kind, 10 * to.max(from).pow(2) + 10),
        }
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        use ArithOp::*;
        let parts = self.parts(ty);
        let constant = first.constant || second.constant;
        match op {
            // `lda; adc; sta` a byte, `clc` once
            Add | Sub | And | Or | Xor | FNeg => self.code(kind, parts, || 6 * parts + 1),
            // a rotate chain, 4 bytes a byte
            Shl | LShr | AShr if second.constant => self.code(kind, parts, || 4 * parts),
            Shl | LShr | AShr => self.call(parts, kind, 6 * parts),
            Mul if second.power_of_two || first.power_of_two => self.code(kind, parts, || 4 * parts),
            Mul if constant => self.code(kind, 3 * parts, || 14 * parts),
            Mul => self.call(parts, kind, 10 * parts * parts + 10),
            UDiv if second.power_of_two => self.code(kind, parts, || 4 * parts),
            URem if second.power_of_two => self.code(kind, parts, || 2 * parts),
            UDiv | URem | SDiv | SRem => self.call(parts, kind, 22 * parts * parts + 3),
            // soft float: a call
            FAdd | FSub | FMul | FDiv | FRem => self.call(parts, kind, 10 * parts * parts + 10),
        }
    }
}
