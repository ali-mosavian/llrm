//! The AArch64 cost model against the pinned `opt -passes='print<cost-model>'`:
//! every row of `tti_oracle.txt` and `tti_intrinsics.txt` (`cpu kind name
//! cost`, made by `tti_oracle.py`).

use llrm_mir::lowering::Lowering;
use llrm_mir::tti::*;

use crate::tti::{AArch64, AArch64Lowering};

const ORACLE: &str = include_str!("tti_oracle.txt");
const INTRINSICS: &str = include_str!("tti_intrinsics.txt");

type Row<'a> = [&'a str; 4];

fn rows(table: &str) -> impl Iterator<Item = Row<'_>> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(
            |line| {
                let Ok(row) = line.split(' ').collect::<Vec<_>>().try_into() else { panic!("{line}") };
                row
            },
        )
}

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
        "ptr" => Ty::Pointer,
        int => Ty::Int(int[1..].parse().unwrap_or_else(|_| panic!("{int}"))),
    }
}

fn cost_of(word: &str) -> Cost {
    word.parse().map_or(Cost::INVALID, Cost::of)
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

/// The operand variants `tti_oracle.py` names.
fn second_of(
    variant: &str,
    ty: Ty,
) -> OperandInfo {
    let width = if let Ty::Int(bits) = ty { bits } else { 32 };
    match variant {
        "reg" => OperandInfo::ANY,
        "c1" => OperandInfo::constant(1, width),
        "c3" => OperandInfo::constant(3, width),
        "c7" => OperandInfo::constant(7, width),
        "c8" => OperandInfo::constant(8, width),
        "cm8" => OperandInfo::constant(-8, width),
        other => panic!("{other}"),
    }
}

fn check(
    what: &str,
    checked: usize,
    least: usize,
    wrong: Vec<String>,
) {
    assert!(checked > least, "{what}: {checked} rows");
    assert!(
        wrong.is_empty(),
        "{what}: {} of {checked} differ:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(40)].join("\n")
    );
}

fn arithmetic_wrong(
    cpu: &str,
    kind: &str,
    name: &str,
    cost: &str,
) -> Option<Option<String>> {
    let parts: Vec<&str> = name.split('.').collect();
    let op = arith_of(parts[0])?;
    let ty = ty_of(parts[1]);
    let got =
        AArch64::of_cpu(cpu).unwrap().arithmetic_cost(op, ty, kind_of(kind), OperandInfo::ANY, second_of(parts[2], ty));
    Some((got != cost_of(cost)).then(|| format!("{cpu} {kind} {name}: {} want {cost}", got.value)))
}

#[test]
fn the_arithmetic_costs_equal_the_pinned_opt_on_both_cpus() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for [cpu, kind, name, cost] in rows(ORACLE) {
        if let Some(verdict) = arithmetic_wrong(cpu, kind, name, cost) {
            checked += 1;
            wrong.extend(verdict);
        }
    }
    check("arithmetic", checked, 900, wrong);
}

#[test]
fn the_cast_costs_equal_the_pinned_opt_on_both_cpus() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for [cpu, kind, name, cost] in rows(ORACLE) {
        let parts: Vec<&str> = name.split('.').collect();
        let Some(op) = cast_of(parts[0]) else { continue };
        // ptrtoint.i32 / inttoptr.i8: the pointer is the other type.
        let (src, dst) = match op {
            CastKind::PtrToInt => (Ty::Pointer, ty_of(parts[1])),
            CastKind::IntToPtr => (ty_of(parts[1]), Ty::Pointer),
            _ => (ty_of(parts[1]), ty_of(parts[2])),
        };
        let got = AArch64::of_cpu(cpu).unwrap().cast_cost(op, dst, src, kind_of(kind));
        checked += 1;
        if got != cost_of(cost) {
            wrong.push(format!("{cpu} {kind} {name}: {} want {cost}", got.value));
        }
    }
    check("casts", checked, 600, wrong);
}

#[test]
fn the_compare_select_and_memory_costs_equal_the_pinned_opt_on_both_cpus() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for [cpu, kind, name, cost] in rows(ORACLE) {
        let parts: Vec<&str> = name.split('.').collect();
        let profile = AArch64::of_cpu(cpu).unwrap();
        let kind = kind_of(kind);
        let got = match parts[0] {
            p if p.starts_with("icmp") => profile.cmp_sel_cost(CmpSel::ICmp, ty_of(parts[1]), kind),
            p if p.starts_with("fcmp") => profile.cmp_sel_cost(CmpSel::FCmp, ty_of(parts[1]), kind),
            "select" => profile.cmp_sel_cost(CmpSel::Select, ty_of(parts[1]), kind),
            "load" => profile.memory_op_cost(MemoryOp::Load, ty_of(parts[1]), kind, false),
            "store" => profile.memory_op_cost(MemoryOp::Store, ty_of(parts[1]), kind, false),
            _ => continue,
        };
        checked += 1;
        if got != cost_of(cost) {
            wrong.push(format!("{cpu} {name}: {} want {cost}", got.value));
        }
    }
    check("compare, select, memory", checked, 400, wrong);
}

#[test]
fn the_intrinsic_costs_equal_the_pinned_opt_on_both_cpus() {
    let mut wrong = Vec::new();
    let mut checked = 0;
    for [cpu, kind, name, cost] in rows(INTRINSICS) {
        let got = AArch64::of_cpu(cpu).unwrap().intrinsic_cost(name, None, &[], kind_of(kind));
        checked += 1;
        if got != cost_of(cost) {
            wrong.push(format!("{cpu} {kind} {name}: {} want {cost}", got.value));
        }
    }
    check("intrinsics", checked, 1000, wrong);
}

/// The table is a measurement: a column of zeros, or a header naming another
/// opt, means the instrument is broken.
#[test]
fn the_oracle_tables_come_from_the_pinned_opt_and_are_not_flat() {
    for table in [ORACLE, INTRINSICS] {
        let header = table.lines().next().unwrap();
        assert!(header.contains("24.0.0git d1106deb71cc"), "{header}");
        assert!(rows(table).count() > 1000);
        for cpu in ["generic", "apple-m1"] {
            for kind in ["throughput", "latency", "code-size", "size-latency"] {
                let costs: Vec<&str> = rows(table).filter(|r| r[0] == cpu && r[1] == kind).map(|r| r[3]).collect();
                assert!(costs.len() > 100, "{cpu} {kind}");
                let nonzero = costs.iter().filter(|c| **c != "0").count();
                assert!(nonzero * 2 > costs.len(), "{cpu} {kind}: {nonzero} of {} nonzero", costs.len());
            }
        }
    }
    // Reading the same rows for the other cpu must differ somewhere (the load
    // latency of a wide integer).
    let load = |cpu: &str| {
        rows(ORACLE).find(|r| r[0] == cpu && r[1] == "latency" && r[2] == "load.i256").map(|r| r[3]).unwrap()
    };
    assert_ne!(load("generic"), load("apple-m1"));
}

/// The table's opt is the pinned one when it is installed (`LLVM_BIN`, default
/// `~/scratch/llvm-build-aarch64/bin`); skipped with a message otherwise.
#[test]
fn the_installed_opt_is_the_pinned_one_and_reproduces_a_row() {
    let bin = std::env::var("LLVM_BIN")
        .unwrap_or_else(|_| format!("{}/scratch/llvm-build-aarch64/bin", std::env::var("HOME").unwrap_or_default()));
    let opt = format!("{bin}/opt");
    let Ok(version) = std::process::Command::new(&opt).arg("--version").output() else {
        eprintln!("skipped: no opt at {opt}");
        return;
    };
    let text = String::from_utf8_lossy(&version.stdout);
    assert!(text.contains("24.0.0git"), "{text}");
    let dir = std::env::temp_dir().join(format!("tti_oracle_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ll = dir.join("one.ll");
    std::fs::write(
        &ll,
        "define void @f(i32 %a, ptr %p) {\n  %r = sdiv i32 %a, 8\n  store i32 %r, ptr %p\n  ret void\n}\n",
    )
    .unwrap();
    let out = std::process::Command::new(&opt)
        .args(["-mtriple=aarch64-unknown-linux-gnu", "-mcpu=apple-m1", "-passes=print<cost-model>", "-disable-output"])
        .arg("-cost-kind=throughput")
        .arg(&ll)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let want = ORACLE.lines().find(|l| l.starts_with("apple-m1 throughput sdiv.i32.c8 ")).unwrap();
    let cost = want.rsplit(' ').next().unwrap();
    assert!(stderr.contains(&format!("estimated cost of {cost} for instruction:   %r = sdiv")), "{stderr}");
}

/// Constants, from `AArch64TTIImpl::getIntImmCost`
/// (AArch64TargetTransformInfo.cpp L390-L428); the cost model has no
/// printer for them, so these are read off the source:
/// - L393 a zero or a logical immediate (`0xff00ff00ff00ff00`) costs nothing,
///   per 64-bit chunk;
/// - L397 a negative value is complemented first, then costed by `expandMOVImm`
///   (one `movn` for `-2`);
/// - L426 `max(1, Cost)` for the whole constant (the 0 row, and a 128-bit one
///   of two chunks);
/// - L431.. an operand an instruction takes as an immediate is free when it
///   fits one chunk per 64 bits.
#[test]
fn an_integer_constant_costs_the_moves_that_build_it() {
    let a = AArch64::of_cpu("generic").unwrap();
    assert_eq!(a.int_imm_cost(0, 32), Cost::of(1));
    assert_eq!(a.int_imm_cost(0xff00_ff00_ff00_ff00_u64 as i64 as i128, 64), Cost::of(1));
    assert_eq!(a.int_imm_cost(-2, 64), Cost::of(1));
    assert_eq!(a.int_imm_cost(0x1234, 32), Cost::of(1));
    assert_eq!(a.int_imm_cost(0x1234_5678, 32), Cost::of(2));
    assert_eq!(a.int_imm_cost(0x1234_5678_9abc_def0, 64), Cost::of(4));
    assert_eq!(a.int_imm_cost(0x1234_5678_9abc_def0, 128), Cost::of(4));
    assert_eq!(a.int_imm_cost_inst(ImmUser::Add, 1, 0x1234_5678, 32), Cost::of(2));
    assert_eq!(a.int_imm_cost_inst(ImmUser::Add, 1, 0x1234, 32), Cost::FREE);
    assert_eq!(a.int_imm_cost_inst(ImmUser::Add, 0, 0x1234_5678, 32), Cost::of(2));
    assert_eq!(a.int_imm_cost_inst(ImmUser::Shift, 1, 0x1234_5678, 32), Cost::FREE);
    assert_eq!(a.int_imm_cost_inst(ImmUser::GetElementPtr, 0, 8, 64), Cost::of(2));
    assert_eq!(a.int_imm_cost_inst(ImmUser::GetElementPtr, 1, 8, 64), Cost::FREE);
    assert_eq!(a.int_imm_cost_inst(ImmUser::Unknown, 0, 0x1234_5678, 32), Cost::FREE);
    assert_eq!(a.int_imm_cost_inst(ImmUser::Store, 0, 0x1234, 32), Cost::FREE);
}

/// `isLegalAddressingMode` (AArch64ISelLowering.cpp L20254) with
/// `AArch64InstrInfo::isLegalAddressingMode` (AArch64InstrInfo.cpp L11809):
/// UNVERIFIED BY ORACLE, read off the source. The trait gives no access type,
/// so the size is the 8 bytes of a register (DEVIATE 3).
#[test]
fn the_addressing_modes_are_those_of_the_lowering() {
    let a = AArch64::of_cpu("generic").unwrap();
    assert!(!a.is_legal_addressing_mode(true, 0, true, 0), "no global is ever a base");
    assert!(!a.is_legal_addressing_mode(false, 8, true, 8), "no reg+reg+imm");
    assert!(a.is_legal_addressing_mode(false, 0, true, 0));
    assert!(a.is_legal_addressing_mode(false, -256, true, 0) && !a.is_legal_addressing_mode(false, -257, true, 0));
    assert!(a.is_legal_addressing_mode(false, 255, true, 0) && a.is_legal_addressing_mode(false, 4095 * 8, true, 0));
    assert!(
        !a.is_legal_addressing_mode(false, 4095 * 8 + 8, true, 0) && !a.is_legal_addressing_mode(false, 260, true, 0)
    );
    assert!(a.is_legal_addressing_mode(false, 0, true, 1) && a.is_legal_addressing_mode(false, 0, true, 8));
    assert!(!a.is_legal_addressing_mode(false, 0, true, 4));
    // `1*reg` and `2*reg` without a base register become `base` and `base +
    // reg`.
    assert!(a.is_legal_addressing_mode(false, 0, false, 1) && a.is_legal_addressing_mode(false, 0, false, 2));
    assert!(!a.is_legal_addressing_mode(false, 0, false, 4) && !a.is_legal_addressing_mode(false, 0, false, 0));
}

/// The LSR hooks, UNVERIFIED BY ORACLE (a hook has no `opt` printer):
/// `isLegalAddImmediate` and `isLegalICmpImmediate` take a 12-bit immediate,
/// shifted by 12 or not, either sign (AArch64ISelLowering.cpp L20177,
/// L20248); `getScalingFactorCost` is 1 for a scale other than 0 and 1
/// (AArch64TargetTransformInfo.cpp L7420); `isLSRCostLess` puts `NumRegs`
/// first, then `Insns`, `NumBaseAdds` (L7461); 31 general registers.
#[test]
fn the_lsr_hooks_are_those_of_the_pin() {
    let a = AArch64::of_cpu("generic").unwrap();
    for imm in [0, 1, 4095, -4095, 0x1000, 0xfff000, -0xfff000] {
        assert!(a.is_legal_add_immediate(imm) && a.is_legal_icmp_immediate(imm), "{imm}");
    }
    for imm in [4096 + 1, 0x1000_000, 0x1001_000, i64::MIN] {
        assert!(!a.is_legal_add_immediate(imm) && !a.is_legal_icmp_immediate(imm), "{imm}");
    }
    assert_eq!(a.scaling_factor_cost(false, 0, true, 0), Cost::of(0));
    assert_eq!(a.scaling_factor_cost(false, 0, true, 1), Cost::of(0));
    assert_eq!(a.scaling_factor_cost(false, 0, true, 8), Cost::of(1));
    assert_eq!(a.scaling_factor_cost(false, 0, true, 3), Cost::INVALID);
    let few_regs = LsrCost { insns: 9, num_regs: 1, ..LsrCost::default() };
    let few_insns = LsrCost { insns: 1, num_regs: 2, ..LsrCost::default() };
    assert!(a.is_lsr_cost_less(&few_regs, &few_insns), "registers first");
    let base_adds = LsrCost { num_base_adds: 1, add_rec_cost: 0, ..LsrCost::default() };
    let rec = LsrCost { num_base_adds: 0, add_rec_cost: 1, ..LsrCost::default() };
    assert!(
        a.is_lsr_cost_less(&rec, &base_adds) && !a.is_lsr_cost_less(&base_adds, &rec),
        "base adds before AddRecCost"
    );
    assert_eq!(a.number_of_gpr_registers(), 31);
}

#[test]
fn a_trunc_to_a_narrower_integer_is_free_and_a_phi_costs_nothing() {
    let a = AArch64::of_cpu("apple-m1").unwrap();
    assert!(a.is_truncate_free(Ty::Int(64), Ty::Int(32)) && !a.is_truncate_free(Ty::Int(16), Ty::Int(32)));
    assert!(!a.is_truncate_free(Ty::Double, Ty::Float));
    assert_eq!(a.cf_instr_cost(true, CostKind::CodeSize), Cost::FREE);
    assert_eq!(a.cf_instr_cost(false, CostKind::CodeSize), Cost::BASIC);
    assert_eq!(a.cf_instr_cost(false, CostKind::RecipThroughput), Cost::FREE);
    assert_eq!(a.cf_instr_cost(true, CostKind::RecipThroughput), Cost::FREE);
}

#[test]
fn pre_and_post_indexed_accesses_are_legal_for_the_register_types_only() {
    let a = AArch64::of_cpu("generic").unwrap();
    for mode in [MemIndexedMode::PreInc, MemIndexedMode::PreDec, MemIndexedMode::PostInc, MemIndexedMode::PostDec] {
        for ty in [Ty::Int(8), Ty::Int(16), Ty::Int(32), Ty::Int(64), Ty::Float, Ty::Double, Ty::Pointer] {
            assert!(a.is_indexed_access_legal(mode, ty), "{mode:?} {ty:?}");
        }
        for ty in [Ty::Int(1), Ty::Int(128), Ty::X87] {
            assert!(!a.is_indexed_access_legal(mode, ty), "{mode:?} {ty:?}");
        }
    }
}

/// `getUnrollingPreferences` (AArch64TargetTransformInfo.cpp L6225) as far as
/// the trait reaches: neither cpu's scheduling model has a loop buffer, so only
/// `UpperBound` and a zero `PartialOptSizeThreshold` are set.
#[test]
fn unrolling_sets_the_upper_bound_and_no_partial_unroll_at_os() {
    let defaults = UnrollingPreferences {
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
    };
    for cpu in ["generic", "apple-m1"] {
        let mut up = defaults.clone();
        AArch64::of_cpu(cpu).unwrap().get_unrolling_preferences(false, &mut up);
        assert!(up.upper_bound && !up.partial && !up.runtime && up.partial_opt_size_threshold == 0, "{cpu}");
    }
}

/// `isMaskAndCmp0FoldingBeneficial` (AArch64ISelLowering.cpp L33646) is true
/// for a single-bit mask; the trait carries no mask, so the unconditional
/// answer is the base class's (DEVIATE 4), and the mask form is checked.
#[test]
fn only_a_single_bit_mask_folds_into_a_test_bit_branch() {
    assert!(!AArch64Lowering.is_mask_and_cmp0_folding_beneficial());
    assert!(AArch64Lowering::is_mask_and_cmp0_folding_beneficial_for(Some(0x100)));
    assert!(!AArch64Lowering::is_mask_and_cmp0_folding_beneficial_for(Some(0x300)));
    assert!(!AArch64Lowering::is_mask_and_cmp0_folding_beneficial_for(None));
}

/// The machine the mid-end asks returns the model, with the cpu its platform
/// names.
#[test]
fn the_machine_returns_its_platforms_cost_model() {
    use llrm_mir::target::Machine;

    use crate::machine::{Arm64Machine, Platform};

    for platform in [Platform::Darwin, Platform::Linux] {
        let machine = Arm64Machine { platform };
        let tti = machine.tti().expect("a cost model");
        assert_eq!(tti.name(), "AArch64");
        assert!(machine.lowering().is_some());
    }
    // A sdiv by 8 is 4 on both; the load latency of an i256 tells the cpus
    // apart.
    let latency = |platform| {
        Arm64Machine { platform }.tti().unwrap().memory_op_cost(MemoryOp::Load, Ty::Int(256), CostKind::Latency, false)
    };
    assert_eq!((latency(Platform::Darwin), latency(Platform::Linux)), (Cost::of(5), Cost::of(7)));
}

/// DEVIATE LFTR1: following the pin's step-one rule left byte-loops of
/// stride 2..8 unreduced and cost bytes; the knob must stay on here.
#[test]
fn lftr_counts_any_constant_stride() {
    use llrm_mir::tti::Tti;
    assert!(AArch64::of_cpu("generic").unwrap().lftr_any_stride());
}

/// AArch64 has no `isSExtCheaperThanZExt` override at the pin (the base class
/// says no, TargetLowering.h#L3329); the hook said yes for 32 -> 64 and cited a
/// pin line that does not exist.
#[test]
fn sign_extension_is_not_cheaper_than_zero_extension() {
    for (from, to) in [(8, 32), (16, 32), (32, 64), (8, 64)] {
        assert!(!AArch64Lowering.is_sext_cheaper_than_zext(from, to), "{from} -> {to}");
    }
}

/// The pin expands a sign-extending load from i1 on AArch64
/// (AArch64ISelLowering.cpp#L1141); the hook said legal, as the base class
/// does.
#[test]
fn a_sign_extending_load_of_a_bit_is_not_legal() {
    use llrm_mir::lowering::LoadExt;
    let arm64 = AArch64Lowering;
    assert!(!arm64.is_load_ext_legal(LoadExt::Sign, 32, 1) && !arm64.is_load_ext_legal(LoadExt::Sign, 64, 1));
    assert!(arm64.is_load_ext_legal(LoadExt::Zero, 32, 1) && arm64.is_load_ext_legal(LoadExt::Any, 32, 1));
    assert!(arm64.is_load_ext_legal(LoadExt::Sign, 32, 8) && arm64.is_load_ext_legal(LoadExt::Sign, 64, 32));
    assert!(!arm64.is_load_ext_legal(LoadExt::Zero, 32, 24));
}

/// The IR-level ext promotion asks `isOperationLegalOrCustom` of the promoted
/// operation; the hook said no to everything (the base class has no registers),
/// so no promotion could pass its legality check.
#[test]
fn arithmetic_is_legal_on_the_registers_and_the_remainders_are_expanded() {
    use llrm_mir::lowering::Isd::*;
    let arm64 = AArch64Lowering;
    for bits in [32, 64] {
        for op in
            [Add, Sub, Mul, UDiv, SDiv, Shl, Srl, Sra, And, Or, Xor, Select, SetCc, Truncate, ZeroExtend, SignExtend]
        {
            assert!(arm64.is_operation_legal_or_custom(op, bits), "{op:?} i{bits}");
        }
        assert!(!arm64.is_operation_legal_or_custom(URem, bits) && !arm64.is_operation_legal_or_custom(SRem, bits));
    }
    assert!(!arm64.is_operation_legal_or_custom(Add, 8) && !arm64.is_operation_legal_or_custom(Add, 16));
}

/// The vector costs of `opt -passes='print<cost-model>' -cost-kind=throughput`
/// on aarch64 (the pin, both cpus): before the vector hooks existed `Tti`
/// priced no vector at all, so no loop could be weighed against its scalar
/// form.
#[test]
fn vector_memory_and_arithmetic_costs_are_the_pins() {
    use llrm_mir::tti::{ArithOp::*, MemoryOp::*};
    for cpu in ["generic", "apple-m1"] {
        let tti = AArch64::of_cpu(cpu).unwrap();
        let v = |lane, count| VectorTy { lane: Ty::Int(lane), count };
        let mem = |op, ty, align| tti.vector_memory_op_cost(op, ty, align, CostKind::RecipThroughput).map(|c| c.value);
        let alu = |op, ty| tti.vector_arithmetic_cost(op, ty, CostKind::RecipThroughput).map(|c| c.value);
        // load <8 x i16> align 2 / <16 x i16> / <4 x i16> align 1 / <16 x i8>
        // align 1 / align 16
        assert_eq!(mem(Load, v(16, 8), 2), Some(1), "{cpu}");
        assert_eq!(mem(Load, v(16, 16), 2), Some(2), "{cpu}");
        assert_eq!(mem(Load, v(16, 4), 1), Some(1), "{cpu}");
        assert_eq!(mem(Load, v(8, 16), 1), Some(1), "{cpu}");
        assert_eq!(mem(Load, v(16, 8), 16), Some(1), "{cpu}");
        assert_eq!(mem(Store, v(16, 8), 2), Some(1), "{cpu}");
        assert_eq!(mem(Store, v(16, 16), 2), Some(2), "{cpu}");
        assert_eq!(alu(Add, v(16, 8)), Some(1), "{cpu}");
        assert_eq!(alu(Add, v(32, 16)), Some(4), "{cpu}");
        assert_eq!(alu(Mul, v(32, 4)), Some(1), "{cpu}");
        assert_eq!(alu(And, v(8, 16)), Some(1), "{cpu}");
        assert_eq!(alu(Shl, v(32, 4)), Some(1), "{cpu}");
        assert_eq!(alu(Sub, v(64, 2)), Some(1), "{cpu}");
        // The pin prices `mul <2 x i64>` 14; this port does not price it
        // (DEVIATE VT3).
        assert_eq!(alu(Mul, v(64, 2)), None, "{cpu}");
    }
}

/// `opt -passes='print<cost-model>' -cost-kind=throughput` on aarch64 (the pin,
/// both cpus, the same numbers). `sfm16_float_add` was left
/// scalar-interleaved-never because `Tti` priced no vector `fadd`, `fcmp`,
/// `select` or cast: the loop's VF 4 could not be weighed against its VF 1 (11,
/// the pin's 48 for VF 4: two scattered loads 14 each, an expanded select 12,
/// an `fcmp uno` 3).
#[test]
fn vector_float_compare_select_cast_and_scalarization_costs_are_the_pins() {
    use llrm_mir::opcode::{FloatPredicate as F, IntPredicate as I};
    use llrm_mir::tti::{
        ArithOp::*,
        CastKind::*,
        CmpSel::*,
        Predicate::{Float, Int},
    };
    let f = |count| VectorTy { lane: Ty::Float, count };
    let d = |count| VectorTy { lane: Ty::Double, count };
    let i = |lane, count| VectorTy { lane: Ty::Int(lane), count };
    for cpu in ["generic", "apple-m1"] {
        let tti = AArch64::of_cpu(cpu).unwrap();
        let k = CostKind::RecipThroughput;
        let alu = |op, ty| tti.vector_arithmetic_cost(op, ty, k).map(|c| c.value);
        let cmp = |op, ty, predicate| tti.vector_cmp_sel_cost(op, ty, predicate, k).map(|c| c.value);
        let cast = |kind, to, from| tti.vector_cast_cost(kind, to, from, k).map(|c| c.value);
        let overhead = |ty, insert, extract| tti.scalarization_overhead(ty, insert, extract, k).map(|c| c.value);
        for ty in [f(2), f(4), d(2)] {
            assert_eq!((alu(FAdd, ty), alu(FSub, ty), alu(FNeg, ty)), (Some(1), Some(1), Some(1)), "{cpu} {ty:?}");
            assert_eq!((alu(FMul, ty), alu(FDiv, ty)), (Some(2), Some(2)), "{cpu} {ty:?}");
            // `frem` of a vector is libcalls in the pin (24 to 52): not priced
            // here.
            assert_eq!(alu(FRem, ty), None, "{cpu} {ty:?}");
            for (predicate, cost) in
                [(F::Oeq, 1), (F::Olt, 1), (F::Une, 1), (F::Ugt, 1), (F::One, 3), (F::Ord, 3), (F::Ueq, 3), (F::Uno, 3)]
            {
                assert_eq!(cmp(FCmp, ty, Some(Float(predicate))), Some(cost), "{cpu} fcmp {predicate:?} {ty:?}");
            }
        }
        // select by a vector of conditions: a compare NEON does feeds a CMxx
        // and BFI pair (1); else the `VectorSelectTbl` row (floats 2);
        // else expanded: lanes plus the inserts that rebuild the vector.
        assert_eq!(cmp(Select, i(32, 4), Some(Int(I::Slt))), Some(1), "{cpu}");
        assert_eq!(cmp(Select, i(8, 16), Some(Int(I::Eq))), Some(1), "{cpu}");
        assert_eq!(cmp(Select, f(4), Some(Float(F::Olt))), Some(1), "{cpu}");
        assert_eq!(cmp(Select, f(4), Some(Float(F::Uno))), Some(2), "{cpu}");
        assert_eq!(cmp(Select, d(2), None), Some(2), "{cpu}");
        assert_eq!(
            (cmp(Select, i(32, 4), None), cmp(Select, i(32, 2), None), cmp(Select, i(64, 2), None)),
            (Some(12), Some(6), Some(6)),
            "{cpu}"
        );
        assert_eq!((cmp(Select, i(16, 8), None), cmp(Select, i(8, 16), None)), (Some(24), Some(48)), "{cpu}");
        assert_eq!(cmp(ICmp, i(32, 4), Some(Int(I::Eq))), Some(1), "{cpu}");
        // casts: the conversion table, and a bitcast between one register's
        // types is free.
        assert_eq!(
            (cast(BitCast, f(4), i(32, 4)), cast(BitCast, d(2), i(64, 2)), cast(BitCast, f(2), i(32, 2))),
            (Some(0), Some(0), Some(0)),
            "{cpu}"
        );
        // `sitofp <N x i64> to <N x float>` scalarizes (double rounding): a
        // conversion per lane, the lanes taken out and put in (`opt
        // -passes=print<cost-model>`: 8, 18, 36, and the same for `uitofp`).
        assert_eq!(
            (
                cast(SIToFP, f(2), i(64, 2)),
                cast(SIToFP, f(4), i(64, 4)),
                cast(SIToFP, f(8), i(64, 8)),
                cast(UIToFP, f(2), i(64, 2))
            ),
            (Some(8), Some(18), Some(36), Some(8)),
            "{cpu}"
        );
        assert_eq!(
            (cast(FPExt, d(2), f(2)), cast(FPExt, d(4), f(4)), cast(FPTrunc, f(2), d(2)), cast(FPTrunc, f(4), d(4))),
            (Some(1), Some(2), Some(1), Some(2)),
            "{cpu}"
        );
        assert_eq!(
            (
                cast(SIToFP, f(4), i(32, 4)),
                cast(UIToFP, d(2), i(64, 2)),
                cast(FPToSI, i(32, 4), f(4)),
                cast(FPToUI, i(64, 2), d(2))
            ),
            (Some(1), Some(1), Some(1), Some(1)),
            "{cpu}"
        );
        assert_eq!((cast(SIToFP, d(2), i(32, 2)), cast(FPToSI, i(32, 2), d(2))), (Some(2), Some(2)), "{cpu}");
        assert_eq!(
            (cast(Trunc, i(16, 4), i(32, 4)), cast(Trunc, i(32, 2), i(64, 2)), cast(Trunc, i(16, 8), i(32, 8))),
            (Some(1), Some(1), Some(1)),
            "{cpu}"
        );
        assert_eq!(
            (cast(SExt, i(64, 4), i(16, 4)), cast(ZExt, i(16, 16), i(8, 16)), cast(SExt, i(64, 4), i(32, 4))),
            (Some(3), Some(2), Some(2)),
            "{cpu}"
        );
        // A cast the table has no row for is not priced, not guessed.
        assert_eq!(cast(SExt, i(32, 4), i(16, 4)), None, "{cpu}");
        // The overhead of building a vector from scalars: 2 per integer lane; a
        // float lane too, but lane 0 of a register is already in it.
        assert_eq!(
            (overhead(i(32, 4), true, false), overhead(i(64, 2), true, false), overhead(i(32, 4), true, true)),
            (Some(8), Some(4), Some(16)),
            "{cpu}"
        );
        assert_eq!(
            (
                overhead(f(4), true, false),
                overhead(f(2), true, false),
                overhead(d(2), true, false),
                overhead(f(8), true, false)
            ),
            (Some(6), Some(2), Some(2), Some(12)),
            "{cpu}"
        );
    }
}

#[test]
fn the_interleave_factor_and_register_counts_follow_the_cpu() {
    let (generic, m1) = (AArch64::of_cpu("generic").unwrap(), AArch64::of_cpu("apple-m1").unwrap());
    // getMaxInterleaveFactor: 4 for a scalar loop, else the subtarget's (2, 4
    // on the A14 tuning).
    assert_eq!((generic.max_interleave_factor(1, false), generic.max_interleave_factor(8, false)), (4, 2));
    assert_eq!((m1.max_interleave_factor(1, false), m1.max_interleave_factor(8, false)), (4, 4));
    assert_eq!(generic.max_interleave_factor(4, true), 4);
    assert_eq!(
        (generic.number_of_registers(RegisterClass::Vector), generic.number_of_registers(RegisterClass::Scalar)),
        (32, 31)
    );
}

/// The horizontal step of a vector reduction, priced as the pin's AArch64 table
/// (`CostTblNoPairwise`, `faddp` per halving, 2 for a min or max). The expected
/// costs are `opt -mtriple=aarch64 -passes='print<cost-model>'` on the same
/// `llvm.vector.reduce.*` calls.
#[test]
fn a_vector_reduction_is_priced_by_the_pins_table() {
    use llrm_mir::intrinsics::ReduceKind;
    use llrm_mir::tti::{CostKind, Ty, VectorTy};
    let tti = AArch64::of_cpu("generic").expect("a cpu");
    let cost = |kind, lane, count, ordered| {
        tti.vector_reduction_cost(kind, VectorTy { lane, count }, ordered, CostKind::RecipThroughput).map(|c| c.value)
    };
    // `addv`: 2 a register, plus 1 for each split.
    assert_eq!(cost(ReduceKind::Add, Ty::Int(16), 8, false), Some(2));
    assert_eq!(cost(ReduceKind::Add, Ty::Int(32), 8, false), Some(3));
    // The or/xor/and sequences of the table, and the split's extra `orr`.
    assert_eq!(cost(ReduceKind::Or, Ty::Int(32), 4, false), Some(5));
    assert_eq!(cost(ReduceKind::Xor, Ty::Int(8), 16, false), Some(7));
    assert_eq!(cost(ReduceKind::And, Ty::Int(32), 8, false), Some(6));
    // `faddp` once per halving of the register.
    assert_eq!(cost(ReduceKind::FAdd, Ty::Float, 4, false), Some(2));
    assert_eq!(cost(ReduceKind::FAdd, Ty::Double, 2, false), Some(1));
    assert_eq!(cost(ReduceKind::SMax, Ty::Int(16), 8, false), Some(2));
    assert_eq!(cost(ReduceKind::SMax, Ty::Int(32), 8, false), Some(3));
    // Not the pin's table: in lane order, a multiply, a 64-bit min.
    assert_eq!(cost(ReduceKind::FAdd, Ty::Float, 4, true), None);
    assert_eq!(cost(ReduceKind::Mul, Ty::Int(32), 4, false), None);
    assert_eq!(cost(ReduceKind::SMax, Ty::Int(64), 2, false), None);
}

/// `getVectorInstrCost` of an `insertelement` and an `extractelement` equals
/// the pinned `opt -passes='print<cost-model>'` (`-cost-kind` throughput and
/// code-size, `-mcpu` generic and apple-m1; one-use load as the inserted
/// scalar for the `ld1` rows). Before `vector_instr_cost` the hook answered
/// `None` for every lane, so the SLP vectorizer could price no build-vector
/// or external use.
#[test]
fn lane_instruction_costs_are_the_pinned_opts() {
    let v = |lane, count| VectorTy { lane, count };
    let (f4, i4, d2, i16x8, f8, i8x16, i64x2) = (
        v(Ty::Float, 4),
        v(Ty::Int(32), 4),
        v(Ty::Double, 2),
        v(Ty::Int(16), 8),
        v(Ty::Float, 8),
        v(Ty::Int(8), 16),
        v(Ty::Int(64), 2),
    );
    for cpu in ["generic", "apple-m1"] {
        let tti = AArch64::of_cpu(cpu).unwrap();
        let cost = |op, ty, index, poison, context, kind| {
            tti.vector_instr_cost(op, ty, Some(index), poison, context, kind).map(|c| c.value)
        };
        let t = CostKind::RecipThroughput;
        let none = VectorInstrContext::None;
        for (ty, index, expected) in [
            (f4, 0, 0),
            (f4, 1, 2),
            (f4, 3, 2),
            (i4, 0, 2),
            (i4, 2, 2),
            (d2, 1, 2),
            (i16x8, 0, 2),
            (f8, 5, 2),
            (f8, 4, 0),
            (i8x16, 9, 2),
            (i64x2, 1, 2),
        ] {
            let got = cost(LaneOp::Extract, ty, index, false, none, t);
            assert_eq!(got, Some(expected), "{cpu} extract {ty:?} lane {index}");
        }
        for (ty, index, poison, expected) in [
            (f4, 0, false, 0),
            (f4, 1, false, 2),
            (i4, 0, false, 2),
            (i4, 3, false, 2),
            (d2, 1, false, 2),
            (f4, 0, true, 0),
            (f4, 1, true, 2),
            (f8, 6, false, 2),
        ] {
            let got = cost(LaneOp::Insert, ty, index, poison, none, t);
            assert_eq!(got, Some(expected), "{cpu} insert {ty:?} lane {index} poison {poison}");
        }
        // code size: one per moved lane, none for lane 0 of a float vector
        let c = CostKind::CodeSize;
        assert_eq!(cost(LaneOp::Extract, f4, 0, false, none, c), Some(0), "{cpu}");
        assert_eq!(cost(LaneOp::Extract, f4, 1, false, none, c), Some(1), "{cpu}");
        assert_eq!(cost(LaneOp::Insert, f4, 1, false, none, c), Some(1), "{cpu}");
        assert_eq!(cost(LaneOp::Insert, f4, 1, true, none, c), Some(1), "{cpu}");
        // an insert of a one-use load is a `ld1` lane: free on apple-m1, base
        // plus one on generic, free in code size
        let load = VectorInstrContext::Load;
        let ld1 = if cpu == "apple-m1" { 0 } else { 3 };
        assert_eq!(cost(LaneOp::Insert, i4, 2, false, load, t), Some(ld1), "{cpu}");
        assert_eq!(cost(LaneOp::Insert, i4, 2, false, load, c), Some(0), "{cpu}");
    }
}

/// `getStoreMinimumVF`: from the pin's source (AArch64TargetTransformInfo.h
/// L537 over BasicTTIImpl L492), not queried: four or more `i8` stores in a
/// power-of-two count are cut to 4; else the VF halves while a store of half
/// the lanes fits one register.
#[test]
fn store_minimum_vf_is_the_pins() {
    let tti = AArch64::of_cpu("generic").unwrap();
    let min = |vf, mem| tti.store_minimum_vf(vf, mem, mem);
    assert_eq!((min(8, Ty::Int(8)), min(16, Ty::Int(8)), min(4, Ty::Int(8))), (4, 4, 4));
    assert_eq!((min(2, Ty::Float), min(4, Ty::Float), min(8, Ty::Float)), (2, 2, 2));
    // sixteen floats: a store of eight is two registers, not legal
    assert_eq!(min(16, Ty::Float), 16);
    // four doubles are 256 bits: not a legal store, so eight stay eight
    assert_eq!((min(2, Ty::Int(32)), min(4, Ty::Int(16)), min(4, Ty::Double), min(8, Ty::Double)), (2, 2, 2, 8));
}

/// The SLP vectorizer's narrowest vector: 64 bits on both cpus
/// (`AArch64Subtarget::MinVectorRegisterBitWidth` default).
#[test]
fn the_narrowest_vector_register_is_64_bits() {
    for cpu in ["generic", "apple-m1"] {
        assert_eq!(AArch64::of_cpu(cpu).unwrap().min_vector_register_bits(), 64, "{cpu}");
    }
}
