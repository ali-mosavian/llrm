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
//! `vector_arithmetic_cost` and `vector_memory_op_cost`, where the target takes
//! vector values (x86-64).

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
        Ty::Int(bits @ (1 | 8 | 16 | 32 | 64)) => format!("i{bits}"),
        // DEVIATE X86V4 (owner x87-cordic-2, exit 2026-12-05): a float vector
        // of half a register (`<2 x float>`) has no selection (its lane
        // extract, conversions and arithmetic widen to `v4f32` in the pin), so
        // it is not priced and no planner forms it.
        Ty::Float if ty.count < 4 => return None,
        Ty::Float => "float".to_owned(),
        Ty::Pointer => "ptr".to_owned(),
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
    // Only the conversions the selector has (`cvtdq2ps`, `cvtdq2pd`,
    // `cvtpd2ps`, `uitofp` of `v4i32`): a loop the planner widens must
    // compile, so a cast it cannot select is not priced.
    let selected = match (kind, dst.lane, src.lane) {
        (CastKind::SIToFP, Ty::Float, Ty::Int(32)) => dst.count == 4,
        (CastKind::SIToFP, Ty::Double, Ty::Int(32)) => matches!(dst.count, 2 | 4),
        // `LowerUINT_TO_FP`: `cvtdq2ps` for a clear sign bit, the
        // mantissa-halves sequence otherwise.
        (CastKind::UIToFP, Ty::Float, Ty::Int(32)) => dst.count == 4,
        (CastKind::FPTrunc, Ty::Float, Ty::Double) => dst.count == 4,
        (CastKind::Trunc | CastKind::ZExt | CastKind::SExt, _, _) => true,
        _ => false,
    };
    if !selected {
        return None;
    }
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
        _ => return None,
    };
    lookup(format!("{} {op} {}", kind_name(cost_kind), type_name(ty)?))
}

/// `getVectorInstrCost` of one `insertelement` or `extractelement` of float
/// lanes at a constant index.
pub(crate) fn float_lane_cost(
    op: LaneOp,
    ty: VectorTy,
    index: u32,
    into_poison: bool,
    cost_kind: CostKind,
) -> Option<Cost> {
    // `<4 x i32>` rows only (the SSE2 gather of an integer counter that feeds a
    // convert); other integer lanes stay unpriced.
    if !matches!(ty.lane, Ty::Float | Ty::Double | Ty::Int(32)) {
        return None;
    }
    let kind = kind_name(cost_kind);
    let ty = type_name(ty)?;
    match op {
        LaneOp::Insert => {
            lookup(format!("{kind} insert {ty} {index} {}", if into_poison { "poison" } else { "vector" }))
        }
        LaneOp::Extract => lookup(format!("{kind} extract {ty} {index}")),
    }
}

/// `getVectorInstrCost` of an `extractelement` of a float or pointer lane at a
/// constant index.
fn lane_extract_cost(
    ty: VectorTy,
    index: u32,
    cost_kind: CostKind,
) -> Option<Cost> {
    if ty.lane == Ty::Pointer {
        return lookup(format!("{} extract {} {index}", kind_name(cost_kind), type_name(ty)?));
    }
    float_lane_cost(LaneOp::Extract, ty, index, false, cost_kind)
}

/// `getScalarizationOverhead` of every lane of a float vector (`ForPoisonSrc`:
/// the inserts start from poison, so lane 0 is free): the lanes' inserts that
/// build it, the extracts that take it apart.
pub(crate) fn float_scalarization_overhead(
    ty: VectorTy,
    insert: bool,
    extract: bool,
    cost_kind: CostKind,
) -> Option<Cost> {
    let mut total = 0;
    for index in 0..ty.count {
        if insert {
            total += float_lane_cost(LaneOp::Insert, ty, index, true, cost_kind)?.value;
        }
        if extract {
            total += lane_extract_cost(ty, index, cost_kind)?.value;
        }
    }
    Some(Cost::of(total))
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
