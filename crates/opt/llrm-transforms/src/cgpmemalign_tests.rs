use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

fn after(align: u32) -> String {
    let mut module = parsed(&format!(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\
         @a = global [100 x i8] zeroinitializer, align {align}\n\
         @b = global [100 x i8] zeroinitializer, align {align}\n\
         declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
         define void @f() {{\n\
         call void @llvm.memcpy.p0.p0.i32(ptr align 1 @b, ptr align 1 @a, i32 100, i1 false)\n\
         ret void\n}}\n"
    ));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// A copy between globals declared `align 4` stated `align 1` and was a call of
/// `memcpy`: the pin's `CodeGenPrepare::optimizeCallInst` states what it knows
/// of the pointers, so x86 copies with `rep movsl` at -O1 and up.
#[test]
fn test_a_memcpy_between_aligned_globals_states_their_alignment() {
    let raised = after(4);
    assert!(raised.contains("align 4 @b") && raised.contains("align 4 @a"), "{raised}");
    assert!(after(1).contains("align 1 @b"), "premise: unaligned globals stay at one");
}
