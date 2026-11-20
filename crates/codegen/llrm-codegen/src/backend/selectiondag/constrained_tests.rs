//! `llvm.experimental.constrained.*` (`visitConstrainedFPIntrinsic`): each
//! call is one `STRICT_*` node on the root's chain. Before, the builder
//! refused them all ("intrinsic call llvm.experimental.constrained.fadd.f80"),
//! so QuickrBasic's `pfpu.c` (`#pragma STDC FENV_ACCESS ON`) never selected.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::dag::{SDNodeFlags, SelectionDAG};
use super::function_lowering::FunctionLoweringInfo;
use super::isd::ISD;

const HEAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n";
const DECLARES: &str = "
declare x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fmul.f80(x86_fp80, x86_fp80, metadata, metadata)
declare x86_fp80 @llvm.experimental.constrained.fpext.f80.f64(double, metadata)
declare double @llvm.experimental.constrained.fptrunc.f64.f80(x86_fp80, metadata, metadata)
declare i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80, x86_fp80, metadata, metadata)
";

fn built(body: &str) -> Result<SelectionDAG, &'static str> {
    let module = llrm_mir::parse::module(&format!("{HEAD}{body}{DECLARES}")).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .expect("f");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::for_pointer_bits(32),
        &llrm_x86_m32::M32,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .map(|built| built.dag)
    .map_err(|why| why.0)
}

fn nodes(
    dag: &SelectionDAG,
    opcode: ISD,
) -> Vec<usize> {
    (0..dag.nodes().len()).filter(|&at| dag.nodes()[at].opcode == opcode).collect()
}

/// The add is a `STRICT_FADD` of (chain, a, b) giving (value, chain); the
/// exception behavior `strict` leaves `NoFPExcept` off.
#[test]
fn a_strict_add_is_one_chained_node_with_a_value_and_an_out_chain() {
    let dag = built(
        "define x86_fp80 @f(x86_fp80 %a, x86_fp80 %b) strictfp {\n  %s = call x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80 %a, x86_fp80 %b, metadata !\"round.dynamic\", metadata !\"fpexcept.strict\") strictfp\n  ret x86_fp80 %s\n}\n",
    )
    .expect("builds");
    let adds = nodes(&dag, ISD::StrictFAdd);
    assert_eq!(adds.len(), 1, "one STRICT_FADD");
    let node = &dag.nodes()[adds[0]];
    assert_eq!(node.values.len(), 2, "value and chain");
    assert_eq!(node.ops.len(), 3, "chain, a, b");
    assert_eq!(node.flags.0 & SDNodeFlags::NO_FP_EXCEPT.0, 0);
    assert!(nodes(&dag, ISD::FAdd).is_empty(), "never the plain node: it carries no chain");
}

/// `fpexcept.ignore` sets `NoFPExcept`; an ignored operation after a strict
/// one waits for it (`getFPOperationRoot`), or it could move across a read of
/// the exception flags.
#[test]
fn an_ignored_operation_after_a_strict_one_is_chained_behind_it() {
    let dag = built(
        "define x86_fp80 @f(x86_fp80 %a, x86_fp80 %b) strictfp {\n  %s = call x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80 %a, x86_fp80 %b, metadata !\"round.dynamic\", metadata !\"fpexcept.strict\") strictfp\n  %m = call x86_fp80 @llvm.experimental.constrained.fmul.f80(x86_fp80 %s, x86_fp80 %b, metadata !\"round.dynamic\", metadata !\"fpexcept.ignore\") strictfp\n  ret x86_fp80 %m\n}\n",
    )
    .expect("builds");
    let (add, mul) = (nodes(&dag, ISD::StrictFAdd)[0], nodes(&dag, ISD::StrictFMul)[0]);
    assert_ne!(dag.nodes()[mul].flags.0 & SDNodeFlags::NO_FP_EXCEPT.0, 0);
    let chain = dag.nodes()[mul].ops[0];
    assert_eq!((chain.node as usize, chain.res), (add, 1), "the multiply hangs on the add's out chain");
}

/// Conversions and the compare carry what the pin gives them: `FP_ROUND` a
/// second operand, the compares a condition code and an i1 result.
#[test]
fn conversions_and_compares_carry_their_extra_operands() {
    let dag = built(
        "define i1 @f(x86_fp80 %a, double %d) strictfp {\n  %t = call double @llvm.experimental.constrained.fptrunc.f64.f80(x86_fp80 %a, metadata !\"round.dynamic\", metadata !\"fpexcept.strict\") strictfp\n  %e = call x86_fp80 @llvm.experimental.constrained.fpext.f80.f64(double %t, metadata !\"fpexcept.strict\") strictfp\n  %c = call i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80 %a, x86_fp80 %e, metadata !\"olt\", metadata !\"fpexcept.strict\") strictfp\n  ret i1 %c\n}\n",
    )
    .expect("builds");
    let round = nodes(&dag, ISD::StrictFpRound)[0];
    assert_eq!(dag.nodes()[round].ops.len(), 3, "chain, value, the pin's second operand");
    let compare = nodes(&dag, ISD::StrictFSetCCS)[0];
    assert_eq!(dag.nodes()[compare].ops.len(), 4, "chain, lhs, rhs, condition code");
    assert_eq!(nodes(&dag, ISD::StrictFpExtend).len(), 1);
}

/// A word that is not an exception behavior is refused, not guessed.
#[test]
fn an_unknown_exception_behavior_is_refused() {
    let why = built(
        "define x86_fp80 @f(x86_fp80 %a, x86_fp80 %b) strictfp {\n  %s = call x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80 %a, x86_fp80 %b, metadata !\"round.dynamic\", metadata !\"fpexcept.sometimes\") strictfp\n  ret x86_fp80 %s\n}\n",
    )
    .err()
    .expect("refused");
    assert!(why.contains("exception behavior"), "{why}");
}

/// The x87 does constrained arithmetic and both widths of conversion with the
/// instructions it does the plain ones with (`X86ISelLowering.cpp` L789..L868
/// make them legal). Before, the legalizer stopped at "convert node to
/// libcall" for each.
#[test]
fn constrained_arithmetic_and_float_conversions_select_to_the_x87_instructions() {
    use super::dag_function::selected;
    let select = |ty: &str, call: &str, decl: &str| {
        let text = format!(
            "{HEAD}define {ty} @f(x86_fp80 %a, x86_fp80 %b, double %d) strictfp {{\n  %s = call {ty} @llvm.experimental.constrained.{call} strictfp\n  ret {ty} %s\n}}\ndeclare {decl}\n"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|why| panic!("{call}: {}", why.0));
        done.body
            .insns()
            .iter()
            .filter_map(|insn| insn.what.as_ref())
            .map(|what| what.name.as_deref().unwrap_or("").to_owned())
            .collect::<Vec<_>>()
    };
    let e = "metadata !\"round.dynamic\", metadata !\"fpexcept.strict\"";
    let add = select(
        "x86_fp80",
        &format!("fadd.f80(x86_fp80 %a, x86_fp80 %b, {e})"),
        "x86_fp80 @llvm.experimental.constrained.fadd.f80(x86_fp80, x86_fp80, metadata, metadata)",
    );
    assert_eq!(add[..3].to_vec(), ["fld", "fld", "fadd"], "{add:?}");
    let div = select(
        "double",
        &format!("fdiv.f64(double %d, double %d, {e})"),
        "double @llvm.experimental.constrained.fdiv.f64(double, double, metadata, metadata)",
    );
    assert!(div.contains(&"fdiv".to_owned()), "{div:?}");
    let extended = select(
        "x86_fp80",
        "fpext.f80.f64(double %d, metadata !\"fpexcept.strict\")",
        "x86_fp80 @llvm.experimental.constrained.fpext.f80.f64(double, metadata)",
    );
    assert_eq!(extended[0], "fld", "{extended:?}");
    let truncated = select(
        "double",
        &format!("fptrunc.f64.f80(x86_fp80 %a, {e})"),
        "double @llvm.experimental.constrained.fptrunc.f64.f80(x86_fp80, metadata, metadata)",
    );
    assert_eq!(truncated[0], "fld", "{truncated:?}");
}

/// A constrained compare is chained and picks its x87 compare by what it
/// promises: `fcmps` signals on a quiet NaN so it is `fcom`, `fcmp` is the
/// quiet `fucom`. `une` is two conditions (`setp` or `setne`) of one compare.
/// Before, the type legalizer stopped at "promote result" on the i1 result.
#[test]
fn constrained_compares_select_fcom_for_signaling_and_fucom_for_quiet() {
    use super::dag_function::selected;
    let select = |call: &str, decl: &str| {
        let text = format!(
            "{HEAD}define i1 @f(x86_fp80 %a, x86_fp80 %b) strictfp {{\n  %s = call i1 @llvm.experimental.constrained.{call} strictfp\n  ret i1 %s\n}}\ndeclare {decl}\n"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|why| panic!("{call}: {}", why.0));
        done.body
            .insns()
            .iter()
            .filter_map(|insn| insn.what.as_ref())
            .map(|what| what.name.as_deref().unwrap_or("").to_owned())
            .collect::<Vec<_>>()
    };
    let signaling = select(
        "fcmps.f80(x86_fp80 %a, x86_fp80 %b, metadata !\"olt\", metadata !\"fpexcept.strict\")",
        "i1 @llvm.experimental.constrained.fcmps.f80(x86_fp80, x86_fp80, metadata, metadata)",
    );
    assert_eq!(signaling[..4].to_vec(), ["fld", "fld", "fcom", "seta"], "{signaling:?}");
    let quiet = select(
        "fcmp.f80(x86_fp80 %a, x86_fp80 %b, metadata !\"une\", metadata !\"fpexcept.strict\")",
        "i1 @llvm.experimental.constrained.fcmp.f80(x86_fp80, x86_fp80, metadata, metadata)",
    );
    assert_eq!(quiet[..6].to_vec(), ["fld", "fld", "fucom", "setp", "setne", "or"], "{quiet:?}");
}

/// The integer conversions and `rint` are chained too: `fild` and `fisttp`
/// through a stack slot on the node's chain, `frndint` for `rint`. Before,
/// each stopped the legalizer ("convert node to libcall"), so a
/// QuickrBasic `CINT` or `INT` under `FENV_ACCESS` never selected.
#[test]
fn constrained_integer_conversions_and_rint_select_to_fild_fisttp_and_frndint() {
    use super::dag_function::selected;
    let select = |ty: &str, call: &str, decl: &str| {
        let text = format!(
            "{HEAD}define {ty} @f(x86_fp80 %a, i32 %i) strictfp {{\n  %s = call {ty} @llvm.experimental.constrained.{call} strictfp\n  ret {ty} %s\n}}\ndeclare {decl}\n"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|why| panic!("{call}: {}", why.0));
        done.body
            .insns()
            .iter()
            .filter_map(|insn| insn.what.as_ref())
            .map(|what| what.name.as_deref().unwrap_or("").to_owned())
            .collect::<Vec<_>>()
    };
    let e = "metadata !\"round.dynamic\", metadata !\"fpexcept.strict\"";
    let to_float = select(
        "x86_fp80",
        &format!("sitofp.f80.i32(i32 %i, {e})"),
        "x86_fp80 @llvm.experimental.constrained.sitofp.f80.i32(i32, metadata, metadata)",
    );
    assert!(to_float.contains(&"fild".into()), "{to_float:?}");
    let to_int = select(
        "i32",
        "fptosi.i32.f80(x86_fp80 %a, metadata !\"fpexcept.strict\")",
        "i32 @llvm.experimental.constrained.fptosi.i32.f80(x86_fp80, metadata)",
    );
    assert_eq!(to_int[..2].to_vec(), ["fld", "fisttp"], "{to_int:?}");
    let rint = select(
        "x86_fp80",
        &format!("rint.f80(x86_fp80 %a, {e})"),
        "x86_fp80 @llvm.experimental.constrained.rint.f80(x86_fp80, metadata, metadata)",
    );
    assert_eq!(rint[..2].to_vec(), ["fld", "frndint"], "{rint:?}");
}

/// At m64 the scalar forms are SSE: `addsd`, `cvtsi2sd`, `cvttsd2si`,
/// `cvtss2sd`, and the signaling compare `comisd` against the quiet
/// `ucomisd`. Before, `fcmps` stopped at emit with `Unmapped("COMISDrr")` and
/// the integer conversions at "convert node to libcall".
#[test]
fn constrained_forms_select_to_sse_at_m64() {
    use super::dag_function::selected;
    let e = "metadata !\"round.dynamic\", metadata !\"fpexcept.strict\"";
    let x = "metadata !\"fpexcept.strict\"";
    let cases: Vec<(&str, String, &str, &str)> = vec![
        ("double", format!("fadd.f64(double %d, double %d, {e})"), "double @llvm.experimental.constrained.fadd.f64(double, double, metadata, metadata)", "addsd"),
        ("double", format!("fpext.f64.f32(float %s, {x})"), "double @llvm.experimental.constrained.fpext.f64.f32(float, metadata)", "cvtss2sd"),
        ("float", format!("fptrunc.f32.f64(double %d, {e})"), "float @llvm.experimental.constrained.fptrunc.f32.f64(double, metadata, metadata)", "cvtsd2ss"),
        ("double", format!("sitofp.f64.i32(i32 %i, {e})"), "double @llvm.experimental.constrained.sitofp.f64.i32(i32, metadata, metadata)", "cvtsi2sd"),
        ("double", format!("sitofp.f64.i64(i64 %l, {e})"), "double @llvm.experimental.constrained.sitofp.f64.i64(i64, metadata, metadata)", "cvtsi2sd"),
        ("double", format!("sitofp.f64.i16(i16 %h, {e})"), "double @llvm.experimental.constrained.sitofp.f64.i16(i16, metadata, metadata)", "cvtsi2sd"),
        ("i32", format!("fptosi.i32.f64(double %d, {x})"), "i32 @llvm.experimental.constrained.fptosi.i32.f64(double, metadata)", "cvttsd2si"),
        ("i64", format!("fptosi.i64.f64(double %d, {x})"), "i64 @llvm.experimental.constrained.fptosi.i64.f64(double, metadata)", "cvttsd2si"),
        ("i16", format!("fptosi.i16.f64(double %d, {x})"), "i16 @llvm.experimental.constrained.fptosi.i16.f64(double, metadata)", "cvttsd2si"),
        ("i1", format!("fcmps.f64(double %d, double %d, metadata !\"olt\", {x})"), "i1 @llvm.experimental.constrained.fcmps.f64(double, double, metadata, metadata)", "comisd"),
        ("i1", format!("fcmp.f64(double %d, double %d, metadata !\"une\", {x})"), "i1 @llvm.experimental.constrained.fcmp.f64(double, double, metadata, metadata)", "ucomisd"),
        ("x86_fp80", format!("sitofp.f80.i64(i64 %l, {e})"), "x86_fp80 @llvm.experimental.constrained.sitofp.f80.i64(i64, metadata, metadata)", "fild"),
        ("i64", format!("fptosi.i64.f80(x86_fp80 %a, {x})"), "i64 @llvm.experimental.constrained.fptosi.i64.f80(x86_fp80, metadata)", "fisttp"),
    ];
    for (ty, call, decl, want) in cases {
        let text = format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"\ndefine {ty} @f(x86_fp80 %a, double %d, float %s, i32 %i, i64 %l, i16 %h) strictfp {{\n  %r = call {ty} @llvm.experimental.constrained.{call} strictfp\n  ret {ty} %r\n}}\ndeclare {decl}\n"
        );
        let module = llrm_mir::parse::module(&text).expect("parses");
        let cpu = crate::backend::cpu::tuned_for(&llrm_x86_m64::M64, llrm_x86_m64::CPUS[0], false).expect("a cpu");
        let done = selected(&module, "f", &llrm_x86_m64::M64, cpu, &[]).unwrap_or_else(|why| panic!("{call}: {}", why.0));
        let names: Vec<String> = done
            .body
            .insns()
            .iter()
            .filter_map(|insn| insn.what.as_ref())
            .map(|what| what.name.as_deref().unwrap_or("").to_owned())
            .collect();
        assert!(names.iter().any(|name| name == want), "{call}: wanted {want}: {names:?}");
    }
}
