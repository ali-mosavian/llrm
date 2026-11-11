//! `gep i8 @g, (shl %i, 2)` and `gep inbounds [4 x i8] @g, %i` were two
//! different variable offsets to BasicAA (no `GetLinearExpression`): MayAlias
//! where the pin says MustAlias. gvn could not forward the store to the load
//! and left the load PRE undone on `lru` (Nib, m16 -O2).
use llrm_mir::passes::PassManager;
use llrm_transforms::gvn::Gvn;

#[test]
fn test_a_store_through_a_shifted_index_feeds_the_load_pre() {
    let text = "target datalayout = \"e-p:16:16-i32:16-n8:16:32\"
@g = internal global [12 x i8] zeroinitializer, align 4
define i32 @f(i16 %i, i1 %c, i1 %d) {
entry:
  br i1 %c, label %join, label %a
a:
  %sh = shl i16 %i, 2
  %q = getelementptr i8, ptr @g, i16 %sh
  store i32 8, ptr %q, align 4
  br i1 %d, label %join, label %out
join:
  %e = getelementptr inbounds [4 x i8], ptr @g, i16 %i
  %v = load i32, ptr %e, align 4
  ret i32 %v
out:
  ret i32 0
}
";
    let mut module = llrm_mir::parse::module(text).expect("parses");
    let mut manager = PassManager::default();
    manager.add(Gvn::default());
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    let after = llrm_mir::print::module(&module);
    assert!(after.contains("phi i32"), "{after}");
}
