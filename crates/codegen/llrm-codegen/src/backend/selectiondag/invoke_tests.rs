//! QB's `invoke` is a call and a branch to the normal block, and its pad's
//! `landingpad` builds nothing (DEVIATE SD102). Before, both stopped the
//! builder: 897 `invoke` and 75 `landingpad` m16 corpus blocks.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;

#[test]
fn an_invoke_is_a_call_then_a_branch_to_the_normal_block() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\
        declare void @g()\ndeclare i32 @p(...)\n\
        define void @f() personality ptr @p {\n  invoke void @g() to label %ok unwind label %pad\n\
        ok:\n  ret void\n\
        pad:\n  %l = landingpad { ptr, i32 } catch ptr null\n  ret void\n}\n";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let build = |info: &mut FunctionLoweringInfo, block| {
        build_block(&module, function, &layout, &BuilderTarget::m16(), &llrm_x86_m16::M16, &convention, info, block)
            .unwrap_or_else(|why| panic!("{}", why.0))
            .dag
            .dump()
    };
    let blocks: Vec<_> = function.layout().to_vec();
    let entry = build(&mut info, blocks[0]);
    assert!(entry.contains("X86ISD::CALL") && entry.contains("= br "), "{entry}");
    assert!(build(&mut info, blocks[2]).contains("RET_GLUE"));
}
