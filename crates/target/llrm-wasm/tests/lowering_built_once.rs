use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::dagisel::lowering::lowerings_built;
use llrm_wasm::driver::{Level, assembly};

const IR: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
target triple = \"wasm32-unknown-unknown\"
define i32 @pick(i32 %a, i32 %b, i32 %c) {
  %x = icmp slt i32 %a, %b
  br i1 %x, label %l, label %r
l:
  %y = icmp slt i32 %b, %c
  br i1 %y, label %m, label %r
m:
  %s = add i32 %a, %c
  ret i32 %s
r:
  %t = sub i32 %b, %c
  ret i32 %t
}
define i32 @other(i32 %a) {
  %x = icmp eq i32 %a, 0
  br i1 %x, label %l, label %r
l:
  ret i32 1
r:
  ret i32 %a
}
define i32 @narrow_default(i32 %a) {
  %t = trunc i32 %a to i8
  %r = sext i8 %t to i32
  ret i32 %r
}
define i32 @narrow_without(i32 %a) #0 {
  %t = trunc i32 %a to i8
  %r = sext i8 %t to i32
  ret i32 %r
}
attributes #0 = { \"target-features\"=\"-sign-ext\" }
";

/// The action table was rebuilt for every basic block: a fixed cost per block
/// that dominated small functions. Kept per feature set: a function without
/// `sign-ext` still expands its sign extensions beside one that has it.
#[test]
fn the_action_table_is_built_once_per_feature_set_not_per_block() {
    let options = Options { level: Level::O2, ..Options::default() };
    let before = lowerings_built();
    let text = assembly(IR, Width::W32, options).unwrap();
    assembly(IR, Width::W32, options).unwrap();
    let built = lowerings_built() - before;
    assert_eq!(built, 2, "two feature sets, ten blocks, built {built} times");
    let body =
        |name: &str| text[text.find(&format!("{name}:")).unwrap()..].split("end_function").next().unwrap().to_string();
    assert!(body("narrow_default").contains("i32.extend8_s"), "{text}");
    assert!(!body("narrow_without").contains("i32.extend8_s"), "{text}");
}
