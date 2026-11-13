//! X86's scalar cost model: `X86TTIImpl` and the `BasicTTIImplBase` rules it
//! falls to, translated from LLVM d1106deb71cc for the scalar types.
//!
//! # How it works
//! A type is first legalized (`getTypeLegalizationCost`): an `i64` on a 32-bit
//! target is two `i32` operations. The operation is then looked up in LLVM's
//! scalar tables (`ScalarVarDivCostTable`, `X86CostTbl`, the SSE tables when
//! the CPU has SSE) before the generic rules answer. Every row of
//! `tti_oracle.txt` (made by `tti_oracle.py` from the pinned `opt
//! -passes='print<cost-model>'`) is checked against these functions, on each
//! supported CPU and cost kind.
//!
//! # Deviations from LLVM
//! - T3: the SSE tables are read for `core2` only, the one supported CPU with
//!   SSE; llrm's back end emits x87 for scalar floats whatever the CPU says.
//! - T4: no instruction context, so a `zext` folded into a GEP index and an
//!   extending load are not seen free.
//! - T5: the constant costs (`int_imm_cost*`) and `is_legal_addressing_mode`
//!   are read off the source; the cost model has no printer for them.
//!
//! - T6: the 16-bit target reads the addressing hooks and the register count
//!   from its description: `getScalingFactorCost` is the cost of the address
//!   form that holds the scale (the prefix and its stall beyond the native `bx
//!   + si` form), `getNumberOfRegisters` the registers a value may sit in
//!   (six). The pin's X86TTI prices every second register 1 and has eight.
//!
//! # Example
//! `sdiv i32 %a, 8` costs `2 * ashr + lshr + add = 4` and `sdiv i32 %a, %b`
//! costs 25 on every x87 CPU (the `ScalarVarDivCostTable` entry).
//!
//! From unit test `the_arithmetic_costs_equal_the_pinned_opt_on_every_cpu`.
//!
//! # Runs
//! Read by InlineCost, the SCEV expander and loop strength reduction; nothing
//! runs it as a pass.
//!
//! # See also
//! `llrm_mir::tti`, the interface; the plan page `tti-plan` in
//! docs/architecture.

use llrm_mir::tti::*;

/// X86's scalar profile: which tables its CPU reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X86 {
    pub sse1: bool,
    pub sse2: bool,
    /// `TuningMacroFusion` (Sandy Bridge and newer tunings, and `core2`'s as
    /// X86.td lists it); `branchfusion` is a Bulldozer-and-newer feature none
    /// of the profiles has.
    pub macro_fusion: bool,
    /// `FeatureCMOV` (X86.td): the i386, i486, pentium, k5 and k6 have none.
    pub cmov: bool,
    /// Long mode: the integer registers are 64 bits wide (`is64Bit`).
    pub quad: bool,
    /// What real mode's addressing and register file make of the hooks LSR
    /// reads (T6); none: the 32-bit mode's.
    pub real_mode: Option<RealMode>,
    /// General registers a value may sit in, where the target's description
    /// states them (frame and stack pointers excluded); none: the pin's count.
    pub registers: Option<u32>,
    /// The target's tuning table (`Machine::tuning`); the pin's by default.
    pub tuning: llrm_mir::target::Tuning,
    /// `getSchedModel().LoopMicroOpBufferSize` of the cpu's model: 28 for the
    /// `SandyBridgeModel` of `core2` and `x86-64`, 0 for the `GenericModel` of
    /// the rest (`LoopMicroOpBufferSize = -1`).
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86SchedSandyBridge.td#L28
    pub loop_micro_op_buffer: u32,
    /// `getSchedModel().IssueWidth`: 4 for the `SandyBridgeModel`, 0 for the
    /// `GenericModel` of the rest.
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86SchedSandyBridge.td#L22
    pub issue_width: u32,
    /// The program's code is position independent (clang's "PIC Level"); set
    /// by `Program::new`. A symbol in long mode is then `[rip + symbol]`
    /// without index or displacement.
    pub position_independent: std::cell::Cell<bool>,
    /// This is the inliner's size view (`Tti::inline_size_view`): `CodeSize`
    /// is bytes of 32-bit code (`sizes`), not the pin's units.
    pub size_bytes: bool,
}

/// T6: the 16-bit target's address forms and registers, from its description
/// (`Machine::address_forms`, `Machine::registers`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealMode {
    /// What an address `base + scale * index` costs above the native form,
    /// for each scale some address form holds: the legal scales.
    pub scale_costs: Vec<(i64, i64)>,
    /// General registers a value may sit in.
    pub registers: u32,
    /// What a scaled index costs in instructions, once for each scaled
    /// register: the address-size prefix and the stall it takes.
    pub scaled_index_insns: u32,
    /// The space of huge pointers: an offset there carries into the selector,
    /// so an address is one register and no more.
    pub huge_space: Option<u32>,
    /// Registers the target's cheapest address form can name (a base and an
    /// index of its classes, the frame's excluded); none: any general
    /// register.
    pub address_registers: Option<u32>,
    /// The target's `-Os` growth guard for inlining (SZ1, D11): a callee that
    /// stays is inlined into an `optsize` caller only if its code does not
    /// outgrow the call. The pin has none (its `OptSizeThreshold` 50 counts
    /// instructions, not bytes); 16-bit code names it so a size row never
    /// grows.
    pub inline_size_guard: bool,
    /// What an instruction that touches a float takes in code, in percent of
    /// its `CodeSize` units: x87 moves each operand through the register
    /// stack (`qmove`: the model tied where the code grew 2 bytes).
    pub float_code_size_percent: i64,
    /// (trips, bar): an `optsize` loop of at most that many trips is fully
    /// unrolled under that unrolled size (DEVIATE U1). (0, 0): the pin's.
    pub unroll_size_few_trips: (u32, u32),
    /// DEVIATE REC-1: (depth, callee size) of the recursion taken at speed;
    /// none: the pin's.
    pub recursive_inlining: Option<(u32, i64)>,
    /// DEVIATE REC-1: the ceilings of the priced copies of a function in
    /// itself (`Tti::priced_self_inlining`); none: `recursive_inlining`.
    pub priced_self_inlining: Option<(u32, i64)>,
}

impl RealMode {
    fn scale_cost(
        &self,
        scale: i64,
    ) -> Option<i64> {
        self.scale_costs.iter().find(|(held, _)| *held == scale).map(|(_, cost)| *cost)
    }
}

impl X86 {
    /// `getIntImmCost(const APInt &, Type *)`; `zext_basic` is ZIMM.
    fn imm_cost(
        &self,
        imm: i128,
        bits: u32,
        zext_basic: bool,
    ) -> Cost {
        if bits == 0 {
            return Cost::of(i64::from(u32::MAX));
        }
        if bits > 128 || imm == 0 {
            return Cost::FREE;
        }
        // Sign-extended to a multiple of 64 bits, costed 64 bits at a time.
        let extended = if bits >= 128 { imm } else { (imm << (128 - bits)) >> (128 - bits) };
        let mut total = 0;
        let mut shift = 0;
        while shift < bits {
            let chunk = (extended >> shift) as i64;
            total += if zext_basic && bits == 64 && chunk > 0 && chunk <= i64::from(u32::MAX) {
                1
            } else {
                int_imm_chunk(chunk)
            };
            shift += 64;
        }
        Cost::of(total.max(1))
    }

    /// `getIntImmCostInst`; `zext_basic` is ZIMM.
    fn imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
        zext_basic: bool,
    ) -> Cost {
        if bits == 0 {
            return Cost::FREE;
        }
        let zext = if bits >= 128 { imm as u128 } else { (imm as u128) & ((1u128 << bits) - 1) };
        let immediate = match op {
            ImmUser::Unknown => return Cost::FREE,
            ImmUser::GetElementPtr => return if index == 0 { Cost::of(2) } else { Cost::FREE },
            ImmUser::Store => Some(0),
            ImmUser::ICmp => {
                if index == 1 && bits == 64 && (zext == 0x1_0000_0000 || zext == 0xffff_ffff) {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::And => {
                if index == 1 && bits == 64 && zext <= u128::from(u32::MAX) {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::Add | ImmUser::Sub => {
                if index == 1 && bits == 64 && zext == 0x8000_0000 {
                    return Cost::FREE;
                }
                Some(1)
            }
            ImmUser::Divide => return Cost::FREE,
            ImmUser::Mul | ImmUser::Or | ImmUser::Xor => Some(1),
            ImmUser::Shift => {
                if index == 1 {
                    return Cost::FREE;
                }
                None
            }
            ImmUser::Other => None,
        };
        let cost = self.imm_cost(imm, bits, zext_basic);
        if immediate == Some(index) {
            let constants = i64::from(bits.div_ceil(64));
            return if cost.value <= constants { Cost::FREE } else { cost };
        }
        cost
    }

    /// The bits of the integer registers.
    pub fn native(&self) -> u32 {
        if self.quad { 64 } else { 32 }
    }

    /// Whether `inst` is a plain load whose one user, in its own block, takes
    /// it as a memory operand beside a register: `op r,[m]` for the integer ALU
    /// operations and the compare (the load in either place of a commutative
    /// one, the second of `sub`), `fadd`/`fmul` for float and double.
    fn load_folds_into_its_only_user(
        &self,
        context: &llrm_mir::context::Context,
        function: &llrm_mir::module::Function,
        inst: llrm_mir::module::InstId,
    ) -> bool {
        use llrm_mir::opcode::{BinaryOp, Opcode};
        use llrm_mir::types::{FloatKind, Type};
        let load = function.instruction(inst);
        let (Opcode::Load { volatile: false, .. }, Some(result)) = (&load.opcode, load.result) else { return false };
        let integer = context.types.int_bits(load.ty);
        let float = matches!(context.types.get(load.ty), Type::Float(FloatKind::Float | FloatKind::Double));
        if !float && !matches!(integer, Some(8 | 16 | 32)) && !(integer == Some(64) && self.native() == 64) {
            return false;
        }
        let mut users = function.users(result).iter();
        let (Some(only), None) = (users.next(), users.next()) else { return false };
        if function.parent(only.user) != function.parent(inst) {
            return false;
        }
        let place = only.index as usize;
        // The memory form needs a register for the other operand: against an immediate
        // `cmp [m],imm` is longer than the shared load plus `test`/`cmp r,imm`, and
        // `add r,imm` cannot take the load at all.
        let Some(&other) = function.instruction(only.user).operands.get(1 - place.min(1)) else { return false };
        if !matches!(other, llrm_mir::module::Operand::Value(_)) {
            return false;
        }
        match &function.instruction(only.user).opcode {
            Opcode::Binary(BinaryOp::Add | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => integer.is_some(),
            Opcode::Binary(BinaryOp::Mul) => matches!(integer, Some(16 | 32 | 64)),
            Opcode::Binary(BinaryOp::Sub) => integer.is_some() && place == 1,
            Opcode::ICmp(_) => integer.is_some(),
            Opcode::Binary(BinaryOp::FAdd | BinaryOp::FMul) => float,
            Opcode::Binary(BinaryOp::FSub | BinaryOp::FDiv) => float && place == 1,
            _ => false,
        }
    }

    /// The profile of the baseline cpu of `triple`'s architecture: the 32-bit
    /// ones by their `-march` (`i686` is `pentiumpro`), `x86_64` long mode.
    pub fn of_triple(triple: &str) -> Option<X86> {
        let cpu = match triple.split('-').next()? {
            "x86_64" => "x86-64",
            "i386" => "i386",
            "i486" => "i486",
            "i586" => "pentium",
            "i686" => "pentiumpro",
            _ => return None,
        };
        X86::of_cpu(cpu)
    }

    /// The profile of a `-march` llrm supports.
    pub fn of_cpu(cpu: &str) -> Option<X86> {
        match cpu {
            "i386" | "i486" | "pentium" | "pentiumpro" | "k5" | "k6" | "athlon" => Some(X86 {
                cmov: !matches!(cpu, "i386" | "i486" | "pentium" | "k5" | "k6"),
                sse1: false,
                sse2: false,
                macro_fusion: false,
                quad: false,
                real_mode: None,
                registers: None,
                tuning: llrm_mir::target::Tuning::PIN,
                loop_micro_op_buffer: 0,
                issue_width: 0,
                position_independent: std::cell::Cell::new(true),
                size_bytes: false,
            }),
            "core2" => Some(X86 {
                cmov: true,
                sse1: true,
                sse2: true,
                macro_fusion: true,
                quad: false,
                real_mode: None,
                registers: None,
                tuning: llrm_mir::target::Tuning::PIN,
                loop_micro_op_buffer: 28,
                issue_width: 4,
                position_independent: std::cell::Cell::new(true),
                size_bytes: false,
            }),
            // `-march=x86-64`: the baseline of long mode, SSE2
            // and `TuningMacroFusion` (X86.td `X86_64V1Tuning`).
            "x86-64" => Some(X86 {
                cmov: true,
                sse1: true,
                sse2: true,
                macro_fusion: true,
                quad: true,
                real_mode: None,
                registers: None,
                tuning: llrm_mir::target::Tuning::PIN,
                loop_micro_op_buffer: 28,
                issue_width: 4,
                position_independent: std::cell::Cell::new(true),
                size_bytes: false,
            }),
            _ => None,
        }
    }

    /// The model a target description names (`tti` and `llvm_cpu` of its
    /// `datalayout.toml`): none for a name no model here has.
    pub fn described(
        tti: &str,
        llvm_cpu: &str,
    ) -> Option<X86> {
        match tti {
            "X86" => X86::of_cpu(llvm_cpu),
            _ => None,
        }
    }

    /// The profile of 16-bit code (T6).
    pub fn in_real_mode(
        &self,
        real_mode: RealMode,
    ) -> X86 {
        X86 { real_mode: Some(real_mode), ..self.clone() }
    }

    /// The registers the target's description allocates.
    pub fn with_registers(
        &self,
        registers: u32,
    ) -> X86 {
        X86 { registers: Some(registers), ..self.clone() }
    }

    /// The target's tuning table.
    pub fn with_tuning(
        &self,
        tuning: llrm_mir::target::Tuning,
    ) -> X86 {
        X86 { tuning, ..self.clone() }
    }
}

/// `getTypeLegalizationCost` for the scalar types: how many operations of what
/// type a value takes, the integer registers being `native` bits wide (32, or
/// 64 in long mode):
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1039
fn legalized(
    ty: Ty,
    native: u32,
) -> Option<(i64, Ty)> {
    match ty {
        Ty::Int(bits) if bits <= 8 => Some((1, Ty::Int(8))),
        Ty::Int(16) => Some((1, Ty::Int(16))),
        Ty::Int(bits) if bits <= 32 => Some((1, Ty::Int(32))),
        Ty::Int(bits) if native == 64 && bits <= 64 => Some((1, Ty::Int(64))),
        Ty::Int(bits) => Some((i64::from(bits.div_ceil(native).next_power_of_two()), Ty::Int(native))),
        Ty::Pointer => Some((1, Ty::Int(native))),
        other => Some((1, other)),
    }
}

/// `CostKindTblEntry`: the four costs of a table row, absent where the row
/// states one only (`{ 1 }` is throughput alone).
type Row = [Option<i64>; 4];

const fn all(
    throughput: i64,
    latency: i64,
    size: i64,
    size_latency: i64,
) -> Row {
    [Some(throughput), Some(latency), Some(size), Some(size_latency)]
}

const fn throughput(value: i64) -> Row {
    [Some(value), None, None, None]
}

/// `ScalarVarDivCostTable`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L757
fn scalar_variable_divide(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    if !matches!(
        op,
        ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
    ) {
        return None;
    }
    match ty {
        Ty::Int(8) => Some(all(15, 20, 2, 4)),
        Ty::Int(16) => Some(all(17, 20, 2, 4)),
        Ty::Int(32) => Some(all(25, 22, 2, 4)),
        Ty::Int(64) => Some(all(41, 24, 2, 4)),
        _ => None,
    }
}

/// `X64CostTbl`, the 64-bit targets' rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1868
fn x64_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (Add | Sub, Ty::Int(64)) => Some(throughput(1)),
        (Mul, Ty::Int(64)) => Some(all(2, 6, 1, 2)),
        _ => None,
    }
}

/// `X86CostTbl`, the 32-bit rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1879
fn x86_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (Add | Sub, Ty::Int(8 | 16 | 32)) => Some(throughput(1)),
        (Mul, Ty::Int(8)) => Some(all(3, 4, 1, 1)),
        (Mul, Ty::Int(16)) => Some(all(2, 4, 1, 1)),
        (Mul, Ty::Int(32)) => Some(all(1, 4, 1, 1)),
        (FNeg, Ty::Double) => Some(all(2, 2, 1, 3)),
        (FAdd | FSub, Ty::Double) => Some(all(2, 3, 1, 1)),
        (FMul, Ty::Double) => Some(all(2, 5, 1, 1)),
        (FDiv, Ty::Double) => Some(all(38, 38, 1, 1)),
        _ => None,
    }
}

/// `SSE2CostTable`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1776
fn sse2_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (FDiv, Ty::Float) => Some(all(23, 23, 1, 1)),
        (FDiv, Ty::Double) => Some(all(38, 38, 1, 1)),
        (FNeg, Ty::Float | Ty::Double) => Some(all(1, 1, 1, 1)),
        (FAdd | FSub, Ty::Float | Ty::Double) => Some(all(2, 3, 1, 1)),
        (FMul, Ty::Double) => Some(all(2, 5, 1, 1)),
        _ => None,
    }
}

/// `SSE1CostTable`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1846
fn sse1_table(
    op: ArithOp,
    ty: Ty,
) -> Option<Row> {
    use ArithOp::*;
    match (op, ty) {
        (FDiv, Ty::Float) => Some(all(17, 18, 1, 1)),
        (FNeg, Ty::Float) => Some(all(2, 2, 1, 2)),
        (FAdd | FSub, Ty::Float) => Some(all(1, 3, 1, 1)),
        (FMul, Ty::Float) => Some(all(2, 5, 1, 1)),
        _ => None,
    }
}

/// `TargetTransformInfoImplBase::getArithmeticInstrCost`, which the generic
/// rules fall to for every kind but throughput:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L733
fn base_arithmetic(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
) -> Cost {
    use ArithOp::*;
    if matches!(op, FDiv | FRem | SDiv | SRem | UDiv | URem) {
        return Cost::EXPENSIVE;
    }
    if kind == CostKind::Latency && ty.is_float() {
        return Cost::of(3);
    }
    Cost::BASIC
}

/// `BasicTTIImplBase::getArithmeticInstrCost`:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1092
fn generic_arithmetic(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
    first: OperandInfo,
    second: OperandInfo,
    profile: &X86,
) -> Cost {
    use ArithOp::*;
    if kind != CostKind::RecipThroughput {
        return base_arithmetic(op, ty, kind);
    }
    let Some((count, _)) = legalized(ty, profile.native()) else { return Cost::INVALID };
    let unit = if ty.is_float() { 2 } else { 1 };
    // X86's operation actions on the scalar types: the divisions and
    // remainders expand (onto DIVREM, which is custom), `frem` is a libcall,
    // the rest are legal.
    match op {
        SDiv | UDiv => Cost::of(unit),
        SRem | URem => {
            let divide = if op == SRem { SDiv } else { UDiv };
            profile.arithmetic_cost(divide, ty, kind, first, second)
                + profile.arithmetic_cost(Mul, ty, kind, OperandInfo::ANY, OperandInfo::ANY)
                + profile.arithmetic_cost(Sub, ty, kind, OperandInfo::ANY, OperandInfo::ANY)
        }
        FRem => Cost::of(count * 2 * unit),
        _ => Cost::of(count * unit),
    }
}

/// `SSE2ConversionTbl`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3526
fn sse2_conversion(
    op: CastKind,
    dst: Ty,
    src: Ty,
) -> Option<Row> {
    use CastKind::*;
    let narrow = |ty: Ty| matches!(ty, Ty::Int(32) | Ty::Int(64) | Ty::Float | Ty::Double);
    if !narrow(dst) || !narrow(src) {
        return None;
    }
    let cost = match (op, dst, src) {
        (SIToFP, Ty::Float | Ty::Double, Ty::Int(32 | 64)) => 3,
        (UIToFP, Ty::Float | Ty::Double, Ty::Int(32)) => 3,
        (UIToFP, Ty::Float, Ty::Int(64)) => 8,
        (UIToFP, Ty::Double, Ty::Int(64)) => 9,
        (FPToSI, Ty::Int(32 | 64), Ty::Float | Ty::Double) => 4,
        (FPToUI, Ty::Int(32), Ty::Float | Ty::Double) => 4,
        (FPToUI, Ty::Int(64), Ty::Float) => 4,
        (FPToUI, Ty::Int(64), Ty::Double) => 15,
        _ => return None,
    };
    Some(all(cost, 1, 1, 1))
}

/// Whether `BITCAST` to `ty` is `Expand` in the lowering (X86ISelLowering): on
/// the x87 targets everything but `f64` is, with SSE2 nothing.
fn bitcast_expands(
    profile: &X86,
    ty: Ty,
) -> bool {
    !profile.sse2 && !matches!(ty, Ty::Double)
}

/// `BasicTTIImplBase::getCastInstrCost` for scalars:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1257
/// (the free ones, first, are `TargetTransformInfoImplBase`'s:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetTransformInfoImpl.h#L789).
fn generic_cast(
    profile: &X86,
    op: CastKind,
    dst: Ty,
    src: Ty,
    kind: CostKind,
) -> Cost {
    use CastKind::*;
    let adjust = |cost: Cost| {
        if kind == CostKind::RecipThroughput {
            cost
        } else if cost.value == 0 {
            cost
        } else {
            Cost::BASIC
        }
    };
    // TargetTransformInfoImplBase::getCastInstrCost: the free ones.
    let free = match op {
        IntToPtr => matches!(src, Ty::Int(8 | 16 | 32)) || (profile.quad && src == Ty::Int(64)),
        PtrToInt => dst == Ty::Int(profile.native()),
        BitCast => dst == src,
        Trunc => matches!(dst, Ty::Int(8 | 16 | 32)) || (profile.quad && dst == Ty::Int(64)),
        _ => false,
    };
    if free {
        return Cost::FREE;
    }
    let (Some((source_count, source)), Some((dest_count, dest))) =
        (legalized(src, profile.native()), legalized(dst, profile.native()))
    else {
        return Cost::INVALID;
    };
    // `X86TargetLowering::isZExtFree`: a 32-bit result is zero-extended into
    // the quadword already.
    if op == ZExt && profile.quad && source == Ty::Int(32) && dest == Ty::Int(64) {
        return Cost::FREE;
    }
    let is_integer = |ty: Ty| matches!(ty, Ty::Int(_) | Ty::Pointer);
    let bits = |ty: Ty| match ty {
        Ty::Int(bits) => bits,
        Ty::Pointer => profile.native(),
        Ty::Float => 32,
        Ty::Double => 64,
        Ty::X87 => 80,
    };
    match op {
        Trunc if bits(source) > bits(dest) && is_integer(source) && is_integer(dest) => return Cost::FREE,
        Trunc | BitCast
            if source_count == dest_count && is_integer(src) == is_integer(dst) && bits(source) == bits(dest) =>
        {
            return Cost::FREE;
        }
        _ => {}
    }
    // Legal or promoted at one count: the count; else a scalar conversion
    // costs 1 unless its operation expands, then 4.
    let expands = matches!(op, BitCast | PtrToInt | IntToPtr) && bitcast_expands(profile, dest);
    // `BITCAST` to an i8 or i16 is legal (promoted); to the other types it
    // expands on the x87 targets, but `f64`'s is custom.
    let legal = !matches!(op, BitCast | PtrToInt | IntToPtr) || profile.sse2 || matches!(dest, Ty::Int(8 | 16));
    adjust(if source_count == dest_count && legal {
        Cost::of(source_count)
    } else if !expands {
        Cost::BASIC
    } else {
        Cost::EXPENSIVE
    })
}

/// `SSE2CostTbl` and `SSE1CostTbl` of `getCmpSelInstrCost`, the scalar rows:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4058
/// and
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4075
fn sse_cmp_sel(
    profile: &X86,
    op: CmpSel,
    ty: Ty,
) -> Option<Row> {
    match (op, ty) {
        (CmpSel::ICmp | CmpSel::FCmp, Ty::Double) if profile.sse2 => Some(all(1, 5, 1, 1)),
        (CmpSel::Select, Ty::Double) if profile.sse2 => Some(all(2, 2, 3, 3)),
        (CmpSel::ICmp | CmpSel::FCmp, Ty::Float) if profile.sse1 => Some(all(1, 5, 1, 1)),
        (CmpSel::Select, Ty::Float) if profile.sse1 => Some(all(2, 2, 3, 3)),
        _ => None,
    }
}

fn int_imm_chunk(value: i64) -> i64 {
    if value == 0 {
        0
    } else if i32::try_from(value).is_ok() {
        1
    } else {
        2
    }
}

const INTRINSIC_PRICES: &str = include_str!("tti_intrinsics.txt");

/// The price of `name` under (profile, kind) in the pinned `opt`'s table (T7):
/// `None` when the table has no row, `Some(None)` for an invalid price. The
/// text is sorted by byte, so a lookup bisects it where it lies; building a
/// map on first use cost 1.06 M Ir on d_faces -O2.
fn intrinsic_price(
    profile: &str,
    kind: &str,
    name: &str,
) -> Option<Option<i64>> {
    let key = || profile.bytes().chain([b' ']).chain(kind.bytes()).chain([b' ']).chain(name.bytes()).chain([b' ']);
    let key_len = profile.len() + kind.len() + name.len() + 3;
    let (mut low, mut high) = (0, INTRINSIC_PRICES.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let start = INTRINSIC_PRICES[low..middle].rfind('\n').map_or(low, |at| low + at + 1);
        let end = INTRINSIC_PRICES[start..].find('\n').map_or(INTRINSIC_PRICES.len(), |at| start + at);
        let line = &INTRINSIC_PRICES[start..end];
        match line.bytes().take(key_len).cmp(key()) {
            std::cmp::Ordering::Less => low = end + 1,
            std::cmp::Ordering::Greater => high = start,
            std::cmp::Ordering::Equal => return Some(line[key_len..].trim_end().parse().ok()),
        }
    }
    None
}

impl X86 {
    /// The intrinsic's row of `tti_intrinsics.txt` for this profile: long mode has its own (`x64`), as the pin's
    /// `X64CostTbl` and 64-bit legalization differ from the 32-bit ones. A `ctlz` or `cttz` called with a zero-poison
    /// flag is priced from its own `.zp` row (the pin looks up `*_ZERO_POISON`).
    fn priced_intrinsic(
        &self,
        name: &str,
        kind: CostKind,
        zero_poison: bool,
    ) -> Cost {
        let profile = if self.quad {
            "x64"
        } else if self.sse2 {
            "sse2"
        } else {
            "nosse"
        };
        let kind = match kind {
            CostKind::RecipThroughput => "throughput",
            CostKind::Latency => "latency",
            CostKind::CodeSize => "code-size",
            CostKind::SizeAndLatency => "size-latency",
        };
        let priced = if zero_poison { intrinsic_price(profile, kind, &format!("{name}.zp")) } else { None };
        match priced.or_else(|| intrinsic_price(profile, kind, name)) {
            Some(Some(cost)) => Cost::of(cost),
            Some(None) => Cost::INVALID,
            None if is_free_intrinsic(name) => Cost::FREE,
            None => base_intrinsic_cost(name).unwrap_or(Cost::INVALID),
        }
    }
}

impl Tti for X86 {
    fn name(&self) -> &'static str {
        "X86"
    }

    fn inline_threshold_override(&self) -> Option<i64> {
        self.tuning.inline_threshold
    }

    fn speed_milliclocks_per_byte(&self) -> i64 {
        self.tuning.speed_milliclocks_per_byte
    }

    fn unroll_milliclocks_per_byte(&self) -> i64 {
        self.tuning.unroll_milliclocks_per_byte
    }

    fn unroll_prices_nest_loops_without_outer_counter(&self) -> bool {
        self.tuning.unroll_prices_nest
    }

    fn folds_phi_constant_chains(&self) -> bool {
        self.tuning.phi_constant_chains
    }

    fn unroll_never_grows_at_o1(&self) -> bool {
        self.tuning.unroll_never_grows_at_o1
    }

    fn unroll_trial_threshold(&self) -> Option<u32> {
        self.tuning.unroll_trial_at_o1
    }

    fn ipra_binds_local(&self) -> bool {
        self.tuning.ipra_binds_local
    }

    fn unroll_trial_at_size(&self) -> Option<u32> {
        self.tuning.unroll_trial_at_size
    }

    /// -O1 is strict on bytes where the -Os trial is (`unroll_trial_bytes_strict`): a peel copies the loop body.
    fn peels_at_o1(&self) -> bool {
        !self.tuning.unroll_trial_bytes_strict
    }

    fn unroll_trial_bytes_strict(&self) -> bool {
        self.tuning.unroll_trial_bytes_strict
    }

    fn unroll_trial_nests_only(&self) -> bool {
        self.tuning.unroll_trial_nests_only
    }

    fn inline_dying_credit_at_os(&self) -> bool {
        self.tuning.inline_dying_credit_at_os
    }

    fn inline_trial_keeps_shrinking_copy(&self) -> bool {
        self.tuning.inline_trial_keeps_shrinking_copy
    }

    fn second_ipsccp(&self) -> bool {
        self.tuning.second_ipsccp
    }

    fn unroll_o1_analyzed_trips(&self) -> Option<u32> {
        self.tuning.unroll_o1_analyzed_trips
    }

    fn pops_arguments_at_size(&self) -> bool {
        self.tuning.pops_arguments_at_size
    }

    fn libc_memcpy(&self) -> bool {
        self.tuning.libc_memcpy
    }

    fn multiply_by_clocks(&self) -> bool {
        self.tuning.multiply_by_clocks
    }

    /// DEVIATE SYMPAIR-1: real mode keeps the two selects (see
    /// `Tti::keeps_icmp_of_swapped_selects`).
    fn keeps_icmp_of_swapped_selects(&self) -> bool {
        self.real_mode.is_some()
    }

    fn keeps_sahf_where_bytes_are_held(&self) -> bool {
        self.tuning.keeps_sahf_where_bytes_are_held
    }

    fn narrow_divide_live_limit(&self) -> u32 {
        self.tuning.narrow_divide_live_limit
    }

    fn narrow_divide_live_max_bits(&self) -> u32 {
        self.tuning.narrow_divide_live_max_bits
    }

    fn narrow_divide_live_at_size(&self) -> bool {
        self.tuning.narrow_divide_live_at_size
    }

    /// DEVIATE INL-O1 SIZE: the inliner counts a callee in bytes of code.
    fn inline_size_view(&self) -> Option<std::rc::Rc<dyn Tti>> {
        ((self.tuning.inline_size_bytes || self.tuning.inline_trials_at_o1) && !self.size_bytes)
            .then(|| std::rc::Rc::new(X86 { size_bytes: true, ..self.clone() }) as std::rc::Rc<dyn Tti>)
    }

    /// `Tuning::inline_trials_at_o1` alone gives the byte view to -O1 only: the -O2 and -O3 callers keep the
    /// units their rate was fitted in.
    fn inline_size_view_at(&self, speed: bool) -> Option<std::rc::Rc<dyn Tti>> {
        if speed && !self.tuning.inline_size_bytes { None } else { self.inline_size_view() }
    }

    fn access_address_cost(
        &self,
        form: AddressForm,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::address(form, self.quad);
        }
        Cost::FREE
    }

    fn memory_intrinsic_cost(
        &self,
        family: &str,
        length: Option<u64>,
        kind: CostKind,
    ) -> Option<Cost> {
        (self.size_bytes && kind == CostKind::CodeSize).then(|| crate::sizes::memory_intrinsic(family, length))
    }

    fn address_computation_cost(
        &self,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return Cost::of(3);
        }
        Cost::BASIC
    }

    fn direct_call_cost(
        &self,
        arguments: i64,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::call(arguments, self.quad);
        }
        Cost::BASIC * (arguments + 1)
    }

    fn inline_never_grows_without_rate(&self) -> bool {
        self.tuning.inline_never_grows_without_rate
    }

    fn cold_site_ignores_sinks(&self) -> bool {
        self.tuning.cold_site_ignores_sinks
    }

    fn unroll_object_bound(&self) -> Option<u32> {
        self.tuning.unroll_object_bound
    }

    fn inline_o1_weighs_entry(&self) -> bool {
        self.tuning.inline_o1_weighs_entry
    }

    fn call_sequence_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        let tuning = &self.tuning;
        if kind != CostKind::Latency || tuning.call_sequence_fixed_clocks == 0 {
            return None;
        }
        let words: i64 = shape
            .arguments
            .iter()
            .map(|argument| match (argument.byval_bytes, argument.ty) {
                (Some(bytes), _) => (bytes as i64 + 3) / 4,
                (None, Some(Ty::Int(bits))) => i64::from(bits.div_ceil(32).max(1)),
                (None, Some(Ty::Double)) => 2,
                (None, Some(Ty::X87)) => 3,
                _ => 1,
            })
            .sum();
        let pop = if words > 0 { tuning.call_sequence_pop_clocks } else { 0 };
        Some(Cost::of(tuning.call_sequence_fixed_clocks + words * tuning.call_sequence_argument_clocks + pop))
    }

    fn sdiv_by_magnitude(&self) -> bool {
        self.tuning.sdiv_by_magnitude
    }

    fn speed_inline_rate_at_o1(&self) -> bool {
        self.tuning.speed_inline_rate_at_o1
    }

    fn inline_trials_at_o1(&self) -> bool {
        self.tuning.inline_size_bytes || self.tuning.inline_trials_at_o1
    }

    fn inline_size_view_is_measured(&self) -> bool {
        self.tuning.inline_size_bytes || self.tuning.inline_trials_at_o1
    }

    fn inline_second_round_at_o1(&self) -> bool {
        self.tuning.inline_size_bytes || self.tuning.inline_trials_at_o1
    }

    fn callee_saved_clocks(&self, peak: usize) -> i64 {
        self.tuning.callee_saved.map_or(0, |(scratch, preserved, clocks)| {
            (i64::try_from(peak).unwrap_or(i64::MAX) - scratch).clamp(0, preserved) * clocks
        })
    }

    fn inline_hot_trial_at_o3(&self) -> bool {
        self.tuning.inline_hot_trial_at_o3
    }

    fn inline_hot_trial_frequency(&self) -> Option<i64> {
        self.tuning.inline_hot_trial_frequency
    }

    fn speed_inline_milliclocks_per_byte(&self) -> i64 {
        self.tuning.speed_inline_milliclocks_per_byte
    }

    fn inline_o1_unit_bytes_milli(&self) -> i64 {
        self.tuning.inline_o1_unit_bytes_milli
    }

    fn inline_o1_milliclocks_per_byte(&self) -> i64 {
        self.tuning.inline_o1_milliclocks_per_byte
    }

    fn promote_va_list(&self) -> bool {
        self.tuning.promote_va_list
    }

    fn branch_compare_trees_at_size(&self) -> bool {
        self.tuning.branch_compare_trees_at_size
    }

    fn sink_spares_ranges(&self) -> bool {
        self.tuning.sink_spares_ranges
    }

    fn divide_by_bytes(&self) -> bool {
        self.tuning.divide_by_bytes
    }

    fn immediate_forms_at_size(&self) -> bool {
        self.tuning.immediate_forms_at_size
    }

    fn price_pin_multiplies(&self) -> bool {
        self.tuning.price_pin_multiplies
    }

    fn lftr_any_stride(&self) -> bool {
        self.tuning.lftr_any_stride
    }

    fn mul_chains_hot_at_size(&self) -> bool {
        self.tuning.mul_chains_hot_at_size
    }

    fn imul_at_size(&self) -> bool {
        self.tuning.imul_at_size
    }

    fn merge_functions_at_size(&self) -> bool {
        self.tuning.merge_functions_at_size
    }

    fn sdiv_pow2_stays_at_size(&self) -> bool {
        self.tuning.sdiv_pow2_stays_at_size
    }

    fn rep_movs_at_size(&self) -> bool {
        self.tuning.rep_movs_at_size
    }

    fn rep_movs_at_size_max(&self) -> u64 {
        self.tuning.rep_movs_at_size_max
    }

    fn rep_stos_at_size(&self) -> bool {
        self.tuning.rep_stos_at_size
    }

    fn rep_stos_at_size_max(&self) -> u64 {
        self.tuning.rep_stos_at_size_max
    }

    fn rep_stos_priced_unit(&self) -> bool {
        self.tuning.rep_stos_priced_unit
    }

    fn has_cmov(&self) -> bool {
        self.cmov
    }

    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (self.tuning.unroll_threshold, self.tuning.unroll_max_percent_threshold_boost)
    }

    /// `X86.td` gives `core2` and `x86-64` the `SandyBridgeModel`; the base
    /// `getUnrollingPreferences` then enables partial and runtime unrolling
    /// (`Tti::get_unrolling_preferences`):
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L768-L803
    fn set_position_independent(
        &self,
        position_independent: bool,
    ) {
        self.position_independent.set(position_independent);
    }

    fn loop_micro_op_buffer_size(&self) -> u32 {
        self.loop_micro_op_buffer
    }

    fn issue_width(&self) -> u32 {
        self.issue_width
    }

    fn vetoes_recurrence_bound_unroll(&self) -> bool {
        self.tuning.unroll_recurrence_veto
    }

    fn x87_stack_slots(&self) -> Option<u32> {
        self.tuning.x87_stack_slots.filter(|_| !self.sse2)
    }

    fn x87_share_negated_products(&self) -> bool {
        self.tuning.x87_share_negated_products && !self.sse2
    }

    fn x87_zero_fill_at_size(&self) -> bool {
        self.tuning.x87_zero_fill_at_size && !self.sse2
    }

    fn wide_float_to_int(&self, bits: u32) -> Option<u32> {
        // `fistp` word, dword and qword: x87 only, where the SSE converts have
        // their own forms (a 32-bit result is a dword, `cvttsd2si`)
        (!self.sse2).then_some(match bits {
            0..=16 => 32,
            17..=32 => 64,
            _ => return None,
        })
    }

    fn size_fill(&self) -> Option<llrm_mir::target::SizeFill> {
        self.tuning.size_fill
    }

    fn unroll_partial_threshold_override(&self) -> Option<u32> {
        self.tuning.unroll_partial_threshold
    }

    fn unroll_jam_exact_count(&self) -> bool {
        self.tuning.unroll_jam_exact_count
    }

    fn constant_hoisting(&self) -> bool {
        self.tuning.constant_hoisting
    }

    fn unroll_jam_partial_threshold_override(&self) -> Option<u32> {
        self.tuning.unroll_jam_partial_threshold
    }

    fn unswitch_threshold_override(&self) -> Option<i64> {
        self.tuning.unswitch_threshold
    }

    fn min_jump_table_density(&self) -> Option<u64> {
        self.tuning.min_jump_table_density
    }

    fn recursive_inlining(&self) -> Option<(u32, i64)> {
        match self.real_mode.as_ref() {
            Some(real) => real.recursive_inlining,
            None => self.tuning.recursive_inlining,
        }
    }

    fn recursive_edge_growth(&self) -> Option<(i64, i64)> {
        self.tuning.recursive_edge_growth
    }

    fn recursive_inlining_growth(&self) -> Option<(i64, i64)> {
        self.tuning.recursive_inlining_growth
    }

    fn recursive_inlining_depth(&self) -> Option<(u32, u32)> {
        self.tuning.recursive_inlining_depth
    }

    fn ipa_cp_clones_at_speed(&self) -> bool {
        self.tuning.ipa_cp_clones_at_speed
    }

    fn recursive_tre_first(&self) -> (bool, bool) {
        self.tuning.recursive_tre_first
    }

    fn peels_folded_entry(&self) -> bool {
        self.tuning.peel_folded_entry
    }

    fn carries_slot_indexes(&self) -> bool {
        self.tuning.carry_slot_indexes
    }

    fn sinks_select_arms(&self) -> bool {
        self.tuning.sink_select_arms
    }

    fn branches_constant_arm_selects(&self) -> bool {
        self.tuning.branch_constant_arm_selects
    }

    fn indvars_widens_narrow_counter_through_masks(&self) -> bool {
        self.tuning.prefixed_16_bit_counters
    }

    fn narrow_counter_is_taxed(
        &self,
        bits: u32,
    ) -> bool {
        bits == 16 && self.tuning.prefixed_16_bit_counters
    }

    fn inlines_variable_memcpy(&self) -> bool {
        self.tuning.inline_variable_memcpy
    }

    fn copy_price_for_size(&self) -> Option<llrm_mir::target::CopyPrice> {
        self.tuning.size_copy_price
    }

    fn ipsccp_widen_keeps_sign(&self) -> bool {
        self.tuning.ipsccp_widen_keeps_sign
    }

    fn ipsccp_widen_keeps_sign_at_size(&self) -> bool {
        self.tuning.ipsccp_widen_keeps_sign_at_size
    }

    fn priced_self_inlining(&self) -> Option<(u32, i64)> {
        self.real_mode.as_ref()?.priced_self_inlining
    }

    fn unroll_size_few_trips(&self) -> Option<(u32, u32)> {
        let real = self.real_mode.as_ref().filter(|real| real.unroll_size_few_trips.0 != 0)?;
        Some(real.unroll_size_few_trips)
    }

    /// SZ1: an inline that stays must not grow the code, where the target
    /// says so (`RealMode::inline_size_guard`): at m32 -Os follows the pin
    /// (crc, tile and recchop grow 52, 42 and 15 bytes there).
    fn opt_size_inline_by_code_size(&self) -> bool {
        self.real_mode.as_ref().is_some_and(|real| real.inline_size_guard) || self.tuning.inline_size_guard_at_size
    }

    /// The pin's rule (`TotalAllocaSizeRecursiveCaller`) outside real mode,
    /// where the stack is not 64 KiB shared with the data.
    fn inline_frames_into_recursive_callers(&self) -> bool {
        self.real_mode.is_none()
    }

    fn lftr_expansion_budget(
        &self,
        optsize: bool,
    ) -> u32 {
        if optsize { self.tuning.lftr_size_budget } else { 4 }
    }

    fn inline_trial_defers_to_shared_growth(&self) -> bool {
        self.tuning.inline_trial_defers_to_shared_growth
    }

    fn add_to_or_disjoint(&self) -> bool {
        self.tuning.add_to_or_disjoint
    }

    fn opt_size_inline_chain_trial(&self) -> bool {
        self.tuning.inline_chain_trial
    }

    fn opt_size_inline_guard_min_sites(&self) -> i64 {
        if self.real_mode.as_ref().is_some_and(|real| real.inline_size_guard)
            || self.tuning.inline_size_guard_single_site
        {
            1
        } else {
            2
        }
    }

    fn unroll_promotion_fits_registers(&self) -> bool {
        self.tuning.unroll_promotion_fits_registers
    }

    fn unroll_store_immediate_bits(&self) -> u32 {
        self.tuning.unroll_store_immediate_bits
    }

    fn lsr_size_model(&self) -> Option<llrm_mir::tti::LsrSizeModel> {
        self.tuning.lsr_size_order.then(|| {
            let model = if self.tuning.lsr_size_x86_64 {
                crate::encoding::lsr_size_model_64()
            } else {
                llrm_mir::tti::LsrSizeModel::X86
            };
            llrm_mir::tti::LsrSizeModel { derived_setup_free: self.tuning.lsr_derived_setup_free, ..model }
        })
    }

    fn lsr_phi_operand_uses(&self) -> bool {
        self.tuning.lsr_phi_operand_uses
    }

    fn lsr_phi_users_first(&self) -> bool {
        self.tuning.lsr_phi_users_first
    }

    fn splits_invariant_fmuladd(&self) -> bool {
        self.tuning.splits_invariant_fmuladd
    }

    fn lsr_users_in_use_list_order(&self) -> bool {
        self.tuning.lsr_users_in_use_list_order
    }

    fn unroll_promotion_fits_x87_stack(&self) -> bool {
        self.tuning.unroll_promotion_fits_x87_stack && !self.sse2
    }

    fn rotation_guard_must_fold_at_size(&self) -> bool {
        self.tuning.rotation_guard_must_fold_at_size
    }

    fn load_pre_off_at_size(&self) -> bool {
        self.tuning.load_pre_off_at_size
    }

    fn rotation_trial_at_size(&self) -> bool {
        self.tuning.rotation_trial_at_size
    }

    fn rotation_hot_at_size(&self) -> bool {
        self.tuning.rotation_hot_at_size
    }

    fn rotation_hot_bytes_per_loop(&self) -> i64 {
        self.tuning.rotation_hot_bytes_per_loop
    }

    fn size_rotation_header_limit(&self) -> Option<u32> {
        self.tuning.size_rotation_header_limit
    }

    fn rotation_header_priced_at_size(&self) -> bool {
        self.tuning.rotation_header_priced_at_size
    }

    fn constant_offset_phi_is_cheap(&self) -> bool {
        !self.tuning.keep_constant_displacements
    }

    fn frame_slot_is_free_base(&self) -> bool {
        self.tuning.frame_slot_is_free_base
    }

    fn lsr_address_fewest_registers(&self) -> bool {
        self.tuning.lsr_address_fewest_registers
    }

    fn constant_arith_phi_is_cheap(
        &self,
        optsize: bool,
    ) -> bool {
        optsize || !self.tuning.keep_constant_arith_operands
    }

    /// A call is 5 bytes and the push of an argument 1 to 4, against 2 to 3 for
    /// an ALU instruction. With 100 the 28 calls of `_savegame_read_block`
    /// stayed (+408 bytes of the module); 150 inlines them and still keeps the
    /// 14-instruction loop of `idioms.bas` out of ten sites.
    fn call_code_size_percent(&self) -> i64 {
        // The size view prices a call in bytes already: pushes and the pop.
        if self.size_bytes { 100 } else { 150 }
    }

    fn float_code_size_percent(&self) -> i64 {
        self.real_mode.as_ref().map_or(100, |real| real.float_code_size_percent)
    }

    // X86TTIImpl::enableMemCmpExpansion
    // (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7513).
    // DEVIATE X1 (exit 2026-11-30, same permalink): the 16-, 32- and 64-byte
    // vector load sizes of an equality compare are left out (vectors are
    // deferred); the pin takes them from `getPreferVectorWidth`.
    fn enable_mem_cmp_expansion(
        &self,
        _opt_size: bool,
        is_zero_cmp: bool,
    ) -> Option<MemCmpExpansionOptions> {
        let mut load_sizes = Vec::new();
        if self.quad {
            load_sizes.push(8);
        }
        load_sizes.extend([4, 2, 1]);
        Some(MemCmpExpansionOptions {
            load_sizes,
            // X86ISelLowering.cpp#L2946: MaxLoadsPerMemcmp and its OptSize
            // twin.
            max_num_loads: 2,
            num_loads_per_block: if is_zero_cmp { 2 } else { 1 },
            allow_overlapping_loads: true,
            allowed_tail_expansions: Vec::new(),
        })
    }

    // X86TargetLowering::allowsMisalignedMemoryAccesses
    // (X86ISelLoweringCall.cpp#L371): any size, but a non-temporal vector.
    fn allows_misaligned_memory_accesses(
        &self,
        _bits: u32,
        _address_space: u32,
        _align: u64,
    ) -> bool {
        true
    }

    // X86TTIImpl::hasDivRemOp (X86TargetTransformInfo.cpp#L7421) asks
    // `isOperationLegal(SDIVREM/UDIVREM, VT)`; X86ISelLowering sets the scalar
    // divides to `Expand` (L340) and leaves the two-result ones at the base
    // class's `Legal`, so every register type has one.
    //
    // DEVIATE DR1 (exit 2026-11-20): the 32-bit mode's inline `i64` divide
    // routine returns both, so `i64` has one too (the pin's x86 has none and
    // `DivRemPairs` writes `x % y` as `x - x / y * y`: a 64-bit multiply and
    // a subtract per remainder, x_ll_div).
    fn has_div_rem_op(
        &self,
        bits: u32,
        _signed: bool,
    ) -> bool {
        self.is_type_legal(Ty::Int(bits)) || (bits == 64 && !self.quad && self.real_mode.is_none())
    }

    /// `FSQRT` is legal on every scalar float: the SSE instruction, or x87's.
    fn have_fast_sqrt(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Float | Ty::Double | Ty::X87)
    }

    fn partial_libcalls_skip_at_size(&self) -> bool {
        self.tuning.partial_libcalls_skip_at_size
    }

    /// `X86TTIImpl::isFCmpOrdCheaperThanFCmpZero`
    /// ([X86TargetTransformInfo.cpp#L7435](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7435)).
    fn is_fcmp_ord_cheaper_than_fcmp_zero(
        &self,
        _ty: Ty,
    ) -> bool {
        false
    }

    /// Long mode's DAG selects an access of any byte size (an `i24` is a 16-bit and an 8-bit access, an `i40` a
    /// dword and a byte), as the pin's does: SROA cuts the slices where the pin cuts them.
    fn selects_odd_sized_integers(&self) -> bool {
        self.quad
    }

    fn is_type_legal(
        &self,
        ty: Ty,
    ) -> bool {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp
        // The GPR classes hold i8, i16, i32 and, in long mode, i64; x87 holds
        // every scalar float.
        match ty {
            Ty::Int(8 | 16 | 32) | Ty::Pointer => true,
            Ty::Int(64) => self.quad,
            Ty::Int(_) => false,
            Ty::Float | Ty::Double | Ty::X87 => true,
        }
    }

    fn cast_of_load_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Option<Cost> {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1297
        // A `sext`/`zext` of a load is the extending load (movsx, movzx) when
        // both types legalize to one register each.
        if let (CastKind::SExt | CastKind::ZExt, Ty::Int(to), Ty::Int(from)) = (op, dst, src)
            && from >= 8
            && let (Some((to_count, _)), Some((from_count, _))) =
                (legalized(Ty::Int(to), self.native()), legalized(Ty::Int(from), self.native()))
            && to_count == from_count
        {
            return Some(Cost::FREE);
        }
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3805
        // For a scalar load the extend would be free.
        (matches!(op, CastKind::SIToFP | CastKind::UIToFP) && matches!(src, Ty::Int(bits) if (2..32).contains(&bits)))
            .then(|| self.cast_cost(CastKind::SIToFP, dst, Ty::Int(32), kind))
    }

    fn indexed_store_cost(
        &self,
        kind: CostKind,
    ) -> Option<Cost> {
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L5818
        (kind != CostKind::RecipThroughput).then(|| Cost::of(2))
    }

    fn reduces_outer_multiply(
        &self,
        multiplier: Option<u128>,
    ) -> bool {
        let llrm_mir::target::Tuning { multiply_clocks, loop_carried_clocks, .. } = self.tuning;
        // a shift-and-add chain is a shift and an add for each term past the first
        let chain = multiplier.map_or(i64::MAX, |m| 2 * (signed_digits(m) as i64 - 1).max(1));
        multiply_clocks > 0 && multiply_clocks.min(chain) > loop_carried_clocks
    }

    fn splits_merged_conditions(&self) -> bool {
        self.tuning.split_merged_conditions
    }

    /// X86's `getJumpConditionMergingParams`: `x86-br-merging-base-cost` 2,
    /// one more for `a == b && a == c` (a fast pattern); an `or` of two `==`
    /// splits (no CCMP on these targets). Likely bias 0, unlikely bias -1.
    fn jump_condition_merging_params(
        &self,
        query: &llrm_mir::jumpmerge::JumpMergeQuery,
    ) -> llrm_mir::jumpmerge::CondMerging {
        use llrm_mir::jumpmerge::{CondMerging, Logic};
        use llrm_mir::module::{Operand, ValueDef};
        use llrm_mir::opcode::{IntPredicate, Opcode};
        let is_eq = |operand: Operand| match operand {
            Operand::Value(value) => match query.function.value(value).def {
                ValueDef::Instruction(inst) => {
                    matches!(
                        query.function.instruction(inst).opcode,
                        Opcode::ICmp(IntPredicate::Eq)
                    )
                }
                _ => false,
            },
            _ => false,
        };
        let both_eq = is_eq(query.lhs) && is_eq(query.rhs);
        // DEVIATE SD103-M32: the `setcc` pair, logic op and `test` a merged
        // pair costs come off the budget, unless the DAG folds the pair into
        // one compare (`Tuning::merged_condition_clocks`).
        let extra = if llrm_mir::jumpmerge::folds_logic_of_setccs(
            query.context,
            query.function,
            query.logic,
            (query.lhs, query.rhs),
        ) {
            0
        } else {
            self.tuning.merged_condition_clocks
        };
        let params = match query.logic {
            Logic::Or if both_eq => return CondMerging::SPLIT,
            Logic::And if both_eq => CondMerging { base_cost: 3, likely_bias: 0, unlikely_bias: -1 },
            _ => CondMerging { base_cost: 2, likely_bias: 0, unlikely_bias: -1 },
        };
        if params.base_cost <= extra { CondMerging::SPLIT } else { CondMerging { base_cost: params.base_cost - extra, ..params } }
    }

    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let _ = (ret, args);
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::intrinsic(name).unwrap_or_else(|| match is_free_intrinsic(name) {
                true => Cost::FREE,
                false => base_intrinsic_cost(name).map_or(Cost::INVALID, |units| units * 4),
            });
        }
        self.priced_intrinsic(name, kind, false)
    }

    fn zero_poison_intrinsic_cost(
        &self,
        name: &str,
        ret: Option<Ty>,
        args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let _ = (ret, args);
        if self.size_bytes && kind == CostKind::CodeSize {
            return self.intrinsic_cost(name, ret, args, kind);
        }
        self.priced_intrinsic(name, kind, true)
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::cmp_sel(op, ty, self.cmov, self.quad);
        }
        let Some((count, _)) = legalized(ty, self.native()) else { return Cost::INVALID };
        if let Some(each) = sse_cmp_sel(self, op, ty).and_then(|row| row[kind.index()]) {
            return Cost::of(count * each);
        }
        // Assume a 3 cycle latency for a floating point select:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L4133
        if kind == CostKind::Latency && op == CmpSel::Select && ty.is_float() {
            return Cost::of(3);
        }
        // BasicTTIImplBase: legal, one per legalized part.
        Cost::of(count)
    }

    /// `X86TTIImpl::getMemoryOpCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L5806
    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        stored_constant: bool,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::memory(op, ty, stored_constant, self.quad);
        }
        // Load latency is not modelled here: the base answers 4.
        if op == MemoryOp::Load && kind == CostKind::Latency {
            return Cost::of(4);
        }
        if kind != CostKind::RecipThroughput {
            return Cost::BASIC;
        }
        let Some((count, _)) = legalized(ty, self.native()) else { return Cost::INVALID };
        // A constant stored through a floating point register is loaded first.
        let extra = if op == MemoryOp::Store && stored_constant && ty.is_float() { count } else { 0 };
        Cost::of(extra + count)
    }

    /// `X86TTIImpl::getCFInstrCost`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7076
    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::control_flow(phi);
        }
        if kind != CostKind::RecipThroughput {
            return if phi { Cost::FREE } else { Cost::BASIC };
        }
        // Branches are assumed to be predicted.
        Cost::FREE
    }

    /// `X86TTIImpl::getIntImmCost(const APInt &, Type *)`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6892
    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.imm_cost(imm, bits, self.tuning.zext_imm_is_basic)
    }

    /// `X86TTIImpl::getIntImmCostInst`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L6927
    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.imm_cost_inst(op, index, imm, bits, self.tuning.zext_imm_is_basic)
    }

    fn int_imm_cost_inst_pin(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        self.imm_cost_inst(op, index, imm, bits, false)
    }

    /// ZIMM does not apply to a function built for size: a hoisted constant
    /// costs a 2-3 byte register copy per use where `mov r32, imm32` is 5.
    fn int_imm_cost_inst_for_size(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
        for_size: bool,
    ) -> Cost {
        self.imm_cost_inst(op, index, imm, bits, self.tuning.zext_imm_is_basic && !for_size)
    }

    /// `X86TTIImpl::getIntImmCostIntrin`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7040
    fn int_imm_cost_intrin(
        &self,
        name: &str,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        if bits == 0 {
            return Cost::FREE;
        }
        let name = name.strip_prefix("llvm.").unwrap_or(name);
        // `getBitWidth() <= 64 && isSignedIntN(width)`.
        let extended = if bits >= 128 { imm } else { (imm << (128 - bits)) >> (128 - bits) };
        let fits = |width: u32| bits <= 64 && (-(1i128 << (width - 1))..1i128 << (width - 1)).contains(&extended);
        let overflow = ["sadd", "uadd", "ssub", "usub", "smul", "umul"]
            .iter()
            .any(|op| name.strip_prefix(op).is_some_and(|rest| rest.starts_with(".with.overflow")));
        if overflow {
            if index == 1 && fits(32) {
                return Cost::FREE;
            }
        } else if name.starts_with("experimental.stackmap") {
            if index < 2 || fits(64) {
                return Cost::FREE;
            }
        } else if name.starts_with("experimental.patchpoint") {
            if index < 4 || fits(64) {
                return Cost::FREE;
            }
        } else {
            return Cost::FREE;
        }
        self.int_imm_cost(imm, bits)
    }

    /// `X86TargetLowering::isIntDivCheap`: only at `minsize`.
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L65389
    fn prefer_to_keep_constants_attached(
        &self,
        div_rem_by_constant: bool,
        min_size: bool,
    ) -> bool {
        div_rem_by_constant && !min_size
    }

    fn keeps_select_mask_attached(
        &self,
        bits: u128,
        width: u32,
    ) -> bool {
        self.tuning.zext_imm_is_basic && width == 64 && bits <= u128::from(u32::MAX)
    }

    /// `X86TargetLowering::isTruncateFree`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36743
    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        match (from, to) {
            (Ty::Int(a), Ty::Int(b)) => a > b,
            _ => false,
        }
    }

    fn is_legal_addressing_mode_in(
        &self,
        addr_space: u32,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        if self.real_mode.as_ref().is_some_and(|real| real.huge_space == Some(addr_space)) {
            return !base_gv && has_base_reg && base_offset == 0 && scale == 0;
        }
        self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    /// `X86TargetLowering::isLegalAddressingMode`:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36621,
    /// Small code model; position independent in long mode, not in 32-bit.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        // A displacement is a signed 32-bit immediate, below 16 MB beside a
        // symbol.
        if let Some(real) = &self.real_mode {
            // 16-bit addresses: a 16-bit displacement and a scale a primary
            // form holds (0 is no index).
            return i16::try_from(base_offset).is_ok() && (scale == 0 || real.scale_cost(scale).is_some());
        }
        if i32::try_from(base_offset).is_err() || (base_gv && base_offset >= 16 * 1024 * 1024) {
            return false;
        }
        // Long mode, position independent (the PIE a default clang makes): a
        // symbol is `[rip + symbol]`, which takes no displacement and no scaled
        // index beside it; a base register is fine (`@sym + reg`, the symbol
        // formed by a `lea`). The pin refuses only an offset or a scale above 1
        // (`isPositionIndependent() && is64Bit()`).
        if self.quad && self.position_independent.get() && base_gv && (base_offset != 0 || scale > 1) {
            return false;
        }
        match scale {
            0 | 1 | 2 | 4 | 8 => true,
            // Formed with base + scale register: only without a base register.
            3 | 5 | 9 => !has_base_reg,
            _ => false,
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36734
    /// `isLegalAddImmediate`: a signed 32-bit immediate (a negative one is a
    /// `sub`).
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L36730
    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L8045
    /// `getScalingFactorCost`: a second register in the address is one, a
    /// plain address nothing; an illegal mode is invalid.
    fn scaling_factor_cost(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> Cost {
        if self.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale) {
            // T6: real mode's native form (bx + si/di) is free at scale 1; the
            // others take the address-size prefix and its stall.
            if let Some(cost) = self.real_mode.as_ref().and_then(|real| real.scale_cost(scale)) {
                return Cost::of(cost);
            }
            Cost::of(i64::from(scale != 0))
        } else {
            Cost::INVALID
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7202
    /// `isLSRCostLess`: "instruction number 1st priority".
    fn is_lsr_cost_less(
        &self,
        a: &LsrCost,
        b: &LsrCost,
    ) -> bool {
        let key = |c: &LsrCost| {
            (
                c.insns,
                c.num_regs,
                c.add_rec_cost,
                c.num_iv_muls,
                c.num_base_adds,
                c.scale_cost,
                c.imm_cost,
                c.setup_cost,
            )
        };
        key(a) < key(b)
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7211
    fn can_macro_fuse_cmp(&self) -> bool {
        self.macro_fusion
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L173
    /// Eight general registers, sixteen in long mode (`getNumberOfRegisters`).
    fn number_of_gpr_registers(&self) -> u32 {
        self.real_mode.as_ref().map_or(self.registers.unwrap_or(if self.quad { 16 } else { 8 }), |real| real.registers)
    }

    /// Real mode declares the registers the allocator has, and the live count
    /// LSR keeps is exact there: the six fill (PLASMA's loop, 4 + 2 live).
    fn lsr_reserved_registers(&self) -> u32 {
        u32::from(self.real_mode.is_none())
    }

    /// `[bp+disp]` and `[esp+disp]` reach a stack slot with no register to hold
    /// it.
    fn frame_address_is_free(&self) -> bool {
        true
    }

    fn ext_load_costs_extra(&self) -> bool {
        self.tuning.ext_load_extra_clocks > 0
    }

    /// 64 in long mode, else 32 (also at m16, on a 386 or later).
    fn gpr_bits(&self) -> u32 {
        if self.quad { 64 } else { 32 }
    }

    /// `lea` forms `base + index + displacement` in one instruction, from 16
    /// bits up: there is no 8-bit `lea`.
    fn address_form_adds_offsets(
        &self,
        bits: u32,
    ) -> bool {
        bits >= 16
    }

    /// DEVIATE LU4: real mode keeps the loop as written where LSR's own model
    /// rates its rewrite worse (`shouldDropLSRSolutionIfLessProfitable`, which
    /// the pin's X86 leaves off). Six registers and a prefix on every scaled
    /// index make the model's best answer a loss in bytes and clocks too.
    fn should_drop_lsr_solution_if_less_profitable(&self) -> bool {
        self.real_mode.is_some()
    }

    /// DEVIATE LU14: real mode searches both use orders (rcflip's RAMP loop:
    /// 4 instructions in the pin's order, 2 in the other). owner:
    /// llrm-target-x86-m16, exit: 2026-11-30, or when the pin's search is
    /// order-free.
    fn lsr_solves_both_use_orders(&self) -> bool {
        self.real_mode.is_some() || self.tuning.lsr_both_use_orders
    }

    fn lsr_charges_outer_nonaffine_ivs(&self) -> bool {
        self.real_mode.is_some()
    }

    fn congruent_ivs_truncate_to_narrowest_integer(&self) -> bool {
        self.real_mode.is_some()
    }

    fn mirrors_conditional_counters(&self) -> bool {
        self.real_mode.is_some()
    }

    /// DEVIATE LS1: real mode keeps every formula of the first use in play
    /// (A/B 2026-11-07: the pin's rule is +1..+8 B in 41 of 192 m16 cells, in
    /// none better; m32 -37 B net, m64 0). owner: w12, exit: 2026-11-30, when
    /// the m16 cause cards close.
    fn lsr_visited_registers_lose(&self) -> bool {
        self.real_mode.is_none()
    }

    /// Real mode forms a scaled index with an address-size prefix.
    fn lsr_drops_spilling_solution(&self) -> bool {
        self.real_mode.is_some()
    }

    fn address_registers(&self) -> u32 {
        self.real_mode
            .as_ref()
            .and_then(|real| real.address_registers)
            .unwrap_or_else(|| self.number_of_gpr_registers())
    }

    fn scaled_index_insns(&self) -> u32 {
        self.real_mode.as_ref().map_or(0, |real| real.scaled_index_insns)
    }

    /// i8 and i16 extending loads are legal, and free when the result
    /// legalizes to as many registers as the loaded type does.
    fn ext_load_free(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
    ) -> bool {
        matches!(op, CastKind::ZExt | CastKind::SExt)
            && matches!(
                (src, dst),
                (Ty::Int(8), Ty::Int(16 | 32)) | (Ty::Int(16), Ty::Int(32))
            )
            && matches!(
                (legalized(src, self.native()), legalized(dst, self.native())),
                (Some((one, _)), Some((same, _))) if one == same
            )
    }

    /// `X86TTIImpl::getCastInstrCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L2772
    /// (T4: no instruction context, so the `zext` folded into a GEP index and
    /// the extending load are not seen).
    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        use CastKind::*;
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::cast(op, dst, src);
        }
        if self.sse2 {
            if let Some(each) = sse2_conversion(op, dst, src).and_then(|row| row[kind.index()]) {
                return Cost::of(each);
            }
        }
        let (Some((source_count, source)), Some((dest_count, dest))) =
            (legalized(src, self.native()), legalized(dst, self.native()))
        else {
            return Cost::INVALID;
        };
        if op == Trunc && source == dest {
            return Cost::FREE;
        }
        if self.sse2 {
            if let Some(each) = sse2_conversion(op, dest, source).and_then(|row| row[kind.index()]) {
                return Cost::of(source_count.max(dest_count) * each);
            }
        }
        // i8 and i16 integers convert through i32:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3805
        if matches!(op, SIToFP | UIToFP) && matches!(src, Ty::Int(bits) if (2..32).contains(&bits)) {
            let extend = if op == SIToFP { SExt } else { ZExt };
            return self.cast_cost(extend, Ty::Int(32), src, kind) + self.cast_cost(SIToFP, dst, Ty::Int(32), kind);
        }
        if matches!(op, FPToSI | FPToUI) && matches!(dst, Ty::Int(bits) if (2..32).contains(&bits)) {
            return self.cast_cost(FPToSI, Ty::Int(32), src, kind) + self.cast_cost(Trunc, dst, Ty::Int(32), kind);
        }
        generic_cast(self, op, dst, src, kind)
    }

    /// `X86TTIImpl::getArithmeticInstrCost`, scalars:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L255
    /// The count must sit in `cl`. Not in real mode: its six registers spill
    /// the multiplier's select (pack_ref -Os m16: +24 B).
    fn variable_shift_count_in_fixed_register(&self) -> bool {
        self.real_mode.is_none()
    }

    /// DEVIATE FOLD-HOIST (x87, exit 2026-11-20): the pin's X86 keeps the base
    /// answer. A load that the arm's own user would take as a memory operand
    /// stays in its arm: hoisted, it has two users and neither folds it, and
    /// the register it holds across the branch is what spills
    /// (x_cordic -O1: 2 spills a pass of the outer loop, +397 clk, +8 B).
    fn is_profitable_to_hoist(
        &self,
        context: &llrm_mir::context::Context,
        function: &llrm_mir::module::Function,
        inst: llrm_mir::module::InstId,
    ) -> bool {
        !self.load_folds_into_its_only_user(context, function, inst)
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
        if self.size_bytes && kind == CostKind::CodeSize {
            return crate::sizes::arithmetic(op, ty, second, self.quad);
        }
        let Some((count, legal)) = legalized(ty, self.native()) else { return Cost::INVALID };
        let none = OperandInfo::ANY;
        // Multiply by a power of two (or its negation) is a shift:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L334
        if op == Mul && second.constant && (second.power_of_two || second.negated_power_of_two) {
            let mut cost = self.arithmetic_cost(Shl, ty, kind, none, none);
            if second.negated_power_of_two {
                cost = cost + self.arithmetic_cost(Sub, ty, kind, none, none);
            }
            return cost;
        }
        // Signed divide by a power of two:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L346
        if matches!(op, SDiv | SRem) && second.constant && second.power_of_two {
            let mut cost = self.arithmetic_cost(AShr, ty, kind, none, none) * 2
                + self.arithmetic_cost(LShr, ty, kind, none, none)
                + self.arithmetic_cost(Add, ty, kind, none, none);
            if op == SRem {
                cost = cost
                    + self.arithmetic_cost(Mul, ty, kind, none, none)
                    + self.arithmetic_cost(Sub, ty, kind, none, none);
            }
            return cost;
        }
        // Unsigned divide by a power of two:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L371
        if matches!(op, UDiv | URem) && second.constant && second.power_of_two {
            return if op == UDiv {
                self.arithmetic_cost(LShr, ty, kind, none, none)
            } else {
                self.arithmetic_cost(And, ty, kind, none, none)
            };
        }
        // A scalar divide by a constant is a multiply-high sequence:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L382
        if second.constant && !second.negated_power_of_two && matches!(op, UDiv | SDiv | URem | SRem) {
            let mut cost = if matches!(op, URem | SRem) { 6 } else { 5 };
            if matches!(kind, CostKind::CodeSize | CostKind::SizeAndLatency) {
                cost += 2;
            }
            return Cost::of(count * cost);
        }
        if !second.constant {
            if let Some(row) = scalar_variable_divide(op, legal) {
                if let Some(each) = row[kind.index()] {
                    return Cost::of(count * each);
                }
            }
        }
        let tables: [(bool, fn(ArithOp, Ty) -> Option<Row>); 4] =
            [(self.sse2, sse2_table), (self.sse1, sse1_table), (self.quad, x64_table), (true, x86_table)];
        for (present, table) in tables {
            if present {
                if let Some(each) = table(op, legal).and_then(|row| row[kind.index()]) {
                    return Cost::of(count * each);
                }
            }
        }
        // Single-instruction code size cases:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L1918
        if kind == CostKind::CodeSize && matches!(op, FAdd | FSub | FMul | FDiv | FNeg | And | Or | Xor) {
            return Cost::of(count);
        }
        generic_arithmetic(op, ty, kind, first, second, self)
    }
}

/// The nonzero digits of `n` in signed-digit (non-adjacent) form: a multiply by
/// `n` is that many shifted terms added or subtracted.
fn signed_digits(n: u128) -> u32 {
    let (mut n, mut count) = (n, 0);
    while n > 0 {
        if n & 1 == 1 {
            // 1 mod 4 takes the digit +1, 3 mod 4 takes -1
            n = if n & 3 == 1 { n - 1 } else { n + 1 };
            count += 1;
        }
        n >>= 1;
    }
    count
}
