//! The pin's cost of the vector instructions a float loop is made of, at
//! x86-64 (`-mcpu=x86-64`, SSE2): `X86TTIImpl::getCastInstrCost`,
//! `getArithmeticInstrCost` and `getMemoryOpCost` on fixed vectors.
//!
//! # How it works
//! The pin's rows are read off its `opt -passes='print<cost-model>'` for every
//! shape the planner forms (`tti_vector_oracle.py` writes
//! `tti_vector_oracle.txt`): a cast between integer and float lanes, an integer
//! width change, `fadd`, `fsub`, `fmul`, `fdiv`, `fneg`, and a load or store of
//! float lanes, per cost kind. A type that is wider than a register is the
//! pin's own split: the rows hold the answer for it, so no splitting is redone
//! here. A shape with no row is not priced (`None`), which the planner reads as
//! "do not vectorize".
//!
//! # Example
//! `sitofp <2 x i32> to <2 x double>` is 4 at throughput, `fadd <2 x double>`
//! 2, `fdiv <2 x double>` 69: a loop of `a[j] = j * 0.5` costs the vector plan
//! 4 + 2 + 1 = 7 for two lanes, against the scalar 9 for one.
//!
//! # Runs
//! Read by the loop vectorizer's cost model through `Tti::vector_cast_cost`,
//! `vector_arithmetic_cost` and `vector_memory_op_cost`, and by the SLP
//! vectorizer's gathers (`vector_instr_cost`, `shuffle_cost`), where the target
//! takes vector values (x86-64).

use std::collections::HashMap;
use std::sync::OnceLock;

use llrm_mir::tti::{ArithOp, CastKind, Cost, CostKind, LaneOp, MemoryOp, Ty, VectorTy};

const ORACLE: &str = include_str!("tti_vector_oracle.txt");

/// `kind op type[ type] -> cost` of the pin, `None` for `Invalid`.
fn rows() -> &'static HashMap<String, Option<i64>> {
    static ROWS: OnceLock<HashMap<String, Option<i64>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        ORACLE
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let (key, cost) = line.rsplit_once(' ').expect("a row ends in its cost");
                (key.to_owned(), cost.parse().ok())
            })
            .collect()
    })
}

fn kind_name(kind: CostKind) -> &'static str {
    match kind {
        CostKind::RecipThroughput => "throughput",
        CostKind::Latency => "latency",
        CostKind::CodeSize => "code-size",
        CostKind::SizeAndLatency => "size-latency",
    }
}

fn type_name(ty: VectorTy) -> Option<String> {
    let lane = match ty.lane {
        Ty::Int(bits @ (8 | 16 | 32 | 64)) => format!("i{bits}"),
        Ty::Float => "float".to_owned(),
        Ty::Double => "double".to_owned(),
        _ => return None,
    };
    Some(format!("{}x{lane}", ty.count))
}

fn lookup(key: String) -> Option<Cost> {
    rows().get(&key).copied().flatten().map(Cost::of)
}

/// `getCastInstrCost` of a cast between vectors of one lane count.
pub(crate) fn cast_cost(
    kind: CastKind,
    dst: VectorTy,
    src: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    let op = match kind {
        CastKind::Trunc => "trunc",
        CastKind::ZExt => "zext",
        CastKind::SExt => "sext",
        CastKind::FPTrunc => "fptrunc",
        CastKind::FPExt => "fpext",
        CastKind::FPToUI => "fptoui",
        CastKind::FPToSI => "fptosi",
        CastKind::UIToFP => "uitofp",
        CastKind::SIToFP => "sitofp",
        _ => return None,
    };
    lookup(format!("{} {op} {} {}", kind_name(cost_kind), type_name(dst)?, type_name(src)?))
}

/// `getArithmeticInstrCost` of a float operation on vectors.
pub(crate) fn float_arithmetic_cost(
    op: ArithOp,
    ty: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    let op = match op {
        ArithOp::FAdd => "fadd",
        ArithOp::FSub => "fsub",
        ArithOp::FMul => "fmul",
        ArithOp::FDiv => "fdiv",
        ArithOp::FNeg => "fneg",
        _ => return None,
    };
    lookup(format!("{} {op} {}", kind_name(cost_kind), type_name(ty)?))
}

/// `getMemoryOpCost` of a contiguous vector of float lanes.
pub(crate) fn float_memory_op_cost(
    op: MemoryOp,
    ty: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    if !matches!(ty.lane, Ty::Float | Ty::Double) {
        return None;
    }
    let op = if op == MemoryOp::Load { "load" } else { "store" };
    lookup(format!("{} {op} {}", kind_name(cost_kind), type_name(ty)?))
}

/// `getVectorInstrCost` of an `insertelement` or `extractelement` at a constant
/// lane. An insert into poison is a different row (`insertp`): the pin prices
/// the first lane of a build as the cheaper `movd`.
pub(crate) fn lane_cost(
    op: LaneOp,
    ty: VectorTy,
    index: Option<u32>,
    into_poison: bool,
    cost_kind: CostKind,
) -> Option<Cost> {
    let name = match (op, into_poison) {
        (LaneOp::Extract, _) => "extract",
        (LaneOp::Insert, true) => "insertp",
        (LaneOp::Insert, false) => "insert",
    };
    lookup(format!("{} {name}{} {}", kind_name(cost_kind), index?, type_name(ty)?))
}

/// `getShuffleCost` of a broadcast of lane 0.
pub(crate) fn broadcast_cost(
    ty: VectorTy,
    cost_kind: CostKind,
) -> Option<Cost> {
    lookup(format!("{} broadcast {}", kind_name(cost_kind), type_name(ty)?))
}
