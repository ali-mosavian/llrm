use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;

/// `fpround` keeps the rounding of the type: the DAG stores the value to a
/// stack slot of its type and loads it back (the old selector's `fstp`/`fld`),
/// and the result is the load. Elided as the pin's no-op same-type
/// `FP_ROUND`, `16777217.0` would stay `16777217.0` as an `f32`.
#[test]
fn an_fpround_stores_and_reloads_its_value() {
    let text = "define float @f(float %x) {\n  %r = fpround float %x to float\n  ret float %r\n}\n";
    let module = llrm_mir::parse::module(&format!("target datalayout = \"e-p:32:32-n8:16:32\"\n{text}"))
        .unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().next().unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let built = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m32(),
        &llrm_x86_m32::M32,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let dump = built.dag.dump();
    let stores: Vec<&str> = dump.lines().filter(|line| line.contains("= store<")).collect();
    assert_eq!(stores.len(), 1, "{dump}");
    assert!(stores[0].contains("store<(store (s32)") && stores[0].contains("FrameIndex:i32<0>"), "{dump}");
    let ret = dump.lines().find(|line| line.contains("RET_GLUE")).expect("a return");
    let reload = dump
        .lines()
        .find(|line| line.contains("f32,ch = load<") && line.contains("FrameIndex:i32<0>"))
        .expect("a reload");
    assert!(reload.contains("FrameIndex:i32<0>"), "{dump}");
    let loaded = reload.trim().split(':').next().unwrap();
    assert!(ret.contains(loaded), "the return reads the reload: {dump}");
}
