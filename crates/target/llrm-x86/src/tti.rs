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
use llrm_sdag::lowering::{TargetLoweringInfo, TypeAction};
use llrm_sdag::mvt::{EVT, MVT};

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
    /// A 32-bit integer register holds the vector `<2 x i16>` or `<4 x i8>`:
    /// a loop of bitwise operations is widened by it, and the vectors are
    /// lowered to the word before selection (gcc's `ix86_preferred_simd_mode`
    /// with no SSE, `tree-vect-generic.cc`). Off: the pin has no such vector.
    pub word_vectors: bool,
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
        let float = matches!(
            context.types.get(load.ty),
            Type::Float(FloatKind::Float | FloatKind::Double)
        );
        if !float && !matches!(integer, Some(8 | 16 | 32)) && !(integer == Some(64) && self.native() == 64) {
            return false;
        }
        let mut users = function.users(result).iter();
        let (Some(only), None) = (users.next(), users.next()) else { return false };
        if function.parent(only.user) != function.parent(inst) {
            return false;
        }
        let place = only.index as usize;
        // The memory form needs a register for the other operand: against an
        // immediate `cmp [m],imm` is longer than the shared load plus
        // `test`/`cmp r,imm`, and `add r,imm` cannot take the load at
        // all.
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
                word_vectors: false,
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
                word_vectors: false,
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
                word_vectors: false,
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

    /// The word is a vector register (`word_vectors`).
    pub fn with_word_vectors(
        &self,
        word_vectors: bool,
    ) -> X86 {
        X86 { word_vectors, ..self.clone() }
    }

    /// The target's tuning table.
    pub fn with_tuning(
        &self,
        tuning: llrm_mir::target::Tuning,
    ) -> X86 {
        X86 { tuning, ..self.clone() }
    }
}

/// `SSE2ConversionTbl`'s integer rows (the pin's `X86TargetTransformInfo.cpp`,
/// L3526): a cast, its destination and source as (lane bits, lanes), and the
/// throughput; the other three cost kinds are 1.
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L3526
const SSE2_INTEGER_CASTS: [(CastKind, (u32, u32), (u32, u32), i64); 23] = [
    (CastKind::ZExt, (64, 2), (8, 16), 4),
    (CastKind::SExt, (64, 2), (8, 16), 4),
    (CastKind::ZExt, (32, 4), (8, 16), 2),
    (CastKind::SExt, (32, 4), (8, 16), 3),
    (CastKind::ZExt, (16, 8), (8, 16), 1),
    (CastKind::SExt, (16, 8), (8, 16), 2),
    (CastKind::ZExt, (64, 2), (16, 8), 2),
    (CastKind::SExt, (64, 2), (16, 8), 3),
    (CastKind::ZExt, (32, 4), (16, 8), 1),
    (CastKind::SExt, (32, 4), (16, 8), 2),
    (CastKind::ZExt, (64, 2), (32, 4), 1),
    (CastKind::SExt, (64, 2), (32, 4), 2),
    (CastKind::Trunc, (8, 16), (16, 8), 2),
    (CastKind::Trunc, (8, 16), (16, 16), 3),
    (CastKind::Trunc, (8, 16), (32, 4), 3),
    (CastKind::Trunc, (8, 16), (32, 16), 7),
    (CastKind::Trunc, (16, 2), (32, 2), 1),
    (CastKind::Trunc, (16, 8), (32, 4), 3),
    (CastKind::Trunc, (16, 8), (32, 8), 5),
    (CastKind::Trunc, (16, 16), (32, 16), 10),
    (CastKind::Trunc, (8, 16), (64, 2), 4),
    (CastKind::Trunc, (16, 8), (64, 2), 2),
    (CastKind::Trunc, (32, 4), (64, 2), 1),
];

/// `getTypeLegalizationCost` of an integer vector of 8 to 64-bit lanes and a
/// power-of-two lane count: the registers it takes ([`xmm_pieces`]) and the
/// register type of each.
fn legalized_vector(ty: VectorTy) -> Option<(i64, (u32, u32))> {
    let Ty::Int(bits @ (8 | 16 | 32 | 64)) = ty.lane else { return None };
    if !ty.count.is_power_of_two() || ty.count < 2 {
        return None;
    }
    Some((xmm_pieces(ty)?, (bits, 128 / bits)))
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
    /// The cost of an `<8 x i64>` multiply of two sign extensions from 16 bits or fewer, lowered by
    /// `combine_vector_mul_width` as `pmullw`, `pmulhw`, `punpcklwd`, `punpckhwd` and the extension of the
    /// 32-bit products.
    fn widening_multiply_cost(
        &self,
        op: ArithOp,
        ty: VectorTy,
        args: [Option<ExtOperand>; 2],
        kind: CostKind,
    ) -> Option<Cost> {
        let narrow = |arg: Option<ExtOperand>| {
            arg.is_some_and(|ext| ext.kind == CastKind::SExt && matches!(ext.src, Ty::Int(bits) if bits <= 16))
        };
        if op != ArithOp::Mul || ty.lane != Ty::Int(64) || ty.count != 8 || !self.vector_values() {
            return None;
        }
        if !narrow(args[0]) || !narrow(args[1]) {
            return None;
        }
        let products = VectorTy { lane: Ty::Int(32), count: 8 };
        let extend = self.vector_cast_cost(CastKind::SExt, ty, products, kind)?;
        Some(Cost::of(4 + extend.value))
    }

    /// The intrinsic's row of `tti_intrinsics.txt` for this profile: long mode
    /// has its own (`x64`), as the pin's `X64CostTbl` and 64-bit
    /// legalization differ from the 32-bit ones. A `ctlz` or `cttz` called with
    /// a zero-poison flag is priced from its own `.zp` row (the pin looks
    /// up `*_ZERO_POISON`).
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
        let cost_kind = kind;
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
            None => base_intrinsic_cost(name)
                .or_else(|| library_call_intrinsic_cost(name, cost_kind))
                .unwrap_or(Cost::INVALID),
        }
    }
}

/// The legalizer's view of the SSE2 vector registers: what
/// `getTypeLegalizationCost` asks the target's types.
static SSE2_TYPES: std::sync::LazyLock<TargetLoweringInfo> = std::sync::LazyLock::new(TargetLoweringInfo::x86_64_sse2);

/// `getTypeLegalizationCost(<count x lane>).first` of an integer vector of 8 to
/// 64-bit lanes: the registers it takes. A vector under 128 bits is widened to
/// one register (`TypeWidenVector`), a wider one split in halves until
/// a half is a register. A type with no `MVT` (a count beyond `v128i8`) is
/// priced by its size alone.
fn xmm_pieces(ty: VectorTy) -> Option<i64> {
    fn registers(
        types: &TargetLoweringInfo,
        vt: EVT,
    ) -> Option<i64> {
        match types.type_conversion(vt) {
            (TypeAction::Legal, _) => Some(1),
            (TypeAction::WidenVector, wide) => registers(types, wide),
            (TypeAction::SplitVector, half) => Some(2 * registers(types, half)?),
            _ => None,
        }
    }
    let lane = match ty.lane {
        Ty::Int(bits @ (8 | 16 | 32 | 64)) => bits,
        _ => return None,
    };
    if let Some(mvt) = MVT::integer(lane).and_then(|element| MVT::vector(element, ty.count)) {
        return registers(&SSE2_TYPES, EVT::Simple(mvt));
    }
    let total = u64::from(lane) * u64::from(ty.count);
    (ty.count.is_power_of_two() && total >= 128 && total % 128 == 0).then(|| (total / 128) as i64)
}

/// A vector of 8- or 16-bit integer lanes that fills a 32-bit word.
fn is_word_vector(ty: VectorTy) -> bool {
    match ty.lane {
        Ty::Int(bits @ (8 | 16)) => u64::from(ty.count) * u64::from(bits) == 32,
        _ => false,
    }
}

impl Tti for X86 {
    fn name(&self) -> &'static str {
        "X86"
    }

    /// x86-64 first (DEVIATE VPEE6, owner x87-cordic-2, exit 2026-12-10): the
    /// other ports' backends are not known to select the vector loop's
    /// `reduce.or` and `reduce.umin`.
    fn vectorizes_early_exit(&self) -> bool {
        self.vector_values()
    }

    fn selects_sub_register_vectors(&self) -> bool {
        self.vector_values()
    }

    /// With the bandwidth deviation: it widens a chain to a lane count its
    /// model prices as `<8 x i64>`.
    fn narrows_induction_chains(&self) -> bool {
        self.tuning.maximize_vector_bandwidth
    }

    fn inline_threshold_override(&self) -> Option<i64> {
        self.tuning.inline_threshold
    }

    fn fills_pattern_at_speed(&self) -> bool {
        self.tuning.fills_pattern_at_speed
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

    /// DEVIATE UNROLL-THROUGH-CALLS: x86-64 (switch_jump_table O2: gcc unrolls
    /// the 10-trip `byte()` loop and folds its switch, 228 instructions).
    /// owner: llrm-target-x86-m16-b, exit: 2026-11-30, or when the pin's
    /// analysis prices calls.
    fn unroll_prices_calls(&self) -> bool {
        self.quad
    }

    fn unroll_trial_threshold(&self) -> Option<u32> {
        self.tuning.unroll_trial_at_o1
    }

    fn ipra_binds_local(&self) -> bool {
        self.tuning.ipra_binds_local
    }

    fn ipra_trial_at_size(&self) -> bool {
        self.tuning.ipra_trial_at_size
    }

    fn unroll_trial_at_size(&self) -> Option<u32> {
        self.tuning.unroll_trial_at_size
    }

    /// -O1 is strict on bytes where the -Os trial is
    /// (`unroll_trial_bytes_strict`): a peel copies the loop body.
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

    fn frame_slot_add_with_saved(&self) -> bool {
        self.tuning.frame_slot_add_with_saved
    }

    fn inline_trial_cold_margin(&self) -> i64 {
        self.tuning.inline_trial_cold_margin
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

    fn pops_arguments_cold_only_at_size(&self) -> bool {
        self.tuning.pops_arguments_cold_only_at_size
    }

    fn pushes_pad_at_size(&self) -> bool {
        self.tuning.pushes_pad_at_size
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

    /// `Tuning::inline_trials_at_o1` alone gives the byte view to -O1 only: the
    /// -O2 and -O3 callers keep the units their rate was fitted in.
    fn inline_size_view_at(
        &self,
        speed: bool,
    ) -> Option<std::rc::Rc<dyn Tti>> {
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

    fn memory_limits(&self) -> Option<[usize; 6]> {
        let tuning = &self.tuning;
        Some([
            tuning.max_stores_per_memset,
            tuning.max_stores_per_memset_opt_size,
            tuning.max_stores_per_memcpy,
            tuning.max_stores_per_memcpy_opt_size,
            tuning.max_stores_per_memmove,
            tuning.max_stores_per_memmove_opt_size,
        ])
    }

    fn memory_intrinsic_cost(
        &self,
        family: &str,
        length: Option<u64>,
        kind: CostKind,
    ) -> Option<Cost> {
        (self.size_bytes && kind == CostKind::CodeSize).then(|| crate::sizes::memory_intrinsic(family, length))
    }

    /// D11-MEM (owner llrm-target-x86-m16-b, exit 2026-11-30): where the size
    /// guard holds, a constant-length copy or fill the DAG builder expands
    /// counts as the loads and stores it becomes, not the pin's one
    /// instruction: a 48-byte copy was inlined for a call of 10 bytes
    /// (`them_dark`, 37 B). Above the -Os store limit it stays a call.
    fn size_caller_memory_units(
        &self,
        family: &str,
        length: u64,
    ) -> Option<i64> {
        if !self.tuning.inline_size_guard_at_size || self.real_mode.is_some() {
            return None;
        }
        let tuning = &self.tuning;
        let limit = match family {
            "memset" => tuning.max_stores_per_memset_opt_size,
            "memmove" => tuning.max_stores_per_memmove_opt_size,
            _ => tuning.max_stores_per_memcpy_opt_size,
        } as u64;
        // The widest piece the DAG builder takes (a vector under SSE2), then
        // one store per power of two of the tail.
        let width = if self.sse2 { 16 } else { 4 };
        let pieces = length / width + u64::from((length % width).count_ones());
        let per_piece = if family == "memset" { 1 } else { 2 };
        (pieces <= limit).then_some(pieces as i64 * per_piece)
    }

    /// An unknown or long length stays a call of three arguments: `justify`
    /// (two fills and a move, 8 units against a call of 9) went into `set`
    /// three times (`file.c`, 445 B against gcc's 212). The guard only: the
    /// size trial's units keep the pin's one instruction, which its byte
    /// model corrects (`idioms.c` m32 Os +94 B when its margin shifted).
    fn size_caller_memory_call_units(&self) -> Option<i64> {
        if !self.tuning.inline_size_guard_at_size || self.real_mode.is_some() {
            return None;
        }
        Some((self.direct_call_cost(3, CostKind::CodeSize).value * self.call_code_size_percent() + 50) / 100)
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

    /// `X86TTIImpl::getAddressComputationCost` of a scalar type: `BasicTTIImpl`'s 0.
    fn scalar_access_address_cost(
        &self,
        kind: CostKind,
    ) -> Cost {
        if self.size_bytes && kind == CostKind::CodeSize {
            return Cost::of(3);
        }
        Cost::FREE
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

    fn switch_lookup_trial_at_size(&self) -> bool {
        self.tuning.switch_lookup_trial_at_size
    }

    fn opt_size_inline_credit_gate(&self) -> bool {
        self.tuning.inline_trial_credit_gate
    }

    fn loop_idiom_copy_trial_at_size(&self) -> bool {
        self.tuning.loop_idiom_copy_trial_at_size
    }

    fn jump_threading_latch_trial_at_size(&self) -> bool {
        self.tuning.jump_threading_latch_trial_at_size
    }

    fn os_trial_clock_cap_percent(&self) -> Option<i64> {
        self.tuning.os_trial_clock_cap_percent
    }

    fn cold_site_ignores_sinks(&self) -> bool {
        self.tuning.cold_site_ignores_sinks
    }

    fn inline_last_call_in_cold_site(&self) -> bool {
        self.tuning.inline_last_call_in_cold_site
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

    fn rotation_of_exiting_latch_when_guard_folds(&self) -> bool {
        self.tuning.rotate_exiting_latch_when_guard_folds
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

    fn memset_gap_merge_at_size(&self) -> bool {
        self.tuning.memset_gap_merge_at_size && self.real_mode.is_none()
    }

    fn callee_saved_clocks(
        &self,
        peak: usize,
    ) -> i64 {
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

    /// DEVIATE MERGEFUNC-LOCAL-X64 (owner llrm-target-x86-m16-b, exit
    /// 2026-11-30): x86-64 drops a local function that is the same code as
    /// another at -O1 to -O3, as gcc's `-fipa-icf` does at -O2 (signbit_idioms:
    /// 8 of 24 functions were copies; each function is an FDE the start-up
    /// unwinder registration walks, 110 instructions a function).
    fn merge_local_functions_at_speed(&self) -> bool {
        self.quad
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

    fn rep_movs_at_speed_max(&self) -> u64 {
        self.tuning.rep_movs_at_speed_max
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

    /// DEVIATE D5-M16 (owner x86-m16-b, exit 2026-11-24): in real mode, with no
    /// conditional move, each select a two-entry phi would become costs the
    /// branch and move it lowers to. m16 grep -O1/-O2 `if (b == 10) { lines +=
    /// inline_hit; inline_hit = 0; }` became two selects and an add run on
    /// every byte of the scan loop (2 clk, and the byte spilled): 34.89M
    /// clk against bcc's 33.86M, 29.72M without the fold.
    fn select_branch_extra(
        &self,
        _optsize: bool,
    ) -> i64 {
        i64::from(!self.cmov && self.real_mode.is_some())
    }

    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (self.tuning.unroll_threshold, self.tuning.unroll_max_percent_threshold_boost)
    }

    fn unroll_threshold_at_o3(&self) -> Option<u32> {
        self.tuning.unroll_threshold_at_o3
    }

    fn split_paths_at_o3(&self) -> bool {
        self.tuning.split_paths_at_o3
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

    fn threads_by_relation(&self) -> bool {
        self.tuning.threads_by_relation
    }

    fn backward_threading(&self) -> bool {
        self.tuning.backward_threading
    }

    fn sink_common_at_size(&self) -> bool {
        self.tuning.sink_common_at_size
    }

    fn late_memcpyopt_at_size(&self) -> bool {
        self.tuning.late_memcpyopt_at_size
    }

    fn sink_common_phi_cost(&self) -> usize {
        self.tuning.sink_common_phi_cost
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

    fn wide_float_to_int(
        &self,
        bits: u32,
    ) -> Option<u32> {
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

    fn phiopt_one_feeding_statement(&self) -> bool {
        self.tuning.phiopt_one_feeding_statement
    }

    fn licm_float_register_limit(&self) -> Option<u32> {
        self.tuning.licm_float_register_limit
    }

    fn min_jump_table_density(&self) -> Option<u64> {
        self.tuning.min_jump_table_density
    }

    fn jump_table_size_growth_ratio(&self) -> Option<u64> {
        self.tuning.jump_table_size_growth_ratio
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

    /// gcc and the pin make no clone at -O2 (`-fipa-cp-clone` is on at -O3
    /// only; the pin has none): QuickrBasic `repl.c` -O2 m32 16,336 -> 10,738 B
    /// (gcc 6,025), `read_file` cloned eight times for 40 clocks a call.
    fn ipa_cp_clones_at_o2(&self) -> bool {
        false
    }

    fn ipa_cp_max_clones(&self) -> usize {
        self.tuning.ipa_cp_max_clones
    }

    fn lsr_spill_pressure_margin(&self) -> Option<u32> {
        self.tuning.lsr_spill_pressure_margin
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

    fn select_optimize_penalty(&self) -> Option<u64> {
        // Where a select is a conditional move, it is the cheaper form.
        self.tuning.select_optimize_penalty.filter(|_| !self.cmov)
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

    fn inlines_variable_memcpy_at_size(&self) -> bool {
        self.tuning.inline_variable_memcpy_at_size
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

    /// The size guard binds -Os; -O1 keeps it only where the target says
    /// so (real mode).
    fn opt_size_inline_rule_at_o1(&self) -> bool {
        self.real_mode.is_some() || self.tuning.inline_size_guard_at_o1
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

    /// `getVectorInstrCost` of a float lane at a constant index: the pin's SSE2 row (`tti_vector_oracle.txt`).
    /// A gather of float lanes is priced by it; integer lanes and a variable index are not priced.
    fn vector_instr_cost(
        &self,
        op: LaneOp,
        ty: VectorTy,
        index: Option<u32>,
        into_poison: bool,
        _context: VectorInstrContext,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        if !self.vector_values() || !self.tuning.vector_float_costs {
            return None;
        }
        crate::tti_vector::float_lane_cost(op, ty, index?, into_poison, cost_kind)
    }

    /// `getScalarizationOverhead` of float lanes: the pin's lane rows summed. A gather of a strided load is
    /// priced by it; other lanes are not priced.
    fn scalarization_overhead(
        &self,
        ty: VectorTy,
        insert: bool,
        extract: bool,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        if !self.vector_values() || !self.tuning.vector_float_costs {
            return None;
        }
        crate::tti_vector::float_scalarization_overhead(ty, insert, extract, cost_kind)
    }

    /// The vectors of a compare are selected where the target has vector values
    /// and says so (`Tuning::vector_compares`).
    fn selects_vector_compares(&self) -> bool {
        self.tuning.vector_compares && self.vector_values()
    }

    /// A word vector is a speed trade of a few bytes (the vector body, its
    /// remainder and the splat); at -Os the bytes rank first, as in gcc, which
    /// does not vectorize there.
    fn vectorizes_at_size(&self) -> bool {
        self.tuning.vectorize_at_size && (self.vector_values() || !self.word_vectors)
    }

    fn should_maximize_vector_bandwidth(&self) -> bool {
        self.tuning.maximize_vector_bandwidth
    }

    fn narrows_reduction_accumulators(&self) -> bool {
        self.tuning.narrow_reduction_accumulators
    }

    fn bandwidth_vf_without_epilogue(&self) -> bool {
        self.tuning.bandwidth_vf_without_epilogue
    }

    /// `bsr` and `tzcnt`/`bsf` are one instruction at x86-64 (LI-18).
    fn shift_loop_counts(&self) -> bool {
        self.tuning.shift_loop_counts
    }

    /// The 128-bit integer vectors need SSE2 and, for now, the 64-bit mode (x86
    /// vectors plan, PR-A).
    fn vector_values(&self) -> bool {
        self.sse2 && self.quad
    }

    /// The SLP vectorizer runs where the target takes vector values: its
    /// store chains (a run of scalar stores of constants or of one tree is one
    /// vector store) are priced by `vector_memory_op_cost`; a tree whose
    /// operation this target does not price is not formed (the hooks are
    /// `None`), so only what the SSE2 rows price vectorizes.
    fn slp_priced(&self) -> bool {
        self.vector_values()
    }

    /// `getRegisterBitWidth(RGK_FixedWidthVector)` of an SSE2 target: one XMM
    /// register. Only where the 128-bit integer vectors are taken
    /// (`vector_values`): the others stay scalar and read 0.
    fn vector_register_bits(&self) -> u32 {
        if self.vector_values() {
            128
        } else if self.word_vectors {
            32
        } else {
            0
        }
    }

    fn word_vectors(&self) -> bool {
        self.word_vectors
    }

    /// A word vector has no horizontal step: `vectorlower` takes no
    /// `llvm.vector.reduce.*`, so a loop with a reduction stays scalar (the
    /// pin's `ExpandReductions` has no word-mode form).
    fn lowers_vector_reductions(&self) -> bool {
        self.vector_values() || !self.word_vectors
    }

    /// `X86TTIImpl::getNumberOfRegisters`: 16 in long mode, for the general
    /// and the vector class.
    fn number_of_registers(
        &self,
        _class: RegisterClass,
    ) -> u32 {
        if self.vector_values() {
            16
        } else if self.word_vectors {
            4
        } else {
            0
        }
    }

    /// `X86TTIImpl::getMaxInterleaveFactor`: a scalar loop is left to the
    /// unroller; a vector one is interleaved twice (4 needs AVX).
    fn max_interleave_factor(
        &self,
        vf: u32,
        _has_unordered_reductions: bool,
    ) -> u32 {
        if vf == 1 || !self.vector_values() { 1 } else { 2 }
    }

    /// `X86TTIImpl::getMemoryOpCost` of a contiguous vector access of whole XMM
    /// registers (`movups`): one per register; a type under a register is one
    /// `movq` or `movd` (widened). A lane that is not an integer of 8 to 64
    /// bits, or a count with no `MVT`, is not priced (DEVIATE X86V1, owner
    /// llrm-target-x86-m64-b, exit 2026-11-30).
    fn vector_memory_op_cost(
        &self,
        op: MemoryOp,
        ty: VectorTy,
        _align: u64,
        kind: CostKind,
    ) -> Option<Cost> {
        if !self.vector_values() {
            // a word vector is one `mov` of a dword, the scalar load's price
            return (self.word_vectors && is_word_vector(ty)).then(|| Cost::of(1));
        }
        if self.tuning.vector_float_costs && matches!(ty.lane, Ty::Float | Ty::Double) {
            return crate::tti_vector::float_memory_op_cost(op, ty, kind);
        }
        let pieces = xmm_pieces(ty)?;
        // Every other kind is `TCC_Basic` ("TODO: Handle other cost kinds"), a
        // load's latency the base's 4.
        Some(match kind {
            CostKind::RecipThroughput => Cost::of(pieces),
            CostKind::Latency if op == MemoryOp::Load => Cost::of(4),
            _ => Cost::BASIC,
        })
    }

    /// `X86TTIImpl::getCmpSelInstrCost` of an integer vector, the SSE2 rows
    /// (L4062 and L4069): the compares `pcmpeqX` and `pcmpgtX` of 8, 16 and 32
    /// bit lanes cost one per register, the selects (`pand`, `pandn`, `por`)
    /// two; the predicates SSE2 has no compare for cost the instructions that
    /// make them (L3877), taking the second operand as no constant. A float
    /// lane or a 64-bit compare is not priced (DEVIATE X86V3, owner
    /// llrm-target-x86-m64-b, exit 2026-12-10).
    fn vector_cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: VectorTy,
        predicate: Option<Predicate>,
        kind: CostKind,
    ) -> Option<Cost> {
        use llrm_mir::opcode::{FloatPredicate, IntPredicate};
        if !self.vector_values() {
            return None;
        }
        // The float lanes of `cmpps`/`cmppd` and the mask select over them: the pin's SSE2 rows, read off
        // `opt -passes='print<cost-model>' -mcpu=x86-64` (`SETCC` {2, 5, 1, 1} a register, `one` and `ueq` two
        // compares and a combine {5, 11, 3, 3}; the select as the integer lanes').
        if let Ty::Float | Ty::Double = ty.lane {
            if !self.tuning.vector_float_costs {
                return None;
            }
            let bits = if ty.lane == Ty::Float { 32 } else { 64 };
            let pieces = xmm_pieces(VectorTy { lane: Ty::Int(bits), ..ty })?;
            let row = match (op, predicate) {
                (CmpSel::FCmp, Some(Predicate::Float(FloatPredicate::One | FloatPredicate::Ueq))) => {
                    all(5, 11, 3, 3)
                }
                (CmpSel::FCmp, _) => all(2, 5, 1, 1),
                (CmpSel::Select, _) => all(2, 2, 3, 3),
                _ => return None,
            };
            return row[kind.index()].map(|each| Cost::of(pieces * each));
        }
        let Ty::Int(bits @ (8 | 16 | 32)) = ty.lane else { return None };
        let pieces = xmm_pieces(ty)?;
        let (row, extra) = match op {
            CmpSel::ICmp => {
                let extra = match predicate {
                    Some(Predicate::Int(IntPredicate::Ne | IntPredicate::Sge | IntPredicate::Sle)) => 1,
                    Some(Predicate::Int(IntPredicate::Ult | IntPredicate::Ugt)) => 2,
                    Some(Predicate::Int(IntPredicate::Ule | IntPredicate::Uge)) if bits < 32 => 1,
                    Some(Predicate::Int(IntPredicate::Ule | IntPredicate::Uge)) => 3,
                    Some(Predicate::Int(_)) => 0,
                    _ => 3,
                };
                (all(1, 1, 1, 1), extra)
            }
            CmpSel::Select => (all(2, 2, 3, 3), 0),
            _ => return None,
        };
        row[kind.index()].map(|each| Cost::of(pieces * (extra + each)))
    }

    /// `X86TTIImpl::getArithmeticInstrCost`, the SSE2 rows of the integer ops
    /// isel selects for every operand, `LT.first` registers times the row of
    /// the legalized type (`SSE2CostTable`): `pand`, `por`, `pxor` cost one;
    /// `paddq`/`psubq` {1, 2, 1, 2}; the multiplies of 8-, 16- and 32-bit lanes
    /// are the pin's rows (the 64-bit lane's comes with its lowering). A vector
    /// of 8-bit lanes within 64 bits is multiplied as 16-bit lanes (extend,
    /// `pmullw`, truncate). Other adds and subtracts have no row: the base
    /// model, one per register for the throughput and one for the other
    /// kinds (`TCK_RecipThroughput` only scales). Byte shifts, a variable
    /// shift count and the divisions are not priced, so a loop holding one
    /// stays scalar (DEVIATE X86V2, owner llrm-target-x86-m64-b, exit
    /// 2026-11-24).
    fn vector_arithmetic_cost(
        &self,
        op: ArithOp,
        ty: VectorTy,
        kind: CostKind,
    ) -> Option<Cost> {
        if !self.vector_values() {
            // only the bitwise operations act lane by lane on a word
            return (self.word_vectors
                && is_word_vector(ty)
                && matches!(op, ArithOp::And | ArithOp::Or | ArithOp::Xor))
            .then(|| Cost::of(1));
        }
        let throughput = kind == CostKind::RecipThroughput;
        if self.tuning.vector_float_costs && matches!(ty.lane, Ty::Float | Ty::Double) {
            return crate::tti_vector::float_arithmetic_cost(op, ty, kind);
        }
        match op {
            ArithOp::And | ArithOp::Or | ArithOp::Xor => xmm_pieces(ty).map(Cost::of),
            // `paddq`, `psubq`: {1, 2, 1, 2}.
            ArithOp::Add | ArithOp::Sub if ty.lane == Ty::Int(64) => {
                xmm_pieces(ty).map(|pieces| Cost::of(pieces * [1, 2, 1, 2][kind.index()]))
            }
            ArithOp::Add | ArithOp::Sub => xmm_pieces(ty).map(|pieces| Cost::of(if throughput { pieces } else { 1 })),
            // `psllX`/`psrlX`/`psraX` by an immediate (a constant splat amount,
            // the only one the lowering takes); there is no 64-bit arithmetic
            // shift before AVX-512.
            ArithOp::Shl | ArithOp::LShr if matches!(ty.lane, Ty::Int(16 | 32 | 64)) => xmm_pieces(ty).map(Cost::of),
            ArithOp::AShr if matches!(ty.lane, Ty::Int(16 | 32)) => xmm_pieces(ty).map(Cost::of),
            // `SSE2UniformConstCostTable`: `v16i8` is `psllw` + `pand` (an
            // arithmetic shift also `pxor`, `psubb`), `v2i64` `psrad` and
            // shuffles.
            ArithOp::Shl | ArithOp::LShr if ty.lane == Ty::Int(8) => {
                xmm_pieces(ty).map(|pieces| Cost::of(pieces * [1, 7, 2, 3][kind.index()]))
            }
            ArithOp::AShr if ty.lane == Ty::Int(8) => {
                xmm_pieces(ty).map(|pieces| Cost::of(pieces * [3, 9, 5, 6][kind.index()]))
            }
            ArithOp::AShr if ty.lane == Ty::Int(64) => {
                xmm_pieces(ty).map(|pieces| Cost::of(pieces * [3, 5, 6, 6][kind.index()]))
            }
            // `vXi8` multiplications are promoted to `vXi16`; a type of 64
            // bits or less is extended and packed more cheaply.
            ArithOp::Mul if ty.lane == Ty::Int(8) && ty.count <= 8 => {
                let wide = VectorTy { lane: Ty::Int(16), count: ty.count };
                Some(Cost::of(
                    self.vector_cast_cost(CastKind::ZExt, wide, ty, kind)?.value
                        + self.vector_cast_cost(CastKind::Trunc, ty, wide, kind)?.value
                        + self.vector_arithmetic_cost(ArithOp::Mul, wide, kind)?.value,
                ))
            }
            // The pin's SSE2 rows, a register each: {throughput, latency, code
            // size, size and latency}.
            // `v16i8`: 2*unpack/2*pmullw/2*and/pack. `v8i16`: `pmullw`, the one
            // multiply that keeps its lane.
            // `v4i32`: 3*pmuludq/4*shuffle.
            ArithOp::Mul => {
                let row = match ty.lane {
                    Ty::Int(8) => [6, 18, 12, 12],
                    Ty::Int(16) => [1, 5, 1, 1],
                    Ty::Int(32) => [6, 8, 7, 7],
                    // `v2i64`: 3*pmuludq/2*shift/add and the shuffles.
                    Ty::Int(64) => [7, 10, 10, 10],
                    _ => return None,
                };
                xmm_pieces(ty).map(|pieces| Cost::of(row[kind.index()] * pieces))
            }
            _ => None,
        }
    }

    /// `getArithmeticInstrCost` of a vector shift by an amount that is not a
    /// constant: the `SSE2CostTable` rows of what `LowerShift` makes of it
    /// (the `pcmpgt` ladders, a shift a lane and a blend). The other kinds of
    /// amount, and every other operation, are `vector_arithmetic_cost`. A
    /// splat of a variable has its own, cheaper rows (`SSE2UniformCostTable`),
    /// and constants that differ by lane dearer ones (`SSE2ConstCostTable`);
    /// the operand information has no uniform flag, so the first is priced as
    /// the dearer variable and the second as a uniform constant (DEVIATE
    /// X86V5, owner llrm-target-x86-m64-b, exit 2026-11-27).
    /// DEVIATE WIDEN-MULT (owner llrm-target-x86-m64-b, exit 2026-11-30): a multiply of two lanes that
    /// are signed extensions of 16 bits or fewer is lowered as `pmullw`, `pmulhw`, the two unpacks and a
    /// sign extension of the 32-bit products (gcc's `vect_recog_widen_mult_pattern`); the pin prices the
    /// `i64` multiply as it is, which `X86TTIImpl` has no row for.
    fn vector_arithmetic_cost_ext(
        &self,
        op: ArithOp,
        ty: VectorTy,
        args: [Option<ExtOperand>; 2],
        kind: CostKind,
    ) -> Result<Option<Cost>, ()> {
        if let Some(cost) = self.widening_multiply_cost(op, ty, args, kind) {
            return Ok(Some(cost));
        }
        Ok(self.vector_arithmetic_cost(op, ty, kind))
    }

    /// The multiplies `widening_multiply_cost` prices: the extensions their operands are leave no
    /// instruction (the 16-bit lanes are what `pmullw` takes).
    fn vector_ext_cost(
        &self,
        kind: CastKind,
        dst: VectorTy,
        src: VectorTy,
        users: &[ExtUser],
        cost_kind: CostKind,
    ) -> Option<Cost> {
        let widened = |user: &ExtUser| {
            user.op == Some(ArithOp::Mul)
                && self.widening_multiply_cost(ArithOp::Mul, dst, user.operands, cost_kind).is_some()
        };
        if kind == CastKind::SExt && !users.is_empty() && users.iter().all(widened) {
            return Some(Cost::FREE);
        }
        self.vector_cast_cost(kind, dst, src, cost_kind)
    }

    fn vector_arithmetic_cost_op2(
        &self,
        op: ArithOp,
        ty: VectorTy,
        second: OperandInfo,
        kind: CostKind,
    ) -> Option<Cost> {
        if !self.vector_values() || second.constant || !matches!(op, ArithOp::Shl | ArithOp::LShr | ArithOp::AShr) {
            return self.vector_arithmetic_cost(op, ty, kind);
        }
        let bits = match ty.lane {
            Ty::Int(bits @ (8 | 16 | 32 | 64)) => bits,
            _ => return None,
        };
        let row: [i64; 4] = match (op, bits) {
            (ArithOp::Shl, 8) => [13, 21, 26, 28],
            (ArithOp::Shl, 16) => [24, 27, 16, 20],
            (ArithOp::Shl, 32) => [17, 19, 10, 12],
            (ArithOp::LShr, 8) => [14, 28, 27, 30],
            (ArithOp::LShr, 16) => [16, 19, 31, 31],
            (ArithOp::LShr | ArithOp::AShr, 32) => [12, 12, 15, 19],
            (ArithOp::Shl | ArithOp::LShr, _) => [4, 6, 5, 7],
            (ArithOp::AShr, 8) => [27, 30, 54, 54],
            (ArithOp::AShr, 16) => [16, 19, 31, 31],
            _ => [8, 11, 12, 16],
        };
        xmm_pieces(ty).map(|pieces| Cost::of(pieces * row[kind.index()]))
    }

    /// `X86TTIImpl::getCastInstrCost` for the integer extends and truncates
    /// (the SSE2 rows): the row of the exact types, else of the legalized
    /// ones times the larger register count; a truncate to the type it already
    /// is, once legalized, is free. Anything else (a float, a mask, a
    /// scalarized cast) is not priced.
    /// `X86TTIImpl::getCastInstrCost` for the integer extends and truncates (the SSE2 rows): the row of the exact
    /// types, else of the legalized ones times the larger register count; a truncate to the type it already is, once
    /// legalized, is free. Anything else (a mask, a scalarized cast) is not priced; a float is `tti_vector`'s.
    fn vector_cast_cost(
        &self,
        kind: CastKind,
        dst: VectorTy,
        src: VectorTy,
        cost_kind: CostKind,
    ) -> Option<Cost> {
        // DEVIATE X86V3: a cast to or from float lanes, and the extension of a mask, are the pin's row
        // (`tti_vector_oracle.txt`).
        if matches!(dst.lane, Ty::Float | Ty::Double)
            || matches!(src.lane, Ty::Float | Ty::Double)
            || src.lane == Ty::Int(1)
        {
            if !self.vector_values() || !self.tuning.vector_float_costs {
                return None;
            }
            return crate::tti_vector::cast_cost(kind, dst, src, cost_kind);
        }
        if !self.vector_values() || !matches!(kind, CastKind::ZExt | CastKind::SExt | CastKind::Trunc) {
            return None;
        }
        let (Ty::Int(to), Ty::Int(from)) = (dst.lane, src.lane) else { return None };
        let row = |kind: CastKind, dst: (u32, u32), src: (u32, u32)| {
            SSE2_INTEGER_CASTS
                .iter()
                .find(|&&(one, to, from, _)| one == kind && to == dst && from == src)
                .map(|&(.., throughput)| if cost_kind == CostKind::RecipThroughput { throughput } else { 1 })
        };
        if let Some(cost) = row(kind, (to, dst.count), (from, src.count)) {
            return Some(Cost::of(cost));
        }
        let ((to_pieces, to_mvt), (from_pieces, from_mvt)) = (legalized_vector(dst)?, legalized_vector(src)?);
        if kind == CastKind::Trunc && to_mvt == from_mvt {
            return Some(Cost::FREE);
        }
        row(kind, to_mvt, from_mvt).map(|cost| Cost::of(to_pieces.max(from_pieces) * cost))
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

    fn opt_size_inline_guard_yields_to_inlinehint(&self) -> bool {
        self.tuning.inline_size_guard_yields_to_inlinehint
    }

    fn opt_size_inline_hot_growth_percent(&self) -> Option<i64> {
        self.tuning.inline_size_hot_growth_percent
    }

    fn unroll_promotion_fits_registers(&self) -> bool {
        self.tuning.unroll_promotion_fits_registers
    }

    fn unroll_store_immediate_bits(&self) -> u32 {
        self.tuning.unroll_store_immediate_bits
    }

    fn lsr_size_model(&self) -> Option<llrm_mir::tti::LsrSizeModel> {
        self.tuning
            .lsr_size_order
            .then(
                || {
                    let model = if self.tuning.lsr_size_x86_64 {
                        crate::encoding::lsr_size_model_64()
                    } else {
                        llrm_mir::tti::LsrSizeModel::X86
                    };
                    llrm_mir::tti::LsrSizeModel { derived_setup_free: self.tuning.lsr_derived_setup_free, ..model }
                },
            )
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

    fn lsr_constant_setup_is_one(&self) -> bool {
        self.tuning.lsr_constant_setup_is_one
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

    /// Long mode's DAG selects an access of any byte size (an `i24` is a 16-bit
    /// and an 8-bit access, an `i40` a dword and a byte), as the pin's
    /// does: SROA cuts the slices where the pin cuts them.
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
        // a shift-and-add chain is a shift and an add for each term past the
        // first
        let chain = multiplier.map_or(i64::MAX, |m| 2 * (signed_digits(m) as i64 - 1).max(1));
        multiply_clocks > 0 && multiply_clocks.min(chain) > loop_carried_clocks
    }

    fn reduces_outer_address_scale(
        &self,
        size: u128,
    ) -> bool {
        let llrm_mir::target::Tuning { multiply_clocks, loop_carried_clocks, counter_exit_compare_clocks, .. } =
            self.tuning;
        // The addressing mode scales by 1, 2, 4 or 8 for nothing; the rest of
        // the size is a `lea` or a shift-and-add chain every trip, and the
        // counter that stands for it also ends the loop, so the compare of the
        // index goes too.
        let rest = size >> size.trailing_zeros().min(3);
        let chain = 2 * (signed_digits(rest) as i64 - 1).max(1);
        rest > 1
            && multiply_clocks > 0
            && multiply_clocks.min(chain) + counter_exit_compare_clocks > loop_carried_clocks
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
        if params.base_cost <= extra {
            CondMerging::SPLIT
        } else {
            CondMerging { base_cost: params.base_cost - extra, ..params }
        }
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
        // DEVIATE X64-PIE-SCALE: a scale of 2, 4 or 8 is a `lea`'d symbol with a
        // scaled index, as it is without PIE.
        let scaled_symbol = self.tuning.pie_scaled_symbol && matches!(scale, 2 | 4 | 8);
        let refused_beside_symbol = base_offset != 0 || (scale > 1 && !scaled_symbol);
        if self.quad && self.position_independent.get() && base_gv && refused_beside_symbol {
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

    /// DEVIATE WIDEN-COLD-CHREC: x86-64 (sieve O2/O3: +595 instructions of the
    /// 15223 for 2 phis the scan loop steps for 11 marking-loop entries).
    /// owner: llrm-target-x86-m16-b, exit: 2026-11-30, or when the pin
    /// weighs the entry frequency.
    fn avoids_cold_outer_addrec_expansion(&self) -> bool {
        self.quad
    }

    fn lsr_drops_worse_solution_at_size(&self) -> bool {
        self.tuning.lsr_drop_worse_at_size
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

    fn xors_truth_value_compare(&self) -> bool {
        self.tuning.truth_compare_xor
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
                (Ty::Int(8), Ty::Int(16 | 32 | 64)) | (Ty::Int(16), Ty::Int(32 | 64)) | (Ty::Int(32), Ty::Int(64))
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
                cost = self.tuning.div_by_constant_size_cost.map_or(cost + 2, i64::from);
            }
            return Cost::of(count * cost);
        }
        if !second.constant {
            if kind == CostKind::CodeSize
                && let Some(cost) = self.tuning.div_by_variable_size_cost
                && scalar_variable_divide(op, legal).is_some()
            {
                return Cost::of(count * i64::from(cost));
            }
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

#[cfg(test)]
mod pieces_tests {
    use super::*;

    /// A vector under a register was priced as none (`<8 x i8>`: no
    /// `getTypeLegalizationCost`), so no loop stored one: `copy_runs`' init
    /// loops stayed scalar. The target widens it to one register
    /// (`TypeWidenVector`); a wider one splits.
    #[test]
    fn a_vector_under_a_register_is_one_piece_and_a_wider_one_is_split() {
        let pieces = |bits: u32, count: u32| xmm_pieces(VectorTy { lane: Ty::Int(bits), count });
        assert_eq!([pieces(8, 8), pieces(16, 4), pieces(32, 2)], [Some(1); 3]);
        assert_eq!([pieces(8, 16), pieces(16, 8), pieces(32, 4), pieces(64, 2)], [Some(1); 4]);
        assert_eq!([pieces(8, 32), pieces(16, 16), pieces(32, 8), pieces(64, 4)], [Some(2); 4]);
        assert_eq!(pieces(8, 64), Some(4));
        assert_eq!(xmm_pieces(VectorTy { lane: Ty::Float, count: 4 }), None);
    }
}
