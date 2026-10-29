//! Loop strength reduction priced by the AArch64 `Tti`.

use llrm_arm64::tti::AArch64;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

fn reduced(ir: &str) -> String {
    let mut module = llrm_mir::parse::module(ir).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("datalayout");
    let tti = AArch64::of_cpu("generic").unwrap();
    let outer = Outer::of(&module, None);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("k").expect("a function");
    llrm_transforms::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, &tti, false);
    llrm_mir::print::module(&module)
}

fn loop_over(element: &str) -> String {
    format!(
        "target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n\
define void @k(ptr %d, ptr %s, i64 %n) {{\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n\
  %sp = getelementptr {element}, ptr %s, i64 %i\n  %dp = getelementptr {element}, ptr %d, i64 %i\n\
  %v = load {element}, ptr %sp\n  store {element} %v, ptr %dp\n\
  %next = add nuw i64 %i, 1\n  %c = icmp ult i64 %next, %n\n  br i1 %c, label %loop, label %exit\n\
exit:\n  ret void\n}}\n"
    )
}

/// The addressing mode scales the index by the access size, so a loop over `i16`s or `i32`s keeps
/// its one index and each access is `[base, index, lsl #n]`. The Tti asked every use as if it
/// were an 8-byte access, so only `i64` elements scaled and the others got a stepping pointer
/// each: two more phis and adds per loop than `llc -O2` (the pin) has.
#[test]
fn an_index_scaled_by_the_access_size_is_kept() {
    for element in ["i16", "i32", "i64"] {
        let text = reduced(&loop_over(element));
        assert_eq!(text.matches("phi").count(), 1, "{element}: {text}");
    }
}
