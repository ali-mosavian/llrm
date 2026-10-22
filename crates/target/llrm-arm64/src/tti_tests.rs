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
