//! The pin's M68k runs `BasicTTIImpl`, which prices an intrinsic by whether the
//! target's lowering makes it legal: a count the CPU does not do in hardware
//! costs `SingleCallCost` (10, 1 under `TCK_CodeSize`), `ctpop` 4. Priced
//! invalid here, SimplifyCFG kept the branch of `x ? ctlz(x) : 32` at the
//! 68020, where `bfffo` counts and the pin's `opt` speculates it.
//! Pin values: `opt -mtriple=m68k -mcpu=M68000 -passes='print<cost-model>'`
//! gives ctlz.i32 10, ctlz.i64 10, ctpop.i32 4, and 1, 1, 4 under
//! `-cost-kind=code-size`.

use llrm_m68k::compile::CodeModel;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly_in};
use llrm_m68k::machine::M68k;
use llrm_mir::tti::{Cost, CostKind, Tti, Ty};

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn guarded(
    name: &str,
    intrinsic: &str,
) -> String {
    format!(
        "{HEAD}declare i32 @llvm.{intrinsic}.i32(i32, i1)
define i32 @{name}(i32 %x) {{
entry:
  %c = icmp eq i32 %x, 0
  br i1 %c, label %m, label %nz
nz:
  %r = call i32 @llvm.{intrinsic}.i32(i32 %x, i1 true)
  br label %m
m:
  %p = phi i32 [ 32, %entry ], [ %r, %nz ]
  ret i32 %p
}}\n"
    )
}

/// The last `pipeline` stage of the middle-end (later stages lower phis
/// whatever the price).
fn pipeline_output(
    name: &str,
    ir: &str,
    cpu: Cpu,
) -> String {
    let dir = std::env::temp_dir().join(format!("ctlz_cost_{}_{name}_{cpu:?}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assembly_in(ir, cpu, Level::Os, false, Some(&dir), CodeModel::Small).expect("compiles");
    let mut stages: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.to_string_lossy().ends_with("-pipeline.ll"))
        .collect();
    stages.sort();
    let last = std::fs::read_to_string(stages.last().expect("a stage")).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    last
}

/// At the 68020 `ctlz.i32` is legal (`bfffo`, M68K-BFFFO), so it costs 1 and
/// the guarded count becomes a select. `cttz` is not legal there and keeps its
/// branch (open: M68K-CTLZ-COST card), as does `ctlz` at the 68000, where
/// nothing counts in hardware.
#[test]
fn a_guarded_count_is_speculated_where_the_cpu_counts_in_hardware() {
    let lead0 = guarded("lead0", "ctlz");
    let out = pipeline_output("lead0", &lead0, Cpu::M68020);
    assert!(
        !out.contains("phi i32"),
        "the branch and the phi stay at the 68020:
{out}"
    );
    let out = pipeline_output("lead0", &lead0, Cpu::M68000);
    assert!(
        out.contains("phi i32"),
        "the 68000 expands ctlz and keeps the branch:
{out}"
    );
}

/// The pin's price at the 68000, which counts nothing in hardware: invalid
/// before, so a guarded count was never speculated and `smul.fix.sat` never
/// priced.
#[test]
fn an_expanded_intrinsic_costs_a_call_by_kind() {
    let tti = M68k::for_cpu(Cpu::M68000);
    let price = |name: &str, bits: u32, kind| tti.intrinsic_cost(name, Some(Ty::Int(bits)), &[], kind);
    for (kind, call) in [(CostKind::SizeAndLatency, 10), (CostKind::RecipThroughput, 10), (CostKind::CodeSize, 1)] {
        assert_eq!(price("llvm.ctlz.i32", 32, kind), Cost::of(call), "ctlz.i32 {kind:?}");
        assert_eq!(price("llvm.ctlz.i64", 64, kind), Cost::of(call), "ctlz.i64 {kind:?}");
        assert_eq!(price("llvm.smul.fix.sat.i32", 32, kind), Cost::of(call), "smul.fix.sat {kind:?}");
        assert_eq!(price("llvm.ctpop.i32", 32, kind), Cost::of(4), "ctpop {kind:?}");
    }
}

/// Where the lowering counts (`bfffo`, M68K-BFFFO) the intrinsic is legal:
/// 1, doubled for the split `i64`.
#[test]
fn a_legal_intrinsic_costs_one_per_register() {
    let tti = M68k::for_cpu(Cpu::M68020);
    let kind = CostKind::SizeAndLatency;
    assert_eq!(tti.intrinsic_cost("llvm.ctlz.i32", Some(Ty::Int(32)), &[], kind), Cost::of(1));
    assert_eq!(tti.intrinsic_cost("llvm.ctlz.i64", Some(Ty::Int(64)), &[], kind), Cost::of(4));
}
