//! What the target does with each operation on each type: the pin's
//! `TargetLoweringBase` action tables and `X86TargetLowering`'s constructor,
//! for i486 and P5.
//!
//! # How it works
//! [`TargetLowering`] holds the type half ([`TargetLoweringInfo`]) and four
//! tables: (operation, type) to [`Action`], (extending load, type, memory
//! type), (type, memory type) for truncating stores, and (condition code,
//! type). Everything starts `Legal`; `init_actions` makes the pin's
//! `initActions` defaults and `x86_32` (or `aarch64`, the scalar integer arms
//! of `AArch64TargetLowering`, with the scalar float and atomic ones) the
//! constructor's calls, in the pin's order, because a later call overwrites an
//! earlier one. `operation_action` is `getOperationAction`: an integer with no
//! simple type is `Expand`. `type_to_promote_to` is `getTypeToPromoteTo`: the
//! recorded type, else the next wider legal type that handles the operation.
//!
//! # Runs
//! Built once per target; read by LegalizeDAG (B1) and the type legalizer.
//!
//! The constructor's arms are translated where a node of [`ISD`] exists and
//! the arm applies to i486/P5 (no SSE, no CMOV before P6, no BMI, POPCNT or
//! LZCNT, x87 on, 32-bit). Not yet translated because [`ISD`] has no such
//! node: `ABDS`/`ABDU`, the `STRICT_*` family, `FSIN`/`FCOS`/`FPOW`-style
//! math, `SHL_PARTS`/`SRA_PARTS`/`SRL_PARTS`, `SETCCCARRY`,
//! atomics, `VASTART`/`VAARG`, `JumpTable`/`BlockAddress`/`GlobalTLSAddress`,
//! `LRINT`, `ADDRSPACECAST`, `INTRINSIC_*`. Each is added with the piece
//! that needs it (B1), in the position the constructor sets it. Arms for
//! SSE, AVX, MMX, vectors, `f16`/`f128` and `is64Bit` are N/A.
//!
//! DEVIATE SD32: the tables are keyed by [`MVT`]; `Far` has no entry and is
//! always expanded by the type legalizer. Reason: SD2. Exit: none. Owner
//! sdag-b. Review 2026-11-13.
//!
//! # See also
//! LLVM's `initActions`
//! ([L764](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L764))
//! and `X86TargetLowering::X86TargetLowering`
//! ([L131](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L131)).

use llrm_support::hash::HashMap;

use super::isd::{CondCode, ISD};
use super::libcalls::{self, Libcall};
use super::lowering::TargetLoweringInfo;
use super::mvt::{EVT, MVT};

/// `TargetLowering::LegalizeAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Legal,
    Promote,
    Expand,
    LibCall,
    Custom,
}

/// The CPU features the translated arms test.
#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    pub cmov: bool,
    pub bmi: bool,
    pub lzcnt: bool,
    pub popcnt: bool,
    pub movbe: bool,
    pub shld_slow: bool,
    /// `hasSSE2()`: f32 and f64 live in xmm registers (m64; D64-4).
    pub sse2: bool,
    /// `useSoftFloat()`: `f32` and `f64` have no register class; their
    /// arithmetic is a call (m16).
    pub soft_float: bool,
}

/// The CPU features the AArch64 constructor's scalar arms test.
#[derive(Clone, Copy, Debug, Default)]
pub struct Aarch64Features {
    /// `hasCSSC()`: scalar `ctpop`, `cttz`, `abs`, `smax` and the like are
    /// instructions.
    pub cssc: bool,
    /// `hasLSE()`: the atomic read-modify-writes are instructions.
    pub lse: bool,
    /// `outlineAtomics()`: without LSE they call the `__aarch64_*` helpers.
    pub outline_atomics: bool,
    /// `hasLSE2()`: a 128-bit atomic load or store is one access.
    pub lse2: bool,
}

/// `ISD::LoadExtType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadExt {
    ExtLoad,
    SExtLoad,
    ZExtLoad,
}

const INTS: [MVT; 6] = [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128];
const FLOATS: [MVT; 3] = [MVT::F32, MVT::F64, MVT::F80];
const ALL: [MVT; 10] =
    [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128, MVT::F32, MVT::F64, MVT::F80, MVT::Other];

/// The action maps, shared by every copy of a table until one writes.
#[derive(Clone, Default)]
struct Maps {
    operations: HashMap<(ISD, MVT), Action>,
    promoted_to: HashMap<(ISD, MVT), MVT>,
    load_ext: HashMap<(LoadExt, MVT, MVT), Action>,
    trunc_store: HashMap<(MVT, MVT), Action>,
    cond_code: HashMap<(CondCode, MVT), Action>,
    /// `getLibcallName` of the integer operations (`RTLIB::MUL_I32` and the
    /// like): the runtime routine a `LibCall` action calls. Empty unless the
    /// target fills it.
    libcall_names: HashMap<(ISD, MVT), &'static str>,
    /// The routine a function tuned for size calls where it differs from
    /// `libcall_names`'.
    size_libcall_names: HashMap<(ISD, MVT), &'static str>,
    /// The `size_libcall_names` whose routine takes a variable amount only: a constant amount keeps the inline
    /// expansion (and, below the type, the shift routine of its halves).
    size_variable_only: HashMap<(ISD, MVT), ()>,
    /// The `size_libcall_names` called only in a function tuned for size: at speed the expansion stays inline.
    size_only: HashMap<(ISD, MVT), ()>,
}

#[derive(Clone)]
pub struct TargetLowering {
    pub types: TargetLoweringInfo,
    maps: std::sync::Arc<Maps>,
    /// `addLegalFPImmediate`: the float constants an instruction makes.
    legal_fp_immediates: Vec<(MVT, u128)>,
    /// `isFPImmLegal` as a function of the value, for a target whose answer is
    /// not a short list.
    fp_imm_predicate: Option<fn(MVT, u128, bool) -> bool>,
    /// The function is optimized for size (`ForCodeSize` of `isFPImmLegal`).
    opt_for_size: bool,
    /// `ShouldShrinkFPConstant` is false for `f64`: with SSE2 `cvtss2sd` costs
    /// more than a straight `movsd`.
    keeps_f64_constants: bool,
    /// `canUseCMOV`: the target has the conditional move.
    cmov: bool,
    /// `(granule, least)`: in a function tuned for size a shift by a constant
    /// is the routine's call where the granules left after the whole moves
    /// times the bits beyond them reach `least` (each is a shift instruction
    /// per granule).
    size_shift_call: Option<(u128, u128)>,
    /// The shift routines are used as a function tuned for size uses them, in a
    /// function that is not (the unsized form the caller prices against the
    /// inline one).
    force_shift_calls: bool,
    /// An `i64` shifted by an amount known below the half width is the routine's call, not the inline
    /// expansion the pin makes (SD241): a target whose inline policy is not priced yet keeps the call.
    libcall_for_small_variable_shift: bool,
    /// A float constant is canonically positive (gcc's `negate_expr_p`): only a negative one is cheap to negate.
    positive_fp_constants: bool,
    /// A constant factor the combiner turns into shifts and adds, so the
    /// multiply is not the library call outside functions tuned for size.
    inline_constant_mul: Option<fn(u128) -> bool>,
    /// The function is built for size: an `i64` divide calls the target's size
    /// routine.
    small_divide: bool,
    /// DEVIATE DIV-HOT: an `i64` divide in a block that runs more than once a call takes the
    /// speed routine even where `small_divide` is on.
    fast_divide_in_loops: bool,
    /// The function is built for size alone (-Os), not -O1 where clocks are
    /// strict too: an `i64` divide by a dword takes the size form that
    /// always does two `div`s.
    size_only: bool,
    /// The object spelling of a routine's name where the convention decorates
    /// it (`___addsf3@3`).
    libcall_symbols: HashMap<String, (String, u32)>,
    /// `isSExtCheaperThanZExt`: `i32` to `i64` where the target extends signed
    /// for free (AArch64's `sxtw` in an addressing mode); no x86 override.
    sext_cheaper_i32_i64: bool,
    /// `X86TargetLowering::BuildSDIVPow2` builds the `cmov` form of a signed
    /// divide by a power of two: `Some(is64)`, `i64` too where `is64`.
    sdiv_pow2_cmov: Option<bool>,
    /// DEVIATE WIDE-EQ-BYTES (owner z80, exit 2026-11-30): equality of a wide
    /// integer with a constant is the or of its bytes' xors, not of its
    /// halves' xors. The pin (`IntegerExpandSetCCOperands`) xors the halves;
    /// on an 8-bit machine that is a byte write-back per xor, and the bytes
    /// accumulate in the accumulator without them. Two registers keep both
    /// operands' bytes live beside the accumulator and spilled (div_constants
    /// +155 B), so they keep the pin's form. Exit: when the allocator holds
    /// that pressure.
    byte_equality: bool,
    /// DEVIATE ASSERT-SEXT-HI (owner z80, exit 2026-11-30): an `AssertSext`
    /// inside the low half keeps the operand's high half when it is a copy
    /// from a register, where the pin (`ExpandIntRes_AssertSext`) makes it
    /// `sra(lo, bits-1)`. The value is the same; the register already holds it
    /// (a call's result, an argument), and the shift is seven instructions on
    /// an 8-bit machine.
    assert_sext_keeps_high: bool,
    /// `getMinStackArgumentAlignment`: a `va_arg` rounds the list pointer
    /// only to an alignment above it. The pin's default is 1.
    min_stack_argument_alignment: u32,
}

impl TargetLowering {
    /// Every action `Legal`: a table nothing has been set in.
    pub fn empty(types: TargetLoweringInfo) -> TargetLowering {
        let mut table = TargetLowering {
            types,
            maps: Default::default(),
            legal_fp_immediates: Vec::new(),
            fp_imm_predicate: None,
            opt_for_size: false,
            keeps_f64_constants: false,
            cmov: false,
            size_shift_call: None,
            force_shift_calls: false,
            libcall_for_small_variable_shift: false,
            positive_fp_constants: false,
            inline_constant_mul: None,
            small_divide: false,
            fast_divide_in_loops: false,
            size_only: false,
            libcall_symbols: HashMap::default(),
            sext_cheaper_i32_i64: false,
            sdiv_pow2_cmov: None,
            byte_equality: false,
            assert_sext_keeps_high: false,
            min_stack_argument_alignment: 1,
        };
        table.set_default_integer_libcalls();
        // `TargetLoweringBase`: `VAARG` is expanded unless the target says
        // otherwise.
        table.set_operation_action(ISD::VAArg, MVT::Other, Action::Expand);
        table
    }

    /// The pin's `RuntimeLibcalls` defaults for the wide integers: libgcc's
    /// names for `SHL_I64`, `MUL_I128` and the others, on every target; a
    /// target's own `set_libcall_name` replaces them.
    fn set_default_integer_libcalls(&mut self) {
        use ISD::*;
        let rows: [(MVT, [&'static str; 8]); 2] = [
            (
                MVT::I64,
                ["__ashldi3", "__lshrdi3", "__ashrdi3", "__muldi3", "__divdi3", "__udivdi3", "__moddi3", "__umoddi3"],
            ),
            (
                MVT::I128,
                ["__ashlti3", "__lshrti3", "__ashrti3", "__multi3", "__divti3", "__udivti3", "__modti3", "__umodti3"],
            ),
        ];
        for (vt, names) in rows {
            for (op, name) in [Shl, Srl, Sra, Mul, SDiv, UDiv, SRem, URem].into_iter().zip(names) {
                self.set_libcall_name(op, vt, name);
            }
        }
        // `RTLIB::CTLZ_I32` and the rest (RuntimeLibcalls.td): the pin names
        // clz and popcount at i32, i64 and i128, no cttz and no i16.
        // DEVIATE BC1 (owner softfloat, exit 2026-11-30): `__ctz*2` for a
        // zero-poison cttz, and libgcc's `__clzhi2`, `__ctzhi2`,
        // `__popcounthi2` for the 8-bit targets' i16. A name is
        // only called where the target's action for the op and type is
        // `LibCall`. Upstream check: at each pin bump, grep
        // RuntimeLibcalls.td for CTTZ and an i16 count; where the pin names
        // one, take its name and drop the row. Exit: 2026-11-30 (decide: keep
        // as a recorded deviation or drop).
        let counts: [(MVT, [&'static str; 3]); 4] = [
            (MVT::I16, ["__clzhi2", "__popcounthi2", "__ctzhi2"]),
            (MVT::I32, ["__clzsi2", "__popcountsi2", "__ctzsi2"]),
            (MVT::I64, ["__clzdi2", "__popcountdi2", "__ctzdi2"]),
            (MVT::I128, ["__clzti2", "__popcountti2", "__ctzti2"]),
        ];
        for (vt, names) in counts {
            for (op, name) in [CtLzZeroPoison, CtPop, CtTzZeroPoison].into_iter().zip(names) {
                self.set_libcall_name(op, vt, name);
            }
        }
    }

    /// `RTLIB::ADD_F32` and the rest: compiler-rt's names for the `f32` and
    /// `f64` arithmetic a soft-float target calls.
    pub fn set_soft_float_libcall_names(&mut self) {
        let arithmetic: [(ISD, fn(MVT) -> Libcall); 4] = [
            (ISD::FAdd, Libcall::Add),
            (ISD::FSub, Libcall::Sub),
            (ISD::FMul, Libcall::Mul),
            (ISD::FDiv, Libcall::Div),
        ];
        for (op, call) in arithmetic {
            for vt in [MVT::F32, MVT::F64] {
                let name = libcalls::name(call(vt)).expect("the pin names f32 and f64 arithmetic");
                self.set_libcall_name(op, vt, name);
            }
        }
    }

    /// `setLibcallName` for the integer operation `op` of `vt`.
    pub fn set_libcall_name(
        &mut self,
        op: ISD,
        vt: MVT,
        name: &'static str,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).libcall_names.insert((op, vt), name);
    }

    /// The routine `op` of `vt` calls in a function tuned for size, where a
    /// smaller one than [`Self::set_libcall_name`]'s exists.
    pub fn set_size_libcall_name(
        &mut self,
        op: ISD,
        vt: MVT,
        name: &'static str,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).size_libcall_names.insert((op, vt), name);
    }

    /// [`Self::set_size_libcall_name`] for a call made in a function tuned for size only: the legalizer's
    /// [`Self::has_size_libcall_name`] is false at speed.
    pub fn set_size_only_libcall_name(
        &mut self,
        op: ISD,
        vt: MVT,
        name: &'static str,
    ) {
        self.set_size_libcall_name(op, vt, name);
        std::sync::Arc::make_mut(&mut self.maps).size_only.insert((op, vt), ());
    }

    /// [`Self::set_size_libcall_name`] for a shift by a variable amount only.
    pub fn set_size_variable_libcall_name(
        &mut self,
        op: ISD,
        vt: MVT,
        name: &'static str,
    ) {
        self.set_size_libcall_name(op, vt, name);
        std::sync::Arc::make_mut(&mut self.maps).size_variable_only.insert((op, vt), ());
    }

    /// A shift by a constant of a `bits`-wide value is a call to the shift
    /// routine in a function tuned for size where the granules still shifted
    /// times the bits beyond whole granules reach `least`.
    pub fn set_min_stack_argument_alignment(
        &mut self,
        align: u32,
    ) {
        self.min_stack_argument_alignment = align;
    }

    /// `getMinStackArgumentAlignment`, in bytes.
    pub fn min_stack_argument_alignment(&self) -> u32 {
        self.min_stack_argument_alignment
    }

    /// SD241: keep the shift routine for a wide shift by a variable amount known below the half width.
    pub fn set_libcall_for_small_variable_shift(&mut self) {
        self.libcall_for_small_variable_shift = true;
    }

    /// DEVIATE FP3 (owner x87, exit 2026-11-28): gcc's `negate_expr_p` calls only a negative real constant
    /// cheap to negate, so `a + y * -K` is `a - y * K` and a pool keeps one cell for `K` and `-K`.
    pub fn set_positive_fp_constants(&mut self) {
        self.positive_fp_constants = true;
    }

    pub fn positive_fp_constants(&self) -> bool {
        self.positive_fp_constants
    }

    pub fn libcall_for_small_variable_shift(&self) -> bool {
        self.libcall_for_small_variable_shift
    }

    pub fn set_size_shift_call(
        &mut self,
        granule: u128,
        least: u128,
    ) {
        self.size_shift_call = Some((granule, least));
    }

    /// Whether the shift of a `bits`-wide value by the constant `amount` is
    /// the routine's call here.
    pub fn calls_for_size_shift(
        &self,
        amount: u128,
        bits: u128,
    ) -> bool {
        (self.opt_for_size || self.force_shift_calls)
            && self.size_shift_call.is_some_and(|(granule, least)| {
                (bits / granule).saturating_sub(amount / granule) * (amount % granule) >= least
            })
    }

    /// Whether `op` of `vt` has a routine for functions tuned for size.
    pub fn has_size_libcall_name(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        matches!(
            vt,
            EVT::Simple(vt) if self.maps.size_libcall_names.contains_key(&(op, vt))
                && !self.maps.size_variable_only.contains_key(&(op, vt))
                && (self.opt_for_size || !self.maps.size_only.contains_key(&(op, vt)))
        )
    }

    /// The routine `name` is spelled `symbol` in the object.
    /// Both tables are the same allocations: neither has been written since
    /// one was copied from the other.
    pub fn shares_maps_with(
        &self,
        other: &TargetLowering,
    ) -> bool {
        std::sync::Arc::ptr_eq(&self.maps, &other.maps)
    }

    pub fn set_libcall_symbol(
        &mut self,
        name: &str,
        symbol: String,
        convention: u32,
    ) {
        self.libcall_symbols.insert(name.to_owned(), (symbol, convention));
    }

    /// The calling convention number the routine `name` was defined under.
    pub fn libcall_convention(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.libcall_symbols.get(name).map(|(_, number)| *number)
    }

    /// How a call to the routine `name` spells it: as the C compiler spelled
    /// the definition, else the name.
    pub fn libcall_symbol(
        &self,
        name: &str,
    ) -> String {
        self.libcall_symbols.get(name).map_or_else(|| name.to_owned(), |(symbol, _)| symbol.clone())
    }

    /// `getLibcallName` of the routine `op` of `vt` calls, if the target has
    /// one.
    pub fn libcall_name(
        &self,
        op: ISD,
        vt: EVT,
    ) -> Option<&'static str> {
        let EVT::Simple(vt) = vt else { return None };
        if vt == MVT::F128 {
            // `RTLIB::ADD_F128` and the rest: every target calls the
            // compiler-rt routine
            return match op {
                ISD::FAdd => libcalls::name(Libcall::Add(vt)),
                ISD::FSub => libcalls::name(Libcall::Sub(vt)),
                ISD::FMul => libcalls::name(Libcall::Mul(vt)),
                ISD::FDiv => libcalls::name(Libcall::Div(vt)),
                _ => None,
            };
        }
        let shift = matches!(op, ISD::Shl | ISD::Srl | ISD::Sra);
        // a forced shift call (a module that links the shift routine anyway) takes the size routine of a
        // constant-or-variable amount, not one that is slower for a size gain only a size-tuned function wants
        let forced = self.force_shift_calls && shift && !self.maps.size_variable_only.contains_key(&(op, vt));
        let sized = (self.opt_for_size || forced)
            .then(|| self.maps.size_libcall_names.get(&(op, vt)).copied())
            .flatten();
        sized.or_else(|| self.maps.libcall_names.get(&(op, vt)).copied())
    }

    pub fn set_operation_action(
        &mut self,
        op: ISD,
        vt: MVT,
        action: Action,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).operations.insert((op, vt), action);
    }

    fn set_ops(
        &mut self,
        ops: &[ISD],
        vts: &[MVT],
        action: Action,
    ) {
        for &op in ops {
            for &vt in vts {
                self.set_operation_action(op, vt, action);
            }
        }
    }

    /// `setOperationPromotedToType`.
    pub fn set_operation_promoted_to_type(
        &mut self,
        op: ISD,
        vt: MVT,
        to: MVT,
    ) {
        self.set_operation_action(op, vt, Action::Promote);
        std::sync::Arc::make_mut(&mut self.maps).promoted_to.insert((op, vt), to);
    }

    pub fn set_load_ext_action(
        &mut self,
        ext: LoadExt,
        vt: MVT,
        mem: MVT,
        action: Action,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).load_ext.insert((ext, vt, mem), action);
    }

    pub fn set_trunc_store_action(
        &mut self,
        vt: MVT,
        mem: MVT,
        action: Action,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).trunc_store.insert((vt, mem), action);
    }

    pub fn set_cond_code_action(
        &mut self,
        cc: CondCode,
        vt: MVT,
        action: Action,
    ) {
        std::sync::Arc::make_mut(&mut self.maps).cond_code.insert((cc, vt), action);
    }

    pub fn add_legal_fp_immediate(
        &mut self,
        vt: MVT,
        bits: u128,
    ) {
        self.legal_fp_immediates.push((vt, bits));
    }

    /// `ShouldShrinkFPConstant`.
    pub fn should_shrink_fp_constant(
        &self,
        vt: MVT,
    ) -> bool {
        vt == MVT::F80 || !self.keeps_f64_constants
    }

    /// `isFPImmLegal`.
    pub fn is_fp_imm_legal(
        &self,
        vt: MVT,
        bits: u128,
    ) -> bool {
        self.legal_fp_immediates.contains(&(vt, bits))
            || self.fp_imm_predicate.is_some_and(|legal| legal(vt, bits, self.opt_for_size))
    }

    pub fn set_fp_imm_predicate(
        &mut self,
        predicate: fn(MVT, u128, bool) -> bool,
    ) {
        self.fp_imm_predicate = Some(predicate);
    }

    /// The function selected next is built for size, so its `i64` divide is the
    /// size routine where the target has one.
    pub fn set_small_divide(
        &mut self,
        small: bool,
    ) {
        self.small_divide = small;
    }

    pub fn small_divide(&self) -> bool {
        self.small_divide
    }

    pub fn set_fast_divide_in_loops(
        &mut self,
        on: bool,
    ) {
        self.fast_divide_in_loops = on;
    }

    pub fn fast_divide_in_loops(&self) -> bool {
        self.fast_divide_in_loops
    }

    pub fn set_size_only(
        &mut self,
        size_only: bool,
    ) {
        self.size_only = size_only;
    }

    pub fn size_only(&self) -> bool {
        self.size_only
    }

    pub fn set_byte_equality(
        &mut self,
        on: bool,
    ) {
        self.byte_equality = on;
    }

    pub fn byte_equality(&self) -> bool {
        self.byte_equality
    }

    pub fn set_assert_sext_keeps_high(
        &mut self,
        on: bool,
    ) {
        self.assert_sext_keeps_high = on;
    }

    pub fn assert_sext_keeps_high(&self) -> bool {
        self.assert_sext_keeps_high
    }

    /// `shouldOptimizeForSize` of the function being selected.
    pub fn is_opt_for_size(&self) -> bool {
        self.opt_for_size
    }

    pub fn set_inline_constant_mul(
        &mut self,
        decomposes: fn(u128) -> bool,
    ) {
        self.inline_constant_mul = Some(decomposes);
    }

    /// Whether a multiply by `c` stays inline here (see `inline_constant_mul`).
    pub fn multiply_by_constant_stays_inline(
        &self,
        c: u128,
    ) -> bool {
        !self.opt_for_size && self.inline_constant_mul.is_some_and(|decomposes| decomposes(c))
    }

    /// Use the shift routines (`set_size_shift_call`) whether or not the
    /// function is tuned for size.
    pub fn set_force_shift_calls(
        &mut self,
        force: bool,
    ) {
        self.force_shift_calls = force;
    }

    /// `DAG.shouldOptForSize()` for the function selected next.
    pub fn set_opt_for_size(
        &mut self,
        size: bool,
    ) {
        self.opt_for_size = size;
    }

    /// `isMultiStoresCheaperThanBitsMerge`: the pin says so for a float and
    /// an integer half only.
    ///
    /// DEVIATE SD240: a target with the far pointer (m16) says so for two
    /// integer halves too. A far pointer or a `long` stored from its two
    /// words is two word stores, where merging them took `shl; movzx; add`
    /// (QCport -Os: 131 stack arguments built so). Owner w10. Exit: none
    /// while the DAG keeps `BUILD_PAIR`. Review 2026-12-10.
    pub fn is_multi_stores_cheaper_than_bits_merge(
        &self,
        lo: EVT,
        hi: EVT,
    ) -> bool {
        (lo.is_floating_point() && hi.is_integer())
            || (lo.is_integer() && hi.is_floating_point())
            || (self.types.has_far_pointer() && lo.is_integer() && hi.is_integer())
    }

    /// `getOperationAction`: a type with no simple form is expanded.
    pub fn operation_action(
        &self,
        op: ISD,
        vt: EVT,
    ) -> Action {
        match vt {
            EVT::Int(_) => Action::Expand,
            EVT::Simple(vt) => self.maps.operations.get(&(op, vt)).copied().unwrap_or(Action::Legal),
        }
    }

    pub fn load_ext_action(
        &self,
        ext: LoadExt,
        vt: MVT,
        mem: MVT,
    ) -> Action {
        self.maps.load_ext.get(&(ext, vt, mem)).copied().unwrap_or(Action::Legal)
    }

    pub fn trunc_store_action(
        &self,
        vt: MVT,
        mem: MVT,
    ) -> Action {
        self.maps.trunc_store.get(&(vt, mem)).copied().unwrap_or(Action::Legal)
    }

    /// `isTruncStoreLegal`
    /// ([TargetLowering.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h)):
    /// the value type is legal, and the truncating store is.
    pub fn is_trunc_store_legal(
        &self,
        vt: MVT,
        mem: MVT,
    ) -> bool {
        self.types.is_type_legal(EVT::Simple(vt)) && self.trunc_store_action(vt, mem) == Action::Legal
    }

    /// `isTruncStoreLegalOrCustom`.
    pub fn is_trunc_store_legal_or_custom(
        &self,
        vt: MVT,
        mem: MVT,
    ) -> bool {
        self.types.is_type_legal(EVT::Simple(vt))
            && matches!(self.trunc_store_action(vt, mem), Action::Legal | Action::Custom)
    }

    pub fn cond_code_action(
        &self,
        cc: CondCode,
        vt: MVT,
    ) -> Action {
        self.maps.cond_code.get(&(cc, vt)).copied().unwrap_or(Action::Legal)
    }

    /// `TargetLowering::isSExtCheaperThanZExt(FromTy, ToTy)`.
    pub fn is_sext_cheaper_than_zext(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        self.sext_cheaper_i32_i64 && from == EVT::Simple(MVT::I32) && to == EVT::Simple(MVT::I64)
    }

    /// `Subtarget.canUseCMOV()`.
    pub fn can_use_cmov(&self) -> bool {
        self.cmov
    }

    /// Whether `X86TargetLowering::BuildSDIVPow2` makes the `cmov` form for
    /// `vt`: `canUseCMOV`, and `i16`, `i32` or (`is64Bit`) `i64` (no `i8`).
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24348>
    pub fn sdiv_pow2_with_cmov(
        &self,
        vt: EVT,
    ) -> bool {
        let Some(is64) = self.sdiv_pow2_cmov else {
            return false;
        };
        match vt {
            EVT::Simple(MVT::I16 | MVT::I32) => true,
            EVT::Simple(MVT::I64) => is64,
            _ => false,
        }
    }

    /// `isOperationLegal`.
    pub fn is_operation_legal(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.types.is_type_legal(vt) && self.operation_action(op, vt) == Action::Legal
    }

    pub fn is_operation_legal_or_custom(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.types.is_type_legal(vt) && matches!(self.operation_action(op, vt), Action::Legal | Action::Custom)
    }

    /// `isOperationLegalOrCustomOrPromote`.
    pub fn is_operation_legal_or_custom_or_promote(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        self.types.is_type_legal(vt)
            && matches!(
                self.operation_action(op, vt),
                Action::Legal | Action::Custom | Action::Promote
            )
    }

    /// `getTypeToPromoteTo`: the recorded type, else the next wider integer
    /// (or float) type that is legal and handles `op`.
    pub fn type_to_promote_to(
        &self,
        op: ISD,
        vt: MVT,
    ) -> MVT {
        if let Some(&to) = self.maps.promoted_to.get(&(op, vt)) {
            return to;
        }
        let ladder: &[MVT] = if INTS.contains(&vt) { &INTS } else { &FLOATS };
        let at = ladder.iter().position(|&candidate| candidate == vt).expect("promoting a non-numeric type");
        ladder[at + 1..]
            .iter()
            .copied()
            .find(|&wider| self.is_operation_legal_or_custom(op, wider.into()))
            .expect("no wider type handles the operation")
    }

    /// `TargetLoweringBase::initActions`, for the opcodes [`ISD`] has.
    pub fn init_actions(&mut self) {
        use ISD::*;
        for vt in ALL {
            self.set_ops(
                &[
                    SMin,
                    SMax,
                    UMin,
                    UMax,
                    Abs,
                    AbsMinPoison,
                    FShl,
                    FShr,
                    SAddSat,
                    AvgFloorS,
                    AvgFloorU,
                    AvgCeilS,
                    AvgCeilU,
                    UAddSat,
                    SSubSat,
                    USubSat,
                    SMulFix,
                    UMulFix,
                    SMulFixSat,
                    UMulFixSat,
                    SShlSat,
                    UShlSat,
                    SCmp,
                    UCmp,
                ],
                &[vt],
                Action::Expand,
            );
            self.set_ops(&[SAddO, SSubO, UAddO, USubO, SMulO, UMulO], &[vt], Action::Expand);
            self.set_ops(
                &[UAddOCarry, USubOCarry, SAddOCarry, SSubOCarry, SetCCCarry, AddC, AddE, SubC, SubE],
                &[vt],
                Action::Expand,
            );
            self.set_ops(&[CtLzZeroPoison, CtTzZeroPoison, CtLs, BitReverse, Parity], &[vt], Action::Expand);
            // A pair that is a legal register type (an i32 at m16) is built by
            // shifts and an or.
            self.set_ops(&[BuildPair], &[vt], Action::Expand);
        }
        self.set_operation_action(Trap, MVT::Other, Action::Expand);
        self.set_operation_action(DebugTrap, MVT::Other, Action::Expand);
        self.set_ops(&[ConstantFP], &FLOATS, Action::Expand);
        // TargetLoweringBase.cpp#L864: a target without minss/maxss has no pseudo min/max node,
        // so `combineSelectToPseudoMinMax` (legal-or-custom only) leaves the select alone.
        self.set_ops(&[PseudoFMin, PseudoFMax], &FLOATS, Action::Expand);
    }

    /// m32 for i486/P5 with the x87 and no SSE: `initActions`, then the
    /// constructor's calls in the pin's order (L179 to L2830, scalar arms).
    /// `ptr` is the pointer type (`PtrVT`).
    pub fn x86_32(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        Self::x86(TargetLoweringInfo::x86_32(), features, ptr, false)
    }

    /// m64: the constructor's `is64Bit()` arms (L183..L533, the scalar integer
    /// ones); the x87 and SSE conversions stay as m32 until D64-4.
    pub fn x86_64(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        Self::x86(TargetLoweringInfo::x86_64(), features, ptr, true)
    }

    /// m16: the same constructor calls over the 386 register file, with `i16`
    /// pointers and the far pointer's type action.
    ///
    /// DEVIATE SD307: the pin has no 16-bit model, so this table is the 32-bit
    /// one, kept to what the old m16 selector covers (`tests/m16_coverage.rs`
    /// lists the functions that must stay selectable), not derived from the
    /// pin. Owner sdag-c. Exit: the m16 shadow against the old selector
    /// matches (11-20); the table is then checked against the old selector's
    /// output until the old selector is deleted at the switch. Review
    /// 2026-12-10.
    pub fn x86_16(
        features: Features,
        ptr: MVT,
    ) -> TargetLowering {
        use ISD::*;
        let info = TargetLoweringInfo::x86_16();
        let mut t = if features.soft_float {
            Self::x86(info.with_soft_float(), features, ptr, false)
        } else {
            Self::x86(info, features, ptr, false)
        };
        t.set_positive_fp_constants();
        // DEVIATE SD542: the pin promotes an `i16` trailing count to `i32` to
        // avoid the 0x66 prefix; at m16 `i16` is the native width and the
        // promoted form is a dword scan plus a dword spill. Owner m16.
        // Exit 2026-11-20.
        if !features.bmi {
            t.set_ops(&[CtTz], &[MVT::I16], Action::Custom);
            t.set_ops(&[CtTzZeroPoison], &[MVT::I16], Action::Legal);
        }
        if !features.soft_float {
            return t;
        }
        // The x87 lowerings of a compare, a select and an extension have no
        // registers to work in: the type legalizer softens them first.
        t.set_ops(&[Select, SetCC, FpExtend], &[MVT::F32, MVT::F64], Action::Expand);
        t.set_ops(&[FpToSInt, FpToUInt, SIntToFp, UIntToFp], &[MVT::I8, MVT::I16, MVT::I32, MVT::I64], Action::Expand);
        // `RTLIB::ADD_F32` and the rest: compiler-rt's names.
        t.set_soft_float_libcall_names();
        t
    }

    /// AArch64: `initActions`, then the scalar integer arms of
    /// `AArch64TargetLowering::AArch64TargetLowering`
    /// ([L481](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L481)),
    /// in the pin's order. Floats, vectors, atomics, `i128` and the arms whose
    /// [`ISD`] node does not exist yet are not translated.
    ///
    /// A `Custom` arm names an operation the pin's `LowerOperation` handles;
    /// the legalizer must refuse it until that is translated, not expand it.
    pub fn aarch64(features: Aarch64Features) -> TargetLowering {
        use Action::*;
        use ISD::*;
        let mut t = TargetLowering::empty(TargetLoweringInfo::aarch64());
        t.sext_cheaper_i32_i64 = true;
        t.init_actions();
        let words = [MVT::I32, MVT::I64];
        // Custom lowerings (`LowerOperation`).
        t.set_ops(&[GlobalAddress, ConstantPool, JumpTable, DynamicStackAlloc], &[MVT::I64], Custom);
        t.set_ops(&[SetCC, BrCC, Select, SelectCC, Xor], &words, Custom);
        t.set_ops(&[BrCond, BrJT, BrInd], &[MVT::Other], Custom);
        t.set_ops(&[SetCCCarry], &[MVT::I64], Custom);
        // `AArch64InstrInfo.td`: `(trap)` is `BRK 1`, `(debugtrap)` is `BRK
        // 0xF000`.
        t.set_ops(&[Trap, DebugTrap], &[MVT::Other], Legal);
        t.set_ops(&[ShlParts, SraParts, SrlParts], &[MVT::I64], Custom);
        t.set_ops(&[BitReverse, CtLs], &words, Legal);
        t.set_ops(&[BuildPair], &[MVT::I64], Expand);
        // The 128-bit integer vectors (`LowerBUILD_VECTOR`,
        // `LowerVECTOR_SHUFFLE`, `LowerINSERT_VECTOR_ELT`); a node of a
        // type the target does not take never exists.
        let vectors = [MVT::V16I8, MVT::V8I16, MVT::V4I32, MVT::V2I64];
        let float_vectors = [MVT::V4F32, MVT::V2F64, MVT::V2F32];
        // The constructor's loop over the vector types: an extending load or
        // a truncating store of a vector is expanded. DEVIATE LV-X6 (owner
        // llrm-arm64-vec, exit 2026-11-24): the `Custom` `v2i32`<-`v2i8`
        // family (`LowerLOAD`) is expanded too.
        let every_vector = [
            MVT::V8I8,
            MVT::V4I16,
            MVT::V2I32,
            MVT::V2F32,
            MVT::V16I8,
            MVT::V8I16,
            MVT::V4I32,
            MVT::V2I64,
            MVT::V4F32,
            MVT::V2F64,
            MVT::V32I8,
            MVT::V16I16,
            MVT::V8I32,
            MVT::V4I64,
            MVT::V8F32,
            MVT::V4F64,
            MVT::V64I8,
            MVT::V32I16,
            MVT::V16I32,
            MVT::V8I64,
        ];
        for vt in every_vector {
            for inner in every_vector {
                t.set_trunc_store_action(vt, inner, Expand);
                for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
                    t.set_load_ext_action(ext, vt, inner, Expand);
                }
            }
        }
        t.set_ops(&[BuildVector, VectorShuffle, InsertVectorElt], &vectors, Custom);
        t.set_ops(&[BuildVector, VectorShuffle, InsertVectorElt], &float_vectors, Custom);
        // `addTypeForNEON`: the shifts are lowered to `VSHL`/`VLSHR`/`VASHR` (`LowerVectorSRA_SRL_SHL`).
        t.set_ops(&[Shl, Srl, Sra], &vectors, Custom);
        t.set_ops(&[FpToSInt, FpToUInt, SIntToFp, UIntToFp], &words, Custom);
        // `addTypeForNEON`: the conversions of a vector are
        // `LowerVectorINT_TO_FP` and `LowerVectorFP_TO_INT`, by the
        // operand's type.
        t.set_ops(&[SIntToFp, UIntToFp], &vectors, Custom);
        t.set_ops(&[FpToSInt, FpToUInt], &[MVT::V4F32, MVT::V2F64, MVT::V2F32], Custom);
        t.set_ops(&[VAStart, VACopy, VAArg], &[MVT::Other], Custom);
        t.set_ops(&[VAEnd, StackSave, StackRestore], &[MVT::Other], Expand);
        // The pin asks for `STACKSAVE` at `MVT::Other`; here the node's value
        // type is asked.
        t.set_ops(&[StackSave], &[MVT::I64], Expand);
        t.set_ops(&[FShr, FShl], &words, Custom);
        t.set_ops(&[Rotl], &words, Expand);
        t.set_ops(&[MulHU, MulHS], &[MVT::I32], Expand);
        t.set_ops(&[UMulLoHi, SMulLoHi], &words, Expand);
        if features.cssc {
            t.set_ops(&[CtPop, CtTz, Abs, SMax, UMax, SMin, UMin], &words, Legal);
            t.set_ops(&[CtPop, Parity], &[MVT::I128], Expand);
            t.set_ops(&[CtTz], &[MVT::I128], Expand);
        } else {
            // DEVIATE SD533 (exit: NEON `cnt` + `addv` when vector MIR lands):
            // the pin's `LowerCTPOP` is `Custom` (v8i8 `cnt`, `uaddlv`); with
            // no vector MIR the population count is the scalar
            // bit-twiddle (`expandCTPOP`), so that the node is
            // expanded, not refused.
            t.set_ops(&[CtPop], &words, Expand);
            t.set_ops(&[Abs], &words, Custom);
            t.set_ops(&[CtPop, Parity], &[MVT::I128], Custom);
            t.set_ops(&[Parity], &[MVT::I64], Custom);
        }
        t.set_ops(&[SDivRem, SRem, UDivRem, URem], &words, Expand);
        t.set_ops(&[SAddO, UAddO, SSubO, USubO, SMulO, UMulO], &words, Custom);
        t.set_ops(&[UAddOCarry, USubOCarry, SAddOCarry, SSubOCarry], &words, Custom);
        for vt in INTS {
            t.set_load_ext_action(LoadExt::SExtLoad, vt, MVT::I1, Expand);
        }

        // Scalar floats (`hasFPARMv8()`; `f16`, `bf16` and `f128` are not types
        // here).
        let floats = [MVT::F32, MVT::F64];
        // `setOperationAction({SETCC, BR_CC, SELECT, SELECT_CC}, {f32, f64},
        // Custom)`
        t.set_ops(&[SetCC, BrCC, Select, SelectCC], &floats, Custom);
        t.set_ops(&[FRem], &floats, LibCall);
        t.set_ops(&[FSin, FCos], &floats, Expand);
        t.set_ops(&[FCopySign], &floats, Custom);
        t.set_ops(&[FpExtend], &[MVT::F32], Legal);
        t.set_ops(&[FpExtend], &[MVT::F64], Custom);
        t.set_ops(&[FpRound], &floats, Custom);
        t.set_ops(&[FRint, LRint], &floats, Legal);
        t.set_ops(&[BitCast], &[MVT::I16], Custom);
        for vt in floats {
            for mem in floats {
                t.set_load_ext_action(LoadExt::ExtLoad, vt, mem, Expand);
            }
        }
        t.set_trunc_store_action(MVT::F64, MVT::F32, Expand);

        // `f128` is a legal type (`FPR128`) without arithmetic: the operations
        // are RTLIB calls, the compares and selects and the conversions to and
        // from it are `Custom` (`LowerF128Call`).
        let quad = [MVT::F128];
        t.set_ops(&[FAdd, FSub, FMul, FDiv], &quad, LibCall);
        // The sign-bit operations are Expand, a rewrite of the bits
        // (`ExpandFNEG`). The pin's FCOS, FMA, FRINT, FSIN and FSQRT
        // are Expand too, to libcalls this table does not name yet.
        // <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L641>
        t.set_ops(&[FAbs, FCopySign, FNeg], &quad, Expand);
        t.set_ops(&[ConstantFP], &quad, Expand);
        t.set_ops(&[SetCC, BrCC, Select, SelectCC], &quad, Custom);
        t.set_ops(&[FpExtend], &quad, Custom);

        // Atomics.
        let narrow = [MVT::I8, MVT::I16, MVT::I32, MVT::I64];
        // `TargetLoweringBase`: the success flag is a compare of the old value.
        t.set_ops(&[AtomicCmpSwapWithSuccess], &narrow, Expand);
        t.set_ops(&[AtomicCmpSwap], &[MVT::I128], Custom);
        let sub = if !features.lse && !features.outline_atomics { LibCall } else { Expand };
        t.set_ops(&[AtomicLoadSub], &words, sub);
        t.set_ops(&[AtomicLoadAnd], &words, Custom);
        if features.outline_atomics && !features.lse {
            t.set_ops(&[AtomicCmpSwap], &[MVT::I8, MVT::I16, MVT::I32, MVT::I64, MVT::I128], LibCall);
            t.set_ops(&[AtomicSwap, AtomicLoadAdd, AtomicLoadOr, AtomicLoadClr, AtomicLoadXor], &narrow, LibCall);
        }
        t.set_ops(&[Load, Store], &[MVT::I128], Custom);
        if features.lse2 {
            t.set_ops(&[AtomicLoad, AtomicStore], &[MVT::I128], Custom);
        }
        t
    }

    fn x86(
        types: TargetLoweringInfo,
        features: Features,
        ptr: MVT,
        is64: bool,
    ) -> TargetLowering {
        use Action::*;
        use ISD::*;
        let mut t = TargetLowering::empty(types);
        t.cmov = features.cmov;
        t.sdiv_pow2_cmov = features.cmov.then_some(is64);
        t.init_actions();
        let ints = [MVT::I8, MVT::I16, MVT::I32];
        // `if (VT == MVT::i64 && !Subtarget.is64Bit()) continue;`
        let native: &[MVT] = if is64 { &[MVT::I8, MVT::I16, MVT::I32, MVT::I64] } else { &ints };
        let words: &[MVT] = if is64 { &[MVT::I32, MVT::I64] } else { &[MVT::I32] };
        let ints64 = [MVT::I8, MVT::I16, MVT::I32, MVT::I64];

        // L186..L203
        for vt in INTS {
            t.set_load_ext_action(LoadExt::SExtLoad, vt, MVT::I1, Promote);
        }
        for (vt, mem) in [
            (MVT::I64, MVT::I32),
            (MVT::I64, MVT::I16),
            (MVT::I64, MVT::I8),
            (MVT::I32, MVT::I16),
            (MVT::I32, MVT::I8),
            (MVT::I16, MVT::I8),
            (MVT::F64, MVT::F32),
        ] {
            t.set_trunc_store_action(vt, mem, Expand);
        }
        for vt in FLOATS {
            t.set_cond_code_action(CondCode::SetOEq, vt, Expand);
            t.set_cond_code_action(CondCode::SetUNe, vt, Expand);
        }
        // L205..L240
        if features.cmov {
            t.set_ops(&[Abs], &native[1..], Custom);
        }
        t.set_ops(&[SSubSat], native, Custom);
        let shld = if features.shld_slow { Custom } else { Legal };
        t.set_ops(&[FShl, FShr], &[MVT::I8, MVT::I16], Custom);
        t.set_ops(&[FShl, FShr], words, shld);
        // L508: the double-width shifts (i64 at 32 bits)
        t.set_ops(&[ShlParts, SraParts, SrlParts], words, Custom);
        // L241..L300: the x87 conversions
        t.set_ops(&[UIntToFp], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[UIntToFp], &[MVT::I32, MVT::I64], Custom);
        t.set_ops(&[SIntToFp], &[MVT::I8], Promote);
        t.set_ops(&[SIntToFp], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToSInt], &[MVT::I8], Promote);
        t.set_ops(&[FpToSInt], &[MVT::I16, MVT::I32, MVT::I64], Custom);
        t.set_ops(&[FpToUInt], &[MVT::I8, MVT::I16], Promote);
        t.set_ops(&[FpToUInt], &[MVT::I32, MVT::I64], Custom);
        // L300-L303, L850: `lrint` of an x87 value is `fist`
        t.set_ops(&[LRint, LLRint], &[MVT::F32, MVT::F64, MVT::F80], Custom);
        // an i64 result at 32 bits is expanded after the same `fist`, as
        // FpToSInt's is
        t.set_ops(&[LRint, LLRint], &[MVT::I64], Custom);
        // L316
        if !features.sse2 {
            t.set_ops(&[BitCast], &[MVT::F32, MVT::I32], Expand);
            if is64 {
                t.set_ops(&[BitCast], &[MVT::F64, MVT::I64], Expand);
            }
        } else if !is64 {
            t.set_operation_action(BitCast, MVT::I64, Custom);
        }
        // L337..L370
        t.set_ops(&[MulHS, MulHU, SDiv, UDiv, SRem, URem], &ints64, Expand);
        // The pin divides an i8 or i16 by a constant with a widened multiply
        // (BuildSDIV finds neither MULHS nor SMUL_LOHI there); only i32
        // has the two-result multiply (`MUL32r`).
        t.set_ops(&[SMulLoHi, UMulLoHi], &[MVT::I8, MVT::I16], Expand);
        // At 64 bits BuildSDIV/BuildUDIV find no two-result multiply for an
        // `i32` either and widen it (`imul rax, rcx, 0xCCCCCCCD; shr rax,
        // 35`), where ours took the one-operand `mul ecx` (edx:eax).
        if is64 {
            t.set_ops(&[SMulLoHi, UMulLoHi], &[MVT::I32], Expand);
        }
        // fixed-point multiply is expanded by `expandFixedPointMul` (the base
        // default)
        t.set_ops(&[SMulFix], &[MVT::I8, MVT::I16, MVT::I32], Expand);
        t.set_operation_action(BrJT, MVT::Other, Expand);
        t.set_operation_action(BrCond, MVT::Other, Custom);
        t.set_ops(&[BrCC, SelectCC], &[MVT::F32, MVT::F64, MVT::F80, MVT::I8, MVT::I16, MVT::I32, MVT::I64], Expand);
        if is64 {
            t.set_operation_action(SignExtendInReg, MVT::I32, Legal);
        }
        t.set_ops(&[SignExtendInReg], &[MVT::I16, MVT::I8], Legal);
        t.set_operation_action(SignExtendInReg, MVT::I1, Expand);
        t.set_ops(&[FRem], &FLOATS, LibCall);
        // L372..L468: bit counts
        for op in [CtTz, CtTzZeroPoison] {
            t.set_operation_promoted_to_type(op, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(op, MVT::I16, MVT::I32);
        }
        if !features.bmi {
            t.set_ops(&[CtTz], words, Custom);
            t.set_ops(&[CtTzZeroPoison], words, Legal);
        }
        if features.lzcnt {
            t.set_operation_promoted_to_type(CtLz, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(CtLzZeroPoison, MVT::I8, MVT::I32);
        } else {
            t.set_ops(&[CtLz, CtLzZeroPoison], native, Custom);
        }
        t.set_ops(&[Parity], native, Custom);
        if features.popcnt {
            t.set_operation_promoted_to_type(CtPop, MVT::I8, MVT::I32);
            t.set_operation_promoted_to_type(CtPop, MVT::I16, MVT::I32);
        } else {
            t.set_ops(&[CtPop], &ints64, Custom);
        }
        // L469..L520
        if !features.movbe {
            t.set_operation_action(BSwap, MVT::I16, Expand);
        }
        t.set_ops(&[Select, SetCC], &FLOATS, Custom);
        t.set_ops(&[Select, SetCC], native, Custom);
        t.set_ops(&[ConstantPool, GlobalAddress, ExternalSymbol, JumpTable], &[ptr], Custom);
        if is64 {
            t.set_ops(&[ConstantPool, GlobalAddress, ExternalSymbol, JumpTable], words, Custom);
        }
        t.set_operation_action(Trap, MVT::Other, Legal);
        t.set_operation_action(DebugTrap, MVT::Other, Legal);
        t.set_operation_action(DynamicStackAlloc, ptr, Custom);
        // The pin expands these to a stack pointer copy; the same, as Custom.
        t.set_operation_action(StackSave, ptr, Custom);
        t.set_operation_action(StackRestore, MVT::Other, Custom);
        // L559..L566: 32-bit va_list is a pointer
        t.set_operation_action(VAStart, MVT::Other, Custom);
        t.set_ops(&[VAEnd, VACopy], &[MVT::Other], Expand);
        if features.sse2 {
            // L618..L674: f32 and f64 use SSE
            t.set_load_ext_action(LoadExt::ExtLoad, MVT::F64, MVT::F32, Expand);
            for vt in [MVT::F32, MVT::F64] {
                // ANDPD simulates FABS, XORP FNEG, ANDPD and ORPD FCOPYSIGN
                t.set_ops(&[FAbs, FNeg, FCopySign], &[vt], Custom);
                t.set_ops(&[FAdd, FSub], &[vt], Custom);
                // L1041: `SetFPMinMaxAction`
                t.set_ops(&[PseudoFMin, PseudoFMax], &[vt], Legal);
                // (no FSINCOS node here: nothing builds one)
                t.set_ops(&[FSin, FCos], &[vt], Expand);
            }
            t.set_ops(&[FpExtend], &[MVT::F32, MVT::F64], Custom);
        } else {
            // L720..L760: x87 float classes (no SSE)
            for vt in [MVT::F32, MVT::F64] {
                t.set_ops(&[Undef, Poison], &[vt], Expand);
                t.set_operation_action(FCopySign, vt, Expand);
            }
        }
        // L806..L870: f80
        t.set_ops(&[Undef, Poison], &[MVT::F80], Expand);
        t.set_operation_action(FCopySign, MVT::F80, Expand);
        t.keeps_f64_constants = features.sse2;
        // L764..L860: the x87 loads of 0.0 and 1.0 (`fldz`, `fld1`, and `fchs`)
        // L690..L706: SSE has only +0.0 (`xorps`, `xorpd`)
        for (vt, one, sign) in [
            (MVT::F32, 0x3F80_0000u128, 1u128 << 31),
            (MVT::F64, 0x3FF0_0000_0000_0000, 1u128 << 63),
            (MVT::F80, (0x3FFFu128 << 64) | (1u128 << 63), 1u128 << 79),
        ] {
            if features.sse2 && vt != MVT::F80 {
                t.add_legal_fp_immediate(vt, 0);
                continue;
            }
            for bits in [0, sign, one, one | sign] {
                t.add_legal_fp_immediate(vt, bits);
            }
        }
        // L2757..L2776
        t.set_ops(
            &[SAddO, UAddO, SSubO, USubO, SMulO, UMulO, UAddOCarry, USubOCarry, SAddOCarry, SSubOCarry, SetCCCarry],
            native,
            Custom,
        );
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lowering::TypeAction;

    fn a64() -> TargetLowering {
        TargetLowering::aarch64(Aarch64Features::default())
    }

    /// A function tuned for size called the speed routine of a divide: a
    /// target's smaller one was never asked for.
    #[test]
    fn a_function_tuned_for_size_calls_the_size_routine() {
        let mut t = TargetLowering::empty(TargetLoweringInfo::aarch64());
        t.set_libcall_name(ISD::UDiv, MVT::I64, "__fast");
        t.set_size_libcall_name(ISD::UDiv, MVT::I64, "__small");
        t.set_libcall_name(ISD::SDiv, MVT::I64, "__only");
        assert_eq!(t.libcall_name(ISD::UDiv, EVT::I64), Some("__fast"));
        t.set_opt_for_size(true);
        assert_eq!(t.libcall_name(ISD::UDiv, EVT::I64), Some("__small"));
        assert_eq!(t.libcall_name(ISD::SDiv, EVT::I64), Some("__only"), "no size routine: the one routine");
    }

    /// z80 prices the shift routines against the inline shifts in a function
    /// that is not tuned for size (`set_force_shift_calls`): -O1 call_keeps
    /// 1876 B -> 1473 B. The switch is off for every other target and
    /// for z80 outside that form; on, it makes shifts (not other ops) take the
    /// size routine and the size threshold, as a function tuned for size
    /// does.
    #[test]
    fn forced_shift_calls_move_shifts_only_and_default_off_changes_nothing() {
        let build = |force: bool| {
            let mut t = TargetLowering::empty(TargetLoweringInfo::aarch64());
            t.set_size_shift_call(8, 10);
            t.set_libcall_name(ISD::Shl, MVT::I32, "__fast_shl");
            t.set_size_libcall_name(ISD::Shl, MVT::I32, "__small_shl");
            t.set_libcall_name(ISD::Mul, MVT::I32, "__fast_mul");
            t.set_size_libcall_name(ISD::Mul, MVT::I32, "__small_mul");
            t.set_force_shift_calls(force);
            t
        };
        let (off, on) = (build(false), build(true));
        // by 13 bits of 32 with 8-bit granules: (4 - 1) * 5 = 15 >= 10
        assert!(!off.calls_for_size_shift(13, 32) && on.calls_for_size_shift(13, 32));
        assert_eq!(off.libcall_name(ISD::Shl, EVT::I32), Some("__fast_shl"));
        assert_eq!(on.libcall_name(ISD::Shl, EVT::I32), Some("__small_shl"));
        for t in [&off, &on] {
            assert_eq!(t.libcall_name(ISD::Mul, EVT::I32), Some("__fast_mul"), "a multiply keeps the speed routine");
        }
        // sized functions are unchanged by the switch
        let mut sized = build(false);
        sized.set_opt_for_size(true);
        assert!(sized.calls_for_size_shift(13, 32));
        assert_eq!(sized.libcall_name(ISD::Mul, EVT::I32), Some("__small_mul"));
    }

    /// z80's 64-bit size shift routine takes a variable amount (`set_size_variable_libcall_name`) and is slower
    /// than the speed one by a factor of 3: a forced shift call at -O1 (`set_force_shift_calls`) took it, so
    /// call_cleanup_keeps_pair was 88 B smaller and 523k T slower. Only a function tuned for size takes it, and
    /// a constant amount never does.
    #[test]
    fn a_variable_only_size_routine_is_for_size_tuned_functions_alone() {
        let build = |force: bool, size: bool| {
            let mut t = TargetLowering::empty(TargetLoweringInfo::aarch64());
            t.set_libcall_name(ISD::Shl, MVT::I64, "__fast_shl");
            t.set_size_variable_libcall_name(ISD::Shl, MVT::I64, "__small_shl");
            t.set_force_shift_calls(force);
            t.set_opt_for_size(size);
            t
        };
        assert_eq!(build(true, false).libcall_name(ISD::Shl, EVT::I64), Some("__fast_shl"), "forced: the speed routine");
        assert_eq!(build(false, true).libcall_name(ISD::Shl, EVT::I64), Some("__small_shl"), "sized: the size routine");
        assert!(!build(false, true).has_size_libcall_name(ISD::Shl, EVT::I64), "a constant amount has no size call");
    }

    /// `PSEUDO_FMIN`/`FMAX` are Expand unless the target says otherwise (TargetLoweringBase.cpp#L864).
    /// They defaulted to Legal, so `combineSelectToPseudoMinMax` made a node wasm32 could not select
    /// (jpeg_single, stft_single: `CannotSelect("PseudoFMin")`). Only x86 with SSE has `minss`/`maxss`.
    #[test]
    fn the_pseudo_min_max_is_expand_except_on_x86_sse() {
        let sse = Features { sse2: true, ..Features::default() };
        let tables = [
            ("aarch64", a64(), Action::Expand),
            ("x86_32 x87", TargetLowering::x86_32(Features::default(), MVT::I32), Action::Expand),
            ("x86_64 sse2", TargetLowering::x86_64(sse, MVT::I64), Action::Legal),
        ];
        for (name, t, want) in tables {
            for op in [ISD::PseudoFMin, ISD::PseudoFMax] {
                for vt in [MVT::F32, MVT::F64] {
                    assert_eq!(t.operation_action(op, EVT::from(vt)), want, "{name} {op:?} {vt:?}");
                }
            }
        }
    }

    /// An x86 table's `i8` set-cc result and `i8` register class were the
    /// only ones: AArch64 compares into an `i32` and has no `i8` register.
    #[test]
    fn aarch64_has_only_word_registers_and_an_i32_compare() {
        let t = a64();
        assert_eq!(t.types.type_conversion(EVT::I8), (TypeAction::PromoteInteger, EVT::I32));
        assert_eq!(t.types.type_conversion(EVT::I16), (TypeAction::PromoteInteger, EVT::I32));
        assert_eq!(t.types.type_conversion(EVT::I64).0, TypeAction::Legal);
        assert_eq!(t.types.type_conversion(EVT::Simple(MVT::I128)).0, TypeAction::ExpandInteger);
        assert_eq!(t.types.set_cc_result_type(EVT::I64), EVT::I32);
    }

    /// The pin marks SETCC, BR_CC and SELECT_CC `Custom` for `i32` and `i64`.
    /// A table that left them `Legal` would let the legalizer pass them on
    /// to a selector that has no pattern for them.
    #[test]
    fn aarch64_compares_are_custom_and_division_remainders_expand() {
        let t = a64();
        for op in [ISD::SetCC, ISD::BrCC, ISD::SelectCC, ISD::Select] {
            assert_eq!(t.operation_action(op, EVT::I32), Action::Custom, "{op:?}");
        }
        for op in [ISD::SetCC, ISD::BrCC, ISD::SelectCC, ISD::Select] {
            assert_eq!(t.operation_action(op, EVT::Simple(MVT::F64)), Action::Custom, "{op:?} f64");
        }
        assert_eq!(t.operation_action(ISD::SRem, EVT::I32), Action::Expand);
        assert_eq!(t.operation_action(ISD::SDiv, EVT::I32), Action::Legal);
        assert_eq!(t.operation_action(ISD::Add, EVT::I64), Action::Legal);
        assert_eq!(t.operation_action(ISD::MulHU, EVT::I32), Action::Expand);
        assert_eq!(t.operation_action(ISD::MulHU, EVT::I64), Action::Legal);
    }

    /// Without LSE the read-modify-writes are helper calls (or an LL/SC loop
    /// when outlining is off); with it they are instructions.
    #[test]
    fn aarch64_atomics_follow_lse_and_outlining() {
        let plain = a64();
        assert_eq!(plain.operation_action(ISD::AtomicLoadSub, EVT::I32), Action::LibCall);
        assert_eq!(plain.operation_action(ISD::AtomicLoadAnd, EVT::I64), Action::Custom);
        let lse = TargetLowering::aarch64(Aarch64Features { lse: true, ..Default::default() });
        assert_eq!(lse.operation_action(ISD::AtomicLoadSub, EVT::I32), Action::Expand);
        let outlined = TargetLowering::aarch64(Aarch64Features { outline_atomics: true, ..Default::default() });
        assert_eq!(outlined.operation_action(ISD::AtomicCmpSwap, EVT::I8), Action::LibCall);
        assert_eq!(outlined.operation_action(ISD::AtomicSwap, EVT::I64), Action::LibCall);
    }

    /// Scalar float arms: `frem` is a libcall, `fsin` expands, a double
    /// extends by hand and a single is free.
    #[test]
    fn aarch64_float_arms() {
        let t = a64();
        assert_eq!(t.operation_action(ISD::FRem, EVT::Simple(MVT::F64)), Action::LibCall);
        assert_eq!(t.operation_action(ISD::FSin, EVT::Simple(MVT::F32)), Action::Expand);
        assert_eq!(t.operation_action(ISD::FpExtend, EVT::Simple(MVT::F32)), Action::Legal);
        assert_eq!(t.operation_action(ISD::FpExtend, EVT::Simple(MVT::F64)), Action::Custom);
    }

    /// CSSC turns scalar `ctpop` into one instruction; without it the node
    /// is expanded until vector MIR lands (DEVIATE SD533), not `Custom`.
    #[test]
    fn aarch64_ctpop_follows_cssc() {
        assert_eq!(a64().operation_action(ISD::CtPop, EVT::I32), Action::Expand);
        let cssc = TargetLowering::aarch64(Aarch64Features { cssc: true, ..Default::default() });
        assert_eq!(cssc.operation_action(ISD::CtPop, EVT::I32), Action::Legal);
    }

    /// The action tables were std maps: SipHash on every `(op, type)` asked.
    /// Does not compile for any other hasher.
    #[test]
    fn test_action_tables_are_keyed_with_the_fixed_key_hasher() {
        let table = TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I32], false));
        let _: &llrm_support::hash::HashMap<(ISD, MVT), Action> = &table.maps.operations;
    }

    /// A per-function copy of the cached table deep-copied eight hash maps
    /// (10.8 M of tiny600's 1 644 M instructions at -O0). The copy shares them
    /// until it sets an action, and then leaves the original alone.
    #[test]
    fn test_a_table_copy_shares_its_maps_until_written() {
        let base = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut copy = base.clone();
        assert!(copy.shares_maps_with(&base), "clone deep-copied the action maps");
        let before = base.operation_action(ISD::Add, EVT::I32);
        copy.set_operation_action(ISD::Add, MVT::I32, Action::Expand);
        assert!(!copy.shares_maps_with(&base));
        assert_eq!(copy.operation_action(ISD::Add, EVT::I32), Action::Expand);
        assert_eq!(base.operation_action(ISD::Add, EVT::I32), before, "write reached the original");
    }
}
