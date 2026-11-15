//! The Z80's cost model, summed from the instruction table (`timing/z80.csv`).
//!
//! Every price is the sum of the forms an operation expands to ([`sum`]): bytes
//! for `CodeSize`, T-states for the other kinds, in units of a cheap
//! instruction (4 T, `ld r,r`), so that `SimplifyCFG`'s speculation budget and
//! the inliner's thresholds, which count in `TCC_Basic`, mean what they mean
//! for the pin.
//!
//! DEVIATE: `BasicTTIImplBase` scales only `RecipThroughput`; here `Latency`
//! and `SizeAndLatency` scale too (as `X86TTIImpl` and `AArch64TTIImpl` do).
//! Reason: a 16-bit `add` is one instruction and an `and` six, which the
//! `Basic` unit price hides. Exit: none; the table is the target's.
//!
//! DEVIATE: the runtime routines (`__mulhi3`, `__udivhi3`, `__ashlhi3`, ...)
//! are not written, so a call is priced as the restoring loop SDCC's measured
//! `mul8` (385 T) agrees with, per bit. Reason: the cost model runs before
//! the runtime exists. Exit: price from the routines' own T-states once
//! `runtime/` has them (by 2026-11-06).
//!
//! DEVIATE: soft float is priced as a multiply-sized call. Reason: no
//! soft-float runtime. Exit: the same.
//!
//! The expansions (operands already in the registers the instruction wants;
//! placing them is the allocator's):
//!
//! | operation | forms |
//! |---|---|
//! | add, sub, and, or, xor, i8 | `add a,r` |
//! | add, i16 | `add hl,rr` |
//! | sub, i16 | `or a; sbc hl,rr` |
//! | and, or, xor, i16 | `ld a,l; and c; ld l,a; ld a,h; and b; ld h,a` |
//! | icmp, i8 / i16 | `cp r` / `push hl; or a; sbc hl,rr; pop hl` |
//! | select, i8 / i16 | `jr nz,e; ld r,r'` per byte |
//! | load, store, i8 / i16 | `ld r,(hl)` / `ld a,(hl); inc hl; ld h,(hl); ld l,a` and its store |
//! | shift by a constant, per bit, i8 / i16 | `sla r` / `add hl,hl` or `srl h; rr l` |
//! | zext, sext i8 to i16 | `ld h,n` / `ld a,l; rla; sbc a,a; ld h,a` |

use llrm_mir::datalayout::DataLayout;
use llrm_mir::tti::*;
use llrm_target::Target as _;

use crate::driver::Level;
use crate::target::Z80;
use crate::timing::cost;

/// The bytes and T-states of `forms`, each taken on its taken path.
fn sum(forms: &[&str]) -> (i64, i64) {
    forms
        .iter()
        .fold(
            (0, 0),
            |(bytes, clocks), form| {
                let one = cost(form).unwrap_or_else(|| panic!("timing/z80.csv has no form `{form}`"));
                (bytes + i64::from(one.bytes), clocks + i64::from(one.taken))
            },
        )
}

/// `forms` in the unit of `kind`.
fn priced(
    forms: &[&str],
    kind: CostKind,
) -> Cost {
    let (bytes, clocks) = sum(forms);
    Cost::of(match kind {
        CostKind::CodeSize => bytes,
        _ => (clocks + 3) / 4,
    })
}

/// The cost model of the Z80 and Z180.
#[derive(Clone, Copy, Debug)]
pub struct Z80Tti {
    /// The native integer, in bits (`n8`).
    byte_bits: u32,
    /// The pointer, in bits (`p:16`).
    pointer_bits: u32,
}

impl Z80Tti {
    pub fn new() -> Z80Tti {
        let layout = DataLayout::parse(&Z80.layout().datalayout).expect("the Z80's datalayout parses");
        Z80Tti { byte_bits: layout.largest_legal_integer(), pointer_bits: layout.pointer(0).bits }
    }

    /// The legal-sized pieces a value of `ty` splits into: bytes for an `i8`,
    /// words for the rest.
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
        i64::from(bits.div_ceil(16).max(1))
    }

    fn is_byte(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(bits) if bits <= self.byte_bits.min(8))
    }

    /// A call of a runtime loop of `bits` passes, each the forms `body`, plus
    /// the call and the return.
    fn call(
        &self,
        parts: i64,
        bits: i64,
        body: &[&str],
        kind: CostKind,
    ) -> Cost {
        let (bytes, clocks) = sum(body);
        let (call_bytes, call_clocks) = sum(&["call nn", "ret"]);
        match kind {
            CostKind::CodeSize => Cost::of(call_bytes + 2 * parts),
            _ => Cost::of((call_clocks + clocks * bits + bytes) / 4 * parts),
        }
    }
}

impl Default for Z80Tti {
    fn default() -> Z80Tti {
        Z80Tti::new()
    }
}

/// A byte of code is worth 3 T at -O2 and -O3 (the rate the program table is judged at), in thousandths of a clock.
pub const SPEED_MILLICLOCKS_PER_BYTE: i64 = 3000;

impl Tti for Z80Tti {
    /// DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; z80 joins it): `va_list` is `char *`, so `va_start` is a
    /// store of the first variadic argument's address and the list lives in a register, as SDCC has it.
    fn promote_va_list(&self) -> bool {
        true
    }

    /// `va_arg` is an IR instruction on this target's clang. The z80 aligns nothing past a byte (`a:8`) and sets no
    /// minimum stack-argument alignment, so the selector's `expandVAArg` steps by the value's size: slot 1 writes
    /// exactly that.
    fn va_arg_slot(&self) -> Option<u32> {
        Some(1)
    }

    /// DEVIATE LFTR1 (owner w12, review 2026-12-20): any constant
    /// non-wrapping stride counts; off, `variadic` grows +30/+51 B.
    fn lftr_any_stride(&self) -> bool {
        true
    }

    /// DEVIATE MERGEFUNC-OS (owner z80, exit 2026-11-24; x86 joins it): `MergeFunctions` is off in the pin's
    /// pipeline; at -Os/-Oz two functions of one body are one (shl_long_by_one: `twice` and `twice_signed`).
    fn merge_functions_at_size(&self) -> bool {
        true
    }

    /// DEVIATE Z80-MERGEFUNC-LOCAL (owner z80, exit 2026-11-24): a local function that is the same code as another is dropped at
    /// -O1, -O2 and -O3 too: fewer bytes, the same clocks (shl_long_by_one `twice_signed`, signbit_idioms `bit_i16`).
    fn merge_local_functions_at_speed(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "z80"
    }

    /// DEVIATE z80-lsr-drop (owner z80, exit 2026-12-15): a loop without an
    /// address keeps its code where LSR's own model rates the rewrite worse
    /// (`shouldDropLSRSolutionIfLessProfitable`, off in the pin's targets
    /// but the m16 X86). switch_empty_cases -Os/-O1: a count-down counter and
    /// the accumulator's old value for the exit took a frame (74 B, now 37 B).
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        true
    }

    fn lsr_dropped_solution_spares_addresses(&self) -> bool {
        true
    }

    /// `long i < 12` compares four bytes a test where a byte would do (`narrowiv`;
    /// div_constants_short -Oz: -1189 B of 9887 with the counter an `int`).
    fn narrows_counters(&self) -> bool {
        true
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let parts = self.parts(ty);
        match op {
            CmpSel::ICmp if self.is_byte(ty) => priced(&["cp b"], kind),
            CmpSel::ICmp => priced(&["push bc", "or b", "sbc hl;bc", "pop bc"], kind) * parts.min(2),
            CmpSel::Select if self.is_byte(ty) => priced(&["jr nz;e", "ld b;c"], kind),
            CmpSel::Select => priced(&["jr nz;e", "ld b;c", "ld b;c"], kind) * parts,
            // soft float: a call
            CmpSel::FCmp => self.call(parts, 8 * parts, &["add hl;bc", "djnz e"], kind),
        }
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        _: bool,
    ) -> Cost {
        let load = op == MemoryOp::Load;
        if self.is_byte(ty) {
            return priced(&[if load { "ld b;(hl)" } else { "ld (hl);b" }], kind);
        }
        let word = if load {
            ["ld b;(hl)", "inc hl", "ld b;(hl)", "ld b;c"]
        } else {
            ["ld (hl);b", "inc hl", "ld (hl);b", "inc hl"]
        };
        priced(&word, kind) * self.parts(ty)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        _: i128,
        bits: u32,
    ) -> Cost {
        Cost::of(i64::from(bits.div_ceil(16).max(1)))
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

    /// What the selector folds into one access: a register alone (`ld a,(hl)`)
    /// or an absolute address with its offset (`ld a,(nn)`). A register
    /// plus a displacement is `(ix+d)`, which is not selected yet, and the Z80
    /// has no register-plus-register or scaled mode. The pin's default
    /// always accepts `r`, so LSR's initial formula is legal; `Basic`
    /// accepts nothing and LSR's `Formula is illegal` assertion fires.
    fn is_legal_addressing_mode(
        &self,
        gv: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        if !matches!(scale, 0 | 1) {
            return false;
        }
        match usize::from(base) + usize::from(scale == 1) {
            0 => true,
            1 => !gv && offset == 0,
            _ => false,
        }
    }

    /// AArch64's `isLSRCostLess`, not X86's: the base adds come before the
    /// add-recurrences. The Z80 has no base+register address, so a base add
    /// is an `add hl,rr` and two moves each trip while another induction
    /// variable is one `inc rr`: lz4's copy loops kept `dst + off` as a base
    /// add and their pointers in the frame (170569 clocks, gcc 35781).
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7461
    /// DEVIATE Z80L3: `num_base_adds` also comes before `num_regs`. Three
    /// pairs hold three induction variables, but each base add is an `add
    /// hl,rr` and moves every trip: reverse -Oz kept `p + i` and `p + j` as
    /// base adds to save one register and spilled the rest (105 B, 1900 T);
    /// SDCC's 68 B / 1270 T. Exit: none; the order is the target's.
    /// Open: `find` -O2/-Oz is 2 B / 27 T worse than the pin's order (a second
    /// induction variable for the returned index).
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            (
                c.spills.saturating_add(c.num_base_adds),
                c.num_regs,
                c.spills,
                c.insns,
                c.add_rec_cost,
                c.num_iv_muls,
                c.scale_cost,
                c.imm_cost,
                c.setup_cost,
            )
        };
        key(a) < key(b)
    }

    /// DEVIATE LU13: a call takes every register of the Z80, so what a loop
    /// keeps across one is in the frame; `spills` plus the base adds (the
    /// instructions that stand for a kept value) rank first.
    fn lsr_call_clobbers_registers(&self) -> bool {
        true
    }

    /// The adds of a use whose fixups all lie after the loop are rated as run
    /// once (DEVIATE LU12, off in the pin): lz4 -Oz keeps 159 instructions
    /// against 168 without it.
    fn lsr_exit_use_adds_run_once(&self) -> bool {
        true
    }

    /// A word add, `add hl,bc`: the Z80 prices in machine cycles (and bytes),
    /// so the expansion budget of four basic instructions counts word
    /// operations. A 16-bit `umax` alone costs 6; the loop count of lz4c's
    /// checksum loop (`umax` and an add) was refused, and the loop kept
    /// `base + i` with its bound and base in the frame (132469 T).
    fn tcc_basic(
        &self,
        kind: CostKind,
    ) -> Cost {
        priced(&["add hl;bc"], kind)
    }

    /// HardwareLoops: a counted loop keeps its count in a byte register,
    /// which `djnz` counts down. Only where the loop's own counter dies (the
    /// byte replaces the word) and nothing in the loop calls (a call takes
    /// every register).
    fn is_hardware_loop_profitable(
        &self,
        info: &mut llrm_mir::tti::HardwareLoopTarget,
    ) -> bool {
        info.count_bits = 8;
        info.counter_in_reg = true;
        info.is_nesting_legal = true;
        info.requires_dead_counter = true;
        info.rejects_calls = true;
        true
    }

    /// `memset.pattern` is 4 in the pin ("the cost of the `memset_pattern16`
    /// libcall"), but this target has no such routine: `PreISelIntrinsicLowering`
    /// expands it into a loop after inlining has priced it. In code size it is
    /// that loop: a guard on the count, the pointers, a store a byte of the
    /// pattern, the test and the branch back (a 2-word store inlined twice was
    /// +82 B at -Os while the callees stayed).
    ///
    /// DEVIATE: other kinds keep the pin's 4. Reason: the unroller and the
    /// speed bound are tuned on it. Exit: price the loop's clocks.
    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        const MEMSET_PATTERN: &str = "llvm.experimental.memset.pattern";
        let pattern = name == MEMSET_PATTERN || name.strip_prefix(MEMSET_PATTERN).is_some_and(|rest| rest.starts_with('.'));
        if let (true, CostKind::CodeSize, Some(Ty::Int(bits))) = (pattern, kind, args.get(1).copied()) {
            let (skeleton, _) = sum(&[
                "ld b;c", "or b", "jr nz;e", "ld hl;nn", "add hl;bc", "ld b;c", "ld b;c", "or b", "sbc hl;bc",
                "add hl;bc", "jr nz;e",
            ]);
            let (each_byte, _) = sum(&["ld (hl);b", "inc bc"]);
            let (each_word, _) = sum(&["ld bc;nn"]);
            let bytes = i64::from(bits.div_ceil(8));
            return Cost::of(skeleton + each_byte * bytes + each_word * ((bytes + 1) / 2));
        }
        let _ = ret;
        if llrm_mir::tti::is_free_intrinsic(name) { Cost::FREE } else { llrm_mir::tti::base_intrinsic_cost(name).unwrap_or(Cost::INVALID) }
    }

    /// DEVIATE UNROLL-SPEED-TRIAL (owner z80, exit 2026-11-24): `compile::effective` compiles the module under the unroll
    /// variants at -O2 and -O3 and keeps the one smaller in bytes, then clocks.
    fn unroll_speed_trial(&self) -> bool {
        true
    }

    /// A byte of growth is worth 3 T at -O2 and -O3, the rate the program table is judged at: an inline that costs
    /// more bytes than its call saves clocks is refused (a callee that stays and is copied into its one caller).
    fn speed_inline_milliclocks_per_byte(&self) -> i64 {
        SPEED_MILLICLOCKS_PER_BYTE
    }

    /// DEVIATE UP1 (the shared priced unroll, `loopunrollprice`): a full unroll is made at -O2/-O3 where the clocks it
    /// saves repay its bytes at the same rate, in place of the pin's threshold alone (0).
    /// Audited 2026-11-13: rate 0 and this rate give identical bytes and T on all 124 `tests/run/c` programs at -O1/-O2/-O3/-Os/-Oz,
    /// so the price refuses nothing there; it guards loops the corpus lacks. A rate of 10^9 unrolls none (the control).
    fn unroll_milliclocks_per_byte(&self) -> i64 {
        SPEED_MILLICLOCKS_PER_BYTE
    }

    /// The same rate for every speed decision that trades bytes for clocks (peeling, the combiner, the inliner's
    /// walk, loop idioms, the backend's byte-for-clock choices): one place, not an inliner-only 3000.
    fn speed_milliclocks_per_byte(&self) -> i64 {
        SPEED_MILLICLOCKS_PER_BYTE
    }

    /// A loop whose bound the call's constants fix unrolls and folds in the copy, which the call's latency does not
    /// show (loop_fusion -O3 39,020 T became 299,430 T without it).
    /// DEVIATE Z80-RATE (owner z80, exit 2026-11-21): the rate sees the call's latency and no other gain of a copy.
    /// Two cells stay net worse at 1 B = 3 T: loop_fusion -O2 (`clear` kept as 11 calls, +40,010 T for -869 B; the
    /// copies would meet the overwriting loops) and memmove_overlap -O3 (`fill` kept, +718 T for -100 B; the estimate
    /// saves 57 clocks, the copies fold more).
    fn inline_hints_known_loop_bound(&self) -> bool {
        true
    }

    fn inline_trial_keeps_shrinking_copy(&self) -> bool {
        true
    }

    /// DEVIATE LOOP-COPY-TRIAL (owner z80, exit 2026-11-24): the module is compiled again with the copies of a
    /// loop-holding callee at several sites of a loop refused, and the cheaper after allocation is kept
    /// (`compile::effective`).
    fn inline_loop_copy_trial(&self) -> bool {
        true
    }

    /// DEVIATE Z80-ROT (owner z80, exit 2026-11-21): -Oz keeps a counted loop proven to run bottom-tested (`djnz`, no
    /// test above the body), as the gcc tail rotate did until its deletion (#8017): 27 -Oz cells grew, sum4 +4 B.
    fn rotation_free_guard_at_minsize(&self) -> bool {
        true
    }

    /// -O1 keeps the size bound's refusals but tries them as -Os does: a copy that folds (struct_odd_sizes: `fives`
    /// -40, refused, 131 B over SDCC) leaves the caller smaller and faster.
    fn inline_trials_at_o1(&self) -> bool {
        true
    }

    /// SZ1, rule 1: a size build keeps a callee bigger than its call. The pin's
    /// `OptSizeThreshold` 50 allows growth, and a word update is 6 B where the
    /// call is 3.
    fn opt_size_inline_by_code_size(&self) -> bool {
        true
    }

    /// Where the pin's own cost is negative the inline shrinks the caller (a
    /// wrapper, a callee that folds away): the bound stands down.
    fn opt_size_inline_guard_yields_to_negative_cost(&self) -> bool {
        false
    }

    /// The pin counts a call as one unit and one per argument; here a unit is a
    /// byte, and `call nn` is 3 with `ld rr,#nn` or a pair move 2 to 3 per
    /// word argument: 250% makes one argument 5 B (2 units) and three 9 B (4).
    fn call_code_size_percent(&self) -> i64 {
        250
    }

    /// The call with what surrounds it, which inlining removes: `call nn` and
    /// `ret`; the first three word arguments in HL, DE and BC (`ld rr,nn`, the
    /// dearer of a constant and a move), a byte in a register (`ld r,n`), the
    /// rest `push` and the `pop` that takes them back; a `byval` argument
    /// copied by `ldir`; and the callee's frame, `push ix; ld ix,#0; add ix,sp`
    /// and its undoing, with a `push` a word of locals (a frame over 6 B is
    /// `ld iy,#-n; add iy,sp; ld sp,iy`).
    ///
    /// DEVIATE: the values live across the call (every register is
    /// caller-saved, so each costs a spill and a reload, 6 B / 38 T) and the
    /// frame the callee's own spills make are not here: the inliner does not
    /// know them before allocation. Exit: none; the estimate is the floor.
    /// `ld sp,ix` and `ld iy,nn` are priced as `ld sp,hl` and `ld ix,nn`: the
    /// table has no form for them.
    fn call_sequence_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        let (mut bytes, mut clocks) = sum(&["call nn", "ret"]);
        let mut registers = 0;
        for argument in &shape.arguments {
            let words = match argument.ty {
                Some(Ty::Int(bits)) if bits <= 8 => 0,
                Some(Ty::Int(bits)) => i64::from(bits.div_ceil(16)),
                Some(ty) => self.parts(ty),
                None => 1,
            };
            if let Some(copied) = argument.byval_bytes {
                // `ld hl,src; ld de,dst; ld bc,n; ldir`: 21 T a byte
                let (b, c) = sum(&["ld hl;nn", "ld hl;nn", "ld bc;nn", "ldir"]);
                bytes += b;
                clocks += c + 21 * (copied as i64 - 1);
            } else if words == 0 {
                let (b, c) = sum(&["ld b;n"]);
                bytes += b;
                clocks += c;
            } else {
                for _ in 0..words {
                    let form = if registers < 3 { &["ld bc;nn"][..] } else { &["push bc", "pop bc"][..] };
                    registers += 1;
                    let (b, c) = sum(form);
                    bytes += b;
                    clocks += c;
                }
            }
        }
        Some(Cost::of(match kind {
            CostKind::CodeSize => bytes,
            _ => (clocks + 3) / 4,
        }))
    }

    /// The callee's frame: `push ix; ld ix,#0; add ix,sp` and its undoing, with
    /// a `push` a word of locals (a frame over 6 B is `ld iy,#-n; add iy,sp;
    /// ld sp,iy`). `ld sp,ix` and `ld iy,nn` are priced as `ld sp,hl` and
    /// `ld ix,nn`: the table has no form for them.
    fn callee_frame_cost(
        &self,
        shape: &CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        if shape.callee_frame_bytes == 0 {
            return None;
        }
        let (mut bytes, mut clocks) = (0, 0);
        let frame: &[&str] = if shape.callee_frame_bytes > 6 {
            &["push ix", "ld ix;nn", "add ix;bc", "ld ix;nn", "add ix;bc", "ld sp;hl", "ld sp;hl", "pop ix"]
        } else {
            &["push ix", "ld ix;nn", "add ix;bc", "ld sp;hl", "pop ix"]
        };
        let (b, c) = sum(frame);
        bytes += b;
        clocks += c;
        if shape.callee_frame_bytes <= 6 {
            let (b, c) = sum(&["push bc"]);
            let words = (shape.callee_frame_bytes as i64 + 1) / 2;
            bytes += b * words;
            clocks += c * words;
        }
        Some(Cost::of(match kind {
            CostKind::CodeSize => bytes,
            _ => (clocks + 3) / 4,
        }))
    }

    // There is no conditional move: a `select` is a branch on the Z80.
    fn has_cmov(&self) -> bool {
        false
    }

    // The long routines `__udivremsi4` and `__divremsi4`, the long long ones
    // (`__udivmddi4`, `__divmddi4`) and the word ones (`__udivhi3` leaves
    // the remainder in `hl`, `__divremhi4`) give both: a pair is one call. The
    // byte routines do not.
    fn has_div_rem_op(
        &self,
        bits: u32,
        _signed: bool,
    ) -> bool {
        matches!(bits, 16 | 32 | 64)
    }

    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        let (to, from) = (self.parts(dst), self.parts(src));
        let widen = self.is_byte(src) && !self.is_byte(dst);
        match op {
            CastKind::Trunc | CastKind::BitCast => Cost::FREE,
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr if widen => {
                priced(&["ld b;n"], kind) * (to - from + 1).max(1)
            }
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr => priced(&["ld b;n"], kind) * (to - from).max(0),
            CastKind::SExt if widen => priced(&["ld b;c", "rla", "sbc a;b", "ld b;c"], kind),
            CastKind::SExt => priced(&["rla", "sbc a;b", "ld b;c"], kind) * (to - from).max(0),
            // soft float: a call
            _ => self.call(to.max(from), 8 * to.max(from), &["add hl;bc", "djnz e"], kind),
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
        // `BasicTTIImpl::getArithmeticInstrCost` hands every kind but throughput to the base class's unit costs, so the
        // loop unroller and SimplifyCFG (`SizeAndLatency`) weigh an add and a 32-bit multiply alike.
        // DEVIATE: code size, latency and throughput keep the z80 forms below.
        if kind == CostKind::SizeAndLatency {
            return match op {
                UDiv | URem | SDiv | SRem | FDiv | FRem => Cost::EXPENSIVE,
                _ => Cost::BASIC,
            };
        }
        let parts = self.parts(ty);
        let byte = self.is_byte(ty);
        let bits = if byte { 8 } else { 16 * parts };
        let constant = first.constant || second.constant;
        match op {
            Add if byte => priced(&["add a;b"], kind),
            Sub | And | Or | Xor if byte => priced(&["sub b"], kind),
            Add => priced(&["add hl;bc"], kind) + priced(&["adc hl;bc"], kind) * (parts - 1),
            Sub => priced(&["or b", "sbc hl;bc"], kind) + priced(&["sbc hl;bc"], kind) * (parts - 1),
            And | Or | Xor => priced(&["ld b;c", "and b", "ld b;c", "ld b;c", "and b", "ld b;c"], kind) * parts,
            FNeg => priced(&["xor n"], kind),
            // one step per bit; the amount is not known here, so one step
            // stands for a constant shift
            Shl | LShr | AShr if second.constant && byte => priced(&["sla b"], kind),
            Shl if second.constant => priced(&["add hl;bc"], kind) * parts,
            Shl | LShr | AShr if second.constant => priced(&["srl b", "rr b"], kind) * parts,
            // by a variable: a loop of one step and a `djnz`, 4 or 8 passes on
            // average
            Shl | LShr | AShr => self.call(parts, bits / 2, &["add hl;bc", "djnz e"], kind),
            Mul if second.power_of_two || first.power_of_two => priced(&["add hl;bc"], kind) * parts,
            Mul if constant => priced(&["add hl;bc", "add hl;bc", "add hl;bc"], kind) * parts,
            // shift-and-add: add hl,hl; jr nc; add hl,de; djnz per bit
            Mul => self.call(parts, bits, &["add hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind),
            UDiv | URem if second.power_of_two => priced(&["srl b", "rr b"], kind) * parts,
            // restoring division: add hl,hl; adc hl,hl; sbc hl,de; jr nc; add
            // hl,de; djnz per bit
            UDiv | URem | SDiv | SRem => {
                self.call(parts, bits, &["add hl;bc", "adc hl;bc", "sbc hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind)
            }
            // soft float: a call
            FAdd | FSub | FMul | FDiv | FRem => {
                self.call(parts, bits, &["add hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind)
            }
        }
    }
}
