//! M68k has no `TargetTransformInfo` in the pin, so `ctlz` and `cttz` cost
//! `TCC_Basic` (TargetTransformInfoImpl.h `getIntrinsicInstrCost`). Priced
//! invalid here, SimplifyCFG kept the branch of `x ? ctlz(x) : 32` and
//! `x ? cttz(x) : 32` (the corpus's lead0/trail0), where the pin speculates it
//! into a select, which InstCombine turns into the bare count.

use llrm_m68k::compile::CodeModel;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly_in};

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

/// Nested 3x3 loops of `smul.fix.sat` and `umul.fix.sat` (clang's saturating
/// fixed-point multiplies, dozens of instructions each) stay loops. At the base
/// class's price of 1 per intrinsic the unroller saw a small body and unrolled
/// both: `fixed_point` -O2 1644 -> 3604 B (M68K-INTRINSIC-EXPANSION).
#[test]
fn nested_loops_of_expanded_intrinsics_are_not_unrolled() {
    let ir = format!(
        "{HEAD}declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32 immarg)
declare i32 @llvm.umul.fix.sat.i32(i32, i32, i32 immarg)
define void @f(ptr %p) {{
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %in, %latch ]
  br label %inner
inner:
  %j = phi i32 [ 0, %outer ], [ %jn, %inner ]
  %a = load volatile i32, ptr %p
  %r = call i32 @llvm.smul.fix.sat.i32(i32 %a, i32 %a, i32 15)
  %u = call i32 @llvm.umul.fix.sat.i32(i32 %r, i32 %a, i32 15)
  store volatile i32 %u, ptr %p
  %jn = add i32 %j, 1
  %cj = icmp ult i32 %jn, 3
  br i1 %cj, label %inner, label %latch
latch:
  %in = add i32 %i, 1
  %ci = icmp ult i32 %in, 3
  br i1 %ci, label %outer, label %exit
exit:
  ret void
}}\n"
    );
    let out = llrm_m68k::driver::assembly(&ir, Cpu::M68020, Level::O2).expect("compiles");
    assert!(out.lines().count() < 200, "unrolled:\n{out}");
}
