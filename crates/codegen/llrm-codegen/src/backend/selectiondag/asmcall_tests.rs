//! A call of an `llrm.ia16.asm.*` block builds `X86ISD::LLRM_ASM` with the
//! registers and symbols its name states. Before, any such call stopped the
//! builder at "intrinsic call" (the Nib block at m32, two QB blocks at m16).

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;

fn dump(
    text: &str,
    target: &BuilderTarget,
    arch: &dyn llrm_target::Target,
) -> String {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .expect("f");
    let convention = crate::backend::isel::convention(&module, &layout, id, arch).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(&module, function, &layout, target, &arch, &convention, &mut info, function.entry().unwrap())
        .unwrap_or_else(|why| panic!("{}", why.0))
        .dag
        .dump()
}

/// Both inputs are copied to their registers, glued, before the node, and
/// the output is read from `eax` after it, on the node's chain and glue.
#[test]
fn a_nib_block_copies_its_inputs_and_reads_its_output() {
    let name = "llrm.ia16.asm.01d883c003.eax_ebx.eax.flags.n";
    let text = format!(
        "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @{name}(i32, i32)\n\
         define i32 @f(i32 %a, i32 %b) {{\n  %r = call i32 @{name}(i32 %a, i32 %b)\n  ret i32 %r\n}}\n"
    );
    let dump = dump(&text, &BuilderTarget::m32(), &llrm_x86_m32::M32);
    assert!(dump.contains("X86ISD::LLRM_ASM"), "{dump}");
    assert!(dump.contains(&format!("TargetExternalSymbol:i32'{name}'")), "{dump}");
    assert_eq!(dump.matches("CopyToReg").count(), 2 + 1, "two inputs and the return value:\n{dump}");
    assert!(dump.contains("CopyFromReg") && dump.contains("$eax") && dump.contains("$ebx"), "{dump}");
}
