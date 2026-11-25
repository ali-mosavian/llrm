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

/// Constants, from `X86TTIImpl::getIntImmCost` (X86TargetTransformInfo.cpp
/// L6882-L7035); the cost model has no printer for them, so these are
/// UNVERIFIED BY ORACLE and read off the source:
/// - L6883 `if (Val == 0) return TTI::TCC_Free;`, L6886 `if (isInt<32>(Val))
///   return TTI::TCC_Basic;`, then `return 2 * TTI::TCC_Basic;` (the 0, 5 and
///   0x1_0000_0000 rows);
/// - L6921 `Cost += getIntImmCost(Val);` per 64-bit chunk, L6924 `return
///   std::max<InstructionCost>(1, Cost);` (the 128-bit row);
/// - L6949 `if (Idx == 0)` of a GEP: `return 2 * TTI::TCC_Basic`, else free;
/// - L6962 `ImmVal == 0x100000000ULL || ImmVal == 0xffffffff` free (the 64-bit
///   compare), L6979 `Imm.isIntN(32)` free (and), L6990 `Imm.getZExtValue() ==
///   0x80000000` free (add and sub);
/// - L6998 division `return TTI::TCC_Free;`, L7007 shift `if (Idx == 1) return
///   TTI::TCC_Free;`;
/// - L7031 `return (Cost <= NumConstants * TTI::TCC_Basic) ? TCC_Free : Cost`
///   for the operand an instruction takes as an immediate.
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

/// `isLegalAddressingMode` (X86ISelLowering.cpp L36621), UNVERIFIED BY ORACLE
/// and read off the source: L36651-36668 `case 0: case 1: case 2: case 4: case
/// 8: // These scales always work.` / `case 3: case 5: case 9: if
/// (AM.HasBaseReg) return false;` / `default: return false;`; and
/// `X86::isOffsetSuitableForCodeModel` (L3191-3217) `if (!isInt<32>(Offset))
/// return false;` ... L3217 `return Offset < 16 * 1024 * 1024;` beside a
/// symbol.
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

/// Position independent long mode reaches a symbol from `rip`: no displacement
/// and no index beside it, a plain symbol with a base register or one displaced
/// without a symbol fine (`isLegalAddressingMode`).
#[test]
fn a_symbol_in_long_mode_takes_neither_a_displacement_nor_an_index() {
    let x64 = X86::of_cpu("x86-64").unwrap();
    // `@sym + reg` (a scale of 1, or a base register) is one address to the
    // pin.
    assert!(x64.is_legal_addressing_mode(true, 0, false, 0) && x64.is_legal_addressing_mode(true, 0, false, 1));
    assert!(x64.is_legal_addressing_mode(true, 0, true, 0) && !x64.is_legal_addressing_mode(true, 0, true, 2));
    assert!(!x64.is_legal_addressing_mode(true, 8, false, 1) && !x64.is_legal_addressing_mode(true, 0, false, 4));
    assert!(x64.is_legal_addressing_mode(false, 8, true, 4));
    assert_eq!(x64.number_of_gpr_registers(), 16);
    assert_eq!(X86::of_cpu("core2").unwrap().number_of_gpr_registers(), 8);
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

/// The LSR hooks, read off the pin and UNVERIFIED BY ORACLE (a hook has no
/// `opt` printer): `isLegalAddImmediate`/`isLegalICmpImmediate` are
/// `isInt<32>` (X86ISelLowering.cpp L36730-36737); `getScalingFactorCost` is
/// `AM.Scale != 0` for a legal mode, invalid otherwise (X86TTI.cpp L8045);
/// `isLSRCostLess` puts `Insns` first (L7202); `canMacroFuseCmp` is
/// `hasMacroFusion || hasBranchFusion` (L7211), set for `core2` in X86.td and
/// for no 386-class cpu. Without them LSR priced every scaled mode free, put
/// registers before instructions and found no immediate legal.
#[test]
fn the_x86_lsr_hooks_are_those_of_the_pin() {
    let (i486, core2) = (X86::of_cpu("i486").unwrap(), X86::of_cpu("core2").unwrap());
    for imm in [0, -1, i64::from(i32::MIN), i64::from(i32::MAX)] {
        assert!(i486.is_legal_add_immediate(imm) && i486.is_legal_icmp_immediate(imm), "{imm}");
    }
    assert!(!i486.is_legal_add_immediate(i64::from(i32::MAX) + 1));
    assert!(!i486.is_legal_icmp_immediate(i64::from(i32::MIN) - 1));

    assert_eq!(i486.scaling_factor_cost(false, 0, true, 0), Cost::of(0));
    assert_eq!(i486.scaling_factor_cost(false, 0, true, 4), Cost::of(1));
    assert_eq!(i486.scaling_factor_cost(false, 0, true, 3), Cost::INVALID, "3*reg beside a base register");

    let few_insns = LsrCost { insns: 1, num_regs: 9, ..LsrCost::default() };
    let few_regs = LsrCost { insns: 2, num_regs: 1, ..LsrCost::default() };
    assert!(i486.is_lsr_cost_less(&few_insns, &few_regs), "instructions first");
    assert!(!Basic.is_lsr_cost_less(&few_insns, &few_regs), "the generic order puts registers first");

    assert!(!i486.can_macro_fuse_cmp() && core2.can_macro_fuse_cmp());
    assert_eq!(i486.number_of_gpr_registers(), 8);
}

/// A real-mode loop LSR rewrites to something its own model rates worse (the
/// `(i + j) % 3` of bench matmul at -Os: +13 B, +355 executed instructions)
/// stayed rewritten because the pin's X86 never drops such a solution.
#[test]
fn real_mode_drops_an_lsr_solution_the_model_rates_worse_and_flat_code_keeps_it() {
    let real = crate::tti::RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    assert!(X86::of_cpu("i486").unwrap().in_real_mode(real).should_drop_lsr_solution_if_less_profitable());
    assert!(!X86::of_cpu("i486").unwrap().should_drop_lsr_solution_if_less_profitable());
}

/// rcflip's RAMP loop kept the `2 * x` address and a `lea` (4 instructions
/// per trip) where the other order of `Solve` finds 2: real mode searches both
/// orders, flat code keeps the pin's one (DEVIATE LU14).
#[test]
fn real_mode_solves_lsr_in_both_use_orders_and_flat_code_in_one() {
    let real = crate::tti::RealMode {
        scale_costs: vec![(1, 0), (2, 2), (4, 2), (8, 2)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    assert!(X86::of_cpu("i486").unwrap().in_real_mode(real.clone()).lsr_solves_both_use_orders());
    assert!(!X86::of_cpu("i486").unwrap().lsr_solves_both_use_orders());
    // CIV1: the narrowest integer phi, in real mode only
    assert!(X86::of_cpu("i486").unwrap().in_real_mode(real).congruent_ivs_truncate_to_narrowest_integer());
    assert!(!X86::of_cpu("i486").unwrap().congruent_ivs_truncate_to_narrowest_integer());
}

#[test]
fn a_triple_picks_the_baseline_cpu_of_its_architecture() {
    assert!(X86::of_triple("x86_64-unknown-linux-gnu").unwrap().native() == 64);
    assert!(X86::of_triple("i686-pc-linux-gnu").unwrap().native() == 32);
    assert!(X86::of_triple("aarch64-unknown-linux-gnu").is_none());
}

/// `X86TTIImpl::hasDivRemOp` is `isOperationLegal(SDIVREM)`: every register
/// type has one. It was stated on the lowering, so a pass that asked the `Tti`
/// (DivRemPairs) found none until it consulted both (#6152's DEVIATE).
#[test]
fn the_tti_states_a_divide_remainder_op_on_every_register_type() {
    let tti = X86::of_cpu("i486").unwrap();
    for bits in [8, 16, 32] {
        assert!(tti.has_div_rem_op(bits, true) && tti.has_div_rem_op(bits, false), "i{bits}");
    }
    assert!(tti.has_div_rem_op(64, true), "DR1: the inline i64 routine returns both");
    assert!(!tti.has_div_rem_op(24, false));
}

/// DIVREM-HELPER is the flat 32-bit mode's alone: 16-bit code states no `i64`
/// divide-remainder op (the pin's x86 has none), so `DivRemPairs` writes its
/// remainders as `x - x / y * y` as before; long mode already had one (a
/// register type).
#[test]
fn only_flat_32_bit_code_has_an_i64_divide_remainder_op() {
    let flat = X86::of_cpu("i486").unwrap();
    let real = crate::tti::RealMode {
        scale_costs: vec![(1, 0)],
        registers: 6,
        scaled_index_insns: 2,
        huge_space: None,
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    assert!(flat.has_div_rem_op(64, false) && flat.has_div_rem_op(64, true));
    assert!(!flat.in_real_mode(real).has_div_rem_op(64, false), "16-bit code");
    assert!(X86::of_cpu("x86-64").unwrap().has_div_rem_op(64, false), "long mode: i64 is a register type, as before");
}

/// The loop unroll counts float values against the x87 stack only where the
/// subtarget keeps floats there: `-march=i486` has the 8 slots the tuning
/// states, `core2` (SSE2 floating point) none.
#[test]
fn test_the_x87_stack_slots_reach_the_unroll_query_only_without_sse2() {
    let tuning = llrm_mir::target::Tuning { x87_stack_slots: Some(8), ..llrm_mir::target::Tuning::PIN };
    let slots = |cpu: &str| X86::of_cpu(cpu).expect("a cpu").with_tuning(tuning.clone()).x87_stack_slots();
    assert_eq!((slots("i486"), slots("pentium"), slots("core2")), (Some(8), Some(8), None));
}

fn unroll_defaults() -> UnrollingPreferences {
    UnrollingPreferences {
        threshold: 150,
        max_percent_threshold_boost: 400,
        opt_size_threshold: 0,
        partial_threshold: 150,
        partial_opt_size_threshold: 150,
        default_unroll_runtime_count: 8,
        max_count: u32::MAX,
        max_upper_bound: 8,
        full_unroll_max_count: u32::MAX,
        be_insns: 2,
        partial: false,
        runtime: false,
        allow_remainder: true,
        unroll_remainder: false,
        allow_expensive_trip_count: false,
        force: false,
        upper_bound: false,
        max_iterations_count_to_analyze: 10,
        unroll_and_jam: false,
        unroll_and_jam_inner_loop_threshold: 60,
    }
}

/// `x86-64` and `core2` run `SandyBridgeModel` (LoopMicroOpBufferSize 28), so
/// the base `getUnrollingPreferences` enables partial and runtime unrolling
/// there, up to 28 micro-ops and not past a call. Without it x86-64 reported no
/// preference and the partial/runtime unroll never ran (0 diffs in the A/B).
/// The `GenericModel` cpus (`i486` and the rest) have no loop buffer: none.
#[test]
fn test_partial_and_runtime_unrolling_follow_the_cpu_models_loop_buffer() {
    for cpu in ["x86-64", "core2"] {
        let tti = X86::of_cpu(cpu).unwrap();
        let mut up = unroll_defaults();
        tti.get_unrolling_preferences(false, &mut up);
        assert!(up.partial && up.runtime && up.upper_bound, "{cpu}");
        assert_eq!((up.partial_threshold, up.opt_size_threshold, up.partial_opt_size_threshold), (28, 0, 0), "{cpu}");
        let mut calls = unroll_defaults();
        tti.get_unrolling_preferences(true, &mut calls);
        assert!(!calls.partial && !calls.runtime && !calls.upper_bound, "{cpu} with a call");
    }
    for cpu in ["i386", "i486", "pentium", "pentiumpro", "k5", "k6", "athlon"] {
        let mut up = unroll_defaults();
        X86::of_cpu(cpu).unwrap().get_unrolling_preferences(false, &mut up);
        assert!(!up.partial && !up.runtime && !up.upper_bound, "{cpu}");
    }
}

/// crc -O1 m64: ConstantHoisting rebased the mask 0xedb88320 against 0xffffffff
/// (`lea r, [rcx-0x12477cdf]`) because the pin prices a 64-bit constant of 32
/// leading zeros at 2 (`isInt<32>` only) though `mov r32, imm32` zero-extends:
/// 147 B became 162 (gcc 118). With zext_imm_is_basic it costs one.
#[test]
fn a_zero_extended_32_bit_constant_costs_one_instruction_where_the_target_says_so() {
    let mut tuning = llrm_mir::target::Tuning::PIN;
    let pin = X86::of_cpu("i486").unwrap();
    assert_eq!(pin.int_imm_cost(0xedb8_8320, 64), Cost::of(2));
    tuning.zext_imm_is_basic = true;
    let zimm = X86::of_cpu("i486").unwrap().with_tuning(tuning);
    assert_eq!(zimm.int_imm_cost(0xedb8_8320, 64), Cost::of(1));
    assert_eq!(zimm.int_imm_cost(0x1_0000_0000, 64), Cost::of(2));
    assert_eq!(zimm.int_imm_cost(0xedb8_8320, 32), Cost::of(1));
}

/// ZIMM priced a constant hoisted at `-Os` as one instruction, so
/// ConstantHoisting kept 21 copies of `mov eax, 0x9E3779B9` (5 B each) where
/// one register held it (3 B copies): `mul_constants` -Os +30 B.
#[test]
fn a_function_built_for_size_keeps_the_pins_cost_for_a_zero_extended_constant() {
    let mut tuning = llrm_mir::target::Tuning::PIN;
    tuning.zext_imm_is_basic = true;
    let zimm = X86::of_cpu("i486").unwrap().with_tuning(tuning);
    let cost = |for_size| zimm.int_imm_cost_inst_for_size(ImmUser::Other, 0, 0x9e37_79b9, 64, for_size);
    assert_eq!(cost(false), Cost::of(1));
    assert_eq!(cost(true), Cost::of(2));
}

/// The SLP vectorizer ran on x86 with default (unpriced) vector costs and
/// changed four to six programs of the identity corpus at O2 and O3
/// (`bitcount_u64.c`, `double_across_a_call_result.c`). It runs on x86-64 now,
/// where the SSE2 vector memory costs are priced (fill_nest's 64 constant
/// stores became eight vector stores) and a tree with an operation the
/// target does not price is not formed; the targets without vector values keep
/// their scalar code.
#[test]
fn slp_runs_on_x86_64_alone() {
    assert!(X86::of_cpu("x86-64").expect("a cpu").slp_priced());
    for cpu in ["i486", "pentium4"] {
        if let Some(tti) = X86::of_cpu(cpu) {
            assert!(!tti.slp_priced(), "{cpu}");
        }
    }
}

const VECTOR_ORACLE: &str = include_str!("tti_oracle_vector.txt");

fn vector_ty_of(word: &str) -> VectorTy {
    let (count, bits) = word[1..].split_once('i').expect(word);
    VectorTy { lane: Ty::Int(bits.parse().expect(word)), count: count.parse().expect(word) }
}

/// What the cost model says of each vector row of `tti_oracle_vector.txt`: `Ok`
/// where it prices it as the pin, `Err(ours)` where it prices it differently
/// (`None`: not priced).
fn vector_rows() -> Vec<(String, Result<(), Option<i64>>)> {
    let x86 = X86::of_cpu("x86-64").unwrap();
    let mut rows = Vec::new();
    for line in VECTOR_ORACLE.lines().filter(|line| !line.starts_with('#')) {
        let [_cpu, kind, name, cost] = line.split(' ').collect::<Vec<_>>()[..] else { panic!("{line}") };
        let parts: Vec<&str> = name.split('.').collect();
        let want = cost.parse::<i64>().unwrap();
        let got = if let Some(op) = arith_of(parts[0]) {
            x86.vector_arithmetic_cost(op, vector_ty_of(parts[1]), kind_of(kind))
        } else {
            x86.vector_cast_cost(
                cast_of(parts[0]).unwrap(),
                vector_ty_of(parts[2]),
                vector_ty_of(parts[1]),
                kind_of(kind),
            )
        };
        rows.push((
            format!("{kind} {name}"),
            if got == Some(Cost::of(want)) { Ok(()) } else { Err(got.map(|c| c.value)) },
        ));
    }
    rows
}

/// Every integer vector row of `tti_oracle_vector.txt`. `mul <N x i64>` was
/// unpriced, so a loop of them (tile's init) stayed scalar: clocks +153%.
/// Before: `mul <N x i8>` within 64 bits took the 128-bit row (6, not 4), and a
/// wide add or sub cost its registers in the latency and size kinds, the pin's
/// base model one.
#[test]
fn the_vector_costs_equal_the_pinned_opt_on_x86_64() {
    let rows = vector_rows();
    let wrong: Vec<_> =
        rows.iter().filter_map(|(name, row)| row.as_ref().err().map(|got| format!("{name}: {got:?}"))).collect();
    assert!(rows.len() > 600, "{} rows", rows.len());
    assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), rows.len(), wrong[..wrong.len().min(40)].join("\n"));
}

/// A shift by an amount that is no constant was priced as the immediate form
/// (1 for `<4 x i32>`, nothing for `<16 x i8>`): a loop of them was widened
/// into the ladder it lowers to. The pin's `SSE2CostTable` rows, and the
/// `SSE2UniformConstCostTable` bytes.
#[test]
fn a_variable_vector_shift_is_priced_as_the_pins_sse2_rows() {
    let x86 = X86::of_cpu("x86-64").unwrap();
    let cost = |op, lane, count, second: OperandInfo| {
        x86.vector_arithmetic_cost_op2(op, VectorTy { lane: Ty::Int(lane), count }, second, CostKind::RecipThroughput)
            .map(|cost| cost.value)
    };
    let variable = OperandInfo::ANY;
    let constant = OperandInfo::constant(3, 8);
    assert_eq!(cost(ArithOp::Shl, 16, 8, variable), Some(24));
    assert_eq!(cost(ArithOp::LShr, 32, 4, variable), Some(12));
    assert_eq!(cost(ArithOp::AShr, 8, 16, variable), Some(27));
    assert_eq!(cost(ArithOp::AShr, 64, 2, variable), Some(8));
    assert_eq!(cost(ArithOp::Shl, 32, 8, variable), Some(34), "two registers");
    assert_eq!(cost(ArithOp::Shl, 32, 4, constant), Some(1));
    assert_eq!(cost(ArithOp::Shl, 8, 16, constant), Some(1));
    assert_eq!(cost(ArithOp::AShr, 8, 16, constant), Some(3));
}

/// `getMemoryInstructionCost` at VF 1 adds `getAddressComputationCost` of the
/// value type, which x86 leaves at the base model's 0. It was 1: dot's scalar
/// loop cost 9 (the pin's 7), so a VF 8 loop at 60 was taken and ran twice
/// as many instructions.
#[test]
fn a_scalar_access_adds_no_address_cost_on_x86_64() {
    let x86 = X86::of_cpu("x86-64").unwrap();
    assert_eq!(x86.scalar_access_address_cost(CostKind::RecipThroughput), Cost::FREE);
}

/// `(long)a[i] * b[i]` of `i16` values priced as `<8 x i64>` ladders (28 for
/// the multiply, 24 for the two extensions) kept dot's loop scalar, 39% behind
/// gcc. Lowered by `pmullw`/`pmulhw` it is 4 plus the extension of
/// the products, and the operand extensions leave no instruction.
#[test]
fn a_multiply_of_two_shorts_widened_to_i64_is_priced_as_pmullw_and_pmulhw() {
    let x86 = X86::of_cpu("x86-64").unwrap();
    let wide = VectorTy { lane: Ty::Int(64), count: 8 };
    let shorts = VectorTy { lane: Ty::Int(16), count: 8 };
    let short = Some(ExtOperand { kind: CastKind::SExt, src: Ty::Int(16) });
    let multiply = x86.vector_arithmetic_cost_ext(ArithOp::Mul, wide, [short, short], CostKind::RecipThroughput);
    let products = VectorTy { lane: Ty::Int(32), count: 8 };
    let extension = x86.vector_cast_cost(CastKind::SExt, wide, products, CostKind::RecipThroughput);
    assert_eq!(multiply, Ok(extension.map(|extend| Cost::of(4 + extend.value))));
    let user = ExtUser { op: Some(ArithOp::Mul), operands: [short, short], index: 0 };
    assert_eq!(x86.vector_ext_cost(CastKind::SExt, wide, shorts, &[user], CostKind::RecipThroughput), Some(Cost::FREE));
    // One operand not extended: the multiply is the pin's `<8 x i64>` one, not
    // the widening.
    let other = x86.vector_arithmetic_cost_ext(ArithOp::Mul, wide, [short, None], CostKind::RecipThroughput);
    assert_ne!(other, multiply);
}

#[test]
fn real_mode_keeps_the_pins_copy_tail_and_flat_code_overlaps_it() {
    let real = crate::tti::RealMode {
        scale_costs: vec![(1, 0)],
        registers: 6,
        scaled_index_insns: 0,
        huge_space: None,
        address_registers: None,
        inline_size_guard: false,
        float_code_size_percent: 100,
        unroll_size_few_trips: (0, 0),
        recursive_inlining: None,
        priced_self_inlining: None,
    };
    assert!(!X86::of_cpu("i486").unwrap().in_real_mode(real).overlapping_copy_tail());
    assert!(X86::of_cpu("i486").unwrap().overlapping_copy_tail());
}
