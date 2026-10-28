//! A call of an `llrm.ia16.code.*` block builds `X86ISD::LLRM_CODE` with a
//! frame index per place its name patches in, and reads the `i32` result from
//! `dx:ax`. Before, it stopped the builder at "intrinsic call" (4 m16 corpus
//! blocks). The pin has no such node; this tests the shape.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;
use crate::testing::m16_ir;

fn dump(body: &str) -> String {
    let text = m16_ir(&format!("target datalayout = \"@m16-datalayout@-ni:2:3\"\n{body}"));
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m16(),
        &llrm_x86_m16::M16,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .unwrap_or_else(|why| panic!("{}", why.0))
    .dag
    .dump()
}

#[test]
fn inline_code_is_one_node_with_its_places_and_a_dx_ax_result() {
    let dump = dump(
        "declare i32 @llrm.ia16.code.89a60000.2p0(ptr)\n\
         define i32 @f() {\n  %slot = alloca i16\n  %r = call i32 @llrm.ia16.code.89a60000.2p0(ptr %slot)\n  ret i32 %r\n}\n",
    );
    assert!(dump.contains("X86ISD::LLRM_CODE") && dump.contains("TargetFrameIndex"), "{dump}");
    assert!(dump.contains("$ax") && dump.contains("$dx") && dump.contains("build_pair"), "{dump}");
}

/// `llrm.ia16.ptrdiff` is one `LLRM_PTRDIFF far, far -> i32`; the legalizer
/// knows the selector stride. Before it stopped the builder (1 corpus block).
#[test]
fn a_huge_pointer_difference_is_one_node() {
    let dump = dump(
        "declare i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3), ptr addrspace(3))\n\
         define i32 @f(ptr addrspace(3) %a, ptr addrspace(3) %b) {\n  %r = call i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3) %a, ptr addrspace(3) %b)\n  ret i32 %r\n}\n",
    );
    assert!(dump.contains("X86ISD::LLRM_PTRDIFF"), "{dump}");
}
