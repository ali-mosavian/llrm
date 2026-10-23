//! A variable-size `memcpy`/`memset` calls the runtime's routine only where
//! the target's `libcalls` name it; elsewhere it is `rep movsb`/`stosb`, and
//! a `memmove` is refused, as the old selector does. A call of a routine the
//! runtime lacks does not link: before, the DAG path emitted one regardless.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;

const BODY: &str = "target datalayout = \"e-p:32:32-n8:16:32\"\n\
    declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
    declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)\n\
    declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)\n\
    define void @copy(ptr %d, ptr %s, i32 %n) {\n  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)\n  ret void\n}\n\
    define void @move(ptr %d, ptr %s, i32 %n) {\n  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)\n  ret void\n}\n\
    define void @fill(ptr %d, i8 %v, i32 %n) {\n  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 %n, i1 false)\n  ret void\n}\n";

fn built(
    function_name: &str,
    target: &BuilderTarget,
) -> Result<String, &'static str> {
    let module = llrm_mir::parse::module(BODY).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some(function_name))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(
        &module,
        function,
        &layout,
        target,
        &llrm_x86_m32::M32,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .map(|built| built.dag.dump())
    .map_err(|why| why.0)
}

#[test]
fn a_variable_copy_calls_memcpy_only_if_the_runtime_has_it() {
    let off = built("copy", &BuilderTarget::m32().with_libcalls(&[])).expect("builds");
    assert!(off.contains("REP_MOVS") && !off.contains("'memcpy'"), "{off}");
    let on = built("copy", &BuilderTarget::m32().with_libcalls(&["memcpy"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memcpy'") && !on.contains("REP_MOVS"), "{on}");
}

#[test]
fn a_variable_fill_calls_memset_only_if_the_runtime_has_it() {
    let off = built("fill", &BuilderTarget::m32().with_libcalls(&[])).expect("builds");
    assert!(off.contains("REP_STOS") && !off.contains("'memset'"), "{off}");
    let on = built("fill", &BuilderTarget::m32().with_libcalls(&["memset"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memset'") && !on.contains("REP_STOS"), "{on}");
}

#[test]
fn a_variable_move_is_refused_without_a_memmove() {
    assert!(built("move", &BuilderTarget::m32().with_libcalls(&[])).is_err());
    let on = built("move", &BuilderTarget::m32().with_libcalls(&["memmove"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memmove'"), "{on}");
}

/// The libcalls the m32 runtime's `c.toml` names, read from the file.
fn real_m32_fact() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/c/x86-m32/c.toml");
    let table: toml::Table = std::fs::read_to_string(path).expect("c.toml").parse().expect("parses");
    table
        .get("libcalls")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|one| one.as_str().map(str::to_owned))
        .collect()
}

/// The fact the m32 runtime states while its QB link lacks the routines (qbrt
/// was 183/218 with `memcpy` named): none. A variable fill is `rep stosb`, a
/// copy `rep movsb`, and a variable move is refused. Turn this into the
/// memcpy/memmove/memset case when the flip returns: a `memcpy` call with the
/// fact on is the MIR pass's (`memcpycall`, with the fact's symbol and
/// convention); the builder, which has neither, refuses one rather than call
/// `memcpy` in the wrong convention.
#[test]
fn the_real_m32_fact_names_no_libcall_while_the_qb_link_lacks_them() {
    assert!(real_m32_fact().is_empty(), "the fact changed: {:?}; state the new one here", real_m32_fact());
    let target = BuilderTarget::m32();
    let fill = built("fill", &target).expect("builds");
    assert!(fill.contains("REP_STOS") && !fill.contains("'memset'"), "{fill}");
    assert!(built("move", &target).is_err());
    assert!(built("copy", &target).is_ok(), "a copy is rep movsb with no memcpy named");
}
