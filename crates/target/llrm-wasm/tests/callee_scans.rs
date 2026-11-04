use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, object};

fn module_of(functions: usize) -> String {
    let mut ir = String::from(
        "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"\ntarget triple = \"wasm32-unknown-unknown\"\n\
         define internal i32 @h(i32 %0) noinline nounwind {\n  %2 = mul nsw i32 %0, 3\n  %3 = add nsw i32 %2, 1\n  ret i32 %3\n}\n",
    );
    for n in 0..functions {
        ir.push_str(&format!(
            "define hidden i32 @f{n}(i32 %0, i32 %1) noinline nounwind {{\n  %3 = alloca i32, align 4\n  %4 = alloca i32, align 4\n  \
             store i32 %0, ptr %3, align 4\n  store i32 %1, ptr %4, align 4\n  %5 = load i32, ptr %3, align 4\n  \
             %6 = call i32 @h(i32 %5)\n  %7 = load i32, ptr %4, align 4\n  %8 = add nsw i32 %6, %7\n  ret i32 %8\n}}\n"
        ));
    }
    ir
}

/// A manager handed the previous one's declarations had listing 0, so every
/// function's callee summaries were worked out for every global again (wasm
/// tiny, 600/1200/2400 functions: 3.3G -> 5.9G, 17G, 55G instructions).
#[test]
fn callee_summaries_are_not_scanned_again_for_each_function() {
    let scans = |functions: usize| {
        let before = llrm_mir::passes::callee_scans();
        let options = Options { level: Level::O2, ..Options::default() };
        object(&module_of(functions), Width::W32, options).expect("compiles");
        llrm_mir::passes::callee_scans() - before
    };
    let (eight, thirty_two) = (scans(8), scans(32));
    assert!(thirty_two <= eight + 8, "8: {eight}, 32: {thirty_two}");
}
