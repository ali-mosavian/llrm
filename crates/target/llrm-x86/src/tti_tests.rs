//! The cost model against the pinned `opt -passes='print<cost-model>'`: every
//! row of `oracle.txt` (`cpu kind name cost`, made by `oracle.py`).

use llrm_mir::tti::*;

use crate::tti::X86;

const ORACLE: &str = include_str!("tti_oracle.txt");

fn kind_of(word: &str) -> CostKind {
    match word {
        "throughput" => CostKind::RecipThroughput,
        "latency" => CostKind::Latency,
        "code-size" => CostKind::CodeSize,
        "size-latency" => CostKind::SizeAndLatency,
        other => panic!("{other}"),
    }
}

fn ty_of(word: &str) -> Ty {
    match word {
        "float" => Ty::Float,
        "double" => Ty::Double,
        "x86_fp80" => Ty::X87,
        "ptr" => Ty::Pointer,
        int => Ty::Int(int[1..].parse().unwrap_or_else(|_| panic!("{int}"))),
    }
}

fn arith_of(word: &str) -> Option<ArithOp> {
    use ArithOp::*;
    Some(match word {
        "add" => Add,
        "sub" => Sub,
        "mul" => Mul,
        "sdiv" => SDiv,
        "udiv" => UDiv,
        "srem" => SRem,
        "urem" => URem,
        "shl" => Shl,
        "lshr" => LShr,
        "ashr" => AShr,
        "and" => And,
        "or" => Or,
        "xor" => Xor,
        "fadd" => FAdd,
        "fsub" => FSub,
        "fmul" => FMul,
        "fdiv" => FDiv,
        "frem" => FRem,
        "fneg" => FNeg,
        _ => return None,
    })
}

/// The operand variants `oracle.py` names.
fn second_of(
    variant: &str,
    ty: Ty,
) -> OperandInfo {
    let width = if let Ty::Int(bits) = ty { bits } else { 32 };
    match variant {
        "reg" => OperandInfo::ANY,
        "c1" => OperandInfo::constant(1, width),
        "c3" => OperandInfo::constant(3, width),
        "c8" => OperandInfo::constant(8, width),
        "cm8" => OperandInfo::constant(-8, width),
        other => panic!("{other}"),
    }
}

#[test]
fn the_arithmetic_costs_equal_the_pinned_opt_on_every_cpu() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for line in ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let [cpu, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { panic!("{line}") };
        let parts: Vec<&str> = name.split('.').collect();
        let Some(op) = arith_of(parts[0]) else { continue };
        let ty = ty_of(parts[1]);
        let second = second_of(parts[2], ty);
        let want = Cost::of(cost.parse().unwrap());
        let got = X86::of_cpu(cpu).unwrap().arithmetic_cost(op, ty, kind_of(kind), OperandInfo::ANY, second);
        checked += 1;
        if got != want {
            wrong.push(format!("{cpu} {kind} {name}: {} want {}", got.value, want.value));
        }
    }
    assert!(checked > 5000, "{checked} rows");
    assert!(wrong.is_empty(), "{} of {checked} differ:\n{}", wrong.len(), wrong[..wrong.len().min(40)].join("\n"));
}

fn cast_of(word: &str) -> Option<CastKind> {
    use CastKind::*;
    Some(match word {
        "trunc" => Trunc,
        "zext" => ZExt,
        "sext" => SExt,
        "fptrunc" => FPTrunc,
        "fpext" => FPExt,
        "fptoui" => FPToUI,
        "fptosi" => FPToSI,
        "uitofp" => UIToFP,
        "sitofp" => SIToFP,
        "ptrtoint" => PtrToInt,
        "inttoptr" => IntToPtr,
        "bitcast" => BitCast,
        _ => return None,
    })
}

#[test]
fn the_cast_costs_equal_the_pinned_opt_on_every_cpu() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for line in ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let [cpu, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { panic!("{line}") };
        let parts: Vec<&str> = name.split('.').collect();
        let Some(op) = cast_of(parts[0]) else { continue };
        // ptrtoint.i32 / inttoptr.i8: the pointer is the other type.
        let (src, dst) = match op {
            CastKind::PtrToInt => (Ty::Pointer, ty_of(parts[1])),
            CastKind::IntToPtr => (ty_of(parts[1]), Ty::Pointer),
            _ => (ty_of(parts[1]), ty_of(parts[2])),
        };
        let want = Cost::of(cost.parse().unwrap());
        let got = X86::of_cpu(cpu).unwrap().cast_cost(op, dst, src, kind_of(kind));
        checked += 1;
        if got != want {
            wrong.push(format!("{cpu} {kind} {name}: {} want {}", got.value, want.value));
        }
    }
    assert!(checked > 2000, "{checked} rows");
    assert!(wrong.is_empty(), "{} of {checked} differ:\n{}", wrong.len(), wrong[..wrong.len().min(40)].join("\n"));
}

/// The oracle tells the SSE cpu from the x87 one: the same rows read for the
/// wrong profile differ, so a pass above is not the harness comparing nothing.
#[test]
fn the_oracle_tells_the_sse_cpu_from_the_x87_one() {
    let wrong = ORACLE
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter(|line| line.starts_with("core2 "))
        .filter(|line| {
            let [_, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { return false };
            let parts: Vec<&str> = name.split('.').collect();
            let Some(op) = arith_of(parts[0]) else { return false };
            let ty = ty_of(parts[1]);
            let got = X86::of_cpu("i486")
                .unwrap()
                .arithmetic_cost(op, ty, kind_of(kind), OperandInfo::ANY, second_of(parts[2], ty));
            got != Cost::of(cost.parse().unwrap())
        })
        .count();
    assert!(wrong > 5, "{wrong}");
}

#[test]
fn the_compare_select_and_memory_costs_equal_the_pinned_opt_on_every_cpu() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for line in ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let [cpu, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { panic!("{line}") };
        let parts: Vec<&str> = name.split('.').collect();
        let profile = X86::of_cpu(cpu).unwrap();
        let kind = kind_of(kind);
        let got = if parts[0].starts_with("icmp") {
            profile.cmp_sel_cost(CmpSel::ICmp, ty_of(parts[1]), kind)
        } else if parts[0].starts_with("fcmp") {
            profile.cmp_sel_cost(CmpSel::FCmp, ty_of(parts[1]), kind)
        } else if parts[0] == "select" {
            profile.cmp_sel_cost(CmpSel::Select, ty_of(parts[1]), kind)
        } else if parts[0] == "load" {
            profile.memory_op_cost(MemoryOp::Load, ty_of(parts[1]), kind, false)
        } else if parts[0] == "store" && parts[1] != "void" {
            profile.memory_op_cost(MemoryOp::Store, ty_of(parts[1]), kind, false)
        } else {
            continue;
        };
        checked += 1;
        let want = Cost::of(cost.parse().unwrap());
        if got != want {
            wrong.push(format!("{cpu} {name}: {} want {}", got.value, want.value));
        }
    }
    assert!(checked > 1500, "{checked} rows");
    assert!(wrong.is_empty(), "{} of {checked} differ:\n{}", wrong.len(), wrong[..wrong.len().min(40)].join("\n"));
}

/// Constants, from `X86TTIImpl::getIntImmCost` (:6882-7060); the cost model has
/// no printer for them, so these are read off the source.
#[test]
fn an_integer_constant_costs_one_instruction_a_64_bit_chunk() {
    let x86 = X86::of_cpu("i486").unwrap();
    assert_eq!(x86.int_imm_cost(0, 32), Cost::FREE);
    assert_eq!(x86.int_imm_cost(5, 32), Cost::of(1));
    assert_eq!(x86.int_imm_cost(-1, 64), Cost::of(1));
    assert_eq!(x86.int_imm_cost(0x1_0000_0000, 64), Cost::of(2));
    assert_eq!(x86.int_imm_cost(0x1_0000_0000, 128), Cost::of(2));
    // The operand a user takes as an immediate is free when it fits one chunk.
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Add, 1, 5, 32), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Add, 0, 5, 32), Cost::of(1));
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Add, 1, 0x1_0000_0000, 64), Cost::of(2));
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Add, 1, 0x8000_0000, 64), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::And, 1, 0xffff_ffff, 64), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::ICmp, 1, 0xffff_ffff, 64), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Divide, 1, 12345, 32), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Shift, 1, 3, 32), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::GetElementPtr, 0, 0x1000, 32), Cost::of(2));
    assert_eq!(x86.int_imm_cost_inst(ImmUser::GetElementPtr, 1, 0x1000, 32), Cost::FREE);
    assert_eq!(x86.int_imm_cost_inst(ImmUser::Unknown, 0, 0x1000, 32), Cost::FREE);
}

/// `isLegalAddressingMode` (:36621): scales 1, 2, 4, 8 beside a base register,
/// 3, 5, 9 without one; a displacement of 32 bits, below 16 MB beside a symbol.
#[test]
fn the_x86_addressing_modes_are_those_of_the_lowering() {
    let x86 = X86::of_cpu("i486").unwrap();
    for scale in [0, 1, 2, 4, 8] {
        assert!(x86.is_legal_addressing_mode(false, 100, true, scale), "{scale}");
    }
    for scale in [3, 5, 9] {
        assert!(
            x86.is_legal_addressing_mode(false, 0, false, scale)
                && !x86.is_legal_addressing_mode(false, 0, true, scale)
        );
    }
    assert!(!x86.is_legal_addressing_mode(false, 0, false, 6));
    assert!(x86.is_legal_addressing_mode(false, i64::from(i32::MIN), true, 1));
    assert!(!x86.is_legal_addressing_mode(false, i64::from(i32::MAX) + 1, true, 1));
    assert!(x86.is_legal_addressing_mode(true, 16 * 1024 * 1024 - 1, false, 1));
    assert!(!x86.is_legal_addressing_mode(true, 16 * 1024 * 1024, false, 1));
}

#[test]
fn a_trunc_to_a_narrower_integer_is_free_and_a_phi_costs_nothing() {
    let x86 = X86::of_cpu("i486").unwrap();
    assert!(x86.is_truncate_free(Ty::Int(32), Ty::Int(16)) && !x86.is_truncate_free(Ty::Int(16), Ty::Int(32)));
    assert!(!x86.is_truncate_free(Ty::Double, Ty::Float));
    assert_eq!(x86.cf_instr_cost(true, CostKind::CodeSize), Cost::FREE);
    assert_eq!(x86.cf_instr_cost(false, CostKind::CodeSize), Cost::BASIC);
    assert_eq!(x86.cf_instr_cost(false, CostKind::RecipThroughput), Cost::FREE);
}

#[test]
fn an_mir_type_is_the_cost_models_scalar_or_none() {
    let module = llrm_mir::parse::module(
        "define void @f(i32 %a, float %b, x86_fp80 %c, ptr %d, <4 x i32> %e) {\nb:\n  ret void\n}\n",
    )
    .expect("parses");
    let (_, _, function) = module.functions().next().expect("@f");
    let types = &module.context.types;
    let of = |at: usize| Ty::of(types, function.value(function.parameters()[at]).ty);
    assert_eq!(
        [of(0), of(1), of(2), of(3), of(4)],
        [Some(Ty::Int(32)), Some(Ty::Float), Some(Ty::X87), Some(Ty::Pointer), None]
    );
}
