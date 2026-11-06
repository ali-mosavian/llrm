//! A call's `RegisterMask` names the flags register.

use llrm_codegen::backend::isel;
use llrm_codegen::backend::selectiondag::NodeData;
use llrm_codegen::backend::selectiondag::builder::build_block;
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_mir::datalayout::DataLayout;
use llrm_z80::dagisel::function::builder_target;
use llrm_z80::dagisel::generated::ids::reg as pin;
use llrm_z80::target::Z80;

/// The scheduler's live-register check read only an op's `implicit_defs`, and
/// the call's mask named nothing: a flags value live across a call was never
/// recomputed after it (arm64 fib10 gave 49 for 55; the shared scheduler is
/// the z80's too). Every call, a callee's or a libcall's, names F.
#[test]
fn a_call_register_mask_names_the_flags() {
    let text = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
        declare i16 @g(i16)\n\
        define i32 @f(i16 %n, i32 %a, i32 %b) {\n\
          %c = icmp eq i16 %n, 12\n  %r = call i16 @g(i16 %n)\n  %q = udiv i32 %a, %b\n\
          %s = select i1 %c, i16 1, i16 2\n  %t = add i16 %r, %s\n  %u = zext i16 %t to i32\n  %v = add i32 %u, %q\n  ret i32 %v\n}\n";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("a datalayout");
    let target = builder_target();
    let arch = Z80;
    let mut masks = 0;
    for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
        let convention = isel::convention(&module, &layout, id, &arch).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        for &block in function.layout() {
            let done = build_block(&module, function, &layout, &target, &arch, &convention, &mut info, block)
                .unwrap_or_else(|why| panic!("{}", why.0));
            for node in done.dag.nodes().iter().filter(|node| node.opcode == ISD::RegisterMask) {
                let NodeData::RegMask(at) = node.data else { panic!("a mask without its registers") };
                assert!(done.dag.register_masks[at as usize].contains(&pin::F), "{:?}", done.dag.register_masks);
                masks += 1;
            }
        }
    }
    assert!(masks >= 1, "the function has a call and no mask was made");
}
